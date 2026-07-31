pub mod error;
pub mod scope;
pub mod symbol;

use crate::lexer::Span;
use crate::parser::ast::*;
use error::{SemanticError, SemanticErrors};
#[allow(unused_imports)]
use scope::{Scope, ScopeKind, SymbolTable};
use symbol::*;

const MAX_ERRORS: usize = 30;

/// Context the analyzer tracks while walking the AST
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct Context {
    in_impl: bool,               // are we inside an impl block?
    current_fn: Option<String>,  // name of the function being analyzed
    impl_target: Option<String>, // name of the type being impl'd
}

#[allow(dead_code)]
pub struct Analyzer {
    symbols: SymbolTable,
    errors: SemanticErrors,
    source: String,
    file: String,
    ctx: Context,
}

impl Analyzer {
    pub fn new(source: impl Into<String>, file: impl Into<String>) -> Self {
        let mut analyzer = Analyzer {
            symbols: SymbolTable::new(),
            errors: SemanticErrors::new(),
            source: source.into(),
            file: file.into(),
            ctx: Context::default(),
        };
        analyzer.register_builtins();
        analyzer
    }

    /// Main entry point
    pub fn analyze(mut self, program: &Program) -> (SemanticErrors, SymbolTable) {
        // Pass 1: register all top-level names
        self.register_top_level(program);

        // Pass 2: analyze all bodies
        for decl in &program.declarations {
            if self.errors.len() >= MAX_ERRORS {
                break;
            }
            self.analyze_declaration(decl);
        }

        (self.errors, self.symbols)
    }

    // ══════════════════════════════════════════════
    //   PASS 1: REGISTER TOP-LEVEL DECLARATIONS
    // ══════════════════════════════════════════════

