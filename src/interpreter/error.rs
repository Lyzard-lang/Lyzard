use std::fmt;

use crate::lexer::Span;

/// A runtime failure raised while executing a program.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    UndefinedName {
        name: String,
        span: Span,
    },
    TypeError {
        expected: String,
        got: String,
    },
    DivisionByZero {
        span: Span,
    },
    IndexOutOfBounds {
        index: i64,
        len: usize,
        span: Span,
    },
    NegativeIndex {
        index: i64,
        span: Span,
    },
    FieldNotFound {
        name: String,
        object_type: String,
        span: Span,
    },
    MethodNotFound {
        name: String,
        object_type: String,
        span: Span,
    },
    WrongArgCount {
        name: String,
        expected: usize,
        got: usize,
        span: Span,
    },
    NotCallable {
        value_type: String,
        span: Span,
    },
    StackOverflow {
        fn_name: String,
    },
    InvalidOperation {
        message: String,
        span: Span,
    },
    NotImplemented {
        feature: String,
        span: Span,
    },
    SignalEscape {
        name: String,
        span: Span,
    },
}

impl RuntimeError {
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::UndefinedName { span, .. }
            | Self::DivisionByZero { span, .. }
            | Self::IndexOutOfBounds { span, .. }
            | Self::NegativeIndex { span, .. }
            | Self::FieldNotFound { span, .. }
            | Self::MethodNotFound { span, .. }
            | Self::WrongArgCount { span, .. }
            | Self::NotCallable { span, .. }
            | Self::InvalidOperation { span, .. }
            | Self::NotImplemented { span, .. }
            | Self::SignalEscape { span, .. } => Some(*span),
            Self::TypeError { .. } => None,
            Self::StackOverflow { .. } => None,
        }
    }

    pub fn file(&self) -> &'static str {
        "<runtime>"
    }

    pub fn message(&self) -> String {
        self.describe()
    }

    pub fn describe(&self) -> String {
        match self {
            Self::UndefinedName { name, .. } => format!("undefined name '{name}'"),
            Self::TypeError { expected, got } => {
                format!("type error: expected {expected}, got {got}")
            }
            Self::DivisionByZero { .. } => "division by zero".to_string(),
            Self::IndexOutOfBounds { index, len, .. } => {
                format!("index out of bounds: index {index} but length is {len}")
            }
            Self::NegativeIndex { index, .. } => {
                format!("negative index {index} is not allowed here")
            }
            Self::FieldNotFound {
                name, object_type, ..
            } => {
                format!("no field '{name}' on {object_type}")
            }
            Self::MethodNotFound {
                name, object_type, ..
            } => {
                format!("no method '{name}' on {object_type}")
            }
            Self::WrongArgCount {
                name,
                expected,
                got,
                ..
            } => format!("function '{name}' expects {expected} argument(s) but got {got}"),
            Self::NotCallable { value_type, .. } => format!("{value_type} is not callable"),
            Self::StackOverflow { fn_name } => {
                format!("stack overflow: call depth exceeded in '{fn_name}'")
            }
            Self::InvalidOperation { message, .. } => message.clone(),
            Self::NotImplemented { feature, .. } => format!("not implemented: {feature}"),
            Self::SignalEscape { name, .. } => {
                format!("control-flow signal '{name}' escaped to the top level")
            }
        }
    }

    /// Renders `file: message` plus the offending source line with a caret
    /// underline, so errors read well in a terminal.
    pub fn format(&self, source: &str) -> String {
        let mut out = format!("{}: {}\n", self.file(), self.describe());
        if let Some(span) = self.span() {
            let line = source
                .lines()
                .nth(span.line.saturating_sub(1))
                .unwrap_or("");
            let indent = span.col.saturating_sub(1);
            let width = span.len().max(1);
            out.push_str(line);
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            out.push_str(&"^".repeat(width));
            out.push('\n');
        }
        out
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.describe())
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod error_tests {
    use super::*;

    fn s() -> Span {
        Span::new(0, 3, 1, 1)
    }

    #[test]
    fn test_undefined_name_describe() {
        let e = RuntimeError::UndefinedName {
            name: "myVar".to_string(),
            span: s(),
        };
        assert_eq!(e.describe(), "undefined name 'myVar'");
        assert_eq!(e.span(), Some(s()));
    }

    #[test]
    fn test_type_error_describe() {
        let e = RuntimeError::TypeError {
            expected: "int".to_string(),
            got: "string".to_string(),
        };
        assert_eq!(e.describe(), "type error: expected int, got string");
        assert_eq!(e.span(), None);
    }

    #[test]
    fn test_division_by_zero_describe() {
        let e = RuntimeError::DivisionByZero { span: s() };
        assert_eq!(e.describe(), "division by zero");
    }

    #[test]
    fn test_index_out_of_bounds_describe() {
        let e = RuntimeError::IndexOutOfBounds {
            index: 5,
            len: 3,
            span: s(),
        };
        assert_eq!(e.describe(), "index out of bounds: index 5 but length is 3");
    }

    #[test]
    fn test_negative_index_describe() {
        let e = RuntimeError::NegativeIndex {
            index: -2,
            span: s(),
        };
        assert_eq!(e.describe(), "negative index -2 is not allowed here");
    }

    #[test]
    fn test_field_and_method_not_found_describe() {
        let e = RuntimeError::FieldNotFound {
            name: "z".to_string(),
            object_type: "Point".to_string(),
            span: s(),
        };
        assert_eq!(e.describe(), "no field 'z' on Point");

        let e = RuntimeError::MethodNotFound {
            name: "fly".to_string(),
            object_type: "Bird".to_string(),
            span: s(),
        };
        assert_eq!(e.describe(), "no method 'fly' on Bird");
    }

    #[test]
    fn test_wrong_arg_count_describe() {
        let e = RuntimeError::WrongArgCount {
            name: "add".to_string(),
            expected: 2,
            got: 3,
            span: s(),
        };
        assert_eq!(
            e.describe(),
            "function 'add' expects 2 argument(s) but got 3"
        );
    }

    #[test]
    fn test_not_callable_describe() {
        let e = RuntimeError::NotCallable {
            value_type: "int".to_string(),
            span: s(),
        };
        assert_eq!(e.describe(), "int is not callable");
    }

    #[test]
    fn test_stack_overflow_describe() {
        let e = RuntimeError::StackOverflow {
            fn_name: "fib".to_string(),
        };
        assert_eq!(e.describe(), "stack overflow: call depth exceeded in 'fib'");
    }

    #[test]
    fn test_format_shows_source_line() {
        let source = "let x = unknownVar + 1";
        let e = RuntimeError::UndefinedName {
            name: "unknownVar".to_string(),
            span: Span::new(8, 18, 1, 9),
        };
        let out = e.format(source);
        assert!(out.contains("undefined name 'unknownVar'"));
        assert!(out.contains("let x = unknownVar + 1"));
        assert!(out.contains("^^^^^^^^^^"));
    }

    #[test]
    fn test_implements_std_error() {
        fn takes_error<E: std::error::Error>(_: E) {}
        takes_error(RuntimeError::StackOverflow {
            fn_name: "main".to_string(),
        });
    }
}
