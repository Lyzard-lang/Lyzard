pub mod error;
pub mod scope;
pub mod symbol;

use crate::lexer::Span;
use crate::parser::ast::*;
use error::{SemanticError, SemanticErrors};
#[allow(unused_imports)]
use scope::{Scope, ScopeKind, SymbolTable};
use symbol::*;

const MAX_ERRORS: usize = 30;

/// Context the analyzer tracks while walking the AST
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct Context {
    in_impl: bool,               // are we inside an impl block?
    current_fn: Option<String>,  // name of the function being analyzed
    impl_target: Option<String>, // name of the type being impl'd
}

#[allow(dead_code)]
pub struct Analyzer {
    symbols: SymbolTable,
    errors: SemanticErrors,
    source: String,
    file: String,
    ctx: Context,
}

impl Analyzer {
    pub fn new(source: impl Into<String>, file: impl Into<String>) -> Self {
        let mut analyzer = Analyzer {
            symbols: SymbolTable::new(),
            errors: SemanticErrors::new(),
            source: source.into(),
            file: file.into(),
            ctx: Context::default(),
        };
        analyzer.register_builtins();
        analyzer
    }

    /// Main entry point
    pub fn analyze(mut self, program: &Program) -> (SemanticErrors, SymbolTable) {
        // Pass 1: register all top-level names
        self.register_top_level(program);

        // Pass 2: analyze all bodies
        for decl in &program.declarations {
            if self.errors.len() >= MAX_ERRORS {
                break;
            }
            self.analyze_declaration(decl);
        }

        (self.errors, self.symbols)
    }

    // ══════════════════════════════════════════════
    //   PASS 1: REGISTER TOP-LEVEL DECLARATIONS
    // ══════════════════════════════════════════════

    fn register_top_level(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => self.register_fn(f),
                Declaration::Struct(s) => self.register_struct(s),
                Declaration::Enum(e) => self.register_enum(e),
                Declaration::Interface(i) => self.register_interface(i),
                _ => {}
            }
        }
    }

    fn register_fn(&mut self, decl: &FnDecl) {
        let sym = Symbol::Function(FunctionSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "function", decl.span);
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        // Check for duplicate fields before registering
        let mut seen_fields = std::collections::HashSet::new();
        for field in &decl.fields {
            if !seen_fields.insert(field.name.clone()) {
                self.push_error(SemanticError::DuplicateField {
                    struct_name: decl.name.clone(),
                    field: field.name.clone(),
                    span: field.span,
                    file: self.file.clone(),
                });
            }
        }

        let sym = Symbol::Struct(StructSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "struct", decl.span);
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        // Check for duplicate variants
        let mut seen_variants = std::collections::HashSet::new();
        for variant in &decl.variants {
            if !seen_variants.insert(variant.name.clone()) {
                self.push_error(SemanticError::DuplicateVariant {
                    enum_name: decl.name.clone(),
                    variant: variant.name.clone(),
                    span: variant.span,
                    file: self.file.clone(),
                });
            }
        }

        let sym = Symbol::Enum(EnumSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "enum", decl.span);
    }

    fn register_interface(&mut self, decl: &InterfaceDecl) {
        let sym = Symbol::Interface(InterfaceSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "interface", decl.span);
    }

    // ══════════════════════════════════════════════
    //   BUILT-IN FUNCTIONS
    // ══════════════════════════════════════════════

    fn register_builtins(&mut self) {
        let builtins: &[(&str, usize)] = &[
            ("print", 1),
            ("println", 1),
            ("readLine", 0),
            ("len", 1),
            ("parseInt", 1),
            ("parseFloat", 1),
            ("panic", 1),
            ("assert", 1),
            ("typeof", 1),
        ];

        for (name, param_count) in builtins {
            let sym = Symbol::Function(FunctionSymbol {
                name: (*name).to_string(),
                param_count: *param_count,
                param_names: vec![],
                return_type: None,
                defined_at: Span::dummy(),
                is_method: false,
            });
            self.symbols.define(name.to_string(), sym);
        }
    }

    // ══════════════════════════════════════════════
    //   PASS 2: ANALYZE DECLARATIONS (stub — filled in later)
    // ══════════════════════════════════════════════

    fn analyze_declaration(&mut self, _decl: &Declaration) {
        // Body analysis is implemented in a later step.
    }

    // ══════════════════════════════════════════════
    //   HELPERS
    // ══════════════════════════════════════════════

    /// Define a symbol or report a duplicate error
    fn define_or_error(&mut self, name: &str, sym: Symbol, kind: &str, span: Span) {
        if let Some(existing) = self.symbols.lookup(name) {
            if self.symbols.defined_in_current_scope(name) {
                self.push_error(SemanticError::DuplicateDefinition {
                    name: name.to_string(),
                    kind: kind.to_string(),
                    first_at: existing.defined_at(),
                    second_at: span,
                    file: self.file.clone(),
                });
                return;
            }
        }
        self.symbols.define(name.to_string(), sym);
    }

    fn push_error(&mut self, err: SemanticError) {
        self.errors.push(err);
    }
}

#[cfg(test)]
mod register_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analyze(src: &str) -> (SemanticErrors, SymbolTable) {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        Analyzer::new(src, "t.lyz").analyze(&prog)
    }

    #[test]
    fn test_fn_registered() {
        let (errs, table) = analyze("fn add(a: int, b: int) -> int { return a + b }");
        assert!(
            errs.is_empty(),
            "{}",
            errs.format_all("fn add(a: int, b: int) -> int { return a + b }")
        );
        assert!(table.is_defined("add"));
    }

    #[test]
    fn test_struct_registered() {
        let (errs, table) = analyze("struct Point { x: float, y: float }");
        assert!(errs.is_empty());
        assert!(table.is_defined("Point"));
    }

    #[test]
    fn test_enum_registered() {
        let (errs, table) = analyze("enum Color { Red, Green, Blue }");
        assert!(errs.is_empty());
        assert!(table.is_defined("Color"));
    }

    #[test]
    fn test_builtin_print_registered() {
        let (_, table) = analyze("");
        assert!(table.is_defined("print"));
        assert!(table.is_defined("len"));
        assert!(table.is_defined("panic"));
    }

    #[test]
    fn test_duplicate_fn_error() {
        let src = "fn foo() {}\nfn foo() {}";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateDefinition { name, .. } if name == "foo")
        );
    }

    #[test]
    fn test_duplicate_struct_error() {
        let src = "struct Foo { x: int }\nstruct Foo { y: int }";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateDefinition { name, .. } if name == "Foo")
        );
    }

    #[test]
    fn test_duplicate_field_error() {
        let src = "struct Bad { x: int, x: float }";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], SemanticError::DuplicateField { field, .. } if field == "x"));
    }

    #[test]
    fn test_duplicate_variant_error() {
        let src = "enum Bad { Ok, Ok }";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateVariant { variant, .. } if variant == "Ok")
        );
    }

    #[test]
    fn test_forward_reference_works() {
        // main calls compute() which is defined AFTER main
        let src = "fn main() { compute() }\nfn compute() -> int { return 42 }";
        let (errs, _) = analyze(src);
        assert!(
            errs.is_empty(),
            "Forward references should work: {:?}",
            errs.0
        );
    }
}
