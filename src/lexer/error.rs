use crate::lexer::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnexpectedChar {
        ch: char,
        span: Span,
        file: String,
    },
    UnterminatedComment {
        span: Span,
        file: String,
    },
    NumberOverflow {
        raw: String,
        span: Span,
        file: String,
    },
}

impl LexError {
    pub fn span(&self) -> &Span {
        match self {
            Self::UnexpectedChar { span, .. }     => span,
            Self::UnterminatedComment { span, .. } => span,
            Self::NumberOverflow { span, .. }      => span,
        }
    }

    pub fn file(&self) -> &str {
        match self {
            Self::UnexpectedChar { file, .. }     => file,
            Self::UnterminatedComment { file, .. } => file,
            Self::NumberOverflow { file, .. }      => file,
        }
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedChar { ch, span, .. } =>
                write!(f, "unexpected character '{}' at {}:{}", ch, span.line, span.col),
            Self::UnterminatedComment { span, .. } =>
                write!(f, "unterminated comment at {}:{}", span.line, span.col),
            Self::NumberOverflow { raw, span, .. } =>
                write!(f, "number '{}' overflows i64 at {}:{}", raw, span.line, span.col),
        }
    }
}

impl std::error::Error for LexError {}