    fn register_top_level(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => self.register_fn(f),
                Declaration::Struct(s) => self.register_struct(s),
                Declaration::Enum(e) => self.register_enum(e),
                Declaration::Interface(i) => self.register_interface(i),
                _ => {}
            }
        }
    }

    fn register_fn(&mut self, decl: &FnDecl) {
        let sym = Symbol::Function(FunctionSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "function", decl.span);
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        // Check for duplicate fields before registering
        let mut seen_fields = std::collections::HashSet::new();
        for field in &decl.fields {
            if !seen_fields.insert(field.name.clone()) {
                self.push_error(SemanticError::DuplicateField {
                    struct_name: decl.name.clone(),
                    field: field.name.clone(),
                    span: field.span,
                    file: self.file.clone(),
                });
            }
        }

        let sym = Symbol::Struct(StructSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "struct", decl.span);
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        // Check for duplicate variants
        let mut seen_variants = std::collections::HashSet::new();
        for variant in &decl.variants {
            if !seen_variants.insert(variant.name.clone()) {
                self.push_error(SemanticError::DuplicateVariant {
                    enum_name: decl.name.clone(),
                    variant: variant.name.clone(),
                    span: variant.span,
                    file: self.file.clone(),
                });
            }
        }

        let sym = Symbol::Enum(EnumSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "enum", decl.span);
    }

    fn register_interface(&mut self, decl: &InterfaceDecl) {
        let sym = Symbol::Interface(InterfaceSymbol::from_decl(decl));
        self.define_or_error(&decl.name, sym, "interface", decl.span);
    }

    // ══════════════════════════════════════════════
    //   BUILT-IN FUNCTIONS
    // ══════════════════════════════════════════════

    fn register_builtins(&mut self) {
        let builtins: &[(&str, usize)] = &[
            ("print", 1),
            ("println", 1),
            ("readLine", 0),
            ("len", 1),
            ("parseInt", 1),
            ("parseFloat", 1),
            ("panic", 1),
            ("assert", 1),
            ("typeof", 1),
        ];

        for (name, param_count) in builtins {
            let sym = Symbol::Function(FunctionSymbol {
                name: (*name).to_string(),
                param_count: *param_count,
                param_names: vec![],
                return_type: None,
                defined_at: Span::dummy(),
                is_method: false,
            });
            self.symbols.define(name.to_string(), sym);
        }
    }

    // ══════════════════════════════════════════════
    //   PASS 2: ANALYZE DECLARATION BODIES
    // ══════════════════════════════════════════════

    fn analyze_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(f) => self.analyze_fn(f, false),
            Declaration::Let(l) => self.analyze_let(l),
            Declaration::Const(c) => self.analyze_const(c),
            Declaration::Impl(i) => self.analyze_impl(i),
            Declaration::Statement(s) => self.analyze_statement(s),
            // Struct/Enum/Interface bodies validated in pass 1
            _ => {}
        }
    }

    fn analyze_fn(&mut self, decl: &FnDecl, in_impl: bool) {
        self.symbols.push(ScopeKind::Function);
        let prev_fn = self.ctx.current_fn.replace(decl.name.clone());

        // Register generic params: T in fn max<T>(...)
        for generic in &decl.generics {
            let sym = Symbol::GenericParam(GenericParamSymbol {
                name: generic.name.clone(),
                bounds: generic.bounds.clone(),
                defined_at: generic.span,
            });
            self.symbols.define(generic.name.clone(), sym);
        }

        // Register parameters — check for duplicates
        let mut seen_params = std::collections::HashSet::new();
        for param in &decl.params {
            if param.is_self {
                // 'self' is only valid inside impl blocks
                if !in_impl {
                    self.push_error(SemanticError::SelfOutsideImpl {
                        span: param.span,
                        file: self.file.clone(),
                    });
                }
                continue;
            }

            if !seen_params.insert(param.name.clone()) {
                self.push_error(SemanticError::DuplicateParam {
                    fn_name: decl.name.clone(),
                    param: param.name.clone(),
                    span: param.span,
                    file: self.file.clone(),
                });
                continue;
            }

            let sym = Symbol::Variable(VariableSymbol {
                name: param.name.clone(),
                mutable: false, // params are immutable by default
                type_annotation: param.param_type.clone(),
                defined_at: param.span,
                is_param: true,
            });
            self.symbols.define(param.name.clone(), sym);
        }

        // Analyze the function body
        match &decl.body {
            FnBody::Block(block) => self.analyze_block(block),
            FnBody::Arrow(expr) => {
                self.analyze_expr(expr);
            }
        }

        self.ctx.current_fn = prev_fn;
        self.symbols.pop();
    }

    fn analyze_impl(&mut self, decl: &ImplDecl) {
        let prev_impl = self.ctx.in_impl;
        let prev_impl_target = self.ctx.impl_target.clone();

        self.ctx.in_impl = true;
        self.ctx.impl_target = Some(decl.target.clone());

        self.symbols.push(ScopeKind::Impl);

        for method in &decl.methods {
            self.analyze_fn(method, true);
        }

        self.symbols.pop();

        self.ctx.in_impl = prev_impl;
        self.ctx.impl_target = prev_impl_target;
    }

    fn analyze_let(&mut self, decl: &LetDecl) {
        // Analyze the value expression first
        self.analyze_expr(&decl.value);

        // Check for duplicate in CURRENT scope (shadowing in outer scope is ok)
        if self.symbols.defined_in_current_scope(&decl.name) {
            let existing_span = self
                .symbols
                .lookup(&decl.name)
                .map(|s| s.defined_at())
                .unwrap_or(Span::dummy());
            self.push_error(SemanticError::DuplicateDefinition {
                name: decl.name.clone(),
                kind: "variable".to_string(),
                first_at: existing_span,
                second_at: decl.span,
                file: self.file.clone(),
            });
            return;
        }

        let sym = Symbol::Variable(VariableSymbol {
            name: decl.name.clone(),
            mutable: decl.mutable,
            type_annotation: decl.type_annotation.clone(),
            defined_at: decl.span,
            is_param: false,
        });
        self.symbols.define(decl.name.clone(), sym);
    }

    fn analyze_const(&mut self, decl: &ConstDecl) {
        self.analyze_expr(&decl.value);

        let sym = Symbol::Variable(VariableSymbol {
            name: decl.name.clone(),
            mutable: false,
            type_annotation: decl.type_annotation.clone(),
            defined_at: decl.span,
            is_param: false,
        });
        self.define_or_error(&decl.name, sym, "constant", decl.span);
    }

    // ══════════════════════════════════════════════
    //   STATEMENT ANALYSIS
    // ══════════════════════════════════════════════

    fn analyze_block(&mut self, block: &Block) {
        self.symbols.push(ScopeKind::Block);
        for stmt in &block.statements {
            if self.errors.len() >= MAX_ERRORS {
                break;
            }
            self.analyze_statement(stmt);
        }
        self.symbols.pop();
    }

    fn analyze_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Let(l) => self.analyze_let(l),
            Statement::Const(c) => self.analyze_const(c),
            Statement::Return(r) => self.analyze_return(r),
            Statement::If(i) => self.analyze_if(i),
            Statement::While(w) => self.analyze_while(w),
            Statement::For(f) => self.analyze_for(f),
            Statement::Loop(l) => self.analyze_loop(l),
            Statement::Match(m) => self.analyze_match_stmt(m),
            Statement::Spawn(s) => self.analyze_spawn(s),
            Statement::Break(b) => self.analyze_break(b),
            Statement::Continue(c) => self.analyze_continue(c),
            Statement::Block(b) => self.analyze_block(b),
            Statement::Expression(e) => self.analyze_expr(&e.expr),
        }
    }

    fn analyze_return(&mut self, stmt: &ReturnStmt) {
        if !self.symbols.inside_function() {
            self.push_error(SemanticError::InvalidContext {
                what: "return",
                required: "inside a function",
                span: stmt.span,
                file: self.file.clone(),
            });
        }
        if let Some(value) = &stmt.value {
            self.analyze_expr(value);
        }
    }

    fn analyze_if(&mut self, stmt: &IfStmt) {
        self.analyze_expr(&stmt.condition);
        self.analyze_block(&stmt.then_branch);
        for branch in &stmt.else_if_branches {
            self.analyze_expr(&branch.condition);
            self.analyze_block(&branch.body);
        }
        if let Some(else_block) = &stmt.else_branch {
            self.analyze_block(else_block);
        }
    }

    fn analyze_while(&mut self, stmt: &WhileStmt) {
        self.analyze_expr(&stmt.condition);
        self.symbols.push(ScopeKind::Loop);
        for s in &stmt.body.statements {
            self.analyze_statement(s);
        }
        self.symbols.pop();
    }

    fn analyze_for(&mut self, stmt: &ForStmt) {
        self.analyze_expr(&stmt.iterable);
        self.symbols.push(ScopeKind::Loop);

        // The loop variable is defined inside the loop scope
        let sym = Symbol::Variable(VariableSymbol {
            name: stmt.variable.clone(),
            mutable: false,
            type_annotation: None,
            defined_at: stmt.span,
            is_param: false,
        });
        self.symbols.define(stmt.variable.clone(), sym);

        for s in &stmt.body.statements {
            self.analyze_statement(s);
        }
        self.symbols.pop();
    }

    fn analyze_loop(&mut self, stmt: &LoopStmt) {
        self.symbols.push(ScopeKind::Loop);
        for s in &stmt.body.statements {
            self.analyze_statement(s);
        }
        self.symbols.pop();
    }

    fn analyze_match_stmt(&mut self, stmt: &MatchStmt) {
        self.analyze_expr(&stmt.subject);
        for arm in &stmt.arms {
            self.analyze_match_arm(arm);
        }
    }

    fn analyze_match_arm(&mut self, arm: &MatchArm) {
        self.symbols.push(ScopeKind::Match);

        // Bindings in patterns become variables in the arm scope
        self.register_pattern_bindings(&arm.pattern);

        if let Some(guard) = &arm.guard {
            self.analyze_expr(guard);
        }

        match &arm.body {
            MatchBody::Expr(e) => self.analyze_expr(e),
            MatchBody::Block(b) => self.analyze_block(b),
        }

        self.symbols.pop();
    }

    /// Register pattern bindings as variables in current scope
    fn register_pattern_bindings(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Binding(b) => {
                let sym = Symbol::Variable(VariableSymbol {
                    name: b.name.clone(),
                    mutable: b.mutable,
                    type_annotation: None,
                    defined_at: b.span,
                    is_param: false,
                });
                self.symbols.define(b.name.clone(), sym);
            }
            Pattern::EnumVariant(v) => {
                for binding in &v.bindings {
                    self.register_pattern_bindings(binding);
                }
            }
            Pattern::Or(o) => {
                // Only register bindings from the first alternative
                // (all alternatives must have same bindings — checked in type phase)
                if let Some(first) = o.alternatives.first() {
                    self.register_pattern_bindings(first);
                }
            }
            _ => {}
        }
    }

    fn analyze_spawn(&mut self, stmt: &SpawnStmt) {
        // Spawn opens a new scope (conceptually a new "thread")
        self.symbols.push(ScopeKind::Block);
        for s in &stmt.body.statements {
            self.analyze_statement(s);
        }
        self.symbols.pop();
    }

    fn analyze_break(&mut self, stmt: &BreakStmt) {
        if !self.symbols.inside_loop() {
            self.push_error(SemanticError::InvalidContext {
                what: "break",
                required: "inside a loop",
                span: stmt.span,
                file: self.file.clone(),
            });
        }
    }

    fn analyze_continue(&mut self, stmt: &ContinueStmt) {
        if !self.symbols.inside_loop() {
            self.push_error(SemanticError::InvalidContext {
                what: "continue",
                required: "inside a loop",
                span: stmt.span,
                file: self.file.clone(),
            });
        }
    }

    // ══════════════════════════════════════════════
    //   EXPRESSION ANALYSIS (expanded in a later step)
    // ══════════════════════════════════════════════

    fn analyze_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Int(_)
            | Expr::Float(_)
            | Expr::Str(_)
            | Expr::Bool(_)
            | Expr::Char(_)
            | Expr::Null(_) => {}
            Expr::Identifier(id) => {
                if id.name == "self" {
                    if !self.ctx.in_impl {
                        self.push_error(SemanticError::SelfOutsideImpl {
                            span: id.span,
                            file: self.file.clone(),
                        });
                    }
                } else if !self.symbols.is_defined(&id.name) {
                    self.push_error(SemanticError::UndefinedName {
                        name: id.name.clone(),
                        span: id.span,
                        file: self.file.clone(),
                        suggestion: None,
                    });
                }
            }
            Expr::Array(a) => {
                for el in &a.elements {
                    self.analyze_expr(el);
                }
            }
            Expr::StructInit(si) => {
                for (_, val) in &si.fields {
                    self.analyze_expr(val);
                }
            }
            Expr::Binary(b) => {
                self.analyze_expr(&b.left);
                self.analyze_expr(&b.right);
            }
            Expr::Unary(u) => self.analyze_expr(&u.operand),
            Expr::Assign(a) => {
                self.analyze_expr(&a.target);
                self.analyze_expr(&a.value);
            }
            Expr::Call(c) => {
                if let Expr::Identifier(id) = &*c.callee {
                    if !self.symbols.is_defined(&id.name) {
                        self.push_error(SemanticError::UndefinedName {
                            name: id.name.clone(),
                            span: id.span,
                            file: self.file.clone(),
                            suggestion: None,
                        });
                    }
                }
                for arg in &c.args {
                    self.analyze_expr(&arg.value);
                }
            }
            Expr::MethodCall(m) => {
                self.analyze_expr(&m.object);
                for arg in &m.args {
                    self.analyze_expr(&arg.value);
                }
            }
            Expr::Field(f) => self.analyze_expr(&f.object),
            Expr::Index(ix) => {
                self.analyze_expr(&ix.object);
                self.analyze_expr(&ix.index);
            }
            Expr::If(i) => {
                self.analyze_expr(&i.condition);
                self.analyze_block(&i.then_branch);
                if let Some(eb) = &i.else_branch {
                    self.analyze_block(eb);
                }
            }
            Expr::Match(m) => {
                self.analyze_expr(&m.subject);
                for arm in &m.arms {
                    self.analyze_match_arm(arm);
                }
            }
            Expr::Block(b) => self.analyze_block(b),
            Expr::Propagate(p) => self.analyze_expr(&p.expr),
            Expr::NullCoalesce(n) => {
                self.analyze_expr(&n.left);
                self.analyze_expr(&n.right);
            }
            Expr::Cast(ct) => self.analyze_expr(&ct.expr),
            Expr::Range(r) => {
                self.analyze_expr(&r.start);
                self.analyze_expr(&r.end);
            }
            Expr::Closure(cl) => {
                self.symbols.push(ScopeKind::Block);
                for param in &cl.params {
                    if param.is_self && !self.ctx.in_impl {
                        self.push_error(SemanticError::SelfOutsideImpl {
                            span: param.span,
                            file: self.file.clone(),
                        });
                        continue;
                    }
                    if !param.is_self {
                        self.symbols.define(
                            param.name.clone(),
                            Symbol::Variable(VariableSymbol {
                                name: param.name.clone(),
                                mutable: false,
                                type_annotation: param.param_type.clone(),
                                defined_at: param.span,
                                is_param: true,
                            }),
                        );
                    }
                }
                match &cl.body {
                    FnBody::Block(b) => self.analyze_block(b),
                    FnBody::Arrow(e) => self.analyze_expr(e),
                }
                self.symbols.pop();
            }
        }
    }

    // ══════════════════════════════════════════════
    //   HELPERS
    // ══════════════════════════════════════════════

    /// Define a symbol or report a duplicate error
    fn define_or_error(&mut self, name: &str, sym: Symbol, kind: &str, span: Span) {
        if let Some(existing) = self.symbols.lookup(name) {
            if self.symbols.defined_in_current_scope(name) {
                self.push_error(SemanticError::DuplicateDefinition {
                    name: name.to_string(),
                    kind: kind.to_string(),
                    first_at: existing.defined_at(),
                    second_at: span,
                    file: self.file.clone(),
                });
                return;
            }
        }
        self.symbols.define(name.to_string(), sym);
    }

    fn push_error(&mut self, err: SemanticError) {
        self.errors.push(err);
    }
}

