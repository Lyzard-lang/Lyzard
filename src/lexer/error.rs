use crate::lexer::token::Span;

/// Every error the lexer can produce
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    /// Found a character we don't recognize at all
    UnexpectedChar {
        ch: char,
        span: Span,
        file: String,
    },

    /// String opened with " but never closed before end of file/line
    UnterminatedString {
        span: Span,
        file: String,
    },

    /// Char literal opened with ' but never closed
    UnterminatedChar {
        span: Span,
        file: String,
    },

    /// Char literal has more than one character: 'ab'
    InvalidCharLiteral {
        content: String,
        span: Span,
        file: String,
    },

    /// An escape sequence we don't support: "\q"
    InvalidEscape {
        ch: char,
        span: Span,
        file: String,
    },

    /// A number that can't be parsed: 999999999999999999999
    NumberOverflow {
        raw: String,
        span: Span,
        file: String,
    },

    /// Multi-line comment opened but never closed
    UnterminatedComment {
        span: Span,
        file: String,
    },
}

impl LexError {
    pub fn span(&self) -> &Span {
        match self {
            Self::UnexpectedChar { span, .. }      => span,
            Self::UnterminatedString { span, .. }  => span,
            Self::UnterminatedChar { span, .. }    => span,
            Self::InvalidCharLiteral { span, .. }  => span,
            Self::InvalidEscape { span, .. }       => span,
            Self::NumberOverflow { span, .. }      => span,
            Self::UnterminatedComment { span, .. } => span,
        }
    }

    pub fn file(&self) -> &str {
        match self {
            Self::UnexpectedChar { file, .. }      => file,
            Self::UnterminatedString { file, .. }  => file,
            Self::UnterminatedChar { file, .. }    => file,
            Self::InvalidCharLiteral { file, .. }  => file,
            Self::InvalidEscape { file, .. }       => file,
            Self::NumberOverflow { file, .. }      => file,
            Self::UnterminatedComment { file, .. } => file,
        }
    }

    /// Format a beautiful error message with source context
    pub fn format(&self, source: &str) -> String {
        let span = self.span();
        let source_line = source
            .lines()
            .nth(span.line.saturating_sub(1))
            .unwrap_or("");

        let pointer = format!(
            "{}{}",
            " ".repeat(span.col.saturating_sub(1)),
            "^".repeat(span.len().max(1))
        );

        let (title, message, hint) = match self {
            Self::UnexpectedChar { ch, .. } => (
                "Unexpected character",
                format!("I found the character '{}' here, but I don't know what it means in LYZARD.", ch),
                Some("Check for typos. Did you mean to use a letter or number here?".to_string()),
            ),
            Self::UnterminatedString { .. } => (
                "Unterminated string",
                "A string was opened with '\"' but was never closed before the end of the line.".to_string(),
                Some("Add a closing '\"' at the end of your string.".to_string()),
            ),
            Self::UnterminatedChar { .. } => (
                "Unterminated char literal",
                "A char literal was opened with ''' but was never closed.".to_string(),
                Some("A char literal looks like this: 'a'. Don't forget the closing '.".to_string()),
            ),
            Self::InvalidCharLiteral { content, .. } => (
                "Invalid char literal",
                format!("'{}' has more than one character. Char literals must contain exactly ONE character.", content),
                Some("Use a string literal if you need multiple characters: \"{}\"".to_string()),
            ),
            Self::InvalidEscape { ch, .. } => (
                "Unknown escape sequence",
                format!("'\\{}' is not a valid escape sequence.", ch),
                Some("Valid escapes: \\n (newline), \\t (tab), \\r (return), \\\\ (backslash), \\\" (quote)".to_string()),
            ),
            Self::NumberOverflow { raw, .. } => (
                "Number too large",
                format!("'{}' is too large to fit in a 64-bit integer.", raw),
                Some("LYZARD integers go up to 9,223,372,036,854,775,807. Use float for larger numbers.".to_string()),
            ),
            Self::UnterminatedComment { .. } => (
                "Unterminated comment",
                "A multi-line comment was opened with '/- ' but was never closed.".to_string(),
                Some("Close it with ' -/' at the end.".to_string()),
            ),
        };

        let hint_line = match hint {
            Some(h) => format!("\n  \u{1F4A1} Hint: {}", h),
            None    => String::new(),
        };

        format!(
            "\n\u{1F98E} LYZARD Lexer Error \u{2014} {title}\n\
             \u{256D}\u{2500} {}:{}:{}\n\
             \u{2502}\n\
             \u{2502}  {source_line}\n\
             \u{2502}  {pointer}\n\
             \u{2502}\n\
             \u{2502}  {message}{hint_line}\n\
             \u{2570}\u{2500}\n",
            self.file(), span.line, span.col
        )
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedChar { ch, span, .. } =>
                write!(f, "unexpected character '{}' at {}:{}", ch, span.line, span.col),
            Self::UnterminatedString { span, .. } =>
                write!(f, "unterminated string at {}:{}", span.line, span.col),
            Self::UnterminatedChar { span, .. } =>
                write!(f, "unterminated char literal at {}:{}", span.line, span.col),
            Self::InvalidCharLiteral { content, span, .. } =>
                write!(f, "invalid char literal '{}' at {}:{}", content, span.line, span.col),
            Self::InvalidEscape { ch, span, .. } =>
                write!(f, "unknown escape sequence '\\{}' at {}:{}", ch, span.line, span.col),
            Self::NumberOverflow { raw, span, .. } =>
                write!(f, "number '{}' overflows i64 at {}:{}", raw, span.line, span.col),
            Self::UnterminatedComment { span, .. } =>
                write!(f, "unterminated comment at {}:{}", span.line, span.col),
        }
    }
}

