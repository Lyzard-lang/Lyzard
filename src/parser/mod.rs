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
        self.skip_newlines();
        let is_pub = self.advance_if(TokenKind::Pub);
        match self.peek().kind.clone() {
            TokenKind::Fn => Ok(Declaration::Function(self.parse_fn(is_pub)?)),
            TokenKind::Let => Ok(Declaration::Let(self.parse_let()?)),
            TokenKind::Const => Ok(Declaration::Const(self.parse_const()?)),
            TokenKind::Struct => Ok(Declaration::Struct(self.parse_struct(is_pub)?)),
            TokenKind::Enum => Ok(Declaration::Enum(self.parse_enum(is_pub)?)),
            TokenKind::Impl => Ok(Declaration::Impl(self.parse_impl()?)),
            TokenKind::Interface => Ok(Declaration::Interface(self.parse_interface(is_pub)?)),
            TokenKind::Import => Ok(Declaration::Import(self.parse_import()?)),
            TokenKind::Module => Ok(Declaration::Module(self.parse_module()?)),
            _ => {
                if is_pub {
                    return Err(self.error_hint(
                        "a declaration",
                        "'pub' must be followed by fn, struct, enum, or interface",
                    ));
                }
                Ok(Declaration::Statement(self.parse_statement()?))
            }
        }
    }

    // ── DECLARATIONS ──────────────────────────────

    fn parse_fn(&mut self, is_pub: bool) -> Result<FnDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'fn'
        let (name, _) = self.expect_identifier("function")?;
        let generics = self.parse_generics()?;
        let params = self.parse_params()?;
        let return_type = if self.advance_if(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.skip_newlines();
        let body = if self.advance_if(TokenKind::FatArrow) {
            let expr = self.parse_expr()?;
            FnBody::Arrow(Box::new(expr))
        } else {
            FnBody::Block(self.parse_block()?)
        };
        let span = start.merge(self.prev_span());
        Ok(FnDecl {
            name,
            generics,
            params,
            return_type,
            body,
            is_pub,
            span,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        self.expect(TokenKind::LeftParen)?;
        let mut params = Vec::new();
        while !self.check(TokenKind::RightParen) && !self.is_at_end() {
            self.skip_newlines();
            let s = self.current_span();
            if self.check_ident("self") {
                let (name, _) = self.expect_identifier("self parameter")?;
                params.push(Param {
                    name,
                    param_type: None,
                    is_self: true,
                    span: s,
                });
            } else {
                let (name, _) = self.expect_identifier("parameter")?;
                let param_type = if self.advance_if(TokenKind::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let span = s.merge(self.prev_span());
                params.push(Param {
                    name,
                    param_type,
                    is_self: false,
                    span,
                });
            }
            self.skip_newlines();
            if !self.advance_if(TokenKind::Comma) {
                break;
            }
        }
        self.expect_hint(TokenKind::RightParen, "Close parameter list with ')'")?;
        Ok(params)
    }

    fn parse_generics(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        if !self.advance_if(TokenKind::Less) {
            return Ok(Vec::new());
        }
        let mut params = Vec::new();
        while !self.check(TokenKind::Greater) && !self.is_at_end() {
            self.skip_newlines();
            let s = self.current_span();
            let (name, _) = self.expect_identifier("generic parameter")?;
            let mut bounds = Vec::new();
            if self.advance_if(TokenKind::Colon) {
                loop {
                    let (bound, _) = self.expect_identifier("trait bound")?;
                    bounds.push(bound);
                    if !self.advance_if(TokenKind::Plus) {
                        break;
                    }
                }
            }
            params.push(GenericParam {
                name,
                bounds,
                span: s.merge(self.prev_span()),
            });
            self.skip_newlines();
            if !self.advance_if(TokenKind::Comma) {
                break;
            }
        }
        self.expect_hint(TokenKind::Greater, "Close generic list with '>'")?;
        Ok(params)
    }

    fn parse_let(&mut self) -> Result<LetDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'let'
        let mutable = self.advance_if(TokenKind::Mut);
        let (name, _) = self.expect_identifier("variable")?;
        let type_annotation = if self.advance_if(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_hint(TokenKind::Equals, "let bindings need '= value'")?;
        let value = self.parse_expr()?;
        let span = start.merge(value.span());
        Ok(LetDecl {
            name,
            mutable,
            type_annotation,
            value,
            span,
        })
    }

    fn parse_const(&mut self) -> Result<ConstDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'const'
        let (name, _) = self.expect_identifier("constant")?;
        let type_annotation = if self.advance_if(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_hint(TokenKind::Equals, "const bindings need '= value'")?;
        let value = self.parse_expr()?;
        let span = start.merge(value.span());
        Ok(ConstDecl {
            name,
            type_annotation,
            value,
            span,
        })
    }

    fn parse_struct(&mut self, is_pub: bool) -> Result<StructDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'struct'
        let (name, _) = self.expect_identifier("struct")?;
        let generics = self.parse_generics()?;
        self.skip_newlines();
        self.expect_hint(TokenKind::LeftBrace, "Struct needs '{ fields }'")?;
        let mut fields = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            let f_start = self.current_span();
            let field_pub = self.advance_if(TokenKind::Pub);
            let (fname, _) = self.expect_identifier("field name")?;
            self.expect_hint(TokenKind::Colon, "Field needs ':' type")?;
            let field_type = self.parse_type()?;
            fields.push(StructField {
                name: fname,
                field_type,
                is_pub: field_pub,
                span: f_start.merge(self.prev_span()),
            });
            self.skip_newlines();
            if self.check(TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
            self.advance_if(TokenKind::Comma);
        }
        self.expect_hint(TokenKind::RightBrace, "Close struct with '}'")?;
        let span = start.merge(self.prev_span());
        Ok(StructDecl {
            name,
            generics,
            fields,
            is_pub,
            span,
        })
    }

    fn parse_enum(&mut self, is_pub: bool) -> Result<EnumDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'enum'
        let (name, _) = self.expect_identifier("enum")?;
        let generics = self.parse_generics()?;
        self.skip_newlines();
        self.expect_hint(TokenKind::LeftBrace, "Enum needs '{ variants }'")?;
        let mut variants = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            let v_start = self.current_span();
            let (vname, _) = self.expect_identifier("variant name")?;
            let kind = if self.advance_if(TokenKind::LeftParen) {
                let mut types = Vec::new();
                while !self.check(TokenKind::RightParen) && !self.is_at_end() {
                    self.skip_newlines();
                    types.push(self.parse_type()?);
                    self.skip_newlines();
                    if !self.advance_if(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect_hint(TokenKind::RightParen, "Close tuple variant with ')'")?;
                EnumVariantKind::Tuple(types)
            } else if self.advance_if(TokenKind::LeftBrace) {
                let mut fields = Vec::new();
                while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
                    self.skip_newlines();
                    let f_start = self.current_span();
                    let field_pub = self.advance_if(TokenKind::Pub);
                    let (fname, _) = self.expect_identifier("field name")?;
                    self.expect_hint(TokenKind::Colon, "Field needs ':' type")?;
                    let field_type = self.parse_type()?;
                    fields.push(StructField {
                        name: fname,
                        field_type,
                        is_pub: field_pub,
                        span: f_start.merge(self.prev_span()),
                    });
                    self.skip_newlines();
                    if self.check(TokenKind::RightBrace) || self.is_at_end() {
                        break;
                    }
                    self.advance_if(TokenKind::Comma);
                }
                self.expect_hint(TokenKind::RightBrace, "Close struct variant with '}'")?;
                EnumVariantKind::Struct(fields)
            } else {
                EnumVariantKind::Unit
            };
            variants.push(EnumVariant {
                name: vname,
                kind,
                span: v_start.merge(self.prev_span()),
            });
            self.skip_newlines();
            if self.check(TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
            self.advance_if(TokenKind::Comma);
        }
        self.expect_hint(TokenKind::RightBrace, "Close enum with '}'")?;
        let span = start.merge(self.prev_span());
        Ok(EnumDecl {
            name,
            generics,
            variants,
            is_pub,
            span,
        })
    }

    fn parse_impl(&mut self) -> Result<ImplDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'impl'
        let generics = self.parse_generics()?;
        let target_name = match self.parse_type()? {
            TypeExpr::Named(t) => t.name,
            TypeExpr::Generic(t) => t.name,
            other => {
                return Err(ParseError::UnexpectedToken {
                    expected: "a named type".to_string(),
                    got: self.peek().kind.clone(),
                    span: other.span(),
                    file: self.file.clone(),
                    hint: Some("impl targets a named type like Point or List<int>".to_string()),
                })
            }
        };
        let for_interface = if self.advance_if(TokenKind::For) {
            let (name, _) = self.expect_identifier("interface name")?;
            Some(name)
        } else {
            None
        };
        self.skip_newlines();
        self.expect_hint(TokenKind::LeftBrace, "impl needs '{ methods }'")?;
        let mut methods = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(TokenKind::RightBrace) {
                break;
            }
            let method_pub = self.advance_if(TokenKind::Pub);
            if !self.check(TokenKind::Fn) {
                return Err(self.error_hint("'fn'", "impl bodies contain methods"));
            }
            methods.push(self.parse_fn(method_pub)?);
        }
        self.expect_hint(TokenKind::RightBrace, "Close impl with '}'")?;
        let span = start.merge(self.prev_span());
        Ok(ImplDecl {
            target: target_name,
            generics,
            for_interface,
            methods,
            span,
        })
    }

    fn parse_interface(&mut self, is_pub: bool) -> Result<InterfaceDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'interface'
        let (name, _) = self.expect_identifier("interface")?;
        let generics = self.parse_generics()?;
        self.skip_newlines();
        self.expect_hint(TokenKind::LeftBrace, "Interface needs '{ methods }'")?;
        let mut methods = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            let m_start = self.current_span();
            self.advance_if(TokenKind::Pub);
            self.expect(TokenKind::Fn)?;
            let (name, _) = self.expect_identifier("method name")?;
            let params = self.parse_params()?;
            let return_type = if self.advance_if(TokenKind::Arrow) {
                Some(self.parse_type()?)
            } else {
                None
            };
            methods.push(InterfaceMethod {
                name,
                params,
                return_type,
                span: m_start.merge(self.prev_span()),
            });
            self.advance_if(TokenKind::Semicolon);
        }
        self.expect_hint(TokenKind::RightBrace, "Close interface with '}'")?;
        let span = start.merge(self.prev_span());
        Ok(InterfaceDecl {
            name,
            generics,
            methods,
            is_pub,
            span,
        })
    }

    fn parse_import(&mut self) -> Result<ImportDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'import'
        let mut path = Vec::new();
        let (first, _) = self.expect_identifier("module path")?;
        path.push(first);
        while self.advance_if(TokenKind::Dot) {
            if self.check(TokenKind::LeftBrace) {
                self.advance();
                let mut items = Vec::new();
                while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
                    self.skip_newlines();
                    let (item, _) = self.expect_identifier("import item")?;
                    items.push(item);
                    self.skip_newlines();
                    if !self.advance_if(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect_hint(TokenKind::RightBrace, "Close import items with '}'")?;
                let span = start.merge(self.prev_span());
                return Ok(ImportDecl {
                    path,
                    items: Some(items),
                    alias: None,
                    span,
                });
            }
            let (part, _) = self.expect_identifier("module path")?;
            path.push(part);
        }
        let alias = if self.check_ident("as") {
            self.advance();
            let (a, _) = self.expect_identifier("import alias")?;
            Some(a)
        } else {
            None
        };
        let span = start.merge(self.prev_span());
        Ok(ImportDecl {
            path,
            items: None,
            alias,
            span,
        })
    }

    fn parse_module(&mut self) -> Result<ModuleDecl, ParseError> {
        let start = self.current_span();
        self.advance(); // 'module'
        let (name, _) = self.expect_identifier("module name")?;
        self.skip_newlines();
        self.expect_hint(TokenKind::LeftBrace, "Module needs '{ declarations }'")?;
        let mut body = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            if self.errors.len() >= MAX_ERRORS {
                break;
            }
            self.skip_newlines();
            if self.check(TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
            match self.parse_declaration() {
                Ok(d) => body.push(d),
                Err(err) => {
                    self.errors.push(err);
                    self.synchronize();
                }
            }
        }
        self.expect_hint(TokenKind::RightBrace, "Close module with '}'")?;
        let span = start.merge(self.prev_span());
        Ok(ModuleDecl { name, body, span })
    }

    // ── TYPES ─────────────────────────────────────

    fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let start = self.current_span();
        let mut ty =
            match &self.peek().kind.clone() {
                TokenKind::Identifier(name) => {
                    let name = name.to_string();
                    let s = self.current_span();
                    self.advance();
                    if name == "never" {
                        TypeExpr::Never(s)
                    } else if name == "map" && self.check(TokenKind::LeftBracket) {
                        self.advance();
                        let key = self.parse_type()?;
                        self.expect_hint(TokenKind::Comma, "Map type needs 'map[K, V]'")?;
                        let value = self.parse_type()?;
                        self.expect_hint(TokenKind::RightBracket, "Close map type with ']'")?;
                        TypeExpr::Map(Box::new(key), Box::new(value), s.merge(self.prev_span()))
                    } else {
                        TypeExpr::Named(NamedType { name, span: s })
                    }
                }
                TokenKind::IntType
                | TokenKind::FloatType
                | TokenKind::BoolType
                | TokenKind::StrType
                | TokenKind::CharType
                | TokenKind::VoidType => {
                    let name = self.peek().kind.name().trim_matches('\'').to_string();
                    self.advance();
                    TypeExpr::Named(NamedType { name, span: start })
                }
                TokenKind::LeftParen => {
                    self.advance();
                    let mut types = Vec::new();
                    while !self.check(TokenKind::RightParen) && !self.is_at_end() {
                        self.skip_newlines();
                        types.push(self.parse_type()?);
                        self.skip_newlines();
                        if !self.advance_if(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect_hint(TokenKind::RightParen, "Close tuple type with ')'")?;
                    TypeExpr::Tuple(types, start.merge(self.prev_span()))
                }
                TokenKind::Fn => {
                    self.advance();
                    self.expect_hint(
                        TokenKind::LeftParen,
                        "Function type needs 'fn(params) -> ret'",
                    )?;
                    let mut params = Vec::new();
                    while !self.check(TokenKind::RightParen) && !self.is_at_end() {
                        self.skip_newlines();
                        params.push(self.parse_type()?);
                        self.skip_newlines();
                        if !self.advance_if(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect_hint(TokenKind::RightParen, "Close function type with ')'")?;
                    self.expect_hint(TokenKind::Arrow, "Function type needs '-> ret'")?;
                    let return_type = self.parse_type()?;
                    TypeExpr::Fn(params, Box::new(return_type), start.merge(self.prev_span()))
                }
                TokenKind::LeftBracket => {
                    let s = self.current_span();
                    self.advance();
                    let inner = self.parse_type()?;
                    self.expect_hint(TokenKind::RightBracket, "Close array type with ']'")?;
                    TypeExpr::Array(Box::new(inner), s.merge(self.prev_span()))
                }
                _ => return Err(self.error_hint(
                    "a type",
                    "Types: int, float, str, bool, char, void, Point, List<int>, map[K, V], int?",
                )),
            };

        if matches!(ty, TypeExpr::Named(_)) && self.advance_if(TokenKind::Less) {
            let mut args = Vec::new();
            while !self.check(TokenKind::Greater) && !self.is_at_end() {
                self.skip_newlines();
                args.push(self.parse_type()?);
                self.skip_newlines();
                if !self.advance_if(TokenKind::Comma) {
                    break;
                }
            }
            self.expect_hint(TokenKind::Greater, "Close generic type with '>'")?;
            if let TypeExpr::Named(n) = ty {
                ty = TypeExpr::Generic(GenericType {
                    name: n.name,
                    args,
                    span: start.merge(self.prev_span()),
                });
            }
        }

        while self.check(TokenKind::LeftBracket) || self.check(TokenKind::Question) {
            if self.advance_if(TokenKind::LeftBracket) {
                self.expect_hint(TokenKind::RightBracket, "Close array type with ']'")?;
                let span = start.merge(self.prev_span());
                ty = TypeExpr::Array(Box::new(ty), span);
            } else if self.advance_if(TokenKind::Question) {
                let span = start.merge(self.prev_span());
                ty = TypeExpr::Optional(Box::new(ty), span);
            } else {
                break;
            }
        }

        Ok(ty)
    }

    // ── BLOCKS & STATEMENTS ───────────────────────

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::LeftBrace)?;
        let mut statements = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
            statements.push(self.parse_statement()?);
            self.skip_newlines();
            self.advance_if(TokenKind::Semicolon);
            self.skip_newlines();
        }
        self.expect_hint(TokenKind::RightBrace, "Close block with '}'")?;
        Ok(Block {
            statements,
            span: start.merge(self.prev_span()),
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::Let => {
                let d = self.parse_let()?;
                let span = d.span;
                Ok(Statement::Let(LetDecl {
                    name: d.name,
                    mutable: d.mutable,
                    type_annotation: d.type_annotation,
                    value: d.value,
                    span,
                }))
            }
            TokenKind::Const => {
                let d = self.parse_const()?;
                let span = d.span;
                Ok(Statement::Const(ConstDecl {
                    name: d.name,
                    type_annotation: d.type_annotation,
                    value: d.value,
                    span,
                }))
            }
            TokenKind::Return => {
                let start = self.current_span();
                self.advance();
                let value = if matches!(
                    self.peek().kind,
                    TokenKind::Newline
                        | TokenKind::RightBrace
                        | TokenKind::EOF
                        | TokenKind::Semicolon
                ) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.advance_if(TokenKind::Semicolon);
                let span = start.merge(self.prev_span());
                Ok(Statement::Return(ReturnStmt { value, span }))
            }
            TokenKind::If => {
                let s = self.parse_if_statement()?;
                Ok(Statement::If(s))
            }
            TokenKind::While => {
                let start = self.current_span();
                self.advance();
                let condition = self.parse_expr()?;
                self.skip_newlines();
                let body = self.parse_block()?;
                let span = start.merge(self.prev_span());
                Ok(Statement::While(WhileStmt {
                    condition,
                    body,
                    span,
                }))
            }
            TokenKind::For => {
                let start = self.current_span();
                self.advance();
                let (variable, _) = self.expect_identifier("loop variable")?;
                self.expect(TokenKind::In)?;
                let iterable = self.parse_expr()?;
                self.skip_newlines();
                let body = self.parse_block()?;
                let span = start.merge(self.prev_span());
                Ok(Statement::For(ForStmt {
                    variable,
                    iterable,
                    body,
                    span,
                }))
            }
            TokenKind::Loop => {
                let start = self.current_span();
                self.advance();
                let label = if matches!(self.peek().kind, TokenKind::Identifier(_)) {
                    let (l, _) = self.expect_identifier("loop label")?;
                    Some(l)
                } else {
                    None
                };
                self.skip_newlines();
                let body = self.parse_block()?;
                let span = start.merge(self.prev_span());
                Ok(Statement::Loop(LoopStmt { body, label, span }))
            }
            TokenKind::Match => {
                let s = self.parse_match_statement()?;
                Ok(Statement::Match(s))
            }
            TokenKind::Spawn => {
                let start = self.current_span();
                self.advance();
                self.skip_newlines();
                let body = self.parse_block()?;
                let span = start.merge(self.prev_span());
                Ok(Statement::Spawn(SpawnStmt { body, span }))
            }
            TokenKind::Break => {
                let start = self.current_span();
                self.advance();
                let label = if matches!(self.peek().kind, TokenKind::Identifier(_)) {
                    let (l, _) = self.expect_identifier("loop label")?;
                    Some(l)
                } else {
                    None
                };
                self.advance_if(TokenKind::Semicolon);
                let span = start.merge(self.prev_span());
                Ok(Statement::Break(BreakStmt { label, span }))
            }
            TokenKind::Continue => {
                let start = self.current_span();
                self.advance();
                let label = if matches!(self.peek().kind, TokenKind::Identifier(_)) {
                    let (l, _) = self.expect_identifier("loop label")?;
                    Some(l)
                } else {
                    None
                };
                self.advance_if(TokenKind::Semicolon);
                let span = start.merge(self.prev_span());
                Ok(Statement::Continue(ContinueStmt { label, span }))
            }
            TokenKind::LeftBrace => {
                let b = self.parse_block()?;
                Ok(Statement::Block(b))
            }
            _ => {
                let s = self.current_span();
                let expr = self.parse_expr()?;
                let span = s.merge(expr.span());
                Ok(Statement::Expression(ExprStmt { expr, span }))
            }
        }
    }

    fn parse_if_statement(&mut self) -> Result<IfStmt, ParseError> {
        let start = self.current_span();
        self.advance(); // 'if'
        let condition = self.parse_expr()?;
        self.skip_newlines();
        let then_branch = self.parse_block()?;
        let mut else_if_branches = Vec::new();
        let mut else_branch = None;
        while self.check(TokenKind::Else) {
            self.advance();
            if self.advance_if(TokenKind::If) {
                let e_start = self.prev_span();
                let condition = self.parse_expr()?;
                self.skip_newlines();
                let body = self.parse_block()?;
                let span = e_start.merge(self.prev_span());
                else_if_branches.push(ElseIfBranch {
                    condition,
                    body,
                    span,
                });
            } else {
                self.skip_newlines();
                else_branch = Some(self.parse_block()?);
                break;
            }
        }
        let span = start.merge(self.prev_span());
        Ok(IfStmt {
            condition,
            then_branch,
            else_if_branches,
            else_branch,
            span,
        })
    }

    fn parse_match_statement(&mut self) -> Result<MatchStmt, ParseError> {
        let start = self.current_span();
        self.advance(); // 'match'
        let subject = self.parse_expr()?;
        self.skip_newlines();
        self.expect(TokenKind::LeftBrace)?;
        let arms = self.parse_match_arms()?;
        if arms.is_empty() {
            return Err(ParseError::EmptyMatch {
                span: start.merge(self.prev_span()),
                file: self.file.clone(),
            });
        }
        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(self.prev_span());
        Ok(MatchStmt {
            subject,
            arms,
            span,
        })
    }

    fn parse_match_arms(&mut self) -> Result<Vec<MatchArm>, ParseError> {
        let mut arms = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(TokenKind::RightBrace) {
                break;
            }
            let s = self.current_span();
            let pattern = self.parse_pattern()?;
            let guard = if self.advance_if(TokenKind::If) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect_hint(TokenKind::Arrow, "Match arm needs '->' before its body")?;
            self.skip_newlines();
            let body = if self.check(TokenKind::LeftBrace) {
                MatchBody::Block(self.parse_block()?)
            } else if self.check(TokenKind::Return) {
                let stmt = self.parse_statement()?;
                let span = s.merge(self.prev_span());
                MatchBody::Block(Block {
                    statements: vec![stmt],
                    span,
                })
            } else {
                MatchBody::Expr(self.parse_expr()?)
            };
            arms.push(MatchArm {
                pattern,
                guard,
                body,
                span: s.merge(self.prev_span()),
            });
        }
        Ok(arms)
    }

    // ── PATTERNS ─────────────────────────────────

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let start = self.current_span();
        let first = self.parse_single_pattern()?;
        if !self.check(TokenKind::Pipe) {
            return Ok(first);
        }
        let mut alternatives = vec![first];
        while self.advance_if(TokenKind::Pipe) {
            self.skip_newlines();
            alternatives.push(self.parse_single_pattern()?);
        }
        let span = start.merge(self.prev_span());
        Ok(Pattern::Or(OrPattern { alternatives, span }))
    }

    fn parse_single_pattern(&mut self) -> Result<Pattern, ParseError> {
        let start = self.current_span();
        match self.peek().kind.clone() {
            TokenKind::Identifier(name) => {
                let name = name.to_string();
                self.advance();
                if name == "_" {
                    return Ok(Pattern::Wildcard(start));
                }
                if self.check(TokenKind::Dot) {
                    self.advance();
                    let (variant_name, _) = self.expect_identifier("variant name")?;
                    let bindings = if self.check(TokenKind::LeftParen) {
                        self.parse_pattern_bindings()?
                    } else {
                        Vec::new()
                    };
                    let span = start.merge(self.prev_span());
                    return Ok(Pattern::EnumVariant(EnumVariantPattern {
                        enum_name: Some(name),
                        variant_name,
                        bindings,
                        span,
                    }));
                }
                if self.check(TokenKind::LeftParen) {
                    let bindings = self.parse_pattern_bindings()?;
                    let span = start.merge(self.prev_span());
                    return Ok(Pattern::EnumVariant(EnumVariantPattern {
                        enum_name: None,
                        variant_name: name,
                        bindings,
                        span,
                    }));
                }
                Ok(Pattern::Binding(BindingPattern {
                    name,
                    mutable: false,
                    span: start,
                }))
            }
            TokenKind::Mut => {
                self.advance();
                let (name, _) = self.expect_identifier("binding name")?;
                let span = start.merge(self.prev_span());
                Ok(Pattern::Binding(BindingPattern {
                    name,
                    mutable: true,
                    span,
                }))
            }
            TokenKind::IntLiteral(v) => {
                self.advance();
                Ok(Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Int(v),
                    span: start,
                }))
            }
            TokenKind::Minus => {
                self.advance();
                if let TokenKind::IntLiteral(v) = self.peek().kind.clone() {
                    self.advance();
                    let span = start.merge(self.prev_span());
                    Ok(Pattern::Literal(LiteralPattern {
                        value: LiteralValue::Int(-v),
                        span,
                    }))
                } else {
                    Err(self.error_hint("integer after '-'", "Negative patterns: -42"))
                }
            }
            TokenKind::FloatLiteral(v) => {
                self.advance();
                Ok(Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Float(v),
                    span: start,
                }))
            }
            TokenKind::StringLiteral(v) => {
                self.advance();
                Ok(Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Str(v),
                    span: start,
                }))
            }
            TokenKind::BoolLiteral(v) => {
                self.advance();
                Ok(Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Bool(v),
                    span: start,
                }))
            }
            TokenKind::CharLiteral(v) => {
                self.advance();
                Ok(Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Char(v),
                    span: start,
                }))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Null,
                    span: start,
                }))
            }
            _ => Err(self.error_hint(
                "a pattern",
                "Patterns: _, x, mut x, 42, \"hi\", Some(x), Color.Red, a | b",
            )),
        }
    }

    fn parse_pattern_bindings(&mut self) -> Result<Vec<Pattern>, ParseError> {
        self.expect(TokenKind::LeftParen)?;
        let mut bindings = Vec::new();
        while !self.check(TokenKind::RightParen) && !self.is_at_end() {
            self.skip_newlines();
            bindings.push(self.parse_pattern()?);
            self.skip_newlines();
            if !self.advance_if(TokenKind::Comma) {
                break;
            }
        }
        self.expect_hint(TokenKind::RightParen, "Close pattern bindings with ')'")?;
        Ok(bindings)
    }

    // ── NAVIGATION ──────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

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

    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.peek().span
        } else {
            self.tokens[self.pos - 1].span
        }
    }

    fn error_hint(&self, expected: impl Into<String>, hint: impl Into<String>) -> ParseError {
        ParseError::UnexpectedToken {
            expected: expected.into(),
            got: self.peek().kind.clone(),
            span: self.current_span(),
            file: self.file.clone(),
            hint: Some(hint.into()),
        }
    }

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
                    | TokenKind::Interface
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

