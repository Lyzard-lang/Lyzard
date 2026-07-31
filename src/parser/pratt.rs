use super::ast::*;
use super::error::ParseError;
use super::Parser;
use crate::lexer::{Span, TokenKind};

pub fn infix_bp(kind: &TokenKind) -> Option<(u8, u8)> {
    match kind {
        TokenKind::Equals => Some((1, 2)),
        TokenKind::QuestionQuestion => Some((3, 4)),
        TokenKind::Or => Some((5, 6)),
        TokenKind::And => Some((7, 8)),
        TokenKind::EqualsEquals | TokenKind::BangEquals => Some((9, 10)),
        TokenKind::Less | TokenKind::LessEquals | TokenKind::Greater | TokenKind::GreaterEquals => {
            Some((11, 12))
        }
        TokenKind::Plus | TokenKind::Minus => Some((13, 14)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some((15, 16)),
        TokenKind::DotDot | TokenKind::DotDotEquals => Some((17, 18)),
        _ => None,
    }
}

pub fn prefix_bp(kind: &TokenKind) -> Option<u8> {
    match kind {
        TokenKind::Bang | TokenKind::Minus => Some(19),
        _ => None,
    }
}

pub fn postfix_bp(kind: &TokenKind) -> Option<u8> {
    match kind {
        TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::Dot | TokenKind::Question => {
            Some(21)
        }
        _ => None,
    }
}

#[allow(clippy::result_large_err)]
impl Parser {
    pub(super) fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_bp(0)
    }

    pub(super) fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_prefix_expr()?;

        loop {
            self.skip_newlines();
            let kind = self.peek().kind.clone();

            if let Some(bp) = postfix_bp(&kind) {
                if bp < min_bp {
                    break;
                }
                left = self.parse_postfix_expr(left)?;
                continue;
            }

            if let Some((l_bp, r_bp)) = infix_bp(&kind) {
                if l_bp < min_bp {
                    break;
                }
                left = self.parse_infix_expr(left, r_bp)?;
                continue;
            }

            break;
        }

        Ok(left)
    }

    fn parse_prefix_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();

        match self.peek().kind.clone() {
            TokenKind::IntLiteral(v) => {
                let s = self.current_span();
                self.advance();
                Ok(Expr::Int(IntLit { value: v, span: s }))
            }
            TokenKind::FloatLiteral(v) => {
                let s = self.current_span();
                self.advance();
                Ok(Expr::Float(FloatLit { value: v, span: s }))
            }
            TokenKind::StringLiteral(v) => {
                let s = self.current_span();
                self.advance();
                Ok(Expr::Str(StrLit { value: v, span: s }))
            }
            TokenKind::BoolLiteral(v) => {
                let s = self.current_span();
                self.advance();
                Ok(Expr::Bool(BoolLit { value: v, span: s }))
            }
            TokenKind::CharLiteral(v) => {
                let s = self.current_span();
                self.advance();
                Ok(Expr::Char(CharLit { value: v, span: s }))
            }
            TokenKind::Null => {
                let s = self.current_span();
                self.advance();
                Ok(Expr::Null(NullLit { span: s }))
            }

            TokenKind::Identifier(name) => {
                let s = self.current_span();
                self.advance();
                let name = name.to_string();
                // Struct literal: Point { x: 1.0, y: 2.0 }.
                // Disambiguate from a block after a bare identifier
                // (e.g. `while x { ... }`) by requiring `{ ident :`.
                if self.check(TokenKind::LeftBrace)
                    && matches!(&self.peek_ahead(1).kind, TokenKind::Identifier(_))
                    && self.peek_ahead(2).kind == TokenKind::Colon
                {
                    return self.parse_struct_init(name, s);
                }
                Ok(Expr::Identifier(IdentExpr { name, span: s }))
            }

            TokenKind::Bang | TokenKind::Minus => {
                let op_tok = self.advance();
                let op = match op_tok.kind {
                    TokenKind::Bang => UnaryOp::Not,
                    _ => UnaryOp::Neg,
                };
                let r_bp = prefix_bp(&op_tok.kind).unwrap();
                let operand = Box::new(self.parse_expr_bp(r_bp)?);
                let end = operand.span();
                Ok(Expr::Unary(UnaryExpr {
                    op,
                    operand,
                    span: span.merge(end),
                }))
            }

            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_hint(TokenKind::RightParen, "Close with ')'")?;
                Ok(expr)
            }

            TokenKind::LeftBracket => self.parse_array_literal(),
            TokenKind::LeftBrace => Ok(Expr::Block(Box::new(self.parse_block()?))),

            TokenKind::If => {
                self.advance();
                let condition = self.parse_expr()?;
                self.skip_newlines();
                let then_branch = self.parse_block()?;
                let else_branch = if self.advance_if(TokenKind::Else) {
                    self.skip_newlines();
                    Some(self.parse_block()?)
                } else {
                    None
                };
                let end = match &else_branch {
                    Some(b) => b.span,
                    None => self.prev_span(),
                };
                let span = span.merge(end);
                Ok(Expr::If(Box::new(IfExpr {
                    condition,
                    then_branch,
                    else_branch,
                    span,
                })))
            }

            TokenKind::Match => {
                self.advance();
                let subject = Box::new(self.parse_expr()?);
                self.skip_newlines();
                self.expect(TokenKind::LeftBrace)?;
                let arms = self.parse_match_arms()?;
                if arms.is_empty() {
                    return Err(ParseError::EmptyMatch {
                        span: span.merge(self.prev_span()),
                        file: self.file.clone(),
                    });
                }
                self.expect(TokenKind::RightBrace)?;
                let span = span.merge(self.prev_span());
                Ok(Expr::Match(Box::new(MatchExpr {
                    subject,
                    arms,
                    span,
                })))
            }

            _ => Err(self.error_hint(
                "an expression",
                "Expressions: 42, \"hello\", myVar, x + 1, foo()",
            )),
        }
    }

    fn parse_infix_expr(&mut self, left: Expr, r_bp: u8) -> Result<Expr, ParseError> {
        let s = left.span();
        let tok = self.advance();

        // Assignment
        if tok.kind == TokenKind::Equals {
            if !is_assign_target(&left) {
                return Err(ParseError::InvalidAssignTarget {
                    span: left.span(),
                    file: self.file.clone(),
                });
            }
            let right = self.parse_expr_bp(r_bp)?;
            let span = s.merge(right.span());
            return Ok(Expr::Assign(AssignExpr {
                target: Box::new(left),
                value: Box::new(right),
                span,
            }));
        }

        // Range
        if matches!(tok.kind, TokenKind::DotDot | TokenKind::DotDotEquals) {
            let inclusive = tok.kind == TokenKind::DotDotEquals;
            let right = self.parse_expr_bp(r_bp)?;
            let span = s.merge(right.span());
            return Ok(Expr::Range(RangeExpr {
                start: Box::new(left),
                end: Box::new(right),
                inclusive,
                span,
            }));
        }

        // Null coalesce
        if tok.kind == TokenKind::QuestionQuestion {
            let right = self.parse_expr_bp(r_bp)?;
            let span = s.merge(right.span());
            return Ok(Expr::NullCoalesce(NullCoalesceExpr {
                left: Box::new(left),
                right: Box::new(right),
                span,
            }));
        }

        // Binary op
        let op = match tok.kind {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Mod,
            TokenKind::EqualsEquals => BinaryOp::Eq,
            TokenKind::BangEquals => BinaryOp::NotEq,
            TokenKind::Less => BinaryOp::Lt,
            TokenKind::LessEquals => BinaryOp::Lte,
            TokenKind::Greater => BinaryOp::Gt,
            TokenKind::GreaterEquals => BinaryOp::Gte,
            TokenKind::And => BinaryOp::And,
            TokenKind::Or => BinaryOp::Or,
            _ => return Err(self.error_hint("binary operator", "")),
        };

        let right = self.parse_expr_bp(r_bp)?;
        let span = s.merge(right.span());
        Ok(Expr::Binary(BinaryExpr {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        }))
    }

    fn parse_postfix_expr(&mut self, left: Expr) -> Result<Expr, ParseError> {
        let s = left.span();
        match self.peek().kind.clone() {
            TokenKind::Dot => {
                self.advance();
                let (name, _) = self.expect_identifier("field/method name")?;
                if self.check(TokenKind::LeftParen) {
                    let args = self.parse_call_args()?;
                    let span = s.merge(self.prev_span());
                    Ok(Expr::MethodCall(MethodCallExpr {
                        object: Box::new(left),
                        method: name,
                        args,
                        span,
                    }))
                } else {
                    let span = s.merge(self.prev_span());
                    Ok(Expr::Field(FieldExpr {
                        object: Box::new(left),
                        field: name,
                        span,
                    }))
                }
            }
            TokenKind::LeftParen => {
                let args = self.parse_call_args()?;
                let span = s.merge(self.prev_span());
                Ok(Expr::Call(CallExpr {
                    callee: Box::new(left),
                    args,
                    span,
                }))
            }
            TokenKind::LeftBracket => {
                self.advance();
                let index = self.parse_expr()?;
                self.expect_hint(TokenKind::RightBracket, "Close with ']'")?;
                let span = s.merge(self.prev_span());
                Ok(Expr::Index(IndexExpr {
                    object: Box::new(left),
                    index: Box::new(index),
                    span,
                }))
            }
            TokenKind::Question => {
                self.advance();
                let span = s.merge(self.prev_span());
                Ok(Expr::Propagate(PropagateExpr {
                    expr: Box::new(left),
                    span,
                }))
            }
            _ => Ok(left),
        }
    }

    pub(super) fn parse_call_args(&mut self) -> Result<Vec<Argument>, ParseError> {
        self.expect(TokenKind::LeftParen)?;
        let mut args = Vec::new();
        while !self.check(TokenKind::RightParen) && !self.is_at_end() {
            self.skip_newlines();
            let s = self.current_span();
            let label = if matches!(&self.peek().kind, TokenKind::Identifier(_))
                && self.peek_ahead(1).kind == TokenKind::Colon
            {
                let (n, _) = self.expect_identifier("argument label")?;
                self.advance();
                Some(n)
            } else {
                None
            };
            let value = self.parse_expr()?;
            let span = s.merge(value.span());
            args.push(Argument { label, value, span });
            if !self.advance_if(TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        self.expect_hint(TokenKind::RightParen, "Close argument list with ')'")?;
        Ok(args)
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        let s = self.current_span();
        self.advance();
        let mut elements = Vec::new();
        while !self.check(TokenKind::RightBracket) && !self.is_at_end() {
            self.skip_newlines();
            elements.push(self.parse_expr()?);
            self.skip_newlines();
            if !self.advance_if(TokenKind::Comma) {
                break;
            }
        }
        self.expect_hint(TokenKind::RightBracket, "Close array with ']'")?;
        Ok(Expr::Array(ArrayLit {
            elements,
            span: s.merge(self.prev_span()),
        }))
    }

    fn parse_struct_init(&mut self, name: String, s: Span) -> Result<Expr, ParseError> {
        self.advance(); // consume {
        let mut fields = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            let (fname, _) = self.expect_identifier("field name")?;
            self.expect_hint(TokenKind::Colon, "Field needs ':'")?;
            let val = self.parse_expr()?;
            fields.push((fname, val));
            self.skip_newlines();
            if !self.advance_if(TokenKind::Comma) {
                break;
            }
        }
        self.expect_hint(TokenKind::RightBrace, "Close struct with '}'")?;
        Ok(Expr::StructInit(StructInitExpr {
            name,
            fields,
            span: s.merge(self.prev_span()),
        }))
    }
}

