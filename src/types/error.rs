use crate::lexer::Span;
use crate::types::ResolvedType;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    /// Using a value of one type where a different type was expected
    TypeMismatch {
        expected: ResolvedType,
        got: ResolvedType,
        span: Span,
        file: String,
    },

    /// Accessing a field that doesn't exist on a struct
    UnknownStructField {
        struct_name: String,
        field: String,
        span: Span,
        file: String,
        available: Vec<String>,
    },

    /// A struct literal is missing a required field
    MissingField {
        struct_name: String,
        field: String,
        span: Span,
        file: String,
    },

    /// A struct literal has a field the struct doesn't define
    UnexpectedField {
        struct_name: String,
        field: String,
        span: Span,
        file: String,
    },

    /// A struct field was given a value of the wrong type
    FieldTypeMismatch {
        struct_name: String,
        field: String,
        expected: ResolvedType,
        got: ResolvedType,
        span: Span,
        file: String,
    },

    /// Calling a function with an argument of the wrong type
    WrongArgType {
        fn_name: String,
        index: usize,
        expected: ResolvedType,
        got: ResolvedType,
        span: Span,
        file: String,
    },

    /// A function returned the wrong type
    WrongReturnType {
        fn_name: String,
        expected: ResolvedType,
        got: ResolvedType,
        span: Span,
        file: String,
    },

    /// Using a type name that was never declared
    UndefinedType {
        name: String,
        span: Span,
        file: String,
    },

    /// Calling something that is not a function
    NotCallable {
        name: String,
        actual: ResolvedType,
        span: Span,
        file: String,
    },

    /// Indexing something that is not an array or string
    NotIndexable {
        name: String,
        actual: ResolvedType,
        span: Span,
        file: String,
    },

    /// Using a void expression where a value was required
    VoidUsedAsValue { span: Span, file: String },

    /// Using an Optional<T> as T without checking for null
    OptionalUsedAsValue {
        name: String,
        actual: ResolvedType,
        span: Span,
        file: String,
    },

    /// A function with a non-void return type is missing a return value
    MissingReturn {
        fn_name: String,
        expected: ResolvedType,
        span: Span,
        file: String,
    },
}

impl TypeError {
    pub fn span(&self) -> Span {
        match self {
            Self::TypeMismatch { span, .. }
            | Self::UnknownStructField { span, .. }
            | Self::MissingField { span, .. }
            | Self::UnexpectedField { span, .. }
            | Self::FieldTypeMismatch { span, .. }
            | Self::WrongArgType { span, .. }
            | Self::WrongReturnType { span, .. }
            | Self::UndefinedType { span, .. }
            | Self::NotCallable { span, .. }
            | Self::NotIndexable { span, .. }
            | Self::VoidUsedAsValue { span, .. }
            | Self::OptionalUsedAsValue { span, .. }
            | Self::MissingReturn { span, .. } => *span,
        }
    }

