pub mod ast;
pub mod error;
pub mod pratt;

use crate::lexer::{Span, Token, TokenKind};
use ast::*;
use error::{ParseError, ParseErrors};

const MAX_ERRORS: usize = 20;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    file: String,
    #[allow(dead_code)]
    source: String,
    errors: ParseErrors,
}

#[allow(clippy::result_large_err)]
impl Parser {
    pub fn new(tokens: Vec<Token>, file: impl Into<String>, source: impl Into<String>) -> Self {
        Parser {
            tokens,
            pos: 0,
            file: file.into(),
            source: source.into(),
            errors: ParseErrors::new(),
        }
    }

    pub fn parse(mut self) -> Result<(Program, ParseErrors), ParseError> {
        let start = self.current_span();
        let mut declarations = Vec::new();
        self.skip_newlines();

        while !self.is_at_end() {
            if self.errors.len() >= MAX_ERRORS {
                self.errors.push(ParseError::TooManyErrors {
                    count: self.errors.len(),
                    span: self.current_span(),
                    file: self.file.clone(),
                });
                break;
            }
            match self.parse_declaration() {
                Ok(d) => declarations.push(d),
                Err(err) => {
                    self.errors.push(err);
                    self.synchronize();
                }
            }
            self.skip_newlines();
        }

        let span = start.merge(self.current_span());
        let program = Program { declarations, span };
        Ok((program, self.errors))
    }

    fn parse_declaration(&mut self) -> Result<Declaration, ParseError> {
        Err(ParseError::UnexpectedToken {
            expected: "declaration".to_string(),
            got: self.peek().kind.clone(),
            span: self.current_span(),
            file: self.file.clone(),
            hint: Some(
                "Declarations start with fn, let, const, struct, enum, impl, interface, import, or module."
                    .to_string(),
            ),
        })
    }

    // ── NAVIGATION ──────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    #[allow(dead_code)]
    fn peek_ahead(&self, n: usize) -> &Token {
        let idx = (self.pos + n).min(self.tokens.len() - 1);
        &self.tokens[idx]
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    #[allow(dead_code)]
    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if self.peek().kind == kind {
            return Ok(self.advance());
        }
        Err(ParseError::UnexpectedToken {
            expected: kind.name().to_string(),
            got: self.peek().kind.clone(),
            span: self.current_span(),
            file: self.file.clone(),
            hint: None,
        })
    }

    #[allow(dead_code)]
    fn expect_hint(&mut self, kind: TokenKind, hint: &str) -> Result<Token, ParseError> {
        if self.peek().kind == kind {
            return Ok(self.advance());
        }
        Err(ParseError::UnexpectedToken {
            expected: kind.name().to_string(),
            got: self.peek().kind.clone(),
            span: self.current_span(),
            file: self.file.clone(),
            hint: Some(hint.to_string()),
        })
    }

    #[allow(dead_code)]
    fn advance_if(&mut self, kind: TokenKind) -> bool {
        if self.peek().kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while self.check(TokenKind::Newline) {
            self.advance();
        }
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::EOF
    }

    fn current_span(&self) -> Span {
        self.peek().span
    }

    #[allow(dead_code)]
    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.peek().span
        } else {
            self.tokens[self.pos - 1].span
        }
    }

    #[allow(dead_code)]
    fn error_hint(&self, expected: impl Into<String>, hint: impl Into<String>) -> ParseError {
        ParseError::UnexpectedToken {
            expected: expected.into(),
            got: self.peek().kind.clone(),
            span: self.current_span(),
            file: self.file.clone(),
            hint: Some(hint.into()),
        }
    }

    #[allow(dead_code)]
    fn expect_identifier(&mut self, context: &str) -> Result<(String, Span), ParseError> {
        match &self.peek().kind {
            TokenKind::Identifier(n) => {
                let name = n.to_string();
                let span = self.current_span();
                self.advance();
                Ok((name, span))
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: format!("identifier ({})", context),
                got: self.peek().kind.clone(),
                span: self.current_span(),
                file: self.file.clone(),
                hint: Some(format!("A {} name must start with a letter or _", context)),
            }),
        }
    }

    #[allow(dead_code)]
    fn check_ident(&self, name: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Identifier(n) if n.as_ref() == name)
    }

    // ── ERROR RECOVERY ───────────────────────────

    fn synchronize(&mut self) {
        while !self.is_at_end() {
            if self.peek().kind == TokenKind::Newline {
                self.advance();
                return;
            }
            if matches!(
                self.peek().kind,
                TokenKind::Fn
                    | TokenKind::Let
                    | TokenKind::Const
                    | TokenKind::Struct
                    | TokenKind::Enum
                    | TokenKind::Impl
                    | TokenKind::Pub
                    | TokenKind::Import
                    | TokenKind::Module
            ) {
                return;
            }
            if self.peek().kind == TokenKind::RightBrace {
                return;
            }
            self.advance();
        }
    }
}

#[cfg(test)]
mod parser_core_tests {
    use super::*;
    use crate::lexer::Lexer;

    fn p(src: &str) -> Parser {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        Parser::new(tokens, "t.lyz", src)
    }

    #[test]
    fn test_peek_no_advance() {
        let parser = p("let x");
        assert_eq!(parser.peek().kind, TokenKind::Let);
        assert_eq!(parser.pos, 0);
    }
    #[test]
    fn test_advance_moves() {
        let mut parser = p("let x");
        parser.advance();
        assert!(matches!(parser.peek().kind, TokenKind::Identifier(_)));
    }
    #[test]
    fn test_check_true() {
        let parser = p("fn foo");
        assert!(parser.check(TokenKind::Fn));
    }
    #[test]
    fn test_check_false() {
        let parser = p("fn foo");
        assert!(!parser.check(TokenKind::Let));
    }
    #[test]
    fn test_expect_ok() {
        let mut parser = p("let x");
        assert!(parser.expect(TokenKind::Let).is_ok());
    }
    #[test]
    fn test_expect_err() {
        let mut parser = p("fn x");
        assert!(parser.expect(TokenKind::Let).is_err());
    }
    #[test]
    fn test_advance_if_true() {
        let mut parser = p("let x");
        assert!(parser.advance_if(TokenKind::Let));
        assert!(!parser.advance_if(TokenKind::Let));
    }
    #[test]
    fn test_is_at_end() {
        let parser = p("");
        assert!(parser.is_at_end());
    }
    #[test]
    fn test_peek_ahead() {
        let parser = p("let x = 5");
        assert_eq!(parser.peek_ahead(2).kind, TokenKind::Equals);
    }
    #[test]
    fn test_expect_identifier() {
        let mut parser = p("myVar");
        let (n, _) = parser.expect_identifier("var").unwrap();
        assert_eq!(n, "myVar");
    }
    #[test]
    fn test_synchronize_at_fn() {
        let mut parser = p("!! 42 fn foo");
        parser.advance();
        parser.advance();
        parser.synchronize();
        assert_eq!(parser.peek().kind, TokenKind::Fn);
    }
}