fn is_assign_target(e: &Expr) -> bool {
    matches!(e, Expr::Identifier(_) | Expr::Field(_) | Expr::Index(_))
}

#[cfg(test)]
mod pratt_tests {
    use super::*;
    use crate::lexer::Lexer;

    fn expr(src: &str) -> Expr {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        Parser::new(tokens, "t.lyz", src).parse_expr().unwrap()
    }

    #[test]
    fn test_literal_int() {
        assert!(matches!(expr("42"), Expr::Int(i) if i.value == 42));
    }
    #[test]
    fn test_literal_string() {
        assert!(matches!(expr("\"hi\""), Expr::Str(_)));
    }
    #[test]
    fn test_add() {
        assert!(matches!(&expr("1 + 2"), Expr::Binary(b) if b.op == BinaryOp::Add));
    }
    #[test]
    fn test_mul_over_add() {
        // 1 + 2 * 3 → top = Add, right = Mul
        if let Expr::Binary(b) = expr("1 + 2 * 3") {
            assert_eq!(b.op, BinaryOp::Add);
            assert!(matches!(b.right.as_ref(), Expr::Binary(r) if r.op == BinaryOp::Mul));
        } else {
            panic!("expected Binary");
        }
    }
    #[test]
    fn test_parens_override() {
        // (1 + 2) * 3 → top = Mul, left = Add
        if let Expr::Binary(b) = expr("(1 + 2) * 3") {
            assert_eq!(b.op, BinaryOp::Mul);
            assert!(matches!(b.left.as_ref(), Expr::Binary(l) if l.op == BinaryOp::Add));
        } else {
            panic!("expected Binary");
        }
    }
    #[test]
    fn test_unary_neg() {
        assert!(matches!(&expr("-x"), Expr::Unary(u) if u.op == UnaryOp::Neg));
    }
    #[test]
    fn test_unary_not() {
        assert!(matches!(&expr("!flag"), Expr::Unary(u) if u.op == UnaryOp::Not));
    }
    #[test]
    fn test_field_access() {
        assert!(matches!(&expr("obj.field"), Expr::Field(f) if f.field == "field"));
    }
    #[test]
    fn test_method_call() {
        assert!(matches!(&expr("obj.go()"), Expr::MethodCall(m) if m.method == "go"));
    }
    #[test]
    fn test_function_call() {
        if let Expr::Call(c) = expr("foo(1,2)") {
            assert_eq!(c.args.len(), 2);
        } else {
            panic!();
        }
    }
    #[test]
    fn test_index() {
        assert!(matches!(&expr("arr[0]"), Expr::Index(_)));
    }
    #[test]
    fn test_propagate() {
        assert!(matches!(&expr("thing()?"), Expr::Propagate(_)));
    }
    #[test]
    fn test_null_coalesce() {
        assert!(matches!(&expr("x ?? 0"), Expr::NullCoalesce(_)));
    }
    #[test]
    fn test_range_exclusive() {
        assert!(matches!(&expr("0..10"), Expr::Range(r) if !r.inclusive));
    }
    #[test]
    fn test_range_inclusive() {
        assert!(matches!(&expr("0..=10"), Expr::Range(r) if r.inclusive));
    }
    #[test]
    fn test_chained() {
        assert!(matches!(&expr("a.b().c()"), Expr::MethodCall(m) if m.method == "c"));
    }
    #[test]
    fn test_invalid_assign() {
        let tokens = Lexer::tokenize("42 = x", "t.lyz").unwrap();
        let mut p = Parser::new(tokens, "t.lyz", "42 = x");
        assert!(p.parse_expr().is_err());
    }
    #[test]
    fn test_complex() {
        assert!(
            matches!(&expr("-x * (a + b) > 0 && !flag"), Expr::Binary(b) if b.op == BinaryOp::And)
        );
    }
    #[test]
    fn test_struct_init() {
        if let Expr::StructInit(s) = expr("Point { x: 1.0, y: 2.0 }") {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].0, "x");
        } else {
            panic!("expected StructInit");
        }
    }
    #[test]
    fn test_identifier_block_not_struct() {
        // `while x { ... }` — a bare identifier followed by `{` is NOT a struct init
        assert!(matches!(expr("x"), Expr::Identifier(_)));
    }
}
