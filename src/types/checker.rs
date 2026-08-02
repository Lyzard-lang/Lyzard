use crate::lexer::Span;
use crate::parser::ast::*;

use super::error::{TypeError, TypeErrors};
use super::{ResolvedType, TypeEnvironment};

const MAX_ERRORS: usize = 30;

/// Two-pass type checker. Pass 1 registers all top-level signatures so that
/// forward references resolve; Pass 2 analyzes declaration bodies.
#[allow(dead_code)]
pub struct TypeChecker {
    pub env: TypeEnvironment,
    pub errors: TypeErrors,
    file: String,
    source: String,
    /// Struct field maps: struct name → field name → field type
    struct_fields: std::collections::HashMap<String, Vec<(String, ResolvedType)>>,
    /// Enum variant maps: enum name → variant names
    enum_variants: std::collections::HashMap<String, Vec<String>>,
}

impl TypeChecker {
    pub fn new(source: impl Into<String>, file: impl Into<String>) -> Self {
        TypeChecker {
            env: TypeEnvironment::new(),
            errors: TypeErrors::new(),
            file: file.into(),
            source: source.into(),
            struct_fields: std::collections::HashMap::new(),
            enum_variants: std::collections::HashMap::new(),
        }
    }

    /// Main entry — type-check a full program
    pub fn check(mut self, program: &Program) -> TypeErrors {
        // Pass 1: register all top-level type signatures
        self.register_top_level(program);
        // Pass 2: check all declaration bodies
        for decl in &program.declarations {
            if self.errors.len() >= MAX_ERRORS {
                break;
            }
            self.check_declaration(decl);
        }
        self.errors
    }

    // ══════════════════════════════════════════
    //   PASS 1: REGISTER TOP-LEVEL SIGNATURES
    // ══════════════════════════════════════════

