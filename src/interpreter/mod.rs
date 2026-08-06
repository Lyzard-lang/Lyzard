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

/// A step along a nested assignment path (`a.b[i].c = v`).
enum PathStep {
    Field(String),
    Index(usize),
}

/// Tree-walking interpreter: walks the validated AST and produces values.
#[derive(Debug)]
pub struct Interpreter {
    pub env: Environment,
    pub output: Vec<String>,
    pub capture_output: bool,
    /// Registered enum type names → their variant names (for `Option.Some(...)`).
    pub enums: HashMap<String, Vec<String>>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut interp = Interpreter {
            env: Environment::new(),
            output: Vec::new(),
            capture_output: false,
            enums: HashMap::new(),
        };
        interp.register_builtins();
        interp
    }

    /// Register all built-in functions into the global environment.
    fn register_builtins(&mut self) {
        for (name, _arity, func) in all_builtins() {
            let val = Value::Builtin { name, func };
            self.env.define(name.to_string(), val);
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
            match decl {
                Declaration::Function(f) => {
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
                Declaration::Enum(e) => {
                    self.enums.insert(
                        e.name.clone(),
                        e.variants.iter().map(|v| v.name.clone()).collect(),
                    );
                }
                Declaration::Impl(imp) => {
                    // Impl methods are registered as `<type>_<method>` so that
                    // method calls can dispatch to them.
                    for method in &imp.methods {
                        let fn_name = format!("{}_{}", imp.target, method.name);
                        shared.borrow_mut().define(
                            fn_name.clone(),
                            Value::Function {
                                name: fn_name,
                                params: method.params.clone(),
                                body: method.body.clone(),
                                closure: Rc::clone(&shared),
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        // Continue executing top-level declarations in that global env.
        self.env = (*shared).borrow().clone();

        for decl in &program.declarations {
            match decl {
                Declaration::Function(_) => {}
                other => {
                    self.eval_declaration(other)?;
                }
            }
        }

        // Run the program entry point `main` if it was defined.
        if let Some(main_fn) = self.env.get("main") {
            self.call_function(main_fn, vec![], crate::lexer::Span::dummy())?;
        }
        Ok(())
    }

    /// Register a function in the environment WITHOUT running its body.
    fn register_fn(&mut self, decl: &FnDecl) {
        let val = Value::Function {
            name: decl.name.clone(),
            params: decl.params.clone(),
            body: decl.body.clone(),
            closure: Rc::new(RefCell::new(self.env.clone())),
        };
        self.env.define(decl.name.clone(), val);
    }

    // ══════════════════════════════════════════
    //   DECLARATION EVALUATION
    // ══════════════════════════════════════════

    pub fn eval_declaration(&mut self, decl: &Declaration) -> Result<Value, RuntimeError> {
        match decl {
            Declaration::Let(l) => self.eval_let(l),
            Declaration::Const(c) => self.eval_const(c),
            Declaration::Function(f) => {
                self.register_fn(f);
                Ok(Value::Void)
            }
            Declaration::Statement(s) => self.eval_statement(s),
            Declaration::Struct(_) => Ok(Value::Void), // type registration
            Declaration::Enum(_) => Ok(Value::Void),   // type registration
            Declaration::Impl(_) => Ok(Value::Void),   // handled separately
            Declaration::Interface(_) => Ok(Value::Void),
            Declaration::Import(_) => Ok(Value::Void), // TODO: module system
            Declaration::Module(_) => Ok(Value::Void),
        }
    }

    fn eval_let(&mut self, decl: &LetDecl) -> Result<Value, RuntimeError> {
        let value = self.eval_expr(&decl.value)?;
        self.env.define(decl.name.clone(), value);
        Ok(Value::Void)
    }

    fn eval_const(&mut self, decl: &ConstDecl) -> Result<Value, RuntimeError> {
        let value = self.eval_expr(&decl.value)?;
        self.env.define(decl.name.clone(), value);
        Ok(Value::Void)
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
                self.eval_binary(&bin.op, left, right)
            }
            Expr::Unary(un) => {
                let operand = self.eval_expr(&un.operand)?;
                self.eval_unary(&un.op, operand)
            }
            Expr::If(if_expr) => self.eval_if_expr(if_expr),
            Expr::Match(m) => self.eval_match_expr(m),
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
                // `Option.Some(42)` — enum constructor call on a type name.
                if let Expr::Identifier(id) = &*mc.object {
                    if let Some(variants) = self.enums.get(&id.name) {
                        if variants.iter().any(|v| v == &mc.method) {
                            if mc.args.len() > 1 {
                                return Err(RuntimeError::TypeError {
                                    expected: "enum variant with at most one payload".to_string(),
                                    got: format!("{} arguments", mc.args.len()),
                                });
                            }
                            let payload = match mc.args.first() {
                                Some(arg) => Some(Box::new(self.eval_expr(&arg.value)?)),
                                None => None,
                            };
                            return Ok(Value::Enum {
                                name: id.name.clone(),
                                variant: mc.method.clone(),
                                payload,
                            });
                        }
                    }
                    // `List.new()` — static constructor on a type name.
                    if !self.env.is_defined(&id.name) {
                        let static_name = format!("{}_{}", id.name, mc.method);
                        if let Some(fn_val) = self.env.get(&static_name) {
                            let mut args = Vec::new();
                            for arg in &mc.args {
                                args.push(self.eval_expr(&arg.value)?);
                            }
                            return self.call_function(fn_val, args, mc.span);
                        }
                    }
                }
                let obj = self.eval_expr(&mc.object)?;
                let mut args = Vec::new();
                for arg in &mc.args {
                    args.push(self.eval_expr(&arg.value)?);
                }
                self.eval_method_call(obj, &mc.method, args, mc.span)
            }
            Expr::Field(f) => {
                // `Option.None` — enum variant with no payload used as a value.
                if let Expr::Identifier(id) = &*f.object {
                    if let Some(variants) = self.enums.get(&id.name) {
                        if variants.iter().any(|v| v == &f.field) {
                            return Ok(Value::Enum {
                                name: id.name.clone(),
                                variant: f.field.clone(),
                                payload: None,
                            });
                        }
                    }
                }
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
                self.eval_assign(&a.target, value)
            }
            Expr::Propagate(p) => {
                let val = self.eval_expr(&p.expr)?;
                match val {
                    Value::Err(e) => {
                        // Propagate as Return signal so it bubbles up.
                        Ok(Value::Return(Box::new(Value::Err(e))))
                    }
                    Value::Enum {
                        name,
                        variant,
                        payload,
                    } if name == "Result" && variant == "Err" => {
                        Ok(Value::Return(Box::new(Value::Enum {
                            name,
                            variant,
                            payload,
                        })))
                    }
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
                // Methods (`impl` blocks) declare an explicit `self` param.
                // The receiver arrives as the first argument and is bound to
                // the name `self` inside the call.
                let mut rest: Vec<Value> = args;
                let mut self_val: Option<Value> = None;
                if params.iter().any(|p| p.is_self) {
                    if rest.is_empty() {
                        return Err(RuntimeError::NotCallable {
                            type_name: format!("method {name} requires a receiver"),
                            span: Some(span),
                        });
                    }
                    self_val = Some(rest.remove(0));
                }
                let expected = params.iter().filter(|p| !p.is_self).count();
                if rest.len() != expected {
                    return Err(RuntimeError::NotCallable {
                        type_name: format!(
                            "wrong arg count: {} expected {}, got {}",
                            name,
                            expected,
                            rest.len()
                        ),
                        span: Some(span),
                    });
                }

                let current_depth = self.env.call_depth();
                let saved_env = std::mem::replace(&mut self.env, (*closure).borrow().clone());
                self.env.set_call_depth(current_depth);
                self.env.push_call(&name)?;
                if let Some(sv) = self_val {
                    self.env.define("self".to_string(), sv);
                }
                for (param, arg) in params.iter().filter(|p| !p.is_self).zip(rest) {
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
        &self,
        obj: Value,
        field: &str,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match obj {
            Value::Struct { name, fields } => {
                fields
                    .get(field)
                    .cloned()
                    .ok_or_else(|| RuntimeError::FieldNotFound {
                        struct_name: name,
                        field: field.to_string(),
                        span: Some(span),
                    })
            }
            other => Err(RuntimeError::TypeError {
                expected: "struct".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Read `object[index]`, supporting negative (from-end) indices.
    fn eval_index(&self, obj: Value, idx: Value, span: Span) -> Result<Value, RuntimeError> {
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
            (other, _) => Err(RuntimeError::NotIndexable {
                type_name: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    /// Assign `value` to a target expression.
    fn eval_assign(&mut self, target: &Expr, value: Value) -> Result<Value, RuntimeError> {
        // Direct variable assignment.
        if let Expr::Identifier(id) = target {
            self.env.set(&id.name, value.clone())?;
            return Ok(value);
        }
        // Path-based assignment (`a.b[i].c = v`): rebuild the value from the
        // innermost step outward, then store it back on the base variable.
        if let Some(base) = Self::base_identifier(target) {
            let mut current = value.clone();
            let mut path = Vec::new();
            self.collect_assign_path(target, &mut path)?;
            for step in path.iter().rev() {
                match step {
                    PathStep::Field(name) => match current {
                        Value::Struct {
                            name: n,
                            mut fields,
                        } => {
                            fields.insert(name.clone(), value.clone());
                            current = Value::Struct { name: n, fields };
                        }
                        other => {
                            return Err(RuntimeError::TypeError {
                                expected: "struct field assignment".to_string(),
                                got: other.type_name().to_string(),
                            })
                        }
                    },
                    PathStep::Index(idx) => match current {
                        Value::Array(mut arr) => {
                            if *idx >= arr.len() {
                                return Err(RuntimeError::IndexOutOfBounds {
                                    index: *idx as i64,
                                    length: arr.len(),
                                    span: None,
                                });
                            }
                            arr[*idx] = value.clone();
                            current = Value::Array(arr);
                        }
                        other => {
                            return Err(RuntimeError::TypeError {
                                expected: "array index assignment".to_string(),
                                got: other.type_name().to_string(),
                            })
                        }
                    },
                }
            }
            self.env.set(&base, current)?;
            return Ok(value);
        }
        Err(RuntimeError::TypeError {
            expected: "assignable target".to_string(),
            got: "expression".to_string(),
        })
    }

    /// Collect the mutation path (from base outward) for a nested target.
    fn collect_assign_path(
        &mut self,
        expr: &Expr,
        path: &mut Vec<PathStep>,
    ) -> Result<(), RuntimeError> {
        match expr {
            Expr::Field(f) => {
                self.collect_assign_path(&f.object, path)?;
                path.push(PathStep::Field(f.field.clone()));
                Ok(())
            }
            Expr::Index(i) => {
                let idx = self.eval_expr(&i.index)?.as_int()? as usize;
                self.collect_assign_path(&i.object, path)?;
                path.push(PathStep::Index(idx));
                Ok(())
            }
            Expr::Identifier(_) => Ok(()),
            _ => Err(RuntimeError::TypeError {
                expected: "assignable target".to_string(),
                got: "expression".to_string(),
            }),
        }
    }

    /// The base variable of a nested assignment target (`a` in `a.b[i].c`).
    fn base_identifier(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(id) => Some(id.name.clone()),
            Expr::Field(f) => Self::base_identifier(&f.object),
            Expr::Index(i) => Self::base_identifier(&i.object),
            _ => None,
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
                "isEmpty" => return Ok(Value::Bool(items.is_empty())),
                "first" => return Ok(items.first().cloned().unwrap_or(Value::Null)),
                "last" => return Ok(items.last().cloned().unwrap_or(Value::Null)),
                "contains" => {
                    let target = args.first().cloned().unwrap_or(Value::Null);
                    return Ok(Value::Bool(items.contains(&target)));
                }
                "push" => {
                    let mut new_items = items.clone();
                    new_items.extend(args);
                    return Ok(Value::Array(new_items));
                }
                "pop" => return Ok(items.last().cloned().unwrap_or(Value::Null)),
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
                "trim" => return Ok(Value::Str(s.trim().to_string())),
                "upper" => return Ok(Value::Str(s.to_uppercase())),
                "lower" => return Ok(Value::Str(s.to_lowercase())),
                "isEmpty" => return Ok(Value::Bool(s.is_empty())),
                "contains" => {
                    let needle = args
                        .first()
                        .and_then(|a| a.as_str().ok())
                        .unwrap_or_default();
                    return Ok(Value::Bool(s.contains(needle)));
                }
                "split" => {
                    let sep = args
                        .first()
                        .and_then(|a| a.as_str().ok())
                        .unwrap_or_default();
                    let parts: Vec<Value> =
                        s.split(sep).map(|p| Value::Str(p.to_string())).collect();
                    return Ok(Value::Array(parts));
                }
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
                _ => {}
            },
            _ => {}
        }

        // User-defined methods are named `<type>_<method>`; for structs and
        // enums the type part is the value's own name.
        let type_base = match &obj {
            Value::Struct { name, .. } => name.clone(),
            Value::Enum { name, .. } => name.clone(),
            other => other.type_name().to_string(),
        };
        let method_name = format!("{type_base}_{method}");
        if let Some(fn_val) = self.env.get(&method_name) {
            let mut full_args = vec![obj];
            full_args.extend(args);
            return self.call_function(fn_val, full_args, span);
        }
        Err(RuntimeError::FieldNotFound {
            struct_name: obj.type_name().to_string(),
            field: method.to_string(),
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

    // ══════════════════════════════════════════
    //   STATEMENT EVALUATION
    // ══════════════════════════════════════════

    pub fn eval_statement(&mut self, stmt: &Statement) -> Result<Value, RuntimeError> {
        match stmt {
            Statement::Let(l) => self.eval_let(l),
            Statement::Const(c) => self.eval_const(c),
            Statement::Return(r) => self.eval_return(r),
            Statement::If(i) => self.eval_if(i),
            Statement::While(w) => self.eval_while(w),
            Statement::For(f) => self.eval_for(f),
            Statement::Loop(l) => self.eval_loop(l),
            Statement::Match(m) => self.eval_match(m),
            Statement::Spawn(s) => self.eval_spawn(s),
            Statement::Break(_) => Ok(Value::Break),
            Statement::Continue(_) => Ok(Value::Continue),
            Statement::Block(b) => self.eval_block(b),
            Statement::Expression(e) => self.eval_expr(&e.expr),
        }
    }

    pub fn eval_block(&mut self, block: &Block) -> Result<Value, RuntimeError> {
        self.env.push_scope();
        let mut last = Value::Void;

        for stmt in &block.statements {
            last = self.eval_statement(stmt)?;

            // If we hit a control flow signal, stop immediately
            if last.is_signal() {
                self.env.pop_scope();
                return Ok(last);
            }
        }

        self.env.pop_scope();
        Ok(last)
    }

    fn eval_return(&mut self, stmt: &ReturnStmt) -> Result<Value, RuntimeError> {
        let val = match &stmt.value {
            Some(expr) => self.eval_expr(expr)?,
            None => Value::Void,
        };
        // Wrap in Return signal so it propagates up to call_function()
        Ok(Value::Return(Box::new(val)))
    }

    fn eval_if(&mut self, stmt: &IfStmt) -> Result<Value, RuntimeError> {
        let condition = self.eval_expr(&stmt.condition)?;

        if condition.is_truthy() {
            return self.eval_block(&stmt.then_branch);
        }

        for branch in &stmt.else_if_branches {
            let cond = self.eval_expr(&branch.condition)?;
            if cond.is_truthy() {
                return self.eval_block(&branch.body);
            }
        }

        if let Some(else_block) = &stmt.else_branch {
            return self.eval_block(else_block);
        }

        Ok(Value::Void)
    }

    fn eval_if_expr(&mut self, expr: &IfExpr) -> Result<Value, RuntimeError> {
        let condition = self.eval_expr(&expr.condition)?;

        if condition.is_truthy() {
            self.eval_block(&expr.then_branch)
        } else if let Some(else_block) = &expr.else_branch {
            self.eval_block(else_block)
        } else {
            Ok(Value::Void)
        }
    }

    fn eval_while(&mut self, stmt: &WhileStmt) -> Result<Value, RuntimeError> {
        loop {
            let condition = self.eval_expr(&stmt.condition)?;
            if !condition.is_truthy() {
                break;
            }

            let result = self.eval_block(&stmt.body)?;
            match result {
                Value::Break => break,
                Value::Continue => continue,
                Value::Return(_) => return Ok(result), // propagate return up
                _ => {}
            }
        }
        Ok(Value::Void)
    }

    fn eval_for(&mut self, stmt: &ForStmt) -> Result<Value, RuntimeError> {
        let iterable = self.eval_expr(&stmt.iterable)?;

        let items: Vec<Value> = match iterable {
            Value::Array(arr) => arr,
            Value::Str(s) => s.chars().map(Value::Char).collect(),
            other => {
                return Err(RuntimeError::TypeError {
                    expected: "array (iterable)".to_string(),
                    got: other.type_name().to_string(),
                })
            }
        };

        for item in items {
            self.env.push_scope();
            self.env.define(stmt.variable.clone(), item);

            let result = {
                let mut r = Value::Void;
                for s in &stmt.body.statements {
                    r = self.eval_statement(s)?;
                    if r.is_signal() {
                        break;
                    }
                }
                r
            };

            self.env.pop_scope();

            match result {
                Value::Break => break,
                Value::Continue => continue,
                Value::Return(_) => return Ok(result),
                _ => {}
            }
        }

        Ok(Value::Void)
    }

    fn eval_loop(&mut self, stmt: &LoopStmt) -> Result<Value, RuntimeError> {
        loop {
            let result = self.eval_block(&stmt.body)?;
            match result {
                Value::Break => break,
                Value::Continue => continue,
                Value::Return(_) => return Ok(result),
                _ => {}
            }
        }
        Ok(Value::Void)
    }

    fn eval_match(&mut self, stmt: &MatchStmt) -> Result<Value, RuntimeError> {
        let subject = self.eval_expr(&stmt.subject)?;

        for arm in &stmt.arms {
            if self.pattern_matches(&arm.pattern, &subject) {
                if let Some(guard) = &arm.guard {
                    if !self.eval_expr(guard)?.is_truthy() {
                        continue;
                    }
                }
                self.env.push_scope();
                self.bind_pattern(&arm.pattern, &subject);

                let result = match &arm.body {
                    MatchBody::Expr(e) => self.eval_expr(e)?,
                    MatchBody::Block(b) => {
                        let mut r = Value::Void;
                        for s in &b.statements {
                            r = self.eval_statement(s)?;
                            if r.is_signal() {
                                break;
                            }
                        }
                        r
                    }
                };

                self.env.pop_scope();
                return Ok(result);
            }
        }

        Ok(Value::Void) // no arm matched (should be caught by type checker)
    }

    fn eval_match_expr(&mut self, expr: &MatchExpr) -> Result<Value, RuntimeError> {
        let subject = self.eval_expr(&expr.subject)?;

        for arm in &expr.arms {
            if self.pattern_matches(&arm.pattern, &subject) {
                if let Some(guard) = &arm.guard {
                    if !self.eval_expr(guard)?.is_truthy() {
                        continue;
                    }
                }
                self.env.push_scope();
                self.bind_pattern(&arm.pattern, &subject);
                let result = match &arm.body {
                    MatchBody::Expr(e) => self.eval_expr(e)?,
                    MatchBody::Block(b) => self.eval_block(b)?,
                };
                self.env.pop_scope();
                return Ok(result);
            }
        }

        Ok(Value::Void)
    }

    fn eval_spawn(&mut self, stmt: &SpawnStmt) -> Result<Value, RuntimeError> {
        // Simple version: runs in the same thread synchronously
        // Full async version comes later with channels
        self.eval_block(&stmt.body)?;
        Ok(Value::Void)
    }

    // ══════════════════════════════════════════
    //   PATTERN MATCHING
    // ══════════════════════════════════════════

    fn pattern_matches(&self, pattern: &Pattern, value: &Value) -> bool {
        match pattern {
            Pattern::Wildcard(_) => true,

            Pattern::Binding(_) => true, // bindings always match

            Pattern::Literal(lit) => match &lit.value {
                LiteralValue::Int(n) => matches!(value, Value::Int(v) if v == n),
                LiteralValue::Float(f) => matches!(value, Value::Float(v) if v == f),
                LiteralValue::Str(s) => matches!(value, Value::Str(v) if v == s),
                LiteralValue::Bool(b) => matches!(value, Value::Bool(v) if v == b),
                LiteralValue::Char(c) => matches!(value, Value::Char(v) if v == c),
                LiteralValue::Null => matches!(value, Value::Null),
            },

            Pattern::EnumVariant(ev) => match value {
                Value::Enum { name, variant, .. } => {
                    let name_ok = match &ev.enum_name {
                        Some(expected) => expected == name,
                        None => true,
                    };
                    name_ok && variant == &ev.variant_name
                }
                _ => false,
            },

            Pattern::Or(or) => or
                .alternatives
                .iter()
                .any(|p| self.pattern_matches(p, value)),
        }
    }

    /// Bind pattern variables into the current scope
    fn bind_pattern(&mut self, pattern: &Pattern, value: &Value) {
        match pattern {
            Pattern::Binding(b) => {
                self.env.define(b.name.clone(), value.clone());
            }
            Pattern::EnumVariant(ev) => {
                // Bind inner values for enum variants with a payload.
                if let Value::Enum {
                    payload: Some(payload),
                    ..
                } = value
                {
                    for binding_pat in &ev.bindings {
                        self.bind_pattern(binding_pat, payload);
                    }
                }
            }
            _ => {} // wildcards and literals don't bind
        }
    }

    pub fn eval_binary(
        &self,
        op: &BinaryOp,
        left: Value,
        right: Value,
    ) -> Result<Value, RuntimeError> {
        Ok(match (op, left, right) {
            // ── ARITHMETIC ──────────────────────────────────────
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Value::Float(a + b),
            (BinaryOp::Add, Value::Int(a), Value::Float(b)) => Value::Float(a as f64 + b),
            (BinaryOp::Add, Value::Float(a), Value::Int(b)) => Value::Float(a + b as f64),
            (BinaryOp::Add, Value::Str(a), Value::Str(b)) => Value::Str(a + &b),
            (BinaryOp::Add, Value::Str(a), other) => Value::Str(a + &other.to_display_string()),

            (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Value::Float(a - b),
            (BinaryOp::Sub, Value::Int(a), Value::Float(b)) => Value::Float(a as f64 - b),
            (BinaryOp::Sub, Value::Float(a), Value::Int(b)) => Value::Float(a - b as f64),

            (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => Value::Int(a * b),
            (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Value::Float(a * b),
            (BinaryOp::Mul, Value::Int(a), Value::Float(b)) => Value::Float(a as f64 * b),
            (BinaryOp::Mul, Value::Float(a), Value::Int(b)) => Value::Float(a * b as f64),

            (BinaryOp::Div, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: None });
                }
                Value::Int(a / b)
            }
            (BinaryOp::Div, Value::Float(a), Value::Float(b)) => Value::Float(a / b),
            (BinaryOp::Div, Value::Int(a), Value::Float(b)) => Value::Float(a as f64 / b),
            (BinaryOp::Div, Value::Float(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: None });
                }
                Value::Float(a / b as f64)
            }

            (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span: None });
                }
                Value::Int(a % b)
            }

            // ── COMPARISON ──────────────────────────────────────
            (BinaryOp::Eq, a, b) => Value::Bool(a == b),
            (BinaryOp::NotEq, a, b) => Value::Bool(a != b),

            (BinaryOp::Lt, Value::Int(a), Value::Int(b)) => Value::Bool(a < b),
            (BinaryOp::Lte, Value::Int(a), Value::Int(b)) => Value::Bool(a <= b),
            (BinaryOp::Gt, Value::Int(a), Value::Int(b)) => Value::Bool(a > b),
            (BinaryOp::Gte, Value::Int(a), Value::Int(b)) => Value::Bool(a >= b),

            (BinaryOp::Lt, Value::Float(a), Value::Float(b)) => Value::Bool(a < b),
            (BinaryOp::Lte, Value::Float(a), Value::Float(b)) => Value::Bool(a <= b),
            (BinaryOp::Gt, Value::Float(a), Value::Float(b)) => Value::Bool(a > b),
            (BinaryOp::Gte, Value::Float(a), Value::Float(b)) => Value::Bool(a >= b),

            (BinaryOp::Lt, Value::Str(a), Value::Str(b)) => Value::Bool(a < b),
            (BinaryOp::Gt, Value::Str(a), Value::Str(b)) => Value::Bool(a > b),

            // ── LOGICAL ─────────────────────────────────────────
            (BinaryOp::And, Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),
            (BinaryOp::Or, Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),

            // ── TYPE ERROR ──────────────────────────────────────
            (op, left, right) => {
                return Err(RuntimeError::TypeError {
                    expected: format!("compatible types for '{:?}'", op),
                    got: format!("{} and {}", left.type_name(), right.type_name()),
                })
            }
        })
    }

    pub fn eval_unary(&self, op: &UnaryOp, operand: Value) -> Result<Value, RuntimeError> {
        match (op, operand) {
            (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
            (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOp::Not, other) => Ok(Value::Bool(!other.is_truthy())),
            (op, operand) => Err(RuntimeError::TypeError {
                expected: format!("compatible type for '{:?}'", op),
                got: operand.type_name().to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod interpreter_tests {
    use super::*;

    fn interp() -> Interpreter {
        Interpreter::new()
    }

    fn int(n: i64) -> Expr {
        Expr::Int(IntLit {
            value: n,
            span: Span::dummy(),
        })
    }
    fn float(f: f64) -> Expr {
        Expr::Float(FloatLit {
            value: f,
            span: Span::dummy(),
        })
    }
    fn boolean(b: bool) -> Expr {
        Expr::Bool(BoolLit {
            value: b,
            span: Span::dummy(),
        })
    }
    fn string(s: &str) -> Expr {
        Expr::Str(StrLit {
            value: s.to_string(),
            span: Span::dummy(),
        })
    }
    fn binary(l: Expr, op: BinaryOp, r: Expr) -> Expr {
        Expr::Binary(BinaryExpr {
            op,
            left: Box::new(l),
            right: Box::new(r),
            span: Span::dummy(),
        })
    }
    fn unary(op: UnaryOp, e: Expr) -> Expr {
        Expr::Unary(UnaryExpr {
            op,
            operand: Box::new(e),
            span: Span::dummy(),
        })
    }

    #[test]
    fn test_int_literal() {
        assert_eq!(interp().eval_expr(&int(42)).unwrap(), Value::Int(42));
    }
    #[test]
    #[allow(clippy::approx_constant)]
    fn test_float_literal() {
        assert_eq!(
            interp().eval_expr(&float(3.14)).unwrap(),
            Value::Float(3.14)
        );
    }
    #[test]
    fn test_bool_literal() {
        assert_eq!(
            interp().eval_expr(&boolean(true)).unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_str_literal() {
        assert_eq!(
            interp().eval_expr(&string("hi")).unwrap(),
            Value::Str("hi".to_string())
        );
    }

    #[test]
    fn test_add_int() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(3), BinaryOp::Add, int(4)))
                .unwrap(),
            Value::Int(7)
        );
    }
    #[test]
    fn test_sub_int() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(10), BinaryOp::Sub, int(3)))
                .unwrap(),
            Value::Int(7)
        );
    }
    #[test]
    fn test_mul_int() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(3), BinaryOp::Mul, int(4)))
                .unwrap(),
            Value::Int(12)
        );
    }
    #[test]
    fn test_div_int() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(10), BinaryOp::Div, int(2)))
                .unwrap(),
            Value::Int(5)
        );
    }
    #[test]
    fn test_mod_int() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(10), BinaryOp::Mod, int(3)))
                .unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn test_div_by_zero() {
        let result = interp().eval_expr(&binary(int(5), BinaryOp::Div, int(0)));
        assert!(matches!(result, Err(RuntimeError::DivisionByZero { .. })));
    }

    #[test]
    fn test_string_concat() {
        let result = interp()
            .eval_expr(&binary(string("hello "), BinaryOp::Add, string("world")))
            .unwrap();
        assert_eq!(result, Value::Str("hello world".to_string()));
    }

    #[test]
    fn test_eq_true() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(5), BinaryOp::Eq, int(5)))
                .unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_eq_false() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(5), BinaryOp::Eq, int(6)))
                .unwrap(),
            Value::Bool(false)
        );
    }
    #[test]
    fn test_lt() {
        assert_eq!(
            interp()
                .eval_expr(&binary(int(3), BinaryOp::Lt, int(5)))
                .unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_and_true() {
        assert_eq!(
            interp()
                .eval_expr(&binary(boolean(true), BinaryOp::And, boolean(true)))
                .unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_and_false() {
        assert_eq!(
            interp()
                .eval_expr(&binary(boolean(true), BinaryOp::And, boolean(false)))
                .unwrap(),
            Value::Bool(false)
        );
    }
    #[test]
    fn test_or_true() {
        assert_eq!(
            interp()
                .eval_expr(&binary(boolean(false), BinaryOp::Or, boolean(true)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_unary_neg_int() {
        assert_eq!(
            interp().eval_expr(&unary(UnaryOp::Neg, int(5))).unwrap(),
            Value::Int(-5)
        );
    }
    #[test]
    #[allow(clippy::approx_constant)]
    fn test_unary_neg_float() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Neg, float(3.14)))
                .unwrap(),
            Value::Float(-3.14)
        );
    }
    #[test]
    fn test_unary_not_true() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Not, boolean(true)))
                .unwrap(),
            Value::Bool(false)
        );
    }
    #[test]
    fn test_unary_not_false() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Not, boolean(false)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_type_mismatch_error() {
        let result = interp().eval_expr(&binary(string("x"), BinaryOp::Sub, int(1)));
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod control_flow_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Parse and run a program, returning the interpreter so tests can
    /// inspect the environment, captured output, or call top-level fns.
    fn run(src: &str) -> Interpreter {
        let tokens = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(tokens, "t.lyz", src).parse().unwrap();
        let mut interp = Interpreter::new();
        interp.capture_output = true;
        interp.run(&prog).unwrap();
        interp
    }

    /// Fetch a top-level function by name and call it with no args.
    fn eval_to_value(interp: &mut Interpreter, name: &str) -> Value {
        let fn_val = interp.env.get(name).expect("function not found").clone();
        interp
            .call_function(fn_val, vec![], crate::lexer::Span::dummy())
            .unwrap()
    }

    #[test]
    fn test_if_true() {
        let src = r#"
            fn __main__() -> int {
                if true {
                    return 42
                }
                0
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(42));
    }

    #[test]
    fn test_while_loop() {
        let src = r#"
            fn __main__() -> int {
                let mut n = 0
                while n < 5 {
                    n = n + 1
                }
                n
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(5));
    }

    #[test]
    fn test_for_loop() {
        let src = r#"
            fn __main__() -> int {
                let mut sum = 0
                for i in range(0, 5) {
                    sum = sum + i
                }
                sum
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(10));
    }

    #[test]
    fn test_loop_break() {
        let src = r#"
            fn __main__() -> int {
                let mut n = 0
                loop {
                    n = n + 1
                    if n >= 5 {
                        break
                    }
                }
                n
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(5));
    }

    #[test]
    fn test_return_from_fn() {
        let src = r#"
            fn double(x: int) -> int {
                return x * 2
            }
        "#;
        let mut interp = run(src);
        let f = interp
            .env
            .get("double")
            .expect("function not found")
            .clone();
        let result = interp
            .call_function(f, vec![Value::Int(21)], crate::lexer::Span::dummy())
            .unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_match_wildcard() {
        let src = r#"
            let x = 1
            match x {
                1 -> print("one")
                _ -> print("other")
            }
        "#;
        let interp = run(src);
        assert_eq!(interp.output, vec!["one".to_string()]);
    }

    #[test]
    fn test_nested_if_else() {
        let src = r#"
            fn __main__() -> int {
                let x = 10
                if x > 100 {
                    return 1
                } else if x > 50 {
                    return 2
                } else {
                    return 3
                }
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(3));
    }

    #[test]
    fn test_block_scope_isolation() {
        let src = r#"
            fn __main__() -> int {
                let x = 1
                {
                    let x = 99
                }
                x
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(1));
    }

    #[test]
    fn test_continue_in_for() {
        let src = r#"
            fn __main__() -> int {
                let mut count = 0
                for i in range(0, 5) {
                    if i == 2 {
                        continue
                    }
                    count = count + 1
                }
                count
            }
        "#;
        assert_eq!(eval_to_value(&mut run(src), "__main__"), Value::Int(4));
    }
}

#[cfg(test)]
mod call_access_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run_ok(src: &str) {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (p, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        Interpreter::new().run(&p).expect("Should succeed");
    }

    fn run_err(src: &str) -> RuntimeError {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (p, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        Interpreter::new().run(&p).unwrap_err()
    }

    #[test]
    fn test_fn_call_basic() {
        run_ok("fn add(a: int, b: int) -> int { return a + b }\nfn main() { add(3, 4) }");
    }

    #[test]
    fn test_fn_call_return_value() {
        run_ok(
            r#"
fn double(x: int) -> int { return x * 2 }
fn main() { let r = double(5) }
"#,
        );
    }

    #[test]
    fn test_recursive_fn() {
        run_ok(
            r#"
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() { fib(10) }
"#,
        );
    }

    #[test]
    fn test_stack_overflow() {
        let err = run_err(
            r#"
fn forever() -> int { return forever() }
fn main() { forever() }
"#,
        );
        assert!(matches!(err, RuntimeError::StackOverflow { .. }));
    }

    #[test]
    fn test_struct_field_access() {
        run_ok(
            r#"
struct Point { x: float, y: float }
fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    print(p.x)
}
"#,
        );
    }

    #[test]
    fn test_field_not_found() {
        let err = run_err(
            r#"
struct Point { x: float, y: float }
fn main() {
    let p = Point { x: 1.0, y: 2.0 }
    print(p.z)
}
"#,
        );
        assert!(matches!(err, RuntimeError::FieldNotFound { field, .. } if field == "z"));
    }

    #[test]
    fn test_array_index() {
        run_ok("fn main() { let arr = [10, 20, 30] print(arr[1]) }");
    }

    #[test]
    fn test_index_out_of_bounds() {
        let err = run_err("fn main() { let arr = [1, 2] print(arr[99]) }");
        assert!(matches!(
            err,
            RuntimeError::IndexOutOfBounds { index: 99, .. }
        ));
    }

    #[test]
    fn test_negative_index() {
        run_ok("fn main() { let arr = [1, 2, 3] print(arr[-1]) }"); // last element
    }

    #[test]
    fn test_string_method_upper() {
        run_ok(r#"fn main() { let s = "hello" print(s.upper()) }"#);
    }

    #[test]
    fn test_string_method_split() {
        run_ok(r#"fn main() { let s = "a,b,c" let parts = s.split(",") print(parts.len()) }"#);
    }

    #[test]
    fn test_array_method_len() {
        run_ok("fn main() { let arr = [1, 2, 3] print(arr.len()) }");
    }

    #[test]
    fn test_not_callable_error() {
        let err = run_err("fn main() { let x = 42 x() }");
        assert!(matches!(err, RuntimeError::NotCallable { .. }));
    }
}
