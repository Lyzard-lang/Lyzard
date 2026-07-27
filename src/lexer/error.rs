use crate::lexer::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnexpectedChar {
        ch: char,
        span: Span,
        file: String,
    },
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedChar { ch, span, .. } =>
                write!(f, "unexpected character '{}' at {}:{}", ch, span.line, span.col),
        }
    }
}

impl std::error::Error for LexError {}