    fn register_top_level(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => self.register_fn_sig(f),
                Declaration::Struct(s) => self.register_struct(s),
                Declaration::Enum(e) => self.register_enum(e),
                _ => {}
            }
        }
    }

    fn register_fn_sig(&mut self, decl: &FnDecl) {
        let params: Vec<ResolvedType> = decl
            .params
            .iter()
            .filter(|p| !p.is_self)
            .map(|p| {
                p.param_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(ResolvedType::Unknown)
            })
            .collect();
        let return_type = decl
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(ResolvedType::Void);
        let fn_type = ResolvedType::Function {
            params,
            return_type: Box::new(return_type),
        };
        self.env.define(decl.name.clone(), fn_type);
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        let fields: Vec<(String, ResolvedType)> = decl
            .fields
            .iter()
            .map(|f| (f.name.clone(), self.resolve_type(&f.field_type)))
            .collect();
        self.struct_fields.insert(decl.name.clone(), fields);
        self.env
            .define(decl.name.clone(), ResolvedType::Struct(decl.name.clone()));
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        let variants: Vec<String> = decl.variants.iter().map(|v| v.name.clone()).collect();
        self.enum_variants.insert(decl.name.clone(), variants);
        self.env
            .define(decl.name.clone(), ResolvedType::Enum(decl.name.clone()));
    }

    // ══════════════════════════════════════════
    //   TYPE RESOLUTION (source → resolved)
    // ══════════════════════════════════════════

    pub fn resolve_type(&self, type_expr: &TypeExpr) -> ResolvedType {
        match type_expr {
            TypeExpr::Named(n) => match n.name.as_str() {
                "int" => ResolvedType::Int,
                "float" => ResolvedType::Float,
                "bool" => ResolvedType::Bool,
                "str" => ResolvedType::Str,
                "char" => ResolvedType::Char,
                "void" => ResolvedType::Void,
                "never" => ResolvedType::Never,
                name => {
                    if self.struct_fields.contains_key(name) {
                        ResolvedType::Struct(name.to_string())
                    } else if self.enum_variants.contains_key(name) {
                        ResolvedType::Enum(name.to_string())
                    } else {
                        ResolvedType::TypeParam(name.to_string())
                    }
                }
            },
            TypeExpr::Optional(inner, _) => {
                ResolvedType::Optional(Box::new(self.resolve_type(inner)))
            }
            TypeExpr::Array(inner, _) => {
                ResolvedType::Array(Box::new(self.resolve_type(inner)))
            }
            TypeExpr::Tuple(types, _) => {
                ResolvedType::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeExpr::Generic(g) => ResolvedType::Generic {
                name: g.name.clone(),
                args: g.args.iter().map(|a| self.resolve_type(a)).collect(),
            },
            TypeExpr::Fn(params, ret, _) => ResolvedType::Function {
                params: params.iter().map(|p| self.resolve_type(p)).collect(),
                return_type: Box::new(self.resolve_type(ret)),
            },
            TypeExpr::Never(_) => ResolvedType::Never,
            _ => ResolvedType::Unknown,
        }
    }

    // ══════════════════════════════════════════
    //   PASS 2: CHECK DECLARATIONS
    // ══════════════════════════════════════════

    pub fn check_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(f)  => self.check_fn(f),
            Declaration::Let(l)       => self.check_let(l),
            Declaration::Const(c)     => self.check_const(c),
            Declaration::Impl(i)      => self.check_impl(i),
            Declaration::Statement(s) => { self.check_statement(s); }
            _                         => {}
        }
    }

    fn check_fn(&mut self, decl: &FnDecl) {
        self.env.push_scope();

        // Register generic params
        for generic in &decl.generics {
            self.env.define(generic.name.clone(), ResolvedType::TypeParam(generic.name.clone()));
        }

        // Register params
        for param in &decl.params {
            if param.is_self {
                if let Some(self_ty) = self.env.self_type().cloned() {
                    self.env.define("self".to_string(), self_ty);
                }
                continue;
            }
            let ty = param.param_type.as_ref()
                .map(|t| self.resolve_type(t))
                .unwrap_or(ResolvedType::Unknown);
            self.env.define(param.name.clone(), ty);
        }

        // Set return type context
        let return_type = decl.return_type.as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(ResolvedType::Void);
        self.env.enter_function(return_type.clone());

        // Check body
        match &decl.body {
            FnBody::Block(block) => {
                self.check_block(block);
                // TODO: check all paths return (requires control flow analysis)
            }
            FnBody::Arrow(expr) => {
                let expr_ty = self.infer_expr(expr);
                if !return_type.is_assignable_from(&expr_ty) && !expr_ty.is_error() {
                    self.type_mismatch(return_type, expr_ty, expr.span(), "arrow function return");
                }
            }
        }

        self.env.exit_function();
        self.env.pop_scope();
    }

    fn check_let(&mut self, decl: &LetDecl) {
        let value_ty = self.infer_expr(&decl.value);

        if let Some(annotation) = &decl.type_annotation {
            let declared_ty = self.resolve_type(annotation);
            if !declared_ty.is_assignable_from(&value_ty) && !value_ty.is_error() {
                self.type_mismatch(declared_ty.clone(), value_ty.clone(), decl.span,
                    &format!("variable `{}` declaration", decl.name));
            }
            self.env.define(decl.name.clone(), declared_ty);
        } else {
            // Infer type from value
            self.env.define(decl.name.clone(), value_ty);
        }
    }

    fn check_const(&mut self, decl: &ConstDecl) {
        let value_ty = self.infer_expr(&decl.value);
        if let Some(annotation) = &decl.type_annotation {
            let declared_ty = self.resolve_type(annotation);
            if !declared_ty.is_assignable_from(&value_ty) && !value_ty.is_error() {
                self.type_mismatch(declared_ty.clone(), value_ty, decl.span, "constant declaration");
            }
            self.env.define(decl.name.clone(), declared_ty);
        } else {
            self.env.define(decl.name.clone(), value_ty);
        }
    }

    fn check_impl(&mut self, decl: &ImplDecl) {
        let self_ty = ResolvedType::Struct(decl.target.clone());
        self.env.enter_impl(self_ty);
        for method in &decl.methods {
            self.check_fn(method);
        }
        self.env.exit_impl();
    }

    // ══════════════════════════════════════════
    //   CHECK STATEMENTS
    // ══════════════════════════════════════════

    pub fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Let(l)       => self.check_let(l),
            Statement::Const(c)     => self.check_const(c),
            Statement::Return(r)    => self.check_return(r),
            Statement::If(i)        => self.check_if(i),
            Statement::While(w)     => self.check_while(w),
            Statement::For(f)       => self.check_for(f),
            Statement::Loop(l)      => self.check_loop_stmt(l),
            Statement::Match(m)     => self.check_match(m),
            Statement::Block(b)     => self.check_block(b),
            Statement::Expression(e)=> { self.infer_expr(&e.expr); }
            Statement::Spawn(s)     => self.check_block(&s.body),
            Statement::Break(_) | Statement::Continue(_) => {}
        }
    }

    fn check_block(&mut self, block: &Block) {
        self.env.push_scope();
        for stmt in &block.statements {
            self.check_statement(stmt);
        }
        self.env.pop_scope();
    }

    fn check_return(&mut self, stmt: &ReturnStmt) {
        let expected = self.env.expected_return_type().cloned().unwrap_or(ResolvedType::Void);
        let found    = stmt.value.as_ref().map(|e| self.infer_expr(e)).unwrap_or(ResolvedType::Void);

        if !expected.is_assignable_from(&found) && !found.is_error() && !expected.is_error() {
            self.type_mismatch(expected, found, stmt.span, "return statement");
        }
    }

    fn check_if(&mut self, stmt: &IfStmt) {
        let cond_ty = self.infer_expr(&stmt.condition);
        if !cond_ty.is_error() && !cond_ty.is_bool() {
            self.push_error(TypeError::NonBoolCondition {
                found: cond_ty, span: stmt.condition.span(),
                file: self.file.clone(), context: "if".to_string(),
            });
        }
        self.check_block(&stmt.then_branch);
        for branch in &stmt.else_if_branches {
            let cond = self.infer_expr(&branch.condition);
            if !cond.is_error() && !cond.is_bool() {
                self.push_error(TypeError::NonBoolCondition {
                    found: cond, span: branch.condition.span(),
                    file: self.file.clone(), context: "else if".to_string(),
                });
            }
            self.check_block(&branch.body);
        }
        if let Some(else_b) = &stmt.else_branch { self.check_block(else_b); }
    }

    fn check_while(&mut self, stmt: &WhileStmt) {
        let cond_ty = self.infer_expr(&stmt.condition);
        if !cond_ty.is_error() && !cond_ty.is_bool() {
            self.push_error(TypeError::NonBoolCondition {
                found: cond_ty, span: stmt.condition.span(),
                file: self.file.clone(), context: "while".to_string(),
            });
        }
        self.env.enter_loop();
        self.check_block(&stmt.body);
        self.env.exit_loop();
    }

    fn check_for(&mut self, stmt: &ForStmt) {
        let iter_ty = self.infer_expr(&stmt.iterable);
        // Determine the element type of the iterable
        let elem_ty = match &iter_ty {
            ResolvedType::Array(inner) => *inner.clone(),
            other if other.is_error() => ResolvedType::Error,
            other => {
                self.push_error(TypeError::TypeMismatch {
                    expected: ResolvedType::Array(Box::new(ResolvedType::Unknown)),
                    found: other.clone(),
                    span: stmt.iterable.span(),
                    file: self.file.clone(),
                    context: "for loop — iterable must be an array".to_string(),
                });
                ResolvedType::Error
            }
        };
        self.env.enter_loop();
        self.env.push_scope();
        self.env.define(stmt.variable.clone(), elem_ty);
        for s in &stmt.body.statements { self.check_statement(s); }
        self.env.pop_scope();
        self.env.exit_loop();
    }

    fn check_loop_stmt(&mut self, stmt: &LoopStmt) {
        self.env.enter_loop();
        self.check_block(&stmt.body);
        self.env.exit_loop();
    }

    fn check_match(&mut self, stmt: &MatchStmt) {
        let _subject_ty = self.infer_expr(&stmt.subject);
        for arm in &stmt.arms {
            self.env.push_scope();
            // Bind pattern variables — type them as Unknown for now
            self.bind_pattern_types(&arm.pattern);
            match &arm.body {
                MatchBody::Expr(e)  => { self.infer_expr(e); }
                MatchBody::Block(b) => self.check_block(b),
            }
            self.env.pop_scope();
        }
    }

    fn bind_pattern_types(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Binding(b) => {
                self.env.define(b.name.clone(), ResolvedType::Unknown);
            }
            Pattern::Or(o) => {
                if let Some(first) = o.alternatives.first() {
                    self.bind_pattern_types(first);
                }
            }
            _ => {}
        }
    }

    // ══════════════════════════════════════════
    //   EXPRESSION TYPE INFERENCE
    // ══════════════════════════════════════════

    pub fn infer_expr(&mut self, expr: &Expr) -> ResolvedType {
        match expr {
            // Literals — always known
            Expr::Int(_) => ResolvedType::Int,
            Expr::Float(_) => ResolvedType::Float,
            Expr::Str(_) => ResolvedType::Str,
            Expr::Bool(_) => ResolvedType::Bool,
            Expr::Char(_) => ResolvedType::Char,
            Expr::Null(_) => ResolvedType::Optional(Box::new(ResolvedType::Unknown)),

            // Identifier — look up its type in env
            Expr::Identifier(id) => match self.lookup_type(&id.name) {
                Some(ty) => ty,
                None => ResolvedType::Error,
            },

            // Binary operations
            Expr::Binary(b) => {
                let left = self.infer_expr(&b.left);
                let right = self.infer_expr(&b.right);
                self.infer_binary(&b.op, left, right, b.span)
            }

            // Unary operations
            Expr::Unary(u) => {
                let operand = self.infer_expr(&u.operand);
                self.infer_unary(&u.op, operand, u.span)
            }

            // Function call
            Expr::Call(c) => {
                let callee_type = self.infer_expr(&c.callee);
                let arg_types: Vec<ResolvedType> = c.args.iter().map(|a| self.infer_expr(&a.value)).collect();
                self.infer_call(callee_type, arg_types, &c.callee, c.span)
            }

            // Method call — return Unknown for now (full resolution needs impl registry)
            Expr::MethodCall(m) => {
                self.infer_expr(&m.object);
                for a in &m.args {
                    self.infer_expr(&a.value);
                }
                ResolvedType::Unknown
            }

            // Array literal: [1, 2, 3] → [int]
            Expr::Array(arr) => {
                if arr.elements.is_empty() {
                    return ResolvedType::Array(Box::new(ResolvedType::Unknown));
                }
                let first_ty = self.infer_expr(&arr.elements[0]);
                for elem in arr.elements.iter().skip(1) {
                    let ty = self.infer_expr(elem);
                    if !first_ty.is_assignable_from(&ty) && !ty.is_error() {
                        self.push_error(TypeError::TypeMismatch {
                            expected: first_ty.clone(),
                            found: ty,
                            span: elem.span(),
                            file: self.file.clone(),
                            context: "array literal — all elements must have the same type".to_string(),
                        });
                    }
                }
                ResolvedType::Array(Box::new(first_ty))
            }

            // Struct literal: Point { x: 1.0, y: 2.0 } → Struct("Point")
            Expr::StructInit(s) => {
                let struct_ty = ResolvedType::Struct(s.name.clone());
                if let Some(fields) = self.struct_fields.get(&s.name).cloned() {
                    for (field_name, field_val) in &s.fields {
                        let val_ty = self.infer_expr(field_val);
                        if let Some((_, expected_ty)) = fields.iter().find(|(n, _)| n == field_name) {
                            if !expected_ty.is_assignable_from(&val_ty) && !val_ty.is_error() {
                                self.push_error(TypeError::TypeMismatch {
                                    expected: expected_ty.clone(),
                                    found: val_ty,
                                    span: field_val.span(),
                                    file: self.file.clone(),
                                    context: format!("field `{}` of struct `{}`", field_name, s.name),
                                });
                            }
                        } else {
                            let available = fields.iter().map(|(n, _)| n.clone()).collect();
                            self.push_error(TypeError::UnknownStructField {
                                struct_name: s.name.clone(),
                                field: field_name.clone(),
                                available,
                                span: field_val.span(),
                                file: self.file.clone(),
                            });
                        }
                    }
                }
                struct_ty
            }

            // Field access: obj.field → type of field
            Expr::Field(f) => {
                let obj_ty = self.infer_expr(&f.object);
                self.infer_field_access(obj_ty, &f.field, f.span)
            }

            // Index: arr[i] → inner type of array
            Expr::Index(i) => {
                let obj_ty = self.infer_expr(&i.object);
                let idx_ty = self.infer_expr(&i.index);

                // Index must be int
                if !idx_ty.is_error() && !matches!(idx_ty, ResolvedType::Int) {
                    self.push_error(TypeError::NonIntegerIndex {
                        found: idx_ty,
                        span: i.index.span(),
                        file: self.file.clone(),
                    });
                }

                // Object must be array or str
                match &obj_ty {
                    ResolvedType::Array(inner) => *inner.clone(),
                    ResolvedType::Str => ResolvedType::Char,
                    other if other.is_error() => ResolvedType::Error,
                    other => {
                        self.push_error(TypeError::IndexOnNonArray {
                            found: other.clone(),
                            span: i.span,
                            file: self.file.clone(),
                        });
                        ResolvedType::Error
                    }
                }
            }

            // Assignment: evaluates to the value's type
            Expr::Assign(a) => {
                let val_ty = self.infer_expr(&a.value);
                let target_ty = self.infer_expr(&a.target);
                if !target_ty.is_error() && !val_ty.is_error() && !target_ty.is_assignable_from(&val_ty) {
                    self.type_mismatch(target_ty, val_ty.clone(), a.span, "assignment");
                }
                val_ty
            }

            // Block: type is the last expression or Void
            Expr::Block(b) => {
                self.env.push_scope();
                let mut ty = ResolvedType::Void;
                for stmt in &b.statements {
                    ty = self.check_statement_type(stmt);
                }
                self.env.pop_scope();
                ty
            }

            // If expression: branches must match
            Expr::If(i) => {
                let cond_ty = self.infer_expr(&i.condition);
                if !cond_ty.is_error() && !cond_ty.is_bool() {
                    self.push_error(TypeError::NonBoolCondition {
                        found: cond_ty,
                        span: i.condition.span(),
                        file: self.file.clone(),
                        context: "if".to_string(),
                    });
                }
                let then_ty = self.infer_block_type(&i.then_branch);
                if let Some(else_block) = &i.else_branch {
                    let else_ty = self.infer_block_type(else_block);
                    if !then_ty.is_error() && !else_ty.is_error() && then_ty != else_ty {
                        self.push_error(TypeError::BranchTypeMismatch {
                            then_type: then_ty.clone(),
                            else_type: else_ty,
                            span: i.span,
                            file: self.file.clone(),
                        });
                    }
                }
                then_ty
            }

            // Range: always produces [int]
            Expr::Range(r) => {
                let start = self.infer_expr(&r.start);
                let end = self.infer_expr(&r.end);
                for ty in [&start, &end] {
                    if !ty.is_error() && !ty.is_int() {
                        self.push_error(TypeError::TypeMismatch {
                            expected: ResolvedType::Int,
                            found: ty.clone(),
                            span: r.span,
                            file: self.file.clone(),
                            context: "range bounds must be int".to_string(),
                        });
                    }
                }
                ResolvedType::Array(Box::new(ResolvedType::Int))
            }

            // Error propagation: ? requires Result<T, E>, returns T
            Expr::Propagate(p) => {
                let inner = self.infer_expr(&p.expr);
                if inner.is_error() {
                    return ResolvedType::Error;
                }
                match inner.as_result() {
                    Some((ok_ty, _)) => ok_ty.clone(),
                    None => {
                        self.push_error(TypeError::PropagateOnNonResult {
                            found: inner,
                            span: p.span,
                            file: self.file.clone(),
                        });
                        ResolvedType::Error
                    }
                }
            }

            // Null coalesce: a ?? b — result is inner type of Optional
            Expr::NullCoalesce(n) => {
                let left = self.infer_expr(&n.left);
                let right = self.infer_expr(&n.right);
                match left.unwrap_optional() {
                    Some(inner) => inner.clone(),
                    None => right,
                }
            }

            _ => ResolvedType::Unknown,
        }
    }

    fn infer_binary(&mut self, op: &BinaryOp, left: ResolvedType, right: ResolvedType, span: Span) -> ResolvedType {
        if left.is_error() || right.is_error() {
            return ResolvedType::Error;
        }

        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                match ResolvedType::arithmetic_result(&left, &right) {
                    Some(result) => result,
                    None => {
                        self.push_error(TypeError::InvalidOperation {
                            op: format!("{:?}", op).to_lowercase(),
                            left,
                            right,
                            span,
                            file: self.file.clone(),
                        });
                        ResolvedType::Error
                    }
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                if left != right && !left.is_error() && !right.is_error() {
                    self.push_error(TypeError::TypeMismatch {
                        expected: left,
                        found: right,
                        span,
                        file: self.file.clone(),
                        context: "equality comparison — both sides must have the same type".to_string(),
                    });
                }
                ResolvedType::Bool
            }
            BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte => {
                if !left.is_numeric() && !left.is_str() {
                    self.push_error(TypeError::InvalidOperation {
                        op: format!("{:?}", op).to_lowercase(),
                        left,
                        right,
                        span,
                        file: self.file.clone(),
                    });
                }
                ResolvedType::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                for ty in [&left, &right] {
                    if !ty.is_bool() && !ty.is_error() {
                        self.push_error(TypeError::TypeMismatch {
                            expected: ResolvedType::Bool,
                            found: ty.clone(),
                            span,
                            file: self.file.clone(),
                            context: format!("logical `{:?}` requires bool operands", op),
                        });
                    }
                }
                ResolvedType::Bool
            }
        }
    }

    fn infer_unary(&mut self, op: &UnaryOp, operand: ResolvedType, span: Span) -> ResolvedType {
        if operand.is_error() {
            return ResolvedType::Error;
        }
        match op {
            UnaryOp::Neg => {
                if operand.is_numeric() {
                    operand
                } else {
                    self.push_error(TypeError::UnaryTypeMismatch {
                        op: "-".to_string(),
                        found: operand,
                        span,
                        file: self.file.clone(),
                    });
                    ResolvedType::Error
                }
            }
            UnaryOp::Not => {
                if operand.is_bool() {
                    ResolvedType::Bool
                } else {
                    self.push_error(TypeError::UnaryTypeMismatch {
                        op: "!".to_string(),
                        found: operand,
                        span,
                        file: self.file.clone(),
                    });
                    ResolvedType::Error
                }
            }
        }
    }

    fn infer_call(&mut self, callee_type: ResolvedType, arg_types: Vec<ResolvedType>, callee: &Expr, span: Span) -> ResolvedType {
        if callee_type.is_error() {
            return ResolvedType::Error;
        }

        let callee_name = match callee {
            Expr::Identifier(id) => id.name.clone(),
            _ => "<expr>".to_string(),
        };

        match callee_type {
            ResolvedType::Function { params, return_type } => {
                // Skip type check for params with Unknown (builtins that accept any type)
                for (i, (expected, got)) in params.iter().zip(arg_types.iter()).enumerate() {
                    if matches!(expected, ResolvedType::Unknown) {
                        continue;
                    }
                    if !expected.is_assignable_from(got) && !got.is_error() {
                        self.push_error(TypeError::ArgumentTypeMismatch {
                            fn_name: callee_name.clone(),
                            param_index: i,
                            expected: expected.clone(),
                            found: got.clone(),
                            span,
                            file: self.file.clone(),
                        });
                    }
                }
                *return_type
            }
            other => {
                self.push_error(TypeError::NotAFunction {
                    name: callee_name,
                    actual_type: other,
                    span,
                    file: self.file.clone(),
                });
                ResolvedType::Error
            }
        }
    }

    fn infer_field_access(&mut self, obj_ty: ResolvedType, field: &str, span: Span) -> ResolvedType {
        if obj_ty.is_error() {
            return ResolvedType::Error;
        }

        match &obj_ty {
            ResolvedType::Struct(name) => {
                let name = name.clone();
                if let Some(fields) = self.struct_fields.get(&name).cloned() {
                    if let Some((_, ty)) = fields.iter().find(|(n, _)| n == field) {
                        return ty.clone();
                    }
                    let available = fields.iter().map(|(n, _)| n.clone()).collect();
                    self.push_error(TypeError::UnknownStructField {
                        struct_name: name,
                        field: field.to_string(),
                        available,
                        span,
                        file: self.file.clone(),
                    });
                }
                ResolvedType::Error
            }
            other => {
                self.push_error(TypeError::FieldOnNonStruct {
                    found: other.clone(),
                    field: field.to_string(),
                    span,
                    file: self.file.clone(),
                });
                ResolvedType::Error
            }
        }
    }

    fn infer_block_type(&mut self, block: &Block) -> ResolvedType {
        self.env.push_scope();
        let mut ty = ResolvedType::Void;
        for stmt in &block.statements {
            ty = self.check_statement_type(stmt);
        }
        self.env.pop_scope();
        ty
    }

    fn check_statement_type(&mut self, stmt: &Statement) -> ResolvedType {
        self.check_statement(stmt);
        ResolvedType::Void
    }

    // ══════════════════════════════════════════
    //   ERROR HELPERS
    // ══════════════════════════════════════════

    fn push_error(&mut self, err: TypeError) {
        self.errors.push(err);
    }

    fn type_mismatch(&mut self, expected: ResolvedType, found: ResolvedType, span: Span, context: &str) {
        self.push_error(TypeError::TypeMismatch {
            expected,
            found,
            span,
            file: self.file.clone(),
            context: context.to_string(),
        });
    }

    fn lookup_type(&self, name: &str) -> Option<ResolvedType> {
        self.env.lookup(name).cloned()
    }
}

