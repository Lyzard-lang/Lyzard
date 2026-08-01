pub mod builtins;
pub mod env;
pub mod error;
pub mod value;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::lexer::Span;
use crate::parser::ast::*;

use self::builtins::all_builtins;
use self::env::Environment;
use self::error::RuntimeError;
use self::value::Value;

/// Tree-walking interpreter: walks the validated AST and produces values.
#[derive(Debug)]
pub struct Interpreter {
    pub env: Environment,
    pub output: Vec<String>,
    pub capture_output: bool,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Environment::new();
        for (name, _, func) in all_builtins() {
            env.define(name.to_string(), Value::Builtin { name, func });
        }
        Interpreter {
            env,
            output: Vec::new(),
            capture_output: false,
        }
    }

    /// Execute a whole program: register top-level functions first so
    /// forward references work, then run every other top-level declaration.
    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        // Register every top-level function against a *shared* global env.
        // All function closures point at that same live environment, so
        // self-recursive and mutually-recursive calls can resolve themselves.
        let shared = Rc::new(RefCell::new(self.env.clone()));
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                shared.borrow_mut().define(
                    f.name.clone(),
                    Value::Function {
                        name: f.name.clone(),
                        params: f.params.clone(),
                        body: f.body.clone(),
                        closure: Rc::clone(&shared),
                    },
                );
            }
        }

        // Continue executing top-level declarations in that global env.
        self.env = (*shared).borrow().clone();

        for decl in &program.declarations {
            match decl {
                Declaration::Function(_) => {}
                Declaration::Let(d) => {
                    let value = self.eval_expr(&d.value)?;
                    self.env.define(d.name.clone(), value);
                }
                Declaration::Const(c) => {
                    let value = self.eval_expr(&c.value)?;
                    self.env.define(c.name.clone(), value);
                }
                Declaration::Statement(s) => {
                    self.eval_statement(s)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Evaluate an expression into a runtime value.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Int(i) => Ok(Value::Int(i.value)),
            Expr::Float(f) => Ok(Value::Float(f.value)),
            Expr::Str(s) => Ok(Value::Str(s.value.clone())),
            Expr::Bool(b) => Ok(Value::Bool(b.value)),
            Expr::Char(c) => Ok(Value::Char(c.value)),
            Expr::Null(_) => Ok(Value::Null),
            Expr::Identifier(id) => {
                self.env
                    .get(&id.name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable {
                        name: id.name.clone(),
                        span: Some(id.span),
                    })
            }
            Expr::Array(arr) => {
                let mut items = Vec::new();
                for e in &arr.elements {
                    items.push(self.eval_expr(e)?);
                }
                Ok(Value::Array(items))
            }
            Expr::StructInit(si) => {
                let mut fields = HashMap::new();
                for (name, e) in &si.fields {
                    fields.insert(name.clone(), self.eval_expr(e)?);
                }
                Ok(Value::Struct {
                    name: si.name.clone(),
                    fields,
                })
            }
            Expr::Binary(bin) => {
                let left = self.eval_expr(&bin.left)?;
                let right = self.eval_expr(&bin.right)?;
                self.eval_binary(bin.op, left, right, bin.span)
            }
            Expr::Unary(un) => {
                let operand = self.eval_expr(&un.operand)?;
                self.eval_unary(un.op, operand)
            }
            Expr::If(if_expr) => {
                let cond = self.eval_expr(&if_expr.condition)?;
                if cond.is_truthy() {
                    self.eval_block(&if_expr.then_branch)
                } else {
                    match &if_expr.else_branch {
                        Some(b) => self.eval_block(b),
                        None => Ok(Value::Void),
                    }
                }
            }
            Expr::Match(m) => {
                let subject = self.eval_expr(&m.subject)?;
                self.eval_match_arms(subject, &m.arms)
            }
            Expr::Block(b) => self.eval_block(b),
            Expr::Call(c) => {
                let callee = self.eval_expr(&c.callee)?;
                let mut args = Vec::new();
                for arg in &c.args {
                    args.push(self.eval_expr(&arg.value)?);
                }
                self.call_function(callee, args, c.span)
            }
            Expr::MethodCall(mc) => {
                let obj = self.eval_expr(&mc.object)?;
                let mut args = Vec::new();
                for arg in &mc.args {
                    args.push(self.eval_expr(&arg.value)?);
                }
                self.eval_method_call(obj, &mc.method, args, mc.span)
            }
            Expr::Field(f) => {
                let obj = self.eval_expr(&f.object)?;
                self.eval_field_access(obj, &f.field, f.span)
            }
            Expr::Index(i) => {
                let obj = self.eval_expr(&i.object)?;
                let idx = self.eval_expr(&i.index)?;
                self.eval_index(obj, idx, i.span)
            }
            Expr::Assign(a) => {
                let value = self.eval_expr(&a.value)?;
                self.eval_assign_target(&a.target, value, a.span)
            }
            Expr::Propagate(p) => {
                let v = self.eval_expr(&p.expr)?;
                match v {
                    Value::Err(e) => Err(RuntimeError::UnhandledError {
                        message: e.to_display_string(),
                    }),
                    other => Ok(other),
                }
            }
            Expr::NullCoalesce(nc) => {
                let left = self.eval_expr(&nc.left)?;
                if matches!(left, Value::Null) {
                    self.eval_expr(&nc.right)
                } else {
                    Ok(left)
                }
            }
            Expr::Range(r) => {
                let start = self.eval_expr(&r.start)?;
                let end = self.eval_expr(&r.end)?;
                self.eval_range(start, end, r.inclusive, r.span)
            }
            _ => Err(RuntimeError::NotImplemented {
                feature: "this expression".to_string(),
            }),
        }
    }

    /// Invoke a function value (user-defined or builtin) with arguments.
    pub fn call_function(
        &mut self,
        fn_val: Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match fn_val {
            Value::Builtin { name, func } => {
                if self.capture_output && (name == "print" || name == "println") {
                    let text = args
                        .iter()
                        .map(|a| a.to_display_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.output.push(text);
                    return Ok(Value::Void);
                }
                func(args)
            }
            Value::Function {
                name,
                params,
                body,
                closure,
            } => {
                let expected = params.iter().filter(|p| !p.is_self).count();
                if args.len() != expected {
                    return Err(RuntimeError::TypeError {
                        expected: format!("{} arguments", expected),
                        got: args.len().to_string(),
                    });
                }

                let saved_env = std::mem::replace(&mut self.env, (*closure).borrow().clone());
                self.env.push_call(&name)?;
                for (param, arg) in params.iter().filter(|p| !p.is_self).zip(args) {
                    self.env.define(param.name.clone(), arg);
                }
                let result = match &body {
                    FnBody::Block(b) => self.eval_block(b)?,
                    FnBody::Arrow(e) => self.eval_expr(e)?,
                };
                self.env.pop_call();
                self.env = saved_env;
                Ok(match result {
                    Value::Return(v) => *v,
                    other => other,
                })
            }
            other => Err(RuntimeError::NotCallable {
                type_name: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    /// Read `object.field`.
    fn eval_field_access(
        &mut self,
        obj: Value,
        field: &str,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match obj {
            Value::Struct { name, fields } => match fields.get(field) {
                Some(v) => Ok(v.clone()),
                None => Err(RuntimeError::FieldNotFound {
                    struct_name: name,
                    field: field.to_string(),
                    span: Some(span),
                }),
            },
            other => Err(RuntimeError::TypeError {
                expected: "struct".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Read `object[index]`, supporting negative (from-end) indices.
    fn eval_index(&mut self, obj: Value, idx: Value, span: Span) -> Result<Value, RuntimeError> {
        match (obj, idx) {
            (Value::Array(items), Value::Int(n)) => {
                let len = items.len() as i64;
                let pos = if n < 0 { len + n } else { n };
                if pos < 0 || pos >= len {
                    Err(RuntimeError::IndexOutOfBounds {
                        index: n,
                        length: items.len(),
                        span: Some(span),
                    })
                } else {
                    Ok(items[pos as usize].clone())
                }
            }
            (Value::Str(s), Value::Int(n)) => {
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let pos = if n < 0 { len + n } else { n };
                if pos < 0 || pos >= len {
                    Err(RuntimeError::IndexOutOfBounds {
                        index: n,
                        length: chars.len(),
                        span: Some(span),
                    })
                } else {
                    Ok(Value::Char(chars[pos as usize]))
                }
            }
            (other, idx) => Err(RuntimeError::TypeError {
                expected: "indexable (array or string)".to_string(),
                got: format!("{} indexed by {}", other.type_name(), idx.type_name()),
            }),
        }
    }

    /// Assign `value` to a target expression.
    fn eval_assign_target(
        &mut self,
        target: &Expr,
        value: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match target {
            Expr::Identifier(id) => {
                self.env.set(&id.name, value.clone())?;
                Ok(value)
            }
            Expr::Field(f) => {
                let obj = self.eval_expr(&f.object)?;
                match obj {
                    Value::Struct { name, mut fields } => match fields.get_mut(&f.field) {
                        Some(slot) => {
                            *slot = value.clone();
                            Ok(value)
                        }
                        None => Err(RuntimeError::FieldNotFound {
                            struct_name: name,
                            field: f.field.clone(),
                            span: Some(span),
                        }),
                    },
                    other => Err(RuntimeError::TypeError {
                        expected: "struct".to_string(),
                        got: other.type_name().to_string(),
                    }),
                }
            }
            Expr::Index(i) => {
                let obj = self.eval_expr(&i.object)?;
                let idx = self.eval_expr(&i.index)?;
                match (obj, idx) {
                    (Value::Array(mut items), Value::Int(n)) => {
                        let len = items.len() as i64;
                        let pos = if n < 0 { len + n } else { n };
                        if pos < 0 || pos >= len {
                            return Err(RuntimeError::IndexOutOfBounds {
                                index: n,
                                length: items.len(),
                                span: Some(span),
                            });
                        }
                        items[pos as usize] = value.clone();
                        Ok(value)
                    }
                    (other, idx) => Err(RuntimeError::TypeError {
                        expected: "indexable array".to_string(),
                        got: format!("{} indexed by {}", other.type_name(), idx.type_name()),
                    }),
                }
            }
            _ => Err(RuntimeError::NotImplemented {
                feature: "this assignment target".to_string(),
            }),
        }
    }

    /// Call `object.method(...)`: built-in array/string methods first, then
    /// user-defined `<type>_<method>` functions.
    fn eval_method_call(
        &mut self,
        obj: Value,
        method: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match &obj {
            Value::Array(items) => match method {
                "len" => return Ok(Value::Int(items.len() as i64)),
                "push" => {
                    let mut new_items = items.clone();
                    if let Some(item) = args.first() {
                        new_items.push(item.clone());
                    }
                    return Ok(Value::Array(new_items));
                }
                "pop" => return Ok(items.last().cloned().unwrap_or(Value::Null)),
                "first" => return Ok(items.first().cloned().unwrap_or(Value::Null)),
                "last" => return Ok(items.last().cloned().unwrap_or(Value::Null)),
                "contains" => {
                    let item = args.first().cloned().unwrap_or(Value::Null);
                    return Ok(Value::Bool(items.iter().any(|v| v == &item)));
                }
                "join" => {
                    let sep = args
                        .first()
                        .and_then(|a| a.as_str().ok())
                        .unwrap_or_default();
                    let parts: Vec<String> = items.iter().map(|v| v.to_display_string()).collect();
                    return Ok(Value::Str(parts.join(sep)));
                }
                _ => {}
            },
            Value::Str(s) => match method {
                "len" => return Ok(Value::Int(s.chars().count() as i64)),
                "upper" => return Ok(Value::Str(s.to_uppercase())),
                "lower" => return Ok(Value::Str(s.to_lowercase())),
                "trim" => return Ok(Value::Str(s.trim().to_string())),
                "startsWith" => {
                    let needle = args
                        .first()
                        .and_then(|a| a.as_str().ok())
                        .unwrap_or_default();
                    return Ok(Value::Bool(s.starts_with(needle)));
                }
                "endsWith" => {
                    let needle = args
                        .first()
                        .and_then(|a| a.as_str().ok())
                        .unwrap_or_default();
                    return Ok(Value::Bool(s.ends_with(needle)));
                }
                "contains" => {
                    let needle = args
                        .first()
                        .and_then(|a| a.as_str().ok())
                        .unwrap_or_default();
                    return Ok(Value::Bool(s.contains(needle)));
                }
                _ => {}
            },
            _ => {}
        }

        // User-defined methods are named `<type>_<method>`; for structs the
        // type part is the struct's own name.
        let type_base = match &obj {
            Value::Struct { name, .. } => name.clone(),
            other => other.type_name().to_string(),
        };
        let method_name = format!("{type_base}_{method}");
        if let Some(fn_val) = self.env.get(&method_name) {
            return self.call_function(fn_val, vec![obj], span);
        }
        Err(RuntimeError::UndefinedFunction {
            name: method.to_string(),
            span: Some(span),
        })
    }

    /// `start..end` / `start..=end` range literals.
    fn eval_range(
        &mut self,
        start: Value,
        end: Value,
        inclusive: bool,
        _span: Span,
    ) -> Result<Value, RuntimeError> {
        match (start, end) {
            (Value::Int(s), Value::Int(e)) => {
                let mut items = Vec::new();
                let mut i = s;
                if inclusive {
                    while i <= e {
                        items.push(Value::Int(i));
                        i += 1;
                    }
                } else {
                    while i < e {
                        items.push(Value::Int(i));
                        i += 1;
                    }
                }
                Ok(Value::Array(items))
            }
            (s, e) => Err(RuntimeError::TypeError {
                expected: "int range bounds".to_string(),
                got: format!("{} and {}", s.type_name(), e.type_name()),
            }),
        }
    }

    /// Evaluate a block: fresh scope, run statements, propagate signals.
    fn eval_block(&mut self, block: &Block) -> Result<Value, RuntimeError> {
        self.env.push_scope();
        let mut result = Value::Void;
        for stmt in &block.statements {
            result = self.eval_statement(stmt)?;
            if result.is_signal() {
                break;
            }
        }
        self.env.pop_scope();
        Ok(result)
    }

    fn eval_statement(&mut self, stmt: &Statement) -> Result<Value, RuntimeError> {
        match stmt {
            Statement::Expression(e) => self.eval_expr(&e.expr),
            Statement::Let(d) => {
                let value = self.eval_expr(&d.value)?;
                self.env.define(d.name.clone(), value);
                Ok(Value::Void)
            }
            Statement::Const(c) => {
                let value = self.eval_expr(&c.value)?;
                self.env.define(c.name.clone(), value);
                Ok(Value::Void)
            }
            Statement::Return(r) => {
                let value = match &r.value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Void,
                };
                Ok(Value::Return(Box::new(value)))
            }
            Statement::If(if_stmt) => {
                let cond = self.eval_expr(&if_stmt.condition)?;
                if cond.is_truthy() {
                    self.eval_block(&if_stmt.then_branch)
                } else {
                    let mut result = Value::Void;
                    let mut matched = false;
                    for branch in &if_stmt.else_if_branches {
                        if self.eval_expr(&branch.condition)?.is_truthy() {
                            result = self.eval_block(&branch.body)?;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        if let Some(b) = &if_stmt.else_branch {
                            result = self.eval_block(b)?;
                        }
                    }
                    Ok(result)
                }
            }
            Statement::While(w) => {
                let mut result = Value::Void;
                while self.eval_expr(&w.condition)?.is_truthy() {
                    let r = self.eval_block(&w.body)?;
                    if r.is_signal() {
                        match r {
                            Value::Break => break,
                            Value::Continue => continue,
                            signal => {
                                result = signal;
                                break;
                            }
                        }
                    } else {
                        result = r;
                    }
                }
                Ok(result)
            }
            Statement::For(f) => {
                let iterable = self.eval_expr(&f.iterable)?;
                let mut result = Value::Void;
                match iterable {
                    Value::Array(items) => {
                        for item in items {
                            self.env.push_scope();
                            self.env.define(f.variable.clone(), item);
                            let r = self.eval_block(&f.body)?;
                            self.env.pop_scope();
                            if r.is_signal() {
                                match r {
                                    Value::Break => break,
                                    Value::Continue => continue,
                                    signal => {
                                        result = signal;
                                        break;
                                    }
                                }
                            } else {
                                result = r;
                            }
                        }
                    }
                    Value::Str(s) => {
                        for ch in s.chars() {
                            self.env.push_scope();
                            self.env.define(f.variable.clone(), Value::Char(ch));
                            let r = self.eval_block(&f.body)?;
                            self.env.pop_scope();
                            if r.is_signal() {
                                match r {
                                    Value::Break => break,
                                    Value::Continue => continue,
                                    signal => {
                                        result = signal;
                                        break;
                                    }
                                }
                            } else {
                                result = r;
                            }
                        }
                    }
                    other => {
                        return Err(RuntimeError::TypeError {
                            expected: "iterable (array or string)".to_string(),
                            got: other.type_name().to_string(),
                        });
                    }
                }
                Ok(result)
            }
            Statement::Loop(l) => {
                let mut result = Value::Void;
                loop {
                    let r = self.eval_block(&l.body)?;
                    if r.is_signal() {
                        match r {
                            Value::Break => break,
                            Value::Continue => continue,
                            signal => {
                                result = signal;
                                break;
                            }
                        }
                    } else {
                        result = r;
                    }
                }
                Ok(result)
            }
            Statement::Break(_) => Ok(Value::Break),
            Statement::Continue(_) => Ok(Value::Continue),
            Statement::Match(m) => {
                let subject = self.eval_expr(&m.subject)?;
                self.eval_match_arms(subject, &m.arms)
            }
            Statement::Spawn(sp) => self.eval_block(&sp.body),
            Statement::Block(b) => self.eval_block(b),
        }
    }

    /// Shared arm dispatch for `match` statements and expressions.
    fn eval_match_arms(
        &mut self,
        subject: Value,
        arms: &[MatchArm],
    ) -> Result<Value, RuntimeError> {
        self.env.push_scope();
        let mut result = Value::Void;
        for arm in arms {
            if pattern_matches(&arm.pattern, &subject) {
                if let Some(guard) = &arm.guard {
                    if !self.eval_expr(guard)?.is_truthy() {
                        continue;
                    }
                }
                bind_pattern(&mut self.env, &arm.pattern, subject.clone());
                result = match &arm.body {
                    MatchBody::Expr(e) => self.eval_expr(e)?,
                    MatchBody::Block(b) => {
                        let mut r = Value::Void;
                        for stmt in &b.statements {
                            r = self.eval_statement(stmt)?;
                            if r.is_signal() {
                                break;
                            }
                        }
                        r
                    }
                };
                break;
            }
        }
        self.env.pop_scope();
        Ok(result)
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match (op, left, right) {
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (BinaryOp::Add, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
            (BinaryOp::Add, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + b as f64)),
            (BinaryOp::Add, Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
            (BinaryOp::Add, Value::Str(a), other) => Ok(Value::Str(a + &other.to_display_string())),

            (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (BinaryOp::Sub, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
            (BinaryOp::Sub, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),

            (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (BinaryOp::Mul, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
            (BinaryOp::Mul, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),

            (BinaryOp::Div, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Int(a / b))
            }
            (BinaryOp::Div, Value::Float(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Float(a / b))
            }
            (BinaryOp::Div, Value::Int(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Float(a as f64 / b))
            }
            (BinaryOp::Div, Value::Float(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Float(a / b as f64))
            }

            (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Int(a % b))
            }
            (BinaryOp::Mod, Value::Float(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Float(a % b))
            }
            (BinaryOp::Mod, Value::Int(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Float(a as f64 % b))
            }
            (BinaryOp::Mod, Value::Float(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: Some(span) });
                }
                Ok(Value::Float(a % b as f64))
            }

            (BinaryOp::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Lt, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) < b)),
            (BinaryOp::Lt, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a < b as f64)),
            (BinaryOp::Lte, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Lte, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Lte, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) <= b)),
            (BinaryOp::Lte, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a <= b as f64)),
            (BinaryOp::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Gt, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) > b)),
            (BinaryOp::Gt, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a > b as f64)),
            (BinaryOp::Gte, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
            (BinaryOp::Gte, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
            (BinaryOp::Gte, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) >= b)),
            (BinaryOp::Gte, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a >= b as f64)),

            (BinaryOp::Eq, a, b) => Ok(Value::Bool(a == b)),
            (BinaryOp::NotEq, a, b) => Ok(Value::Bool(a != b)),
            (BinaryOp::And, a, b) => Ok(Value::Bool(a.is_truthy() && b.is_truthy())),
            (BinaryOp::Or, a, b) => Ok(Value::Bool(a.is_truthy() || b.is_truthy())),

            (op, left, right) => Err(RuntimeError::TypeError {
                expected: format!("operands for {}", op.as_str()),
                got: format!("{} and {}", left.type_name(), right.type_name()),
            }),
        }
    }

    fn eval_unary(&mut self, op: UnaryOp, operand: Value) -> Result<Value, RuntimeError> {
        match (op, operand) {
            (UnaryOp::Neg, Value::Int(i)) => Ok(Value::Int(-i)),
            (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOp::Not, b) => Ok(Value::Bool(!b.is_truthy())),
            (op, _) => Err(RuntimeError::TypeError {
                expected: format!("valid operand for {}", op.as_str()),
                got: "unsupported type".to_string(),
            }),
        }
    }
}