    pub fn file(&self) -> &str {
        match self {
            Self::TypeMismatch { file, .. }
            | Self::UnknownStructField { file, .. }
            | Self::MissingField { file, .. }
            | Self::UnexpectedField { file, .. }
            | Self::FieldTypeMismatch { file, .. }
            | Self::WrongArgType { file, .. }
            | Self::WrongReturnType { file, .. }
            | Self::UndefinedType { file, .. }
            | Self::NotCallable { file, .. }
            | Self::NotIndexable { file, .. }
            | Self::VoidUsedAsValue { file, .. }
            | Self::OptionalUsedAsValue { file, .. }
            | Self::MissingReturn { file, .. } => file,
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
            "\n🦎 LYZARD Type Error — {title}\n\
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
            Self::TypeMismatch { expected, got, .. } => (
                "Type mismatch".to_string(),
                format!(
                    "Expected '{}' but got '{}'.",
                    expected.name(),
                    got.name()
                ),
                Some(format!(
                    "Convert the value to '{}', or change the declared type.",
                    expected.name()
                )),
            ),

            Self::UnknownStructField {
                struct_name,
                field,
                available,
                ..
            } => (
                "Unknown struct field".to_string(),
                format!("Struct '{}' has no field '{}'.", struct_name, field),
                if available.is_empty() {
                    Some(format!(
                        "Check the spelling — fields are defined in struct '{}'.",
                        struct_name
                    ))
                } else {
                    Some(format!("Available fields: {}", available.join(", ")))
                },
            ),

            Self::MissingField { struct_name, field, .. } => (
                "Missing struct field".to_string(),
                format!(
                    "Struct '{}' requires field '{}'.",
                    struct_name, field
                ),
                Some(format!("Add '{}: <value>' to the struct literal.", field)),
            ),

            Self::UnexpectedField { struct_name, field, .. } => (
                "Unexpected struct field".to_string(),
                format!(
                    "Struct '{}' does not define field '{}'.",
                    struct_name, field
                ),
                Some(format!("Remove '{}' from the struct literal.", field)),
            ),

            Self::FieldTypeMismatch {
                struct_name,
                field,
                expected,
                got,
                ..
            } => (
                "Field type mismatch".to_string(),
                format!(
                    "Field '{}' of struct '{}' expects '{}' but got '{}'.",
                    field,
                    struct_name,
                    expected.name(),
                    got.name()
                ),
                Some(format!(
                    "Pass a '{}' value for field '{}'.",
                    expected.name(),
                    field
                )),
            ),

            Self::WrongArgType {
                fn_name,
                index,
                expected,
                got,
                ..
            } => (
                "Wrong argument type".to_string(),
                format!(
                    "Function '{}' expects argument #{} to be '{}' but got '{}'.",
                    fn_name,
                    index + 1,
                    expected.name(),
                    got.name()
                ),
                Some(format!(
                    "Pass a '{}' value as argument #{}.",
                    expected.name(),
                    index + 1
                )),
            ),

            Self::WrongReturnType {
                fn_name,
                expected,
                got,
                ..
            } => (
                "Wrong return type".to_string(),
                format!(
                    "Function '{}' should return '{}' but returned '{}'.",
                    fn_name,
                    expected.name(),
                    got.name()
                ),
                Some(format!(
                    "Return a '{}' value, or change the function's declared return type.",
                    expected.name()
                )),
            ),

            Self::UndefinedType { name, .. } => (
                "Undefined type".to_string(),
                format!("Type '{}' was never declared.", name),
                Some(format!(
                    "Define it with 'struct {} {{ ... }}', or fix the spelling.",
                    name
                )),
            ),

            Self::NotCallable { name, actual, .. } => (
                "Not callable".to_string(),
                format!(
                    "'{}' is of type '{}' and cannot be called.",
                    name,
                    actual.name()
                ),
                Some(format!(
                    "Only functions can be called — check the definition of '{}'.",
                    name
                )),
            ),

            Self::NotIndexable { name, actual, .. } => (
                "Not indexable".to_string(),
                format!(
                    "'{}' is of type '{}' and cannot be indexed.",
                    name,
                    actual.name()
                ),
                Some("Only arrays and strings can be indexed with [].".to_string()),
            ),

            Self::VoidUsedAsValue { .. } => (
                "Void used as value".to_string(),
                "This expression produces no value (type 'void').".to_string(),
                Some("Call the function separately — it produces no value.".to_string()),
            ),

            Self::OptionalUsedAsValue { name, actual, .. } => (
                "Optional used as value".to_string(),
                format!(
                    "'{}' is of type '{}' and may be null.",
                    name,
                    actual.name()
                ),
                Some(format!(
                    "Check for null first: 'if {} != null', or unwrap it.",
                    name
                )),
            ),

            Self::MissingReturn { fn_name, expected, .. } => (
                "Missing return".to_string(),
                format!(
                    "Function '{}' is missing a return value.",
                    fn_name
                ),
                Some(format!(
                    "Add 'return <value>' at the end, returning '{}' on every path.",
                    expected.name()
                )),
            ),
        }
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (title, msg, _) = self.describe();
        write!(f, "{}: {}", title, msg)
    }
}

impl std::error::Error for TypeError {}

/// A collection of type errors (type checker collects all before stopping)
#[derive(Debug, Default)]
pub struct TypeErrors(pub Vec<TypeError>);

impl TypeErrors {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn push(&mut self, e: TypeError) {
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
mod type_error_tests {
    use super::*;
    use crate::lexer::Span;
    use crate::types::ResolvedType;

    fn s() -> Span {
        Span::new(0, 3, 1, 1)
    }