#[cfg(test)]
mod register_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analyze(src: &str) -> (SemanticErrors, SymbolTable) {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        Analyzer::new(src, "t.lyz").analyze(&prog)
    }

    #[test]
    fn test_fn_registered() {
        let (errs, table) = analyze("fn add(a: int, b: int) -> int { return a + b }");
        assert!(
            errs.is_empty(),
            "{}",
            errs.format_all("fn add(a: int, b: int) -> int { return a + b }")
        );
        assert!(table.is_defined("add"));
    }

    #[test]
    fn test_struct_registered() {
        let (errs, table) = analyze("struct Point { x: float, y: float }");
        assert!(errs.is_empty());
        assert!(table.is_defined("Point"));
    }

    #[test]
    fn test_enum_registered() {
        let (errs, table) = analyze("enum Color { Red, Green, Blue }");
        assert!(errs.is_empty());
        assert!(table.is_defined("Color"));
    }

    #[test]
    fn test_builtin_print_registered() {
        let (_, table) = analyze("");
        assert!(table.is_defined("print"));
        assert!(table.is_defined("len"));
        assert!(table.is_defined("panic"));
    }

    #[test]
    fn test_duplicate_fn_error() {
        let src = "fn foo() {}\nfn foo() {}";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateDefinition { name, .. } if name == "foo")
        );
    }

    #[test]
    fn test_duplicate_struct_error() {
        let src = "struct Foo { x: int }\nstruct Foo { y: int }";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateDefinition { name, .. } if name == "Foo")
        );
    }

    #[test]
    fn test_duplicate_field_error() {
        let src = "struct Bad { x: int, x: float }";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], SemanticError::DuplicateField { field, .. } if field == "x"));
    }

    #[test]
    fn test_duplicate_variant_error() {
        let src = "enum Bad { Ok, Ok }";
        let (errs, _) = analyze(src);
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateVariant { variant, .. } if variant == "Ok")
        );
    }

    #[test]
    fn test_forward_reference_works() {
        // main calls compute() which is defined AFTER main
        let src = "fn main() { compute() }\nfn compute() -> int { return 42 }";
        let (errs, _) = analyze(src);
        assert!(
            errs.is_empty(),
            "Forward references should work: {:?}",
            errs.0
        );
    }
}