/// Does `value` match `pattern`?
fn pattern_matches(pattern: &Pattern, value: &Value) -> bool {
    match pattern {
        Pattern::Wildcard(_) => true,
        Pattern::Binding(_) => true,
        Pattern::Literal(lit) => match &lit.value {
            LiteralValue::Int(i) => matches!(value, Value::Int(v) if v == i),
            LiteralValue::Float(f) => matches!(value, Value::Float(v) if v == f),
            LiteralValue::Str(s) => matches!(value, Value::Str(v) if v == s),
            LiteralValue::Bool(b) => matches!(value, Value::Bool(v) if v == b),
            LiteralValue::Char(c) => matches!(value, Value::Char(v) if v == c),
            LiteralValue::Null => matches!(value, Value::Null),
        },
        Pattern::EnumVariant(ev) => match value {
            Value::Struct { name, .. } => name == &ev.variant_name,
            _ => false,
        },
        Pattern::Or(o) => o.alternatives.iter().any(|p| pattern_matches(p, value)),
    }
}

/// Define pattern bindings in `env`. Enum variants bind positionally from
/// their field list; `Or` patterns bind the first alternative only.
fn bind_pattern(env: &mut Environment, pattern: &Pattern, value: Value) {
    match pattern {
        Pattern::Binding(bp) => {
            env.define(bp.name.clone(), value);
        }
        Pattern::EnumVariant(ev) => {
            if let Value::Struct { fields, .. } = value {
                let mut ordered: Vec<(&String, &Value)> = fields.iter().collect();
                ordered.sort_by(|a, b| a.0.cmp(b.0));
                for (i, sub) in ev.bindings.iter().enumerate() {
                    let field_value = ordered
                        .get(i)
                        .map(|(_, v)| (*v).clone())
                        .unwrap_or(Value::Null);
                    bind_pattern(env, sub, field_value);
                }
            }
        }
        Pattern::Or(o) => {
            if let Some(first) = o.alternatives.first() {
                bind_pattern(env, first, value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod interpreter_tests {
    use super::*;

    fn interp() -> Interpreter {
        let mut i = Interpreter::new();
        i.env.define("x".to_string(), Value::Int(42));
        i.env
            .define("name".to_string(), Value::Str("hi".to_string()));
        i
    }

    fn s() -> Span {
        Span::dummy()
    }

    fn int_lit(v: i64) -> Expr {
        Expr::Int(IntLit {
            value: v,
            span: s(),
        })
    }
    fn float_lit(v: f64) -> Expr {
        Expr::Float(FloatLit {
            value: v,
            span: s(),
        })
    }
    fn str_lit(v: &str) -> Expr {
        Expr::Str(StrLit {
            value: v.to_string(),
            span: s(),
        })
    }
    fn bool_lit(v: bool) -> Expr {
        Expr::Bool(BoolLit {
            value: v,
            span: s(),
        })
    }
    fn char_lit(v: char) -> Expr {
        Expr::Char(CharLit {
            value: v,
            span: s(),
        })
    }
    fn ident(name: &str) -> Expr {
        Expr::Identifier(IdentExpr {
            name: name.to_string(),
            span: s(),
        })
    }
    fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
        Expr::Binary(BinaryExpr {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span: s(),
        })
    }
    fn unary(op: UnaryOp, operand: Expr) -> Expr {
        Expr::Unary(UnaryExpr {
            op,
            operand: Box::new(operand),
            span: s(),
        })
    }

    #[test]
    fn test_int_literal() {
        assert_eq!(interp().eval_expr(&int_lit(42)).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(
            interp().eval_expr(&float_lit(3.5)).unwrap(),
            Value::Float(3.5)
        );
    }

    #[test]
    fn test_str_literal() {
        assert_eq!(
            interp().eval_expr(&str_lit("hi")).unwrap(),
            Value::Str("hi".to_string())
        );
    }

    #[test]
    fn test_bool_literal() {
        assert_eq!(
            interp().eval_expr(&bool_lit(true)).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_char_literal() {
        assert_eq!(
            interp().eval_expr(&char_lit('z')).unwrap(),
            Value::Char('z')
        );
    }

    #[test]
    fn test_null_literal() {
        assert_eq!(
            interp()
                .eval_expr(&Expr::Null(NullLit { span: s() }))
                .unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_array_literal() {
        let e = Expr::Array(ArrayLit {
            elements: vec![int_lit(1), int_lit(2)],
            span: s(),
        });
        assert_eq!(
            interp().eval_expr(&e).unwrap(),
            Value::Array(vec![Value::Int(1), Value::Int(2)])
        );
    }

    #[test]
    fn test_struct_init() {
        let e = Expr::StructInit(StructInitExpr {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), int_lit(1)), ("y".to_string(), int_lit(2))],
            span: s(),
        });
        let got = interp().eval_expr(&e).unwrap();
        assert_eq!(got.to_display_string(), "Point { x: 1, y: 2 }");
        assert_eq!(got.type_name(), "struct");
    }

    #[test]
    fn test_identifier_lookup() {
        assert_eq!(interp().eval_expr(&ident("x")).unwrap(), Value::Int(42));
        assert_eq!(
            interp().eval_expr(&ident("name")).unwrap(),
            Value::Str("hi".to_string())
        );
    }

    #[test]
    fn test_undefined_name_error() {
        let mut i = interp();
        assert!(matches!(
            i.eval_expr(&ident("nope")),
            Err(RuntimeError::UndefinedVariable { .. })
        ));
    }

    #[test]
    fn test_add_ints() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, int_lit(2), int_lit(3)))
                .unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_add_floats() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, float_lit(1.5), float_lit(2.5)))
                .unwrap(),
            Value::Float(4.0)
        );
    }

    #[test]
    fn test_add_mixed() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, int_lit(1), float_lit(2.5)))
                .unwrap(),
            Value::Float(3.5)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, float_lit(2.5), int_lit(1)))
                .unwrap(),
            Value::Float(3.5)
        );
    }

    #[test]
    fn test_add_str_concat() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, str_lit("Hello "), str_lit("World")))
                .unwrap(),
            Value::Str("Hello World".to_string())
        );
    }

    #[test]
    fn test_sub_mul_div() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Sub, int_lit(10), int_lit(4)))
                .unwrap(),
            Value::Int(6)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Mul, int_lit(6), int_lit(7)))
                .unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Div, int_lit(10), int_lit(2)))
                .unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_div_by_zero() {
        let mut i = interp();
        assert!(matches!(
            i.eval_expr(&binary(BinaryOp::Div, int_lit(1), int_lit(0))),
            Err(RuntimeError::DivisionByZero { .. })
        ));
        assert!(matches!(
            i.eval_expr(&binary(BinaryOp::Div, float_lit(1.0), float_lit(0.0))),
            Err(RuntimeError::DivisionByZero { .. })
        ));
    }

    #[test]
    fn test_mod() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Mod, int_lit(10), int_lit(3)))
                .unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn test_comparisons() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Lt, int_lit(1), int_lit(2)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Gte, int_lit(3), int_lit(3)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Lt, int_lit(1), float_lit(1.5)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_equality() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Eq, int_lit(1), int_lit(1)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::NotEq, int_lit(1), int_lit(2)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Eq, str_lit("a"), str_lit("a")))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Eq, int_lit(1), float_lit(1.0)))
                .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_and_or() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::And, bool_lit(true), bool_lit(false)))
                .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Or, bool_lit(true), bool_lit(false)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_unary_neg() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Neg, int_lit(5)))
                .unwrap(),
            Value::Int(-5)
        );
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Neg, float_lit(2.5)))
                .unwrap(),
            Value::Float(-2.5)
        );
    }

    #[test]
    fn test_unary_not() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Not, bool_lit(true)))
                .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Not, int_lit(0)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_type_mismatch_error() {
        let mut i = interp();
        assert!(matches!(
            i.eval_expr(&binary(BinaryOp::Sub, str_lit("x"), int_lit(1))),
            Err(RuntimeError::TypeError { .. })
        ));
    }

    #[test]
    fn test_call_builtin_print() {
        let mut i = interp();
        let e = Expr::Call(CallExpr {
            callee: Box::new(ident("print")),
            args: vec![Argument {
                label: None,
                value: Expr::Str(StrLit {
                    value: "hi".to_string(),
                    span: s(),
                }),
                span: s(),
            }],
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Void);
    }
}

