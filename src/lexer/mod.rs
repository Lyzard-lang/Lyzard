mod token;
mod error;

pub use token::{Token, TokenKind, Span};
pub use error::LexError;

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    byte_pos: usize,
    line: usize,
    col: usize,
    file: String,
}

impl Lexer {
    pub fn new(source: &str, file: impl Into<String>) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            byte_pos: 0,
            line: 1,
            col: 1,
            file: file.into(),
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn current(&self) -> char {
        if self.is_at_end() { '\0' } else { self.source[self.pos] }
    }

    fn peek(&self) -> char {
        if self.pos + 1 >= self.source.len() { '\0' } else { self.source[self.pos + 1] }
    }

    fn peek2(&self) -> char {
        if self.pos + 2 >= self.source.len() { '\0' } else { self.source[self.pos + 2] }
    }

    fn advance(&mut self) -> char {
        let ch = self.current();
        self.pos += 1;
        self.byte_pos += ch.len_utf8();
        self.col += 1;
        ch
    }

    fn advance_if(&mut self, expected: char) -> bool {
        if !self.is_at_end() && self.current() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance_while<F: Fn(char) -> bool>(&mut self, predicate: F) {
        while !self.is_at_end() && predicate(self.current()) {
            self.advance();
        }
    }

    fn current_pos(&self) -> usize {
        self.byte_pos
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.byte_pos, self.line, self.col.saturating_sub(self.byte_pos - start))
    }

    fn single_span(&self) -> Span {
        Span::new(
            self.byte_pos.saturating_sub(1),
            self.byte_pos,
            self.line,
            self.col.saturating_sub(1),
        )
    }

    fn make_token(&self, kind: TokenKind, span: Span) -> Token {
        Token::new(kind, span, self.file.clone())
    }

    fn is_digit(ch: char) -> bool {
        ch.is_ascii_digit()
    }

    fn is_alpha(ch: char) -> bool {
        ch.is_ascii_alphabetic() || ch == '_'
    }

    fn is_alphanumeric(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_'
    }

    fn is_whitespace(ch: char) -> bool {
        matches!(ch, ' ' | '\t' | '\r')
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() && Self::is_whitespace(self.current()) {
            self.advance();
        }
    }

    fn skip_comments(&mut self) -> Result<bool, LexError> {
        let mut skipped = false;

        loop {
            if self.current() == '-' && self.peek() == '-' {
                self.advance();
                self.advance();
                while !self.is_at_end() && self.current() != '\n' {
                    self.advance();
                }
                skipped = true;
                continue;
            }

            if self.current() == '/' && self.peek() == '-' {
                let start = self.current_pos();
                let start_line = self.line;
                let start_col = self.col;

                self.advance();
                self.advance();

                let mut closed = false;
                while !self.is_at_end() {
                    if self.current() == '-' && self.peek() == '/' {
                        self.advance();
                        self.advance();
                        closed = true;
                        break;
                    }
                    if self.current() == '\n' {
                        self.line += 1;
                        self.col = 1;
                        self.advance();
                    } else {
                        self.advance();
                    }
                }

                if !closed {
                    return Err(LexError::UnterminatedComment {
                        span: Span::new(start, self.byte_pos, start_line, start_col),
                        file: self.file.clone(),
                    });
                }

                skipped = true;
                continue;
            }

            break;
        }

        Ok(skipped)
    }

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let start = self.current_pos();
        let start_col = self.col;
        let start_line = self.line;

        if self.current() == '0' {
            match self.peek() {
                'x' | 'X' => return self.lex_hex(start, start_line, start_col),
                'b' | 'B' => return self.lex_binary(start, start_line, start_col),
                'o' | 'O' => return self.lex_octal(start, start_line, start_col),
                _ => {}
            }
        }

        let mut raw = String::with_capacity(16);
        self.collect_decimal_digits(&mut raw);

