use crate::lexer::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticError {
    /// Using a name that was never declared
    UndefinedName {
        name: String,
        span: Span,
        file: String,
        suggestion: Option<String>, // "did you mean 'userName'?"
    },

    /// Defining the same name twice in the same scope
    DuplicateDefinition {
        name: String,
        kind: String, // "variable", "function", "struct", ...
        first_at: Span,
        second_at: Span,
        file: String,
    },

    /// Using a statement in wrong context
    InvalidContext {
        what: &'static str,     // "return", "break", "continue"
        required: &'static str, // "inside a function", "inside a loop"
        span: Span,
        file: String,
    },

    /// Calling with wrong number of arguments
    WrongArgCount {
        name: String,
        expected: usize,
        got: usize,
        span: Span,
        file: String,
    },

    /// Assigning to an immutable variable
    ImmutableAssignment {
        name: String,
        defined_at: Span,
        span: Span,
        file: String,
    },

    /// Accessing a field that doesn't exist
    UnknownField {
        type_name: String,
        field: String,
        span: Span,
        file: String,
        available: Vec<String>,
    },

    /// Using an enum variant that doesn't exist
    UnknownVariant {
        enum_name: String,
        variant: String,
        span: Span,
        file: String,
        available: Vec<String>,
    },

    /// Duplicate field name in a struct definition
    DuplicateField {
        struct_name: String,
        field: String,
        span: Span,
        file: String,
    },

    /// Duplicate variant name in an enum definition
    DuplicateVariant {
        enum_name: String,
        variant: String,
        span: Span,
        file: String,
    },

    /// Duplicate parameter name in a function
    DuplicateParam {
        fn_name: String,
        param: String,
        span: Span,
        file: String,
    },

    /// Using 'self' outside of an impl block
    SelfOutsideImpl { span: Span, file: String },

    /// Too many errors — analyzer stopped early
    TooManyErrors {
        count: usize,
        span: Span,
        file: String,
    },
}

impl SemanticError {
    pub fn span(&self) -> Span {
        match self {
            Self::UndefinedName { span, .. } => *span,
            Self::DuplicateDefinition { second_at, .. } => *second_at,
            Self::InvalidContext { span, .. } => *span,
            Self::WrongArgCount { span, .. } => *span,
            Self::ImmutableAssignment { span, .. } => *span,
            Self::UnknownField { span, .. } => *span,
            Self::UnknownVariant { span, .. } => *span,
            Self::DuplicateField { span, .. } => *span,
            Self::DuplicateVariant { span, .. } => *span,
            Self::DuplicateParam { span, .. } => *span,
            Self::SelfOutsideImpl { span, .. } => *span,
            Self::TooManyErrors { span, .. } => *span,
        }
    }

    pub fn file(&self) -> &str {
        match self {
            Self::UndefinedName { file, .. } => file,
            Self::DuplicateDefinition { file, .. } => file,
            Self::InvalidContext { file, .. } => file,
            Self::WrongArgCount { file, .. } => file,
            Self::ImmutableAssignment { file, .. } => file,
            Self::UnknownField { file, .. } => file,
            Self::UnknownVariant { file, .. } => file,
            Self::DuplicateField { file, .. } => file,
            Self::DuplicateVariant { file, .. } => file,
            Self::DuplicateParam { file, .. } => file,
            Self::SelfOutsideImpl { file, .. } => file,
            Self::TooManyErrors { file, .. } => file,
        }
    }

    pub fn format(&self, source: &str) -> String {
        let span = self.span();
        let src_line = source
            .lines()
            .nth(span.line.saturating_sub(1))
            .unwrap_or("");
        let pointer = format!(
            "{}{}",
            " ".repeat(span.col.saturating_sub(1)),
            "^".repeat(span.len().max(1))
        );
        let (title, message, hint) = self.describe();
        let hint_line = hint
            .map(|h| format!("\n  💡 Hint: {}", h))
            .unwrap_or_default();

        format!(
            "\n🦎 LYZARD Semantic Error — {title}\n\
             ╭─ {}:{}:{}\n│\n\
             │  {src_line}\n\
             │  {pointer}\n│\n\
             │  {message}{hint_line}\n\
             ╰─\n",
            self.file(),
            span.line,
            span.col
        )
    }