#[cfg(test)]
mod decl_tests {
    use super::*;
    use crate::lexer::Lexer;

    fn first_decl(src: &str) -> Declaration {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (program, errors) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        assert!(errors.is_empty(), "errors: {}", errors.format_all(src));
        program
            .declarations
            .into_iter()
            .next()
            .expect("expected at least one declaration")
    }

    #[test]
    fn test_fn_no_args_no_return() {
        let d = first_decl("fn main() { }");
        if let Declaration::Function(f) = d {
            assert_eq!(f.name, "main");
            assert!(f.generics.is_empty());
            assert!(f.params.is_empty());
            assert!(f.return_type.is_none());
            assert!(!f.is_pub);
            assert!(matches!(f.body, FnBody::Block(b) if b.statements.is_empty()));
        } else {
            panic!("expected Function declaration");
        }
    }

    #[test]
    fn test_fn_with_params_and_return() {
        let d = first_decl("fn add(a: int, b: int) -> int { let x = a + b }");
        if let Declaration::Function(f) = d {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name, "a");
            assert!(!f.params[0].is_self);
            assert!(matches!(
                f.params[0].param_type,
                Some(TypeExpr::Named(ref n)) if n.name == "int"
            ));
            assert!(matches!(f.return_type, Some(TypeExpr::Named(ref n)) if n.name == "int"));
            if let FnBody::Block(b) = f.body {
                assert_eq!(b.statements.len(), 1);
                assert!(matches!(b.statements[0], Statement::Let(_)));
            } else {
                panic!("expected block body");
            }
        } else {
            panic!("expected Function declaration");
        }
    }

    #[test]
    fn test_fn_arrow_body() {
        let d = first_decl("fn double(x: int) -> int => x * 2");
        if let Declaration::Function(f) = d {
            assert!(matches!(f.body, FnBody::Arrow(_)));
        } else {
            panic!("expected Function declaration");
        }
    }

    #[test]
    fn test_fn_generics() {
        let d = first_decl("fn max<T: Comparable>(a: T, b: T) -> T { a }");
        if let Declaration::Function(f) = d {
            assert_eq!(f.generics.len(), 1);
            assert_eq!(f.generics[0].name, "T");
            assert_eq!(f.generics[0].bounds, vec!["Comparable"]);
            assert!(matches!(
                f.params[0].param_type,
                Some(TypeExpr::Named(ref n)) if n.name == "T"
            ));
        } else {
            panic!("expected Function declaration");
        }
    }

    #[test]
    fn test_fn_self_param() {
        let d = first_decl("fn area(self) -> float { 0.0 }");
        if let Declaration::Function(f) = d {
            assert_eq!(f.params.len(), 1);
            assert!(f.params[0].is_self);
            assert_eq!(f.params[0].name, "self");
        } else {
            panic!("expected Function declaration");
        }
    }

    #[test]
    fn test_pub_fn_decl() {
        let d = first_decl("pub fn helper() { }");
        if let Declaration::Function(f) = d {
            assert!(f.is_pub);
        } else {
            panic!("expected Function declaration");
        }
    }

    #[test]
    fn test_let_decl() {
        let d = first_decl("let x = 5");
        if let Declaration::Let(l) = d {
            assert_eq!(l.name, "x");
            assert!(!l.mutable);
            assert!(l.type_annotation.is_none());
            assert!(matches!(l.value, Expr::Int(i) if i.value == 5));
        } else {
            panic!("expected Let declaration");
        }
    }

    #[test]
    fn test_let_mut_typed() {
        let d = first_decl("let mut count: int = 10");
        if let Declaration::Let(l) = d {
            assert!(l.mutable);
            assert!(matches!(
                l.type_annotation,
                Some(TypeExpr::Named(ref n)) if n.name == "int"
            ));
        } else {
            panic!("expected Let declaration");
        }
    }

    #[test]
    fn test_const_decl() {
        let d = first_decl("const MAX: int = 100");
        if let Declaration::Const(c) = d {
            assert_eq!(c.name, "MAX");
            assert!(matches!(
                c.type_annotation,
                Some(TypeExpr::Named(ref n)) if n.name == "int"
            ));
            assert!(matches!(c.value, Expr::Int(i) if i.value == 100));
        } else {
            panic!("expected Const declaration");
        }
    }

    #[test]
    fn test_struct_decl() {
        let d = first_decl("struct Point { x: float, y: float }");
        if let Declaration::Struct(s) = d {
            assert_eq!(s.name, "Point");
            assert!(!s.is_pub);
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].name, "x");
            assert!(!s.fields[0].is_pub);
            assert!(matches!(s.fields[1].field_type, TypeExpr::Named(ref n) if n.name == "float"));
        } else {
            panic!("expected Struct declaration");
        }
    }

    #[test]
    fn test_struct_pub_generic() {
        let d = first_decl("pub struct Box<T> { pub value: T }");
        if let Declaration::Struct(s) = d {
            assert!(s.is_pub);
            assert_eq!(s.generics.len(), 1);
            assert_eq!(s.generics[0].name, "T");
            assert!(s.fields[0].is_pub);
            assert!(matches!(s.fields[0].field_type, TypeExpr::Named(ref n) if n.name == "T"));
        } else {
            panic!("expected Struct declaration");
        }
    }

    #[test]
    fn test_struct_complex_types() {
        let d = first_decl("struct Profile { age: int?, tags: str[], first: Box<int> }");
        if let Declaration::Struct(s) = d {
            assert!(
                matches!(s.fields[0].field_type, TypeExpr::Optional(ref inner, _)
                if matches!(inner.as_ref(), TypeExpr::Named(ref n) if n.name == "int"))
            );
            assert!(
                matches!(s.fields[1].field_type, TypeExpr::Array(ref inner, _)
                if matches!(inner.as_ref(), TypeExpr::Named(ref n) if n.name == "str"))
            );
            assert!(matches!(s.fields[2].field_type, TypeExpr::Generic(ref g)
                if g.name == "Box" && g.args.len() == 1));
        } else {
            panic!("expected Struct declaration");
        }
    }

    #[test]
    fn test_enum_unit_variants() {
        let d = first_decl("enum Color { Red, Green, Blue }");
        if let Declaration::Enum(e) = d {
            assert_eq!(e.name, "Color");
            assert!(!e.is_pub);
            assert_eq!(e.variants.len(), 3);
            assert!(matches!(e.variants[0].kind, EnumVariantKind::Unit));
            assert_eq!(e.variants[2].name, "Blue");
        } else {
            panic!("expected Enum declaration");
        }
    }

    #[test]
    fn test_enum_tuple_and_struct_variants() {
        let d = first_decl("enum Shape { Circle(float), Point { x: float, y: float } }");
        if let Declaration::Enum(e) = d {
            assert_eq!(e.variants.len(), 2);
            assert!(matches!(&e.variants[0].kind, EnumVariantKind::Tuple(t) if t.len() == 1));
            assert!(matches!(&e.variants[1].kind, EnumVariantKind::Struct(f) if f.len() == 2));
        } else {
            panic!("expected Enum declaration");
        }
    }

    #[test]
    fn test_impl_with_methods() {
        let d = first_decl("impl Point { fn x(self) -> float { self.x } }");
        if let Declaration::Impl(i) = d {
            assert_eq!(i.target, "Point");
            assert!(i.generics.is_empty());
            assert!(i.for_interface.is_none());
            assert_eq!(i.methods.len(), 1);
            assert_eq!(i.methods[0].name, "x");
            assert!(i.methods[0].params[0].is_self);
        } else {
            panic!("expected Impl declaration");
        }
    }

    #[test]
    fn test_impl_generic_for() {
        let d = first_decl("impl<T> Foo<T> for Bar { fn go(self) { } }");
        if let Declaration::Impl(i) = d {
            assert_eq!(i.target, "Foo");
            assert_eq!(i.generics.len(), 1);
            assert_eq!(i.for_interface.as_deref(), Some("Bar"));
            assert_eq!(i.methods.len(), 1);
        } else {
            panic!("expected Impl declaration");
        }
    }

    #[test]
    fn test_interface_decl() {
        let d = first_decl("interface Shape { fn area() -> float; fn name() -> str; }");
        if let Declaration::Interface(i) = d {
            assert_eq!(i.name, "Shape");
            assert!(!i.is_pub);
            assert_eq!(i.methods.len(), 2);
            assert_eq!(i.methods[0].name, "area");
            assert!(
                matches!(i.methods[0].return_type, Some(TypeExpr::Named(ref n)) if n.name == "float")
            );
            assert!(i.methods[0].params.is_empty());
        } else {
            panic!("expected Interface declaration");
        }
    }

    #[test]
    fn test_import_path() {
        let d = first_decl("import foo.bar.baz");
        if let Declaration::Import(i) = d {
            assert_eq!(i.path, vec!["foo", "bar", "baz"]);
            assert!(i.items.is_none());
            assert!(i.alias.is_none());
        } else {
            panic!("expected Import declaration");
        }
    }

    #[test]
    fn test_import_alias() {
        let d = first_decl("import foo.bar as fb");
        if let Declaration::Import(i) = d {
            assert_eq!(i.path, vec!["foo", "bar"]);
            assert_eq!(i.alias.as_deref(), Some("fb"));
        } else {
            panic!("expected Import declaration");
        }
    }

    #[test]
    fn test_import_items() {
        let d = first_decl("import foo.{a, b}");
        if let Declaration::Import(i) = d {
            assert_eq!(i.path, vec!["foo"]);
            assert_eq!(
                i.items.as_deref(),
                Some(&["a".to_string(), "b".to_string()][..])
            );
        } else {
            panic!("expected Import declaration");
        }
    }

    #[test]
    fn test_module_decl() {
        let d = first_decl("module math { fn abs(x: int) -> int { x } }");
        if let Declaration::Module(m) = d {
            assert_eq!(m.name, "math");
            assert_eq!(m.body.len(), 1);
            assert!(matches!(m.body[0], Declaration::Function(_)));
        } else {
            panic!("expected Module declaration");
        }
    }

    #[test]
    fn test_top_level_expression_statement() {
        let d = first_decl("main()");
        assert!(matches!(
            d,
            Declaration::Statement(Statement::Expression(_))
        ));
    }

    #[test]
    fn test_multiple_declarations() {
        let src = "fn a() { }\nfn b() { }";
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (program, errors) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        assert!(errors.is_empty());
        assert_eq!(program.declarations.len(), 2);
    }
}