#[cfg(test)]
mod checker_init_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> TypeErrors {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        TypeChecker::new(src, "t.lyz").check(&prog)
    }

    #[test]
    fn test_empty_program_no_errors() {
        assert!(check("").is_empty());
    }

    #[test]
    fn test_fn_registered_as_function_type() {
        let src = "fn add(a: int, b: int) -> int { return a + b }";
        let errs = check(src);
        assert!(errs.is_empty(), "{}", errs.format_all(src));
    }

    #[test]
    fn test_struct_registered() {
        let errs = check("struct Point { x: float, y: float }");
        assert!(errs.is_empty());
    }

    #[test]
    fn test_forward_reference_fn() {
        let src = "fn main() { let r = compute(5) }\nfn compute(n: int) -> int { return n }";
        let errs = check(src);
        assert!(errs.is_empty(), "{}", errs.format_all(src));
    }

    #[test]
    fn test_resolve_type_int() {
        let c = TypeChecker::new("", "t");
        assert_eq!(
            c.resolve_type(&TypeExpr::Named(NamedType {
                name: "int".to_string(),
                span: crate::lexer::Span::dummy()
            })),
            ResolvedType::Int
        );
    }

    #[test]
    fn test_resolve_type_optional() {
        let c = TypeChecker::new("", "t");
        let inner = TypeExpr::Named(NamedType {
            name: "str".to_string(),
            span: crate::lexer::Span::dummy(),
        });
        let opt = TypeExpr::Optional(Box::new(inner), crate::lexer::Span::dummy());
        assert_eq!(
            c.resolve_type(&opt),
            ResolvedType::Optional(Box::new(ResolvedType::Str))
        );
    }

    #[test]
    fn test_resolve_type_array() {
        let c = TypeChecker::new("", "t");
        let inner = TypeExpr::Named(NamedType {
            name: "int".to_string(),
            span: crate::lexer::Span::dummy(),
        });
        let arr = TypeExpr::Array(Box::new(inner), crate::lexer::Span::dummy());
        assert_eq!(
            c.resolve_type(&arr),
            ResolvedType::Array(Box::new(ResolvedType::Int))
        );
    }
}