#[cfg(test)]
mod fn_analysis_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analyze(src: &str) -> SemanticErrors {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        Analyzer::new(src, "t.lyz").analyze(&prog).0
    }

    #[test]
    fn test_valid_fn_no_errors() {
        let errs = analyze("fn add(a: int, b: int) -> int { return a + b }");
        assert!(errs.is_empty(), "{:?}", errs.0);
    }

    #[test]
    fn test_duplicate_param() {
        let errs = analyze("fn bad(x: int, x: float) {}");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], SemanticError::DuplicateParam { param, .. } if param == "x"));
    }

    #[test]
    fn test_self_outside_impl_error() {
        let errs = analyze("fn standalone(self, x: int) {}");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], SemanticError::SelfOutsideImpl { .. }));
    }

    #[test]
    fn test_self_valid_inside_impl() {
        let errs = analyze("struct P { x: float }\nimpl P { fn get(self) -> float => self.x }");
        assert!(errs.is_empty(), "{:?}", errs.0);
    }

    #[test]
    fn test_let_in_fn_body() {
        let errs = analyze("fn f() { let x = 5 let y = x + 1 }");
        assert!(errs.is_empty(), "{:?}", errs.0);
    }

    #[test]
    fn test_duplicate_let_same_scope() {
        let errs = analyze("fn f() { let x = 1 let x = 2 }");
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::DuplicateDefinition { name, .. } if name == "x")
        );
    }

    #[test]
    fn test_shadowing_allowed_in_nested_scope() {
        // let x in outer, let x in inner block — this is OK (shadowing)
        let errs = analyze("fn f() { let x = 1 { let x = 2 } }");
        assert!(errs.is_empty(), "Shadowing should be allowed: {:?}", errs.0);
    }

    #[test]
    fn test_fn_arrow_body_analyzed() {
        // Arrow body: fn double(x: int) => x * 2 — 'x' must be in scope
        let errs = analyze("fn double(x: int) -> int => x * 2");
        assert!(errs.is_empty(), "{:?}", errs.0);
    }

    #[test]
    fn test_generic_param_registered() {
        // T must be visible inside the function
        let errs = analyze("fn identity<T>(x: T) -> T { return x }");
        assert!(errs.is_empty(), "{:?}", errs.0);
    }
}

