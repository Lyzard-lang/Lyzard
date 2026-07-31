use crate::lexer::{Span, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken {
        expected: String,
        got: TokenKind,
        span: Span,
        file: String,
        hint: Option<String>,
    },
    UnexpectedEof {
        expected: String,
        span: Span,
        file: String,
    },
    InvalidContext {
        what: String,
        context: String,
        span: Span,
        file: String,
    },
    EmptyMatch {
        span: Span,
        file: String,
    },
    InvalidAssignTarget {
        span: Span,
        file: String,
    },
    TooManyErrors {
        count: usize,
        span: Span,
        file: String,
    },
}

impl ParseError {
    pub fn span(&self) -> Span {
        match self {
            Self::UnexpectedToken { span, .. } => *span,
            Self::UnexpectedEof { span, .. } => *span,
            Self::InvalidContext { span, .. } => *span,
            Self::EmptyMatch { span, .. } => *span,
            Self::InvalidAssignTarget { span, .. } => *span,
            Self::TooManyErrors { span, .. } => *span,
        }
    }

    pub fn file(&self) -> &str {
        match self {
            Self::UnexpectedToken { file, .. } => file,
            Self::UnexpectedEof { file, .. } => file,
            Self::InvalidContext { file, .. } => file,
            Self::EmptyMatch { file, .. } => file,
            Self::InvalidAssignTarget { file, .. } => file,
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
            .map(|h| format!("\n  \u{1F4A1} Hint: {}", h))
            .unwrap_or_default();

        format!(
            "\n\u{1F98E} LYZARD Parser Error \u{2014} {title}\n             \
             \u{256D}\u{2500} {}:{}:{}\n             \
             \u{2502}\n             \
             \u{2502}  {src_line}\n             \
             \u{2502}  {pointer}\n             \
             \u{2502}\n             \
             \u{2502}  {message}{hint_line}\n             \
             \u{2570}\u{2500}\n",
            self.file(),
            span.line,
            span.col
        )
    }

    fn describe(&self) -> (String, String, Option<String>) {
        match self {
            Self::UnexpectedToken {
                expected,
                got,
                hint,
                ..
            } => (
                "Unexpected token".to_string(),
                format!("Expected {} but found {}.", expected, got.name()),
                hint.clone().or_else(|| {
                    Some(format!(
                        "Remove or replace {} with {}.",
                        got.name(),
                        expected
                    ))
                }),
            ),
            Self::UnexpectedEof { expected, .. } => (
                "Unexpected end of file".to_string(),
                format!("File ended while I expected {}.", expected),
                Some("Make sure all your blocks are closed with \'}\'".to_string()),
            ),
            Self::InvalidContext { what, context, .. } => (
                "Invalid statement context".to_string(),
                format!("\'{}\'  cannot be used {}.", what, context),
                match what.as_str() {
                    "return" => {
                        Some("\'return\' can only be used inside a function body.".to_string())
                    }
                    "break" => Some("\'break\' can only be used inside a loop.".to_string()),
                    "continue" => Some("\'continue\' can only be used inside a loop.".to_string()),
                    _ => None,
                },
            ),
            Self::EmptyMatch { .. } => (
                "Empty match expression".to_string(),
                "A match must have at least one arm.".to_string(),
                Some("Add arms like: 0 -> \"zero\", _ -> \"other\"".to_string()),
            ),
            Self::InvalidAssignTarget { .. } => (
                "Invalid assignment target".to_string(),
                "Left side of \'=\' must be a variable, field, or index expression.".to_string(),
                Some("Valid targets: x = 5, obj.field = 5, arr[i] = 5".to_string()),
            ),
            Self::TooManyErrors { count, .. } => (
                "Too many errors".to_string(),
                format!(
                    "Parser stopped after {} errors. Fix earlier errors first.",
                    count
                ),
                Some("Errors usually cascade \u{2014} fix the first one and re-run.".to_string()),
            ),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (title, msg, _) = self.describe();
        write!(f, "{}: {}", title, msg)
    }
}
impl std::error::Error for ParseError {}

#[derive(Debug, Default)]
pub struct ParseErrors(pub Vec<ParseError>);
impl ParseErrors {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn push(&mut self, e: ParseError) {
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
mod parse_error_tests {
    use super::*;
    use crate::lexer::Span;
    fn s() -> Span {
        Span::new(0, 3, 1, 1)
    }

    #[test]
    fn test_display() {
        let e = ParseError::UnexpectedToken {
            expected: "identifier".to_string(),
            got: TokenKind::IntLiteral(5),
            span: s(),
            file: "t.lyz".to_string(),
            hint: None,
        };
        assert!(format!("{}", e).contains("Unexpected token"));
    }

    #[test]
    fn test_format_has_emoji_and_hint() {
        let src = "fn 42bad() {}";
        let e = ParseError::UnexpectedToken {
            expected: "function name".to_string(),
            got: TokenKind::IntLiteral(42),
            span: Span::new(3, 5, 1, 4),
            file: "t.lyz".to_string(),
            hint: Some("Names must start with a letter".to_string()),
        };
        let out = e.format(src);
        assert!(out.contains("\u{1F98E}"));
        assert!(out.contains("^^"));
        assert!(out.contains("Hint:"));
    }

    #[test]
    fn test_empty_match_hint() {
        let e = ParseError::EmptyMatch {
            span: s(),
            file: "t.lyz".to_string(),
        };
        let (_, _, hint) = e.describe();
        assert!(hint.is_some());
    }

    #[test]
    fn test_implements_std_error() {
        fn takes_error<E: std::error::Error>(_: E) {}
        takes_error(ParseError::UnexpectedEof {
            expected: "\'}\'".to_string(),
            span: s(),
            file: "t.lyz".to_string(),
        });
    }

    #[test]
    fn test_parse_errors_collection() {
        let mut errs = ParseErrors::new();
        assert!(errs.is_empty());
        errs.push(ParseError::EmptyMatch {
            span: s(),
            file: "t.lyz".to_string(),
        });
        assert_eq!(errs.len(), 1);
    }
}
