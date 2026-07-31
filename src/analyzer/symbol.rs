use crate::lexer::Span;
use crate::parser::ast::{EnumDecl, FnDecl, InterfaceDecl, StructDecl, TypeExpr};

/// What a name in LYZARD refers to
#[derive(Debug, Clone, PartialEq)]
pub enum Symbol {
    /// A variable: let x = 5 or fn parameter
    Variable(VariableSymbol),

    /// A function: fn add(a: int, b: int) -> int
    Function(FunctionSymbol),

    /// A struct type: struct Point { x: float, y: float }
    Struct(StructSymbol),

    /// An enum type: enum Color { Red, Green, Blue }
    Enum(EnumSymbol),

    /// An interface: interface Printable { fn print(self) }
    Interface(InterfaceSymbol),

    /// A generic type parameter: T in fn max<T>(a: T, b: T) -> T
    GenericParam(GenericParamSymbol),
}

impl Symbol {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Variable(_) => "variable",
            Self::Function(_) => "function",
            Self::Struct(_) => "struct",
            Self::Enum(_) => "enum",
            Self::Interface(_) => "interface",
            Self::GenericParam(_) => "generic type parameter",
        }
    }

    pub fn defined_at(&self) -> Span {
        match self {
            Self::Variable(s) => s.defined_at,
            Self::Function(s) => s.defined_at,
            Self::Struct(s) => s.defined_at,
            Self::Enum(s) => s.defined_at,
            Self::Interface(s) => s.defined_at,
            Self::GenericParam(s) => s.defined_at,
        }
    }

    pub fn is_variable(&self) -> bool {
        matches!(self, Self::Variable(_))
    }
    pub fn is_function(&self) -> bool {
        matches!(self, Self::Function(_))
    }
    pub fn is_type(&self) -> bool {
        matches!(self, Self::Struct(_) | Self::Enum(_) | Self::Interface(_))
    }

    /// If this symbol is a function, return its param count
    pub fn param_count(&self) -> Option<usize> {
        match self {
            Self::Function(f) => Some(f.param_count),
            _ => None,
        }
    }
}

// ── VARIABLE ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct VariableSymbol {
    pub name: String,
    pub mutable: bool,
    pub type_annotation: Option<TypeExpr>, // what the programmer wrote, if anything
    pub defined_at: Span,
    pub is_param: bool, // is it a function parameter?
}

// ── FUNCTION ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSymbol {
    pub name: String,
    pub param_count: usize,
    pub param_names: Vec<String>,
    pub return_type: Option<TypeExpr>,
    pub defined_at: Span,
    pub is_method: bool, // true if first param is "self"
}

impl FunctionSymbol {
    pub fn from_decl(decl: &FnDecl) -> Self {
        let param_names: Vec<String> = decl
            .params
            .iter()
            .filter(|p| !p.is_self)
            .map(|p| p.name.clone())
            .collect();

        FunctionSymbol {
            name: decl.name.clone(),
            param_count: param_names.len(),
            param_names,
            return_type: decl.return_type.clone(),
            defined_at: decl.span,
            is_method: decl.params.first().map(|p| p.is_self).unwrap_or(false),
        }
    }
}

// ── STRUCT ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct StructSymbol {
    pub name: String,
    pub field_names: Vec<String>,
    pub defined_at: Span,
}

impl StructSymbol {
    pub fn from_decl(decl: &StructDecl) -> Self {
        StructSymbol {
            name: decl.name.clone(),
            field_names: decl.fields.iter().map(|f| f.name.clone()).collect(),
            defined_at: decl.span,
        }
    }

    pub fn has_field(&self, name: &str) -> bool {
        self.field_names.iter().any(|f| f == name)
    }
}

// ── ENUM ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct EnumSymbol {
    pub name: String,
    pub variant_names: Vec<String>,
    pub defined_at: Span,
}

impl EnumSymbol {
    pub fn from_decl(decl: &EnumDecl) -> Self {
        EnumSymbol {
            name: decl.name.clone(),
            variant_names: decl.variants.iter().map(|v| v.name.clone()).collect(),
            defined_at: decl.span,
        }
    }

    pub fn has_variant(&self, name: &str) -> bool {
        self.variant_names.iter().any(|v| v == name)
    }
}

// ── INTERFACE ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceSymbol {
    pub name: String,
    pub method_names: Vec<String>,
    pub defined_at: Span,
}