#[cfg(test)]
mod stmt_type_pat_tests {
    use super::*;
    use crate::lexer::Lexer;

    fn in_fn(src: &str) -> Statement {
        let wrapped = format!("fn __t__() {{ {} }}", src);
        let t = Lexer::tokenize(&wrapped, "t.lyz").unwrap();
        let (p, e) = Parser::new(t, "t.lyz", &wrapped).parse().unwrap();
        if !e.is_empty() {
            panic!("{}", e.format_all(&wrapped));
        }
        if let Declaration::Function(f) = &p.declarations[0] {
            if let FnBody::Block(b) = &f.body {
                return b.statements[0].clone();
            }
        }
        panic!("no stmt");
    }

    fn parse_type(src: &str) -> TypeExpr {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        Parser::new(t, "t.lyz", src).parse_type().unwrap()
    }

    fn parse_pat(src: &str) -> Pattern {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        Parser::new(t, "t.lyz", src).parse_pattern().unwrap()
    }

    // Statements
    #[test]
    fn test_return_value() {
        assert!(matches!(&in_fn("return 42"), Statement::Return(r) if r.value.is_some()));
    }
    #[test]
    fn test_return_none() {
        assert!(matches!(&in_fn("return"), Statement::Return(r) if r.value.is_none()));
    }
    #[test]
    fn test_if_else() {
        if let Statement::If(i) = in_fn("if x>0 { return 1 } else { return 0 }") {
            assert!(i.else_branch.is_some());
        }
    }
    #[test]
    fn test_else_if() {
        if let Statement::If(i) =
            in_fn("if a>0 { return 1 } else if a<0 { return -1 } else { return 0 }")
        {
            assert_eq!(i.else_if_branches.len(), 1);
        }
    }
    #[test]
    fn test_while() {
        assert!(matches!(
            in_fn("while x > 0 { x = x - 1 }"),
            Statement::While(_)
        ));
    }
    #[test]
    fn test_for() {
        if let Statement::For(f) = in_fn("for i in 0..10 { print(i) }") {
            assert_eq!(f.variable, "i");
        }
    }
    #[test]
    fn test_loop_break() {
        assert!(matches!(in_fn("loop { break }"), Statement::Loop(_)));
    }
    #[test]
    fn test_match_arms() {
        if let Statement::Match(m) = in_fn("match x { 0 -> print(x) _ -> print(x) }") {
            assert_eq!(m.arms.len(), 2);
        }
    }
    #[test]
    fn test_spawn() {
        assert!(matches!(in_fn("spawn { doWork() }"), Statement::Spawn(_)));
    }
    #[test]
    fn test_let_in_block() {
        assert!(matches!(in_fn("let y = 10"), Statement::Let(_)));
    }