impl std::error::Error for LexError {}

#[cfg(test)]
mod error_tests {
    use super::*;
    use crate::lexer::token::Span;

    fn dummy_span() -> Span { Span::new(0, 1, 1, 1) }

    #[test]
    fn test_unexpected_char_display() {
        let err = LexError::UnexpectedChar {
            ch: '@',
            span: dummy_span(),
            file: "test.lyz".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains('@'));
        assert!(msg.contains("1:1"));
    }

    #[test]
    fn test_unterminated_string_display() {
        let err = LexError::UnterminatedString {
            span: Span::new(5, 6, 3, 10),
            file: "main.lyz".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("unterminated string"));
        assert!(msg.contains("3:10"));
    }

    #[test]
    fn test_format_with_source() {
        let source = "let x = @bad";
        let err = LexError::UnexpectedChar {
            ch: '@',
            span: Span::new(8, 9, 1, 9),
            file: "test.lyz".to_string(),
        };
        let formatted = err.format(source);
        assert!(formatted.contains("Unexpected character"));
        assert!(formatted.contains("@bad"));
        assert!(formatted.contains("Hint:"));
        assert!(formatted.contains("\u{1F98E}"));
    }

    #[test]
    fn test_invalid_escape_display() {
        let err = LexError::InvalidEscape {
            ch: 'q',
            span: dummy_span(),
            file: "test.lyz".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("\\q"));
    }

    #[test]
    fn test_unterminated_char_display() {
        let err = LexError::UnterminatedChar {
            span: dummy_span(),
            file: "test.lyz".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("unterminated char"));
    }

    #[test]
    fn test_invalid_char_literal_display() {
        let err = LexError::InvalidCharLiteral {
            content: "ab".to_string(),
            span: dummy_span(),
            file: "test.lyz".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("ab"));
        assert!(msg.contains("invalid char literal"));
    }

    #[test]
    fn test_number_overflow_display() {
        let err = LexError::NumberOverflow {
            raw: "99999999999999999999999999".to_string(),
            span: dummy_span(),
            file: "test.lyz".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("overflows"));
    }

    #[test]
    fn test_error_implements_std_error() {
        fn takes_error<E: std::error::Error>(_e: E) {}
        takes_error(LexError::UnterminatedString {
            span: dummy_span(),
            file: "x".to_string(),
        });
    }
}
