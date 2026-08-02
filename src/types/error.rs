use crate::lexer::Span;

use super::ResolvedType;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    TypeMismatch {
        expected: ResolvedType,
        found: ResolvedType,
        span: Span,
        file: String,
        context: String,
    },
    InvalidOperation {
        op: String,
        left: ResolvedType,
        right: ResolvedType,
        span: Span,
        file: String,
    },
    NonBoolCondition {
        found: ResolvedType,
        span: Span,
        file: String,
        context: String,
    },
    MissingReturn {
        fn_name: String,
        expected: ResolvedType,
        span: Span,
        file: String,
    },
    BranchTypeMismatch {
        then_type: ResolvedType,
        else_type: ResolvedType,
        span: Span,
        file: String,
    },
    NotAFunction {
        name: String,
        actual_type: ResolvedType,
        span: Span,
        file: String,
    },
    ArgumentTypeMismatch {
        fn_name: String,
        param_index: usize,
        expected: ResolvedType,
        found: ResolvedType,
        span: Span,
        file: String,
    },
    FieldOnNonStruct {
        found: ResolvedType,
        field: String,
        span: Span,
        file: String,
    },
    IndexOnNonArray {
        found: ResolvedType,
        span: Span,
        file: String,
    },
    NonIntegerIndex {
        found: ResolvedType,
        span: Span,
        file: String,
    },
    PropagateOnNonResult {
        found: ResolvedType,
        span: Span,
        file: String,
    },
    UnknownStructField {
        struct_name: String,
        field: String,
        available: Vec<String>,
        span: Span,
        file: String,
    },
    UnknownEnumVariant {
        enum_name: String,
        variant: String,
        available: Vec<String>,
        span: Span,
        file: String,
    },
    UnaryTypeMismatch {
        op: String,
        found: ResolvedType,
        span: Span,
        file: String,
    },
}

impl TypeError {
    pub fn span(&self) -> Span {
        match self {
            Self::TypeMismatch { span, .. } => *span,
            Self::InvalidOperation { span, .. } => *span,
            Self::NonBoolCondition { span, .. } => *span,
            Self::MissingReturn { span, .. } => *span,
            Self::BranchTypeMismatch { span, .. } => *span,
            Self::NotAFunction { span, .. } => *span,
            Self::ArgumentTypeMismatch { span, .. } => *span,
            Self::FieldOnNonStruct { span, .. } => *span,
            Self::IndexOnNonArray { span, .. } => *span,
            Self::NonIntegerIndex { span, .. } => *span,
            Self::PropagateOnNonResult { span, .. } => *span,
            Self::UnknownStructField { span, .. } => *span,
            Self::UnknownEnumVariant { span, .. } => *span,
            Self::UnaryTypeMismatch { span, .. } => *span,
        }
    }

    pub fn file(&self) -> &str {
        match self {
            Self::TypeMismatch { file, .. } => file,
            Self::InvalidOperation { file, .. } => file,
            Self::NonBoolCondition { file, .. } => file,
            Self::MissingReturn { file, .. } => file,
            Self::BranchTypeMismatch { file, .. } => file,
            Self::NotAFunction { file, .. } => file,
            Self::ArgumentTypeMismatch { file, .. } => file,
            Self::FieldOnNonStruct { file, .. } => file,
            Self::IndexOnNonArray { file, .. } => file,
            Self::NonIntegerIndex { file, .. } => file,
            Self::PropagateOnNonResult { file, .. } => file,
            Self::UnknownStructField { file, .. } => file,
            Self::UnknownEnumVariant { file, .. } => file,
            Self::UnaryTypeMismatch { file, .. } => file,
        }
    }