    // Types
    #[test]
    fn test_type_int() {
        assert!(matches!(parse_type("int"), TypeExpr::Named(_)));
    }
    #[test]
    fn test_type_optional() {
        assert!(matches!(parse_type("str?"), TypeExpr::Optional(_, _)));
    }
    #[test]
    fn test_type_array() {
        assert!(matches!(parse_type("[float]"), TypeExpr::Array(_, _)));
    }
    #[test]
    fn test_type_generic() {
        assert!(matches!(
            parse_type("Result<int,str>"),
            TypeExpr::Generic(_)
        ));
    }
    #[test]
    fn test_type_tuple() {
        assert!(matches!(parse_type("(int, str)"), TypeExpr::Tuple(_, _)));
    }

    // Patterns
    #[test]
    fn test_pat_wildcard() {
        assert!(matches!(parse_pat("_"), Pattern::Wildcard(_)));
    }
    #[test]
    fn test_pat_binding() {
        assert!(matches!(parse_pat("x"), Pattern::Binding(_)));
    }
    #[test]
    fn test_pat_int() {
        assert!(matches!(parse_pat("42"), Pattern::Literal(_)));
    }
    #[test]
    fn test_pat_neg() {
        if let Pattern::Literal(l) = parse_pat("-5") {
            assert_eq!(l.value, LiteralValue::Int(-5));
        }
    }
    #[test]
    fn test_pat_enum() {
        assert!(matches!(
            parse_pat("Shape.Circle(r)"),
            Pattern::EnumVariant(_)
        ));
    }
    #[test]
    fn test_pat_or() {
        assert!(matches!(parse_pat("0 | 1"), Pattern::Or(_)));
    }
}