        if self.current() == '.' && Self::is_digit(self.peek()) {
            raw.push('.');
            self.advance();
            self.collect_decimal_digits(&mut raw);

            if matches!(self.current(), 'e' | 'E') {
                raw.push('e');
                self.advance();
                if matches!(self.current(), '+' | '-') {
                    raw.push(self.advance());
                }
                self.collect_decimal_digits(&mut raw);
            }

            let span = Span::new(start, self.byte_pos, start_line, start_col);
            let val: f64 = raw.parse().map_err(|_| LexError::NumberOverflow {
                raw: raw.clone(),
                span,
                file: self.file.clone(),
            })?;
            return Ok(self.make_token(TokenKind::FloatLiteral(val), span));
        }

        let span = Span::new(start, self.byte_pos, start_line, start_col);
        let val: i64 = raw.parse().map_err(|_| LexError::NumberOverflow {
            raw: raw.clone(),
            span,
            file: self.file.clone(),
        })?;
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn collect_decimal_digits(&mut self, buf: &mut String) {
        while !self.is_at_end() && (Self::is_digit(self.current()) || self.current() == '_') {
            if self.current() != '_' {
                buf.push(self.current());
            }
            self.advance();
        }
    }

    fn lex_hex(&mut self, start: usize, line: usize, col: usize) -> Result<Token, LexError> {
        self.advance();
        self.advance();

        let mut raw = String::with_capacity(8);
        while !self.is_at_end() && (self.current().is_ascii_hexdigit() || self.current() == '_') {
            if self.current() != '_' { raw.push(self.current()); }
            self.advance();
        }

        let span = Span::new(start, self.byte_pos, line, col);
        let val = i64::from_str_radix(&raw, 16).map_err(|_| LexError::NumberOverflow {
            raw: format!("0x{}", raw),
            span,
            file: self.file.clone(),
        })?;
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn lex_binary(&mut self, start: usize, line: usize, col: usize) -> Result<Token, LexError> {
        self.advance();
        self.advance();

        let mut raw = String::with_capacity(8);
        while !self.is_at_end() && matches!(self.current(), '0' | '1' | '_') {
            if self.current() != '_' { raw.push(self.current()); }
            self.advance();
        }

        let span = Span::new(start, self.byte_pos, line, col);
        let val = i64::from_str_radix(&raw, 2).map_err(|_| LexError::NumberOverflow {
            raw: format!("0b{}", raw),
            span,
            file: self.file.clone(),
        })?;
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn lex_octal(&mut self, start: usize, line: usize, col: usize) -> Result<Token, LexError> {
        self.advance();
        self.advance();

        let mut raw = String::with_capacity(8);
        while !self.is_at_end() && (matches!(self.current(), '0'..='7') || self.current() == '_') {
            if self.current() != '_' { raw.push(self.current()); }
            self.advance();
        }

        let span = Span::new(start, self.byte_pos, line, col);
        let val = i64::from_str_radix(&raw, 8).map_err(|_| LexError::NumberOverflow {
            raw: format!("0o{}", raw),
            span,
            file: self.file.clone(),
        })?;
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn lex_string(&mut self) -> Result<Token, LexError> {
        let start = self.current_pos();
        let start_line = self.line;
        let start_col = self.col;

        self.advance();

        let mut value = String::with_capacity(32);

        loop {
            if self.is_at_end() || self.current() == '\n' {
                return Err(LexError::UnterminatedString {
                    span: Span::new(start, self.byte_pos, start_line, start_col),
                    file: self.file.clone(),
                });
            }

            if self.current() == '"' {
                self.advance();
                break;
            }

            if self.current() == '\\' {
                self.advance();
                let escaped = self.lex_escape_sequence(start, start_line, start_col)?;
                value.push(escaped);
                continue;
            }

            value.push(self.advance());
        }

        let span = Span::new(start, self.byte_pos, start_line, start_col);
        Ok(self.make_token(TokenKind::StringLiteral(value), span))
    }

    fn lex_escape_sequence(
        &mut self,
        string_start: usize,
        string_line: usize,
        string_col: usize,
    ) -> Result<char, LexError> {
        if self.is_at_end() {
            return Err(LexError::UnterminatedString {
                span: Span::new(string_start, self.byte_pos, string_line, string_col),
                file: self.file.clone(),
            });
        }

        let escape_col = self.col;
        let escape_line = self.line;
        let ch = self.advance();

        match ch {
            'n'  => Ok('\n'),
            't'  => Ok('\t'),
            'r'  => Ok('\r'),
            '\\' => Ok('\\'),
            '"'  => Ok('"'),
            '\'' => Ok('\''),
            '0'  => Ok('\0'),
            'u'  => self.lex_unicode_escape(string_start, string_line, string_col),
            other => Err(LexError::InvalidEscape {
                ch: other,
                span: Span::new(
                    self.byte_pos.saturating_sub(2),
                    self.byte_pos,
                    escape_line,
                    escape_col.saturating_sub(1),
                ),
                file: self.file.clone(),
            }),
        }
    }

    fn lex_unicode_escape(
        &mut self,
        string_start: usize,
        string_line: usize,
        string_col: usize,
    ) -> Result<char, LexError> {
        if !self.advance_if('{') {
            return Err(LexError::InvalidEscape {
                ch: 'u',
                span: Span::new(string_start, self.byte_pos, string_line, string_col),
                file: self.file.clone(),
            });
        }

        let mut hex = String::with_capacity(6);
        while !self.is_at_end() && self.current() != '}' {
            if self.current().is_ascii_hexdigit() {
                hex.push(self.advance());
            } else {
                return Err(LexError::InvalidEscape {
                    ch: self.current(),
                    span: Span::new(string_start, self.byte_pos, string_line, string_col),
                    file: self.file.clone(),
                });
            }
        }

        if !self.advance_if('}') {
            return Err(LexError::UnterminatedString {
                span: Span::new(string_start, self.byte_pos, string_line, string_col),
                file: self.file.clone(),
            });
        }

        let codepoint = u32::from_str_radix(&hex, 16).map_err(|_| LexError::InvalidEscape {
            ch: 'u',
            span: Span::new(string_start, self.byte_pos, string_line, string_col),
            file: self.file.clone(),
        })?;

        char::from_u32(codepoint).ok_or_else(|| LexError::InvalidEscape {
            ch: 'u',
            span: Span::new(string_start, self.byte_pos, string_line, string_col),
            file: self.file.clone(),
        })
    }

    fn newline_is_significant(&self, last_token: Option<&TokenKind>) -> bool {
        match last_token {
            None => false,
            Some(kind) => matches!(kind,
                TokenKind::IntLiteral(_)    |
                TokenKind::FloatLiteral(_)  |
                TokenKind::StringLiteral(_) |
                TokenKind::BoolLiteral(_)   |
                TokenKind::CharLiteral(_)   |
                TokenKind::Identifier(_)    |
                TokenKind::RightParen       |
                TokenKind::RightBracket     |
                TokenKind::RightBrace       |
                TokenKind::Return           |
                TokenKind::Break            |
                TokenKind::Continue
            ),
        }
    }
}

#[cfg(test)]
mod lexer_core_tests {
    use super::*;

    #[test]
    fn test_new_lexer() {
        let lexer = Lexer::new("hello", "test.lyz");
        assert!(!lexer.is_at_end());
        assert_eq!(lexer.current(), 'h');
        assert_eq!(lexer.line, 1);
        assert_eq!(lexer.col, 1);
    }

    #[test]
    fn test_empty_source() {
        let lexer = Lexer::new("", "test.lyz");
        assert!(lexer.is_at_end());
        assert_eq!(lexer.current(), '\0');
    }

    #[test]
    fn test_advance() {
        let mut lexer = Lexer::new("abc", "test.lyz");
        assert_eq!(lexer.advance(), 'a');
        assert_eq!(lexer.current(), 'b');
        assert_eq!(lexer.advance(), 'b');
        assert_eq!(lexer.current(), 'c');
    }

    #[test]
    fn test_peek() {
        let lexer = Lexer::new("abc", "test.lyz");
        assert_eq!(lexer.current(), 'a');
        assert_eq!(lexer.peek(), 'b');
        assert_eq!(lexer.peek2(), 'c');
    }

    #[test]
    fn test_peek_at_end() {
        let lexer = Lexer::new("a", "test.lyz");
        assert_eq!(lexer.peek(), '\0');
        assert_eq!(lexer.peek2(), '\0');
    }

    #[test]
    fn test_advance_if_matches() {
        let mut lexer = Lexer::new("=>", "test.lyz");
        assert!(lexer.advance_if('='));
        assert_eq!(lexer.current(), '>');
        assert!(!lexer.advance_if('x'));
        assert_eq!(lexer.current(), '>');
    }

    #[test]
    fn test_advance_while() {
        let mut lexer = Lexer::new("   hello", "test.lyz");
        lexer.advance_while(|c| c == ' ');
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_character_classification() {
        assert!(Lexer::is_digit('5'));
        assert!(!Lexer::is_digit('a'));
        assert!(Lexer::is_alpha('a'));
        assert!(Lexer::is_alpha('_'));
        assert!(!Lexer::is_alpha('5'));
        assert!(Lexer::is_alphanumeric('a'));
        assert!(Lexer::is_alphanumeric('5'));
        assert!(Lexer::is_whitespace(' '));
        assert!(Lexer::is_whitespace('\t'));
        assert!(!Lexer::is_whitespace('\n'));
    }

    #[test]
    fn test_unicode_advance() {
        let mut lexer = Lexer::new("café", "test.lyz");
        assert_eq!(lexer.advance(), 'c');
        assert_eq!(lexer.advance(), 'a');
        assert_eq!(lexer.advance(), 'f');
        let e_with_accent = lexer.advance();
        assert_eq!(e_with_accent, 'é');
        assert_eq!(lexer.byte_pos, 5);
    }
}

#[cfg(test)]
mod whitespace_tests {
    use super::*;

    #[test]
    fn test_skip_spaces() {
        let mut lexer = Lexer::new("   hello", "test.lyz");
        lexer.skip_whitespace();
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_skip_tabs() {
        let mut lexer = Lexer::new("\t\thello", "test.lyz");
        lexer.skip_whitespace();
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_newline_not_skipped_by_whitespace() {
        let mut lexer = Lexer::new("\nhello", "test.lyz");
        lexer.skip_whitespace();
        assert_eq!(lexer.current(), '\n');
    }

    #[test]
    fn test_skip_single_line_comment() {
        let mut lexer = Lexer::new("-- this is a comment\nhello", "test.lyz");
        let skipped = lexer.skip_comments().unwrap();
        assert!(skipped);
        assert_eq!(lexer.current(), '\n');
    }

    #[test]
    fn test_skip_multi_line_comment() {
        let mut lexer = Lexer::new("/- this\nis\na comment -/hello", "test.lyz");
        let skipped = lexer.skip_comments().unwrap();
        assert!(skipped);
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_unterminated_comment_error() {
        let mut lexer = Lexer::new("/- this never closes", "test.lyz");
        let result = lexer.skip_comments();
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::UnterminatedComment { .. } => {}
            other => panic!("Expected UnterminatedComment, got {:?}", other),
        }
    }

    #[test]
    fn test_newline_significance() {
        let lexer = Lexer::new("", "test.lyz");

        assert!(lexer.newline_is_significant(Some(&TokenKind::RightParen)));
        assert!(lexer.newline_is_significant(Some(&TokenKind::Identifier("x".to_string()))));
        assert!(lexer.newline_is_significant(Some(&TokenKind::IntLiteral(42))));
        assert!(!lexer.newline_is_significant(Some(&TokenKind::Plus)));
        assert!(!lexer.newline_is_significant(Some(&TokenKind::LeftBrace)));
        assert!(!lexer.newline_is_significant(None));
    }

    #[test]
    fn test_multi_line_comment_updates_line_count() {
        let mut lexer = Lexer::new("/- line1\nline2\nline3 -/x", "test.lyz");
        lexer.skip_comments().unwrap();
        assert_eq!(lexer.line, 3);
        assert_eq!(lexer.current(), 'x');
    }
}

#[cfg(test)]
mod number_tests {
    use super::*;

    fn lex_one(src: &str) -> Result<Token, LexError> {
        let mut lexer = Lexer::new(src, "test.lyz");
        lexer.lex_number()
    }

    #[test]
    fn test_simple_integer() {
        let tok = lex_one("42").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(42));
    }

    #[test]
    fn test_zero() {
        let tok = lex_one("0").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(0));
    }

    #[test]
    fn test_large_integer() {
        let tok = lex_one("9223372036854775807").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(i64::MAX));
    }

    #[test]
    fn test_integer_with_underscores() {
        let tok = lex_one("1_000_000").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(1_000_000));
    }

    #[test]
    fn test_float() {
        let tok = lex_one("3.14").unwrap();
        assert_eq!(tok.kind, TokenKind::FloatLiteral(3.14));
    }

    #[test]
    fn test_float_scientific() {
        let tok = lex_one("1.5e10").unwrap();
        assert_eq!(tok.kind, TokenKind::FloatLiteral(1.5e10));
    }

    #[test]
    fn test_hex() {
        let tok = lex_one("0xFF").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(255));
    }

    #[test]
    fn test_binary() {
        let tok = lex_one("0b1010").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(10));
    }