#[cfg(test)]
mod infer_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> TypeErrors {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        TypeChecker::new(src, "t.lyz").check(&prog)
    }

    #[test]
    fn test_let_int_no_err() {
        assert!(check("let x = 42").is_empty());
    }
    #[test]
    fn test_let_typed_match() {
        assert!(check("let x: int = 42").is_empty());
    }
    #[test]
    fn test_let_typed_mismatch() {
        let errs = check("let x: int = \"hello\"");
        assert!(!errs.is_empty());
        assert!(matches!(
            &errs.0[0],
            TypeError::TypeMismatch {
                expected: ResolvedType::Int,
                found: ResolvedType::Str,
                ..
            }
        ));
    }
    #[test]
    fn test_add_int_ok() {
        assert!(check("fn f() { let r = 1 + 2 }").is_empty());
    }
    #[test]
    fn test_add_str_ok() {
        assert!(check("fn f() { let r = \"a\" + \"b\" }").is_empty());
    }
    #[test]
    fn test_add_int_bool_err() {
        let errs = check("fn f() { let r = 1 + true }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::InvalidOperation { .. }));
    }
    #[test]
    fn test_if_non_bool_cond() {
        let errs = check("fn f() { if 42 { } }");
        assert!(!errs.is_empty());
        assert!(matches!(
            &errs.0[0],
            TypeError::NonBoolCondition {
                found: ResolvedType::Int,
                ..
            }
        ));
    }
    #[test]
    fn test_if_bool_cond_ok() {
        assert!(check("fn f() { if true { } }").is_empty());
    }
    #[test]
    fn test_index_non_array() {
        let errs = check("fn f() { let x = 42 let r = x[0] }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::IndexOnNonArray { .. }));
    }
    #[test]
    fn test_index_array_ok() {
        assert!(check("fn f() { let a = [1,2,3] let r = a[0] }").is_empty());
    }
    #[test]
    fn test_call_arg_type_mismatch() {
        let src = "fn add(a: int, b: int) -> int { return a + b }\nfn f() { add(1, \"x\") }";
        let errs = check(src);
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::ArgumentTypeMismatch { .. }));
    }
    #[test]
    fn test_call_correct_args_ok() {
        assert!(check("fn add(a: int, b: int) -> int { return a + b }\nfn f() { add(1, 2) }").is_empty());
    }
    #[test]
    fn test_neg_numeric_ok() {
        assert!(check("fn f() { let r = -5 }").is_empty());
    }
    #[test]
    fn test_neg_bool_err() {
        let errs = check("fn f() { let r = -true }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::UnaryTypeMismatch { .. }));
    }
    #[test]
    fn test_struct_field_ok() {
        let src = "struct P { x: float } fn f() { let p = P { x: 1.0 } let r = p.x }";
        assert!(check(src).is_empty());
    }
    #[test]
    fn test_struct_wrong_field() {
        let src = "struct P { x: float } fn f() { let p = P { x: 1.0 } let r = p.z }";
        let errs = check(src);
        assert!(!errs.is_empty());
        assert!(matches!(
            &errs.0[0],
            TypeError::UnknownStructField { field, .. } if field == "z"
        ));
    }
}