    fn describe(&self) -> (String, String, Option<String>) {
        match self {
            Self::UndefinedName {
                name, suggestion, ..
            } => {
                let hint = match suggestion {
                    Some(s) => Some(format!("Did you mean '{}'?", s)),
                    None => Some(format!("Declare it first: let {} = ...", name)),
                };
                (
                    "Undefined name".to_string(),
                    format!("'{}' was used here but was never declared.", name),
                    hint,
                )
            }

            Self::DuplicateDefinition {
                name,
                kind,
                first_at,
                ..
            } => (
                "Duplicate definition".to_string(),
                format!(
                    "'{}' ({}) is defined more than once in this scope.",
                    name, kind
                ),
                Some(format!(
                    "First defined at line {}. Rename or remove one of them.",
                    first_at.line
                )),
            ),

            Self::InvalidContext { what, required, .. } => (
                "Invalid context".to_string(),
                format!("'{}' can only be used {}.", what, required),
                match *what {
                    "return" => Some("Move this 'return' inside a function body.".to_string()),
                    "break" => Some("Move this 'break' inside a while, for, or loop.".to_string()),
                    "continue" => {
                        Some("Move this 'continue' inside a while, for, or loop.".to_string())
                    }
                    _ => None,
                },
            ),

            Self::WrongArgCount {
                name,
                expected,
                got,
                ..
            } => (
                "Wrong number of arguments".to_string(),
                format!(
                    "'{}' expects {} argument(s) but got {}.",
                    name, expected, got
                ),
                Some(if *got < *expected {
                    format!("You're missing {} argument(s).", expected - got)
                } else {
                    format!("You passed {} extra argument(s).", got - expected)
                }),
            ),

            Self::ImmutableAssignment {
                name, defined_at, ..
            } => (
                "Assignment to immutable variable".to_string(),
                format!("'{}' is not mutable — you cannot assign to it.", name),
                Some(format!(
                    "Change the declaration at line {} to: let mut {}",
                    defined_at.line, name
                )),
            ),

            Self::UnknownField {
                type_name,
                field,
                available,
                ..
            } => (
                "Unknown field".to_string(),
                format!("'{}' has no field '{}'.", type_name, field),
                if available.is_empty() {
                    None
                } else {
                    Some(format!("Available fields: {}", available.join(", ")))
                },
            ),

            Self::UnknownVariant {
                enum_name,
                variant,
                available,
                ..
            } => (
                "Unknown variant".to_string(),
                format!("'{}' has no variant '{}'.", enum_name, variant),
                if available.is_empty() {
                    None
                } else {
                    Some(format!("Available variants: {}", available.join(", ")))
                },
            ),

            Self::DuplicateField {
                struct_name, field, ..
            } => (
                "Duplicate field".to_string(),
                format!(
                    "Field '{}' is defined more than once in struct '{}'.",
                    field, struct_name
                ),
                Some(format!("Remove or rename the duplicate '{}' field.", field)),
            ),

            Self::DuplicateVariant {
                enum_name, variant, ..
            } => (
                "Duplicate variant".to_string(),
                format!(
                    "Variant '{}' is defined more than once in enum '{}'.",
                    variant, enum_name
                ),
                Some(format!(
                    "Remove or rename the duplicate '{}' variant.",
                    variant
                )),
            ),

            Self::DuplicateParam { fn_name, param, .. } => (
                "Duplicate parameter".to_string(),
                format!(
                    "Parameter '{}' appears more than once in function '{}'.",
                    param, fn_name
                ),
                Some(format!(
                    "Rename one of the '{}' parameters in '{}'.",
                    param, fn_name
                )),
            ),

            Self::SelfOutsideImpl { .. } => (
                "'self' outside impl block".to_string(),
                "'self' can only be used as a parameter inside an impl block.".to_string(),
                Some("Move this function inside an 'impl SomeType { }' block.".to_string()),
            ),

            Self::TooManyErrors { count, .. } => (
                "Too many errors".to_string(),
                format!("Stopped after {} errors. Fix earlier errors first.", count),
                Some("Errors cascade — fix the first one and rerun.".to_string()),
            ),
        }
    }
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (title, msg, _) = self.describe();
        write!(f, "{}: {}", title, msg)
    }
}

impl std::error::Error for SemanticError {}

/// A collection of semantic errors (analyzer collects all before stopping)
#[derive(Debug, Default)]
pub struct SemanticErrors(pub Vec<SemanticError>);

impl SemanticErrors {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn push(&mut self, e: SemanticError) {
        self.0.push(e);
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn format_all(&self, source: &str) -> String {
        self.0
            .iter()
            .map(|e| e.format(source))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;
    use crate::lexer::Span;

    fn s() -> Span {
        Span::new(0, 3, 1, 1)
    }

    #[test]
    fn test_undefined_name_display() {
        let e = SemanticError::UndefinedName {
            name: "myVar".to_string(),
            span: s(),
            file: "t.lyz".to_string(),
            suggestion: None,
        };
        assert!(format!("{}", e).contains("myVar"));
    }

    #[test]
    fn test_format_has_all_components() {
        let source = "let x = undeclaredVar";
        let e = SemanticError::UndefinedName {
            name: "undeclaredVar".to_string(),
            span: Span::new(8, 21, 1, 9),
            file: "t.lyz".to_string(),
            suggestion: None,
        };
        let out = e.format(source);
        assert!(out.contains("🦎"));
        assert!(out.contains("undeclaredVar"));
        assert!(out.contains("^"));
        assert!(out.contains("Hint:"));
        assert!(out.contains("t.lyz"));
    }

    #[test]
    fn test_wrong_arg_count_hint() {
        let e = SemanticError::WrongArgCount {
            name: "add".to_string(),
            expected: 2,
            got: 3,
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("extra"));
    }

    #[test]
    fn test_wrong_arg_count_too_few() {
        let e = SemanticError::WrongArgCount {
            name: "add".to_string(),
            expected: 3,
            got: 1,
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("missing"));
    }

    #[test]
    fn test_unknown_field_shows_available() {
        let e = SemanticError::UnknownField {
            type_name: "Point".to_string(),
            field: "z".to_string(),
            span: s(),
            file: "t.lyz".to_string(),
            available: vec!["x".to_string(), "y".to_string()],
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("x"));
    }

    #[test]
    fn test_immutable_assignment_hint() {
        let e = SemanticError::ImmutableAssignment {
            name: "count".to_string(),
            defined_at: Span::new(0, 5, 3, 1),
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("mut count"));
    }

    #[test]
    fn test_suggestion_in_undefined() {
        let e = SemanticError::UndefinedName {
            name: "userNme".to_string(),
            span: s(),
            file: "t.lyz".to_string(),
            suggestion: Some("userName".to_string()),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("userName"));
    }

    #[test]
    fn test_implements_std_error() {
        fn takes_error<E: std::error::Error>(_: E) {}
        takes_error(SemanticError::SelfOutsideImpl {
            span: s(),
            file: "t.lyz".to_string(),
        });
    }
}