#[cfg(test)]
mod control_flow_tests {
    use super::*;

    fn s() -> Span {
        Span::dummy()
    }

    fn int_lit(v: i64) -> Expr {
        Expr::Int(IntLit {
            value: v,
            span: s(),
        })
    }
    fn bool_lit(v: bool) -> Expr {
        Expr::Bool(BoolLit {
            value: v,
            span: s(),
        })
    }
    fn ident(name: &str) -> Expr {
        Expr::Identifier(IdentExpr {
            name: name.to_string(),
            span: s(),
        })
    }
    fn block(stmts: Vec<Statement>) -> Block {
        Block {
            statements: stmts,
            span: s(),
        }
    }
    fn expr_stmt(e: Expr) -> Statement {
        Statement::Expression(ExprStmt { expr: e, span: s() })
    }
    fn let_stmt(name: &str, e: Expr) -> Statement {
        Statement::Let(LetDecl {
            name: name.to_string(),
            mutable: false,
            type_annotation: None,
            value: e,
            span: s(),
        })
    }
    fn return_stmt(e: Option<Expr>) -> Statement {
        Statement::Return(ReturnStmt {
            value: e,
            span: s(),
        })
    }
    fn break_stmt() -> Statement {
        Statement::Break(BreakStmt {
            label: None,
            span: s(),
        })
    }
    fn continue_stmt() -> Statement {
        Statement::Continue(ContinueStmt {
            label: None,
            span: s(),
        })
    }