#[cfg(test)]
mod statement_analysis_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analyze(src: &str) -> SemanticErrors {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        Analyzer::new(src, "t.lyz").analyze(&prog).0
    }

    #[test]
    fn test_return_valid() {
        assert!(analyze("fn f() { return 1 }").is_empty());
    }

    #[test]
    fn test_break_valid() {
        assert!(analyze("fn f() { while true { break } }").is_empty());
    }

    #[test]
    fn test_continue_valid() {
        assert!(analyze("fn f() { for i in 0..10 { continue } }").is_empty());
    }

    #[test]
    fn test_break_outside_loop() {
        let errs = analyze("fn f() { break }");
        assert!(!errs.is_empty());
        assert!(matches!(
            &errs.0[0],
            SemanticError::InvalidContext { what: "break", .. }
        ));
    }

    #[test]
    fn test_continue_outside_loop() {
        let errs = analyze("fn f() { continue }");
        assert!(!errs.is_empty());
        assert!(matches!(
            &errs.0[0],
            SemanticError::InvalidContext {
                what: "continue",
                ..
            }
        ));
    }

    #[test]
    fn test_for_loop_variable_in_scope() {
        assert!(analyze("fn f() { for i in 0..10 { print(i) } }").is_empty());
    }

    #[test]
    fn test_for_loop_variable_not_after_loop() {
        let errs = analyze("fn f() { for i in 0..10 {} print(i) }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], SemanticError::UndefinedName { name, .. } if name == "i"));
    }

    #[test]
    fn test_if_condition_analyzed() {
        let errs = analyze("fn f() { if undeclared { print(1) } }");
        assert!(!errs.is_empty());
        assert!(
            matches!(&errs.0[0], SemanticError::UndefinedName { name, .. } if name == "undeclared")
        );
    }

    #[test]
    fn test_match_binding_in_arm() {
        // 'n' is bound in the match arm and should be visible inside it
        assert!(
            analyze("fn f() { match x { n -> print(n) } }").is_empty()
                || analyze("fn f() { let x = 1 match x { n -> print(n) } }").is_empty()
        );
    }

    #[test]
    fn test_nested_loops_break_valid() {
        assert!(analyze("fn f() { while true { for i in 0..5 { break } } }").is_empty());
    }

    #[test]
    fn test_if_else_both_analyzed() {
        let errs = analyze("fn f() { if true { print(a) } else { print(b) } }");
        // Both a and b are undefined — should get 2 errors
        assert!(errs.len() >= 2);
    }
}
