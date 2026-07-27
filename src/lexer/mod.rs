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