impl InterfaceSymbol {
    pub fn from_decl(decl: &InterfaceDecl) -> Self {
        InterfaceSymbol {
            name: decl.name.clone(),
            method_names: decl.methods.iter().map(|m| m.name.clone()).collect(),
            defined_at: decl.span,
        }
    }
}

// ── GENERIC PARAM ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParamSymbol {
    pub name: String,
    pub bounds: Vec<String>,
    pub defined_at: Span,
}

#[cfg(test)]
mod symbol_tests {
    use super::*;
    use crate::lexer::Span;
    use crate::parser::ast::*;

    fn s() -> Span {
        Span::dummy()
    }

    fn make_fn_decl(name: &str, param_names: &[&str]) -> FnDecl {
        FnDecl {
            name: name.to_string(),
            generics: vec![],
            params: param_names
                .iter()
                .map(|n| Param {
                    name: n.to_string(),
                    param_type: Some(TypeExpr::Named(NamedType {
                        name: "int".to_string(),
                        span: s(),
                    })),
                    is_self: false,
                    span: s(),
                })
                .collect(),
            return_type: None,
            body: FnBody::Block(Block {
                statements: vec![],
                span: s(),
            }),
            is_pub: false,
            span: s(),
        }
    }

    #[test]
    fn test_symbol_kind_name() {
        let sym = Symbol::Variable(VariableSymbol {
            name: "x".to_string(),
            mutable: false,
            type_annotation: None,
            defined_at: s(),
            is_param: false,
        });
        assert_eq!(sym.kind_name(), "variable");
    }

    #[test]
    fn test_function_symbol_from_decl() {
        let decl = make_fn_decl("add", &["a", "b"]);
        let sym = FunctionSymbol::from_decl(&decl);
        assert_eq!(sym.name, "add");
        assert_eq!(sym.param_count, 2);
        assert_eq!(sym.param_names, vec!["a", "b"]);
    }

    #[test]
    fn test_function_symbol_param_count() {
        let decl = make_fn_decl("greet", &["name"]);
        let sym = Symbol::Function(FunctionSymbol::from_decl(&decl));
        assert_eq!(sym.param_count(), Some(1));
    }

    #[test]
    fn test_struct_has_field() {
        let sym = StructSymbol {
            name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
            defined_at: s(),
        };
        assert!(sym.has_field("x"));
        assert!(sym.has_field("y"));
        assert!(!sym.has_field("z"));
    }

    #[test]
    fn test_enum_has_variant() {
        let sym = EnumSymbol {
            name: "Color".to_string(),
            variant_names: vec!["Red".to_string(), "Green".to_string()],
            defined_at: s(),
        };
        assert!(sym.has_variant("Red"));
        assert!(!sym.has_variant("Blue"));
    }

    #[test]
    fn test_is_helpers() {
        let var_sym = Symbol::Variable(VariableSymbol {
            name: "x".to_string(),
            mutable: true,
            type_annotation: None,
            defined_at: s(),
            is_param: false,
        });
        assert!(var_sym.is_variable());
        assert!(!var_sym.is_function());
        assert!(!var_sym.is_type());
    }

    #[test]
    fn test_defined_at_returns_span() {
        let sym = Symbol::Variable(VariableSymbol {
            name: "x".to_string(),
            mutable: false,
            type_annotation: None,
            defined_at: Span::new(0, 3, 1, 1),
            is_param: false,
        });
        assert_eq!(sym.defined_at().line, 1);
    }

    #[test]
    fn test_interface_symbol_from_decl() {
        let decl = InterfaceDecl {
            name: "Printable".to_string(),
            generics: vec![],
            methods: vec![
                InterfaceMethod {
                    name: "print".to_string(),
                    params: vec![Param {
                        name: "self".to_string(),
                        param_type: None,
                        is_self: true,
                        span: s(),
                    }],
                    return_type: None,
                    span: s(),
                },
                InterfaceMethod {
                    name: "to_string".to_string(),
                    params: vec![],
                    return_type: Some(TypeExpr::Named(NamedType {
                        name: "string".to_string(),
                        span: s(),
                    })),
                    span: s(),
                },
            ],
            is_pub: false,
            span: s(),
        };
        let sym = InterfaceSymbol::from_decl(&decl);
        assert_eq!(sym.name, "Printable");
        assert_eq!(sym.method_names, vec!["print", "to_string"]);
    }
}