    #[test]
    fn test_octal() {
        let tok = lex_one("0o17").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(15));
    }

    #[test]
    fn test_integer_overflow_error() {
        let result = lex_one("99999999999999999999999999");
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::NumberOverflow { .. } => {}
            other => panic!("Expected NumberOverflow, got {:?}", other),
        }
    }

    #[test]
    fn test_span_is_correct() {
        let tok = lex_one("123").unwrap();
        assert_eq!(tok.span.line, 1);
        assert_eq!(tok.span.col, 1);
        assert_eq!(tok.span.len(), 3);
    }
}

#[cfg(test)]
mod string_tests {
    use super::*;

    fn lex_str(src: &str) -> Result<Token, LexError> {
        let mut lexer = Lexer::new(src, "test.lyz");
        lexer.lex_string()
    }

    fn unwrap_str(tok: Token) -> String {
        match tok.kind {
            TokenKind::StringLiteral(s) => s,
            other => panic!("Expected StringLiteral, got {:?}", other),
        }
    }

    #[test]
    fn test_simple_string() {
        let s = unwrap_str(lex_str("\"hello\"").unwrap());
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_empty_string() {
        let s = unwrap_str(lex_str("\"\"").unwrap());
        assert_eq!(s, "");
    }

    #[test]
    fn test_escape_newline() {
        let s = unwrap_str(lex_str("\"line1\\nline2\"").unwrap());
        assert_eq!(s, "line1\nline2");
    }

    #[test]
    fn test_escape_tab() {
        let s = unwrap_str(lex_str("\"a\\tb\"").unwrap());
        assert_eq!(s, "a\tb");
    }

    #[test]
    fn test_escape_backslash() {
        let s = unwrap_str(lex_str("\"path\\\\to\\\\file\"").unwrap());
        assert_eq!(s, "path\\to\\file");
    }

    #[test]
    fn test_escape_quote() {
        let s = unwrap_str(lex_str("\"say \\\"hi\\\"\"").unwrap());
        assert_eq!(s, "say \"hi\"");
    }

    #[test]
    fn test_unicode_escape() {
        let s = unwrap_str(lex_str("\"\\u{0041}\"").unwrap());
        assert_eq!(s, "A");
    }

    #[test]
    fn test_emoji_unicode_escape() {
        let s = unwrap_str(lex_str("\"\\u{1F600}\"").unwrap());
        assert_eq!(s, "\u{1F600}");
    }

    #[test]
    fn test_unterminated_string() {
        let result = lex_str("\"never closed");
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::UnterminatedString { .. } => {}
            e => panic!("Expected UnterminatedString, got {:?}", e),
        }
    }

    #[test]
    fn test_invalid_escape() {
        let result = lex_str("\"\\q\"");
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::InvalidEscape { ch: 'q', .. } => {}
            e => panic!("Expected InvalidEscape('q'), got {:?}", e),
        }
    }

    #[test]
    fn test_string_with_utf8_content() {
        let s = unwrap_str(lex_str("\"\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}\"").unwrap());
        assert_eq!(s, "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}");
    }

    #[test]
    fn test_span_covers_full_string_including_quotes() {
        let tok = lex_str("\"hi\"").unwrap();
        assert_eq!(tok.span.len(), 4);
    }
}
