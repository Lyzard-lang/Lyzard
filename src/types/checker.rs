use crate::lexer::Span;
use crate::parser::ast::*;

use super::error::{TypeError, TypeErrors};
use super::{ResolvedType, TypeEnvironment};

const MAX_ERRORS: usize = 30;

/// Two-pass type checker. Pass 1 registers all top-level signatures so that
/// forward references resolve; Pass 2 analyzes declaration bodies.
#[allow(dead_code)]
pub struct TypeChecker {
    pub env: TypeEnvironment,
    pub errors: TypeErrors,
    file: String,
    source: String,
    /// Struct field maps: struct name → field name → field type
    struct_fields: std::collections::HashMap<String, Vec<(String, ResolvedType)>>,
    /// Enum variant maps: enum name → variant names
    enum_variants: std::collections::HashMap<String, Vec<String>>,
}

impl TypeChecker {
    pub fn new(source: impl Into<String>, file: impl Into<String>) -> Self {
        TypeChecker {
            env: TypeEnvironment::new(),
            errors: TypeErrors::new(),
            file: file.into(),
            source: source.into(),
            struct_fields: std::collections::HashMap::new(),
            enum_variants: std::collections::HashMap::new(),
        }
    }

    /// Main entry — type-check a full program
    pub fn check(mut self, program: &Program) -> TypeErrors {
        // Pass 1: register all top-level type signatures
        self.register_top_level(program);
        // Pass 2: check all declaration bodies
        for decl in &program.declarations {
            if self.errors.len() >= MAX_ERRORS {
                break;
            }
            self.check_declaration(decl);
        }
        self.errors
    }

    // ══════════════════════════════════════════
    //   PASS 1: REGISTER TOP-LEVEL SIGNATURES
    // ══════════════════════════════════════════

    fn register_top_level(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => self.register_fn_sig(f),
                Declaration::Struct(s) => self.register_struct(s),
                Declaration::Enum(e) => self.register_enum(e),
                _ => {}
            }
        }
    }

    fn register_fn_sig(&mut self, decl: &FnDecl) {
        let params: Vec<ResolvedType> = decl
            .params
            .iter()
            .filter(|p| !p.is_self)
            .map(|p| {
                p.param_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(ResolvedType::Unknown)
            })
            .collect();
        let return_type = decl
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(ResolvedType::Void);
        let fn_type = ResolvedType::Function {
            params,
            return_type: Box::new(return_type),
        };
        self.env.define(decl.name.clone(), fn_type);
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        let fields: Vec<(String, ResolvedType)> = decl
            .fields
            .iter()
            .map(|f| (f.name.clone(), self.resolve_type(&f.field_type)))
            .collect();
        self.struct_fields.insert(decl.name.clone(), fields);
        self.env
            .define(decl.name.clone(), ResolvedType::Struct(decl.name.clone()));
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        let variants: Vec<String> = decl.variants.iter().map(|v| v.name.clone()).collect();
        self.enum_variants.insert(decl.name.clone(), variants);
        self.env
            .define(decl.name.clone(), ResolvedType::Enum(decl.name.clone()));
    }

    // ══════════════════════════════════════════
    //   TYPE RESOLUTION (source → resolved)
    // ══════════════════════════════════════════

    pub fn resolve_type(&self, type_expr: &TypeExpr) -> ResolvedType {
        match type_expr {
            TypeExpr::Named(n) => match n.name.as_str() {
                "int" => ResolvedType::Int,
                "float" => ResolvedType::Float,
                "bool" => ResolvedType::Bool,
                "str" => ResolvedType::Str,
                "char" => ResolvedType::Char,
                "void" => ResolvedType::Void,
                "never" => ResolvedType::Never,
                name => {
                    if self.struct_fields.contains_key(name) {
                        ResolvedType::Struct(name.to_string())
                    } else if self.enum_variants.contains_key(name) {
                        ResolvedType::Enum(name.to_string())
                    } else {
                        ResolvedType::TypeParam(name.to_string())
                    }
                }
            },
            TypeExpr::Optional(inner, _) => {
                ResolvedType::Optional(Box::new(self.resolve_type(inner)))
            }
            TypeExpr::Array(inner, _) => {
                ResolvedType::Array(Box::new(self.resolve_type(inner)))
            }
            TypeExpr::Tuple(types, _) => {
                ResolvedType::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeExpr::Generic(g) => ResolvedType::Generic {
                name: g.name.clone(),
                args: g.args.iter().map(|a| self.resolve_type(a)).collect(),
            },
            TypeExpr::Never(_) => ResolvedType::Never,
            _ => ResolvedType::Unknown,
        }
    }

    // ══════════════════════════════════════════
    //   PASS 2: DECLARATION BODIES (later phase)
    // ══════════════════════════════════════════

    fn check_declaration(&mut self, _decl: &Declaration) {}

    // ══════════════════════════════════════════
    //   ERROR HELPERS
    // ══════════════════════════════════════════

    fn push_error(&mut self, err: TypeError) {
        self.errors.push(err);
    }

    #[allow(dead_code)]
    fn type_mismatch(&mut self, expected: ResolvedType, found: ResolvedType, span: Span, context: &str) {
        self.push_error(TypeError::TypeMismatch {
            expected,
            found,
            span,
            file: self.file.clone(),
            context: context.to_string(),
        });
    }

    #[allow(dead_code)]
    fn lookup_type(&self, name: &str) -> Option<ResolvedType> {
        self.env.lookup(name).cloned()
    }
}

#[cfg(test)]
mod checker_init_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> TypeErrors {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        TypeChecker::new(src, "t.lyz").check(&prog)
    }

    #[test]
    fn test_empty_program_no_errors() {
        assert!(check("").is_empty());
    }

    #[test]
    fn test_fn_registered_as_function_type() {
        let src = "fn add(a: int, b: int) -> int { return a + b }";
        let errs = check(src);
        assert!(errs.is_empty(), "{}", errs.format_all(src));
    }

    #[test]
    fn test_struct_registered() {
        let errs = check("struct Point { x: float, y: float }");
        assert!(errs.is_empty());
    }

    #[test]
    fn test_forward_reference_fn() {
        let src = "fn main() { let r = compute(5) }\nfn compute(n: int) -> int { return n }";
        let errs = check(src);
        assert!(errs.is_empty(), "{}", errs.format_all(src));
    }

    #[test]
    fn test_resolve_type_int() {
        let c = TypeChecker::new("", "t");
        assert_eq!(
            c.resolve_type(&TypeExpr::Named(NamedType {
                name: "int".to_string(),
                span: crate::lexer::Span::dummy()
            })),
            ResolvedType::Int
        );
    }

    #[test]
    fn test_resolve_type_optional() {
        let c = TypeChecker::new("", "t");
        let inner = TypeExpr::Named(NamedType {
            name: "str".to_string(),
            span: crate::lexer::Span::dummy(),
        });
        let opt = TypeExpr::Optional(Box::new(inner), crate::lexer::Span::dummy());
        assert_eq!(
            c.resolve_type(&opt),
            ResolvedType::Optional(Box::new(ResolvedType::Str))
        );
    }

    #[test]
    fn test_resolve_type_array() {
        let c = TypeChecker::new("", "t");
        let inner = TypeExpr::Named(NamedType {
            name: "int".to_string(),
            span: crate::lexer::Span::dummy(),
        });
        let arr = TypeExpr::Array(Box::new(inner), crate::lexer::Span::dummy());
        assert_eq!(
            c.resolve_type(&arr),
            ResolvedType::Array(Box::new(ResolvedType::Int))
        );
    }
}