    #[test]
    fn test_return_produces_signal() {
        let mut i = Interpreter::new();
        let r = i.eval_statement(&return_stmt(Some(int_lit(5)))).unwrap();
        assert!(matches!(r, Value::Return(v) if matches!(*v, Value::Int(5))));
    }

    #[test]
    fn test_return_void_signal() {
        let mut i = Interpreter::new();
        let r = i.eval_statement(&return_stmt(None)).unwrap();
        assert!(matches!(r, Value::Return(v) if matches!(*v, Value::Void)));
    }

    #[test]
    fn test_if_true_takes_then_branch() {
        let mut i = Interpreter::new();
        let stmt = Statement::If(IfStmt {
            condition: bool_lit(true),
            then_branch: block(vec![expr_stmt(int_lit(1))]),
            else_if_branches: vec![],
            else_branch: Some(block(vec![expr_stmt(int_lit(2))])),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(1));
    }

    #[test]
    fn test_if_false_takes_else_branch() {
        let mut i = Interpreter::new();
        let stmt = Statement::If(IfStmt {
            condition: bool_lit(false),
            then_branch: block(vec![expr_stmt(int_lit(1))]),
            else_if_branches: vec![],
            else_branch: Some(block(vec![expr_stmt(int_lit(2))])),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(2));
    }

    #[test]
    fn test_if_else_if_chain() {
        let mut i = Interpreter::new();
        let stmt = Statement::If(IfStmt {
            condition: bool_lit(false),
            then_branch: block(vec![expr_stmt(int_lit(1))]),
            else_if_branches: vec![ElseIfBranch {
                condition: bool_lit(true),
                body: block(vec![expr_stmt(int_lit(3))]),
                span: s(),
            }],
            else_branch: Some(block(vec![expr_stmt(int_lit(2))])),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(3));
    }

    #[test]
    fn test_block_scope_isolation() {
        let mut i = Interpreter::new();
        let b = block(vec![let_stmt("x", int_lit(10)), expr_stmt(ident("x"))]);
        assert_eq!(i.eval_block(&b).unwrap(), Value::Int(10));
        assert!(!i.env.is_defined("x"));
    }

    #[test]
    fn test_while_false_skips_body() {
        let mut i = Interpreter::new();
        let stmt = Statement::While(WhileStmt {
            condition: bool_lit(false),
            body: block(vec![expr_stmt(int_lit(1))]),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Void);
    }

    #[test]
    fn test_while_true_break_terminates() {
        let mut i = Interpreter::new();
        let stmt = Statement::While(WhileStmt {
            condition: bool_lit(true),
            body: block(vec![break_stmt()]),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Void);
    }

    #[test]
    fn test_loop_break_terminates() {
        let mut i = Interpreter::new();
        let stmt = Statement::Loop(LoopStmt {
            body: block(vec![break_stmt()]),
            label: None,
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Void);
    }

    #[test]
    fn test_continue_returns_signal() {
        let mut i = Interpreter::new();
        let r = i.eval_statement(&continue_stmt()).unwrap();
        assert!(matches!(r, Value::Continue));
    }

    #[test]
    fn test_for_over_array() {
        let mut i = Interpreter::new();
        let stmt = Statement::For(ForStmt {
            variable: "item".to_string(),
            iterable: Expr::Array(ArrayLit {
                elements: vec![int_lit(1), int_lit(2), int_lit(3)],
                span: s(),
            }),
            body: block(vec![expr_stmt(ident("item"))]),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(3));
        assert!(!i.env.is_defined("item"));
    }

    #[test]
    fn test_for_over_string() {
        let mut i = Interpreter::new();
        let stmt = Statement::For(ForStmt {
            variable: "ch".to_string(),
            iterable: Expr::Str(StrLit {
                value: "ab".to_string(),
                span: s(),
            }),
            body: block(vec![expr_stmt(ident("ch"))]),
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Char('b'));
    }

    #[test]
    fn test_for_wrong_iterable() {
        let mut i = Interpreter::new();
        let stmt = Statement::For(ForStmt {
            variable: "x".to_string(),
            iterable: int_lit(5),
            body: block(vec![]),
            span: s(),
        });
        assert!(matches!(
            i.eval_statement(&stmt),
            Err(RuntimeError::TypeError { .. })
        ));
    }

    #[test]
    fn test_match_binds_variable() {
        let mut i = Interpreter::new();
        let stmt = Statement::Match(MatchStmt {
            subject: int_lit(42),
            arms: vec![MatchArm {
                pattern: Pattern::Binding(BindingPattern {
                    name: "n".to_string(),
                    mutable: false,
                    span: s(),
                }),
                guard: None,
                body: MatchBody::Expr(ident("n")),
                span: s(),
            }],
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(42));
        assert!(!i.env.is_defined("n"));
    }

    #[test]
    fn test_match_wildcard_fallback() {
        let mut i = Interpreter::new();
        let stmt = Statement::Match(MatchStmt {
            subject: int_lit(7),
            arms: vec![
                MatchArm {
                    pattern: Pattern::Literal(LiteralPattern {
                        value: LiteralValue::Int(1),
                        span: s(),
                    }),
                    guard: None,
                    body: MatchBody::Expr(int_lit(100)),
                    span: s(),
                },
                MatchArm {
                    pattern: Pattern::Wildcard(s()),
                    guard: None,
                    body: MatchBody::Expr(int_lit(200)),
                    span: s(),
                },
            ],
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(200));
    }

    #[test]
    fn test_match_literal_arm() {
        let mut i = Interpreter::new();
        let stmt = Statement::Match(MatchStmt {
            subject: int_lit(1),
            arms: vec![MatchArm {
                pattern: Pattern::Literal(LiteralPattern {
                    value: LiteralValue::Int(1),
                    span: s(),
                }),
                guard: None,
                body: MatchBody::Expr(int_lit(100)),
                span: s(),
            }],
            span: s(),
        });
        assert_eq!(i.eval_statement(&stmt).unwrap(), Value::Int(100));
    }

    #[test]
    fn test_if_expression() {
        let mut i = Interpreter::new();
        let e = Expr::If(Box::new(IfExpr {
            condition: bool_lit(true),
            then_branch: block(vec![expr_stmt(int_lit(1))]),
            else_branch: Some(block(vec![expr_stmt(int_lit(2))])),
            span: s(),
        }));
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(1));
    }

    #[test]
    fn test_match_expression() {
        let mut i = Interpreter::new();
        let e = Expr::Match(Box::new(MatchExpr {
            subject: Box::new(int_lit(9)),
            arms: vec![MatchArm {
                pattern: Pattern::Binding(BindingPattern {
                    name: "n".to_string(),
                    mutable: false,
                    span: s(),
                }),
                guard: None,
                body: MatchBody::Expr(ident("n")),
                span: s(),
            }],
            span: s(),
        }));
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(9));
    }
}

#[cfg(test)]
mod call_access_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn s() -> Span {
        Span::dummy()
    }

    fn int_lit(v: i64) -> Expr {
        Expr::Int(IntLit {
            value: v,
            span: s(),
        })
    }
    fn ident(name: &str) -> Expr {
        Expr::Identifier(IdentExpr {
            name: name.to_string(),
            span: s(),
        })
    }
    fn param(name: &str) -> Param {
        Param {
            name: name.to_string(),
            param_type: None,
            is_self: false,
            span: s(),
        }
    }
    fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
        Expr::Binary(BinaryExpr {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span: s(),
        })
    }
    fn fn_value(name: &str, params: Vec<Param>, body: FnBody) -> Value {
        Value::Function {
            name: name.to_string(),
            params,
            body,
            closure: Rc::new(RefCell::new(Environment::new())),
        }
    }
    fn run_source(src: &str) -> Interpreter {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        let mut i = Interpreter::new();
        i.run(&prog).unwrap();
        i
    }

    #[test]
    fn test_call_user_function() {
        let mut i = Interpreter::new();
        let f = fn_value(
            "double",
            vec![param("x")],
            FnBody::Arrow(Box::new(binary(BinaryOp::Mul, ident("x"), int_lit(2)))),
        );
        assert_eq!(
            i.call_function(f, vec![Value::Int(21)], s()).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn test_call_builtin_captured_output() {
        let mut i = Interpreter::new();
        i.capture_output = true;
        let f = Value::Builtin {
            name: "print",
            func: |_| Ok(Value::Void),
        };
        i.call_function(f, vec![Value::Str("hi".to_string())], s())
            .unwrap();
        assert_eq!(i.output, vec!["hi".to_string()]);
    }

    #[test]
    fn test_call_wrong_arity() {
        let mut i = Interpreter::new();
        let f = fn_value(
            "add",
            vec![param("a"), param("b")],
            FnBody::Block(Block {
                statements: vec![],
                span: s(),
            }),
        );
        let r = i.call_function(f, vec![Value::Int(1)], s());
        assert!(matches!(r, Err(RuntimeError::TypeError { .. })));
    }

    #[test]
    fn test_call_not_callable() {
        let mut i = Interpreter::new();
        let r = i.call_function(Value::Int(5), vec![], s());
        assert!(matches!(r, Err(RuntimeError::NotCallable { .. })));
    }

    #[test]
    fn test_closure_capture() {
        let mut i = Interpreter::new();
        i.env.define("base".to_string(), Value::Int(10));
        let f = Value::Function {
            name: "addBase".to_string(),
            params: vec![param("n")],
            body: FnBody::Arrow(Box::new(binary(BinaryOp::Add, ident("base"), ident("n")))),
            closure: Rc::new(RefCell::new(i.env.clone())),
        };
        assert_eq!(
            i.call_function(f, vec![Value::Int(5)], s()).unwrap(),
            Value::Int(15)
        );
    }

    #[test]
    fn test_run_and_call_function() {
        let mut i = run_source("fn add(a, b) { return a + b }");
        let f = i.env.get("add").unwrap();
        assert_eq!(
            i.call_function(f, vec![Value::Int(2), Value::Int(3)], s())
                .unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_recursive_function() {
        let mut i = run_source("fn fact(n) { if n <= 1 { return 1 } return n * fact(n - 1) }");
        let f = i.env.get("fact").unwrap();
        assert_eq!(
            i.call_function(f, vec![Value::Int(5)], s()).unwrap(),
            Value::Int(120)
        );
    }

    #[test]
    fn test_field_access() {
        let mut i = Interpreter::new();
        i.env.define(
            "p".to_string(),
            Value::Struct {
                name: "Point".to_string(),
                fields: HashMap::from([
                    ("x".to_string(), Value::Int(3)),
                    ("y".to_string(), Value::Int(4)),
                ]),
            },
        );
        let e = Expr::Field(FieldExpr {
            object: Box::new(ident("p")),
            field: "y".to_string(),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(4));
    }

    #[test]
    fn test_field_not_found() {
        let mut i = Interpreter::new();
        i.env.define(
            "p".to_string(),
            Value::Struct {
                name: "Point".to_string(),
                fields: HashMap::from([("x".to_string(), Value::Int(3))]),
            },
        );
        let e = Expr::Field(FieldExpr {
            object: Box::new(ident("p")),
            field: "z".to_string(),
            span: s(),
        });
        assert!(matches!(
            i.eval_expr(&e),
            Err(RuntimeError::FieldNotFound { .. })
        ));
    }

    #[test]
    fn test_index_array() {
        let mut i = Interpreter::new();
        i.env.define(
            "arr".to_string(),
            Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
        );
        let e = Expr::Index(IndexExpr {
            object: Box::new(ident("arr")),
            index: Box::new(int_lit(1)),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(20));
    }

    #[test]
    fn test_index_negative() {
        let mut i = Interpreter::new();
        i.env.define(
            "arr".to_string(),
            Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
        );
        let e = Expr::Index(IndexExpr {
            object: Box::new(ident("arr")),
            index: Box::new(int_lit(-1)),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(30));
    }

    #[test]
    fn test_index_out_of_bounds() {
        let mut i = Interpreter::new();
        i.env
            .define("arr".to_string(), Value::Array(vec![Value::Int(10)]));
        let e = Expr::Index(IndexExpr {
            object: Box::new(ident("arr")),
            index: Box::new(int_lit(5)),
            span: s(),
        });
        assert!(matches!(
            i.eval_expr(&e),
            Err(RuntimeError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn test_assign_variable() {
        let mut i = Interpreter::new();
        i.env.define("x".to_string(), Value::Int(1));
        let e = Expr::Assign(AssignExpr {
            target: Box::new(ident("x")),
            value: Box::new(int_lit(99)),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(99));
        assert_eq!(i.env.get("x"), Some(Value::Int(99)));
    }

    #[test]
    fn test_assign_undefined() {
        let mut i = Interpreter::new();
        let e = Expr::Assign(AssignExpr {
            target: Box::new(ident("missing")),
            value: Box::new(int_lit(1)),
            span: s(),
        });
        assert!(matches!(
            i.eval_expr(&e),
            Err(RuntimeError::UndefinedVariable { .. })
        ));
    }

    #[test]
    fn test_assign_array_index() {
        let mut i = Interpreter::new();
        i.env.define(
            "arr".to_string(),
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        );
        let e = Expr::Assign(AssignExpr {
            target: Box::new(Expr::Index(IndexExpr {
                object: Box::new(ident("arr")),
                index: Box::new(int_lit(0)),
                span: s(),
            })),
            value: Box::new(int_lit(100)),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(100));
    }

    #[test]
    fn test_method_string_upper() {
        let mut i = Interpreter::new();
        let e = Expr::MethodCall(MethodCallExpr {
            object: Box::new(Expr::Str(StrLit {
                value: "hello".to_string(),
                span: s(),
            })),
            method: "upper".to_string(),
            args: vec![],
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Str("HELLO".to_string()));
    }

    #[test]
    fn test_method_array_len() {
        let mut i = Interpreter::new();
        let e = Expr::MethodCall(MethodCallExpr {
            object: Box::new(Expr::Array(ArrayLit {
                elements: vec![int_lit(1), int_lit(2)],
                span: s(),
            })),
            method: "len".to_string(),
            args: vec![],
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(2));
    }

    #[test]
    fn test_method_not_found() {
        let mut i = Interpreter::new();
        let e = Expr::MethodCall(MethodCallExpr {
            object: Box::new(Expr::Int(IntLit {
                value: 5,
                span: s(),
            })),
            method: "fly".to_string(),
            args: vec![],
            span: s(),
        });
        assert!(matches!(
            i.eval_expr(&e),
            Err(RuntimeError::UndefinedFunction { .. })
        ));
    }

    #[test]
    fn test_null_coalesce() {
        let mut i = Interpreter::new();
        let e = Expr::NullCoalesce(NullCoalesceExpr {
            left: Box::new(Expr::Null(NullLit { span: s() })),
            right: Box::new(int_lit(5)),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e).unwrap(), Value::Int(5));

        let e2 = Expr::NullCoalesce(NullCoalesceExpr {
            left: Box::new(int_lit(3)),
            right: Box::new(int_lit(5)),
            span: s(),
        });
        assert_eq!(i.eval_expr(&e2).unwrap(), Value::Int(3));
    }

    #[test]
    fn test_range_expression() {
        let mut i = Interpreter::new();
        let e = Expr::Range(RangeExpr {
            start: Box::new(int_lit(1)),
            end: Box::new(int_lit(4)),
            inclusive: false,
            span: s(),
        });
        assert_eq!(
            i.eval_expr(&e).unwrap(),
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }
}
