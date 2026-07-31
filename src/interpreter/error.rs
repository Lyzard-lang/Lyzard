use crate::lexer::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    /// Division by zero: 10 / 0
    DivisionByZero { span: Option<Span> },

    /// Type mismatch in operation: "hello" + 5
    TypeError { expected: String, got: String },

    /// A variable was used but has no value
    UndefinedVariable { name: String, span: Option<Span> },

    /// A function was called that doesn't exist
    UndefinedFunction { name: String, span: Option<Span> },

    /// Array index out of bounds: arr[99] when arr has 3 items
    IndexOutOfBounds {
        index: i64,
        length: usize,
        span: Option<Span>,
    },

    /// Tried to index a non-array value
    NotIndexable {
        type_name: String,
        span: Option<Span>,
    },

    /// Accessing a field that doesn't exist on a struct
    FieldNotFound {
        struct_name: String,
        field: String,
        span: Option<Span>,
    },

    /// Tried to call something that isn't a function
    NotCallable {
        type_name: String,
        span: Option<Span>,
    },

    /// Stack overflow from infinite recursion
    StackOverflow { fn_name: String },

    /// assert(false) was called
    AssertionFailed {
        message: Option<String>,
        span: Option<Span>,
    },

    /// panic("message") was called
    Panic { message: String, span: Option<Span> },

    /// Feature not yet implemented in the interpreter
    NotImplemented { feature: String },

    /// An Err() value was propagated with ? and not handled
    UnhandledError { message: String },
}

impl RuntimeError {
    pub fn format(&self, source: &str) -> String {
        let (title, message, hint) = self.describe();

        // Try to show source context if we have a span
        let location_line = self
            .span()
            .and_then(|s| source.lines().nth(s.line.saturating_sub(1)))
            .map(|line| {
                let span = self.span().unwrap();
                let pointer = format!(
                    "{}{}",
                    " ".repeat(span.col.saturating_sub(1)),
                    "^".repeat(span.len().max(1))
                );
                format!("\n│  {}\n│  {}", line, pointer)
            })
            .unwrap_or_default();

        let location = self
            .span()
            .map(|s| format!("{}:{}", s.line, s.col))
            .unwrap_or_else(|| "unknown location".to_string());

        let hint_line = hint
            .map(|h| format!("\n  💡 Hint: {}", h))
            .unwrap_or_default();

        format!(
            "\n🦎 LYZARD Runtime Error — {title}\n\
             ╭─ {location}{location_line}\n│\n\
             │  {message}{hint_line}\n\
             ╰─\n"
        )
    }

    fn span(&self) -> Option<Span> {
        match self {
            Self::DivisionByZero { span } => *span,
            Self::UndefinedVariable { span, .. } => *span,
            Self::UndefinedFunction { span, .. } => *span,
            Self::IndexOutOfBounds { span, .. } => *span,
            Self::NotIndexable { span, .. } => *span,
            Self::FieldNotFound { span, .. } => *span,
            Self::NotCallable { span, .. } => *span,
            Self::AssertionFailed { span, .. } => *span,
            Self::Panic { span, .. } => *span,
            _ => None,
        }
    }