    #[test]
    fn test_type_mismatch_display() {
        let e = TypeError::TypeMismatch {
            expected: ResolvedType::Int,
            got: ResolvedType::Str,
            span: s(),
            file: "t.lyz".to_string(),
        };
        let out = format!("{}", e);
        assert!(out.contains("Type mismatch"));
        assert!(out.contains("int"));
        assert!(out.contains("str"));
    }

    #[test]
    fn test_format_has_emoji_and_components() {
        let source = "let x: int = \"hi\"";
        let e = TypeError::TypeMismatch {
            expected: ResolvedType::Int,
            got: ResolvedType::Str,
            span: Span::new(13, 17, 1, 14),
            file: "t.lyz".to_string(),
        };
        let out = e.format(source);
        assert!(out.contains("🦎"));
        assert!(out.contains("int"));
        assert!(out.contains("str"));
        assert!(out.contains("^"));
        assert!(out.contains("Hint:"));
        assert!(out.contains("t.lyz"));
        assert!(out.contains("1:14"));
    }

    #[test]
    fn test_unknown_struct_field_shows_available() {
        let e = TypeError::UnknownStructField {
            struct_name: "Point".to_string(),
            field: "z".to_string(),
            span: s(),
            file: "t.lyz".to_string(),
            available: vec!["x".to_string(), "y".to_string()],
        };
        let (_, _, hint) = e.describe();
        let hint = hint.unwrap();
        assert!(hint.contains("x"));
        assert!(hint.contains("y"));
    }

    #[test]
    fn test_all_variants_have_hint() {
        let t = ResolvedType::Int;
        let cases: Vec<TypeError> = vec![
            TypeError::TypeMismatch { expected: t.clone(), got: ResolvedType::Str, span: s(), file: "t.lyz".into() },
            TypeError::UnknownStructField { struct_name: "P".into(), field: "z".into(), span: s(), file: "t.lyz".into(), available: vec!["x".into(), "y".into()] },
            TypeError::MissingField { struct_name: "P".into(), field: "y".into(), span: s(), file: "t.lyz".into() },
            TypeError::UnexpectedField { struct_name: "P".into(), field: "z".into(), span: s(), file: "t.lyz".into() },
            TypeError::FieldTypeMismatch { struct_name: "P".into(), field: "x".into(), expected: t.clone(), got: ResolvedType::Str, span: s(), file: "t.lyz".into() },
            TypeError::WrongArgType { fn_name: "add".into(), index: 0, expected: t.clone(), got: ResolvedType::Bool, span: s(), file: "t.lyz".into() },
            TypeError::WrongReturnType { fn_name: "f".into(), expected: t.clone(), got: ResolvedType::Float, span: s(), file: "t.lyz".into() },
            TypeError::UndefinedType { name: "Foo".into(), span: s(), file: "t.lyz".into() },
            TypeError::NotCallable { name: "x".into(), actual: ResolvedType::Int, span: s(), file: "t.lyz".into() },
            TypeError::NotIndexable { name: "x".into(), actual: ResolvedType::Int, span: s(), file: "t.lyz".into() },
            TypeError::VoidUsedAsValue { span: s(), file: "t.lyz".into() },
            TypeError::OptionalUsedAsValue { name: "x".into(), actual: ResolvedType::Optional(Box::new(ResolvedType::Int)), span: s(), file: "t.lyz".into() },
            TypeError::MissingReturn { fn_name: "f".into(), expected: ResolvedType::Int, span: s(), file: "t.lyz".into() },
        ];
        for e in &cases {
            let (_, _, hint) = e.describe();
            assert!(hint.is_some(), "missing hint for {:?}", e);
        }
    }

    #[test]
    fn test_wrong_arg_type_mentions_index() {
        let e = TypeError::WrongArgType {
            fn_name: "max".to_string(),
            index: 1,
            expected: ResolvedType::Int,
            got: ResolvedType::Str,
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("#2"));
    }

    #[test]
    fn test_void_used_as_value_hint() {
        let e = TypeError::VoidUsedAsValue {
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("value"));
    }

    #[test]
    fn test_implements_std_error() {
        fn takes_error<E: std::error::Error>(_: E) {}
        takes_error(TypeError::VoidUsedAsValue {
            span: s(),
            file: "t.lyz".to_string(),
        });
    }
}