    pub fn format(&self, source: &str) -> String {
        let span = self.span();
        let src_line = source.lines().nth(span.line.saturating_sub(1)).unwrap_or("");
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
            "\n🦎 LYZARD Type Error — {title}\n             ╭─ {}:{}:{}\n│\n             │  {src_line}\n│  {pointer}\n│\n             │  {message}{hint_line}\n╰─\n",
            self.file(),
            span.line,
            span.col
        )
    }

    fn describe(&self) -> (String, String, Option<String>) {
        match self {
            Self::TypeMismatch {
                expected,
                found,
                context,
                ..
            } => (
                "Type mismatch".to_string(),
                format!(
                    "In {context}: expected `{expected}` but found `{found}`."
                ),
                Some(format!(
                    "Change the value to `{expected}`, or update the type annotation to `{found}`."
                )),
            ),
            Self::InvalidOperation {
                op, left, right, ..
            } => (
                "Invalid operation".to_string(),
                format!("Cannot apply `{op}` to `{left}` and `{right}`."),
                Some(match op.as_str() {
                    "+" => format!(
                        "`+` works on int+int, float+float, str+str. Got {left} + {right}."
                    ),
                    "-" | "*" | "/" => format!(
                        "`{op}` only works on numeric types. Got `{left}` and `{right}`."
                    ),
                    _ => format!("Both operands must be compatible for `{op}`."),
                }),
            ),
            Self::NonBoolCondition {
                found, context, ..
            } => (
                "Non-boolean condition".to_string(),
                format!("`{context}` condition must be `bool`, found `{found}`."),
                Some(format!(
                    "Change the condition to produce a `bool`. Example: `{context} x > 0 {{`"
                )),
            ),
            Self::MissingReturn {
                fn_name, expected, ..
            } => (
                "Missing return".to_string(),
                format!(
                    "Function `{fn_name}` must return `{expected}` but has no return statement."
                ),
                Some(format!(
                    "Add `return value` at the end of `{fn_name}` where value is `{expected}`."
                )),
            ),
            Self::BranchTypeMismatch {
                then_type,
                else_type,
                ..
            } => (
                "Branch type mismatch".to_string(),
                format!(
                    "`then` branch is `{then_type}`, `else` branch is `{else_type}` — must match."
                ),
                Some("Make both branches return the same type.".to_string()),
            ),
            Self::NotAFunction {
                name, actual_type, ..
            } => (
                "Not a function".to_string(),
                format!(
                    "`{name}` is `{actual_type}` and cannot be called with `()`."
                ),
                Some(format!(
                    "Only functions can be called. `{name}` is `{actual_type}`."
                )),
            ),
            Self::ArgumentTypeMismatch {
                fn_name,
                param_index,
                expected,
                found,
                ..
            } => (
                "Argument type mismatch".to_string(),
                format!(
                    "Argument {} of `{fn_name}`: expected `{expected}`, got `{found}`.",
                    param_index + 1
                ),
                Some(format!(
                    "Pass a `{expected}` value, or update `{fn_name}`'s signature."
                )),
            ),
            Self::FieldOnNonStruct { found, field, .. } => (
                "Field on non-struct".to_string(),
                format!(
                    "Cannot access `.{field}` on `{found}` — only structs have fields."
                ),
                Some("Only struct values have named fields.".to_string()),
            ),
            Self::IndexOnNonArray { found, .. } => (
                "Index on non-array".to_string(),
                format!(
                    "Cannot use `[i]` on `{found}` — only arrays and strings support indexing."
                ),
                Some("Use `[i]` only on `[T]` arrays or `str` strings.".to_string()),
            ),
            Self::NonIntegerIndex { found, .. } => (
                "Non-integer index".to_string(),
                format!("Index must be `int`, found `{found}`."),
                Some("Use an integer expression: `arr[0]`, `arr[i]`.".to_string()),
            ),
            Self::PropagateOnNonResult { found, .. } => (
                "? on non-Result".to_string(),
                format!("`?` requires `Result<T, E>`, found `{found}`."),
                Some("Remove `?` or change the expression to return `Result<T, E>`.".to_string()),
            ),
            Self::UnknownStructField {
                struct_name,
                field,
                available,
                ..
            } => (
                "Unknown struct field".to_string(),
                format!("Struct `{struct_name}` has no field `{field}`."),
                if available.is_empty() {
                    Some(format!(
                        "Check the spelling — fields are defined in struct `{struct_name}`."
                    ))
                } else {
                    Some(format!("Available fields: {}", available.join(", ")))
                },
            ),
            Self::UnknownEnumVariant {
                enum_name,
                variant,
                available,
                ..
            } => (
                "Unknown enum variant".to_string(),
                format!("Enum `{enum_name}` has no variant `{variant}`."),
                if available.is_empty() {
                    Some(format!(
                        "Check the spelling — variants are defined in enum `{enum_name}`."
                    ))
                } else {
                    Some(format!("Available variants: {}", available.join(", ")))
                },
            ),
            Self::UnaryTypeMismatch { op, found, .. } => (
                "Unary type mismatch".to_string(),
                format!("`{op}` cannot be applied to `{found}`."),
                match op.as_str() {
                    "-" => Some(format!(
                        "Negation `-` requires numeric type. Got `{found}`."
                    )),
                    "!" => Some(format!(
                        "`!` requires `bool`. Got `{found}`. Try `x == false`?"
                    )),
                    _ => Some(format!("Check the operand type for unary `{op}`.")),
                },
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
    fn test_mismatch_display() {
        let e = TypeError::TypeMismatch {
            expected: ResolvedType::Int,
            found: ResolvedType::Str,
            span: s(),
            file: "t.lyz".to_string(),
            context: "variable".to_string(),
        };
        let msg = format!("{}", e);
        assert!(msg.contains("int") && msg.contains("str"));
    }

    #[test]
    fn test_format_emoji() {
        let e = TypeError::NonBoolCondition {
            found: ResolvedType::Int,
            span: s(),
            file: "t.lyz".to_string(),
            context: "if".to_string(),
        };
        assert!(e.format("").contains("🦎"));
    }

    #[test]
    fn test_missing_return_hint() {
        let e = TypeError::MissingReturn {
            fn_name: "calc".to_string(),
            expected: ResolvedType::Int,
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("calc"));
    }

    #[test]
    fn test_unknown_field_available() {
        let e = TypeError::UnknownStructField {
            struct_name: "Point".to_string(),
            field: "z".to_string(),
            available: vec!["x".to_string(), "y".to_string()],
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("x"));
    }

    #[test]
    fn test_implements_std_error() {
        fn takes_error<E: std::error::Error>(_: E) {}
        takes_error(TypeError::IndexOnNonArray {
            found: ResolvedType::Int,
            span: s(),
            file: "t".to_string(),
        });
    }

    #[test]
    fn test_type_errors_collection() {
        let mut errs = TypeErrors::new();
        errs.push(TypeError::IndexOnNonArray {
            found: ResolvedType::Int,
            span: s(),
            file: "t".to_string(),
        });
        assert_eq!(errs.len(), 1);
    }
}