#[cfg(test)]
mod statement_check_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> TypeErrors {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (p, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        TypeChecker::new(src, "t.lyz").check(&p)
    }

    #[test]
    fn test_return_correct_type()  { assert!(check("fn f() -> int { return 42 }").is_empty()); }
    #[test]
    fn test_return_wrong_type()    {
        let errs = check("fn f() -> int { return \"hello\" }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::TypeMismatch { expected: ResolvedType::Int, found: ResolvedType::Str, .. }));
    }
    #[test]
    fn test_if_bool_cond_ok()      { assert!(check("fn f() { if true { } }").is_empty()); }
    #[test]
    fn test_if_int_cond_err()      {
        let errs = check("fn f() { if 42 { } }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::NonBoolCondition { .. }));
    }
    #[test]
    fn test_while_bool_ok()        { assert!(check("fn f() { while true { } }").is_empty()); }
    #[test]
    fn test_while_int_err()        {
        let errs = check("fn f() { while 1 { } }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::NonBoolCondition { context, .. } if context == "while"));
    }
    #[test]
    fn test_for_array_ok()         { assert!(check("fn f() { for i in [1,2,3] { } }").is_empty()); }
    #[test]
    fn test_for_non_array_err()    {
        let errs = check("fn f() { for i in 42 { } }");
        assert!(!errs.is_empty());
        assert!(matches!(&errs.0[0], TypeError::TypeMismatch { .. }));
    }
    #[test]
    fn test_let_type_inferred()    { assert!(check("fn f() { let x = 42 let y = x + 1 }").is_empty()); }
    #[test]
    fn test_for_var_typed_correctly() {
        // After for i in [1,2,3], i should be int
        let src = "fn f() { for i in [1, 2, 3] { let r = i + 1 } }";
        assert!(check(src).is_empty());
    }
    #[test]
    fn test_nested_fn_calls_ok() {
        let src = "fn double(x: int) -> int { return x * 2 }\nfn main() { let r = double(double(5)) }";
        assert!(check(src).is_empty());
    }
}