    fn describe(&self) -> (String, String, Option<String>) {
        match self {
            Self::DivisionByZero { .. } => (
                "Division by zero".to_string(),
                "You tried to divide a number by zero, which is undefined.".to_string(),
                Some("Check that your divisor is not zero before dividing.".to_string()),
            ),
            Self::TypeError { expected, got } => (
                "Type mismatch".to_string(),
                format!("Expected a value of type '{}' but got '{}'.", expected, got),
                Some("LYZARD is type-safe — make sure you're using the right type.".to_string()),
            ),
            Self::UndefinedVariable { name, .. } => (
                "Undefined variable".to_string(),
                format!("Variable '{}' was used but was never declared.", name),
                Some(format!("Did you forget: let {} = ...?", name)),
            ),
            Self::UndefinedFunction { name, .. } => (
                "Undefined function".to_string(),
                format!("Function '{}' was called but was never defined.", name),
                Some(format!("Define it with: fn {}(...) {{ ... }}", name)),
            ),
            Self::IndexOutOfBounds { index, length, .. } => (
                "Index out of bounds".to_string(),
                format!(
                    "Tried to access index {} but the array has {} element(s).",
                    index, length
                ),
                Some("Array indices start at 0. Check your loop bounds.".to_string()),
            ),
            Self::NotIndexable { type_name, .. } => (
                "Not indexable".to_string(),
                format!("'{}' cannot be indexed with [].", type_name),
                Some("Only arrays and maps support [] indexing.".to_string()),
            ),
            Self::FieldNotFound {
                struct_name, field, ..
            } => (
                "Field not found".to_string(),
                format!("Struct '{}' has no field '{}'.", struct_name, field),
                Some("Check the struct definition for the correct field name.".to_string()),
            ),
            Self::NotCallable { type_name, .. } => (
                "Not callable".to_string(),
                format!(
                    "'{}' is not a function and cannot be called with ().",
                    type_name
                ),
                Some(
                    "Only functions can be called. Make sure you have the right variable."
                        .to_string(),
                ),
            ),
            Self::StackOverflow { fn_name } => (
                "Stack overflow".to_string(),
                format!(
                    "Function '{}' called itself too many times (infinite recursion?).",
                    fn_name
                ),
                Some(
                    "Check your recursive function for a base case that stops the recursion."
                        .to_string(),
                ),
            ),
            Self::AssertionFailed { message, .. } => (
                "Assertion failed".to_string(),
                message
                    .as_deref()
                    .unwrap_or("assert() was called with false.")
                    .to_string(),
                Some(
                    "This is an intentional check in your code that failed at runtime.".to_string(),
                ),
            ),
            Self::Panic { message, .. } => (
                "Program panicked".to_string(),
                format!("panic! was called: {}", message),
                Some(
                    "panic() is used for unrecoverable errors. Consider using Result<T,E> instead."
                        .to_string(),
                ),
            ),
            Self::NotImplemented { feature } => (
                "Not implemented".to_string(),
                format!(
                    "'{}' is not yet implemented in the LYZARD interpreter.",
                    feature
                ),
                Some("This feature is coming soon! Follow LYZARD development.".to_string()),
            ),
            Self::UnhandledError { message } => (
                "Unhandled error".to_string(),
                format!("An Err() value was not handled: {}", message),
                Some("Use match or ?? to handle the error, or add ? to propagate it.".to_string()),
            ),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (title, msg, _) = self.describe();
        write!(f, "{}: {}", title, msg)
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_division_by_zero_display() {
        let e = RuntimeError::DivisionByZero { span: None };
        assert!(format!("{}", e).contains("zero"));
    }

    #[test]
    fn test_type_error_display() {
        let e = RuntimeError::TypeError {
            expected: "int".to_string(),
            got: "str".to_string(),
        };
        let msg = format!("{}", e);
        assert!(msg.contains("int"));
        assert!(msg.contains("str"));
    }

    #[test]
    fn test_undefined_variable_hint() {
        let e = RuntimeError::UndefinedVariable {
            name: "myVar".to_string(),
            span: None,
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("myVar"));
    }

    #[test]
    fn test_index_out_of_bounds_display() {
        let e = RuntimeError::IndexOutOfBounds {
            index: 5,
            length: 3,
            span: None,
        };
        let msg = format!("{}", e);
        assert!(msg.contains("5"));
        assert!(msg.contains("3"));
    }

    #[test]
    fn test_stack_overflow_hint() {
        let e = RuntimeError::StackOverflow {
            fn_name: "fib".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.unwrap().contains("base case"));
    }

    #[test]
    fn test_format_has_emoji() {
        let e = RuntimeError::DivisionByZero { span: None };
        assert!(e.format("").contains("🦎"));
    }

    #[test]
    fn test_panic_message() {
        let e = RuntimeError::Panic {
            message: "something went wrong".to_string(),
            span: None,
        };
        let msg = format!("{}", e);
        assert!(msg.contains("something went wrong"));
    }

    #[test]
    fn test_implements_std_error() {
        fn takes_error<E: std::error::Error>(_: E) {}
        takes_error(RuntimeError::DivisionByZero { span: None });
    }
}
