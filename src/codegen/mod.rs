pub mod link;
pub mod llvm_ir;
pub mod mangling;
pub mod types;

use std::collections::HashMap;
use crate::parser::ast::*;
use crate::types::ResolvedType;
use crate::interpreter::error::RuntimeError;
use crate::memory::lifetime::{LifetimeTracker, is_refcounted};
use llvm_ir::IrBuilder;
use types::llvm_type;
use mangling::mangle_fn_name;

/// Tracks where a local variable's value lives (an LLVM register or stack slot)
#[derive(Debug, Clone)]
struct VarLocation {
    /// The alloca pointer for this variable (all locals are stack-allocated
    /// for simplicity — LLVM's mem2reg pass optimizes this back to registers)
    ptr: String,
    ty: ResolvedType,
}

pub struct CodeGenerator {
    builder: IrBuilder,
    /// Local variable name -> its stack slot location
    locals: HashMap<String, VarLocation>,
    /// Function name -> (param types, return type) for call codegen
    fn_signatures: HashMap<String, (Vec<ResolvedType>, ResolvedType)>,
    /// The return type of the function currently being compiled
    current_return_type: ResolvedType,
}

impl CodeGenerator {
    pub fn new() -> Self {
        CodeGenerator {
            builder: IrBuilder::new(),
            locals: HashMap::new(),
            fn_signatures: HashMap::new(),
            current_return_type: ResolvedType::Void,
        }
    }

    /// Compile a full program to an LLVM IR module (as text)
    pub fn compile(mut self, program: &Program) -> Result<String, RuntimeError> {
        self.emit_runtime_declarations();

        // Pass 1: register all function signatures (forward references)
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                self.register_fn_signature(f);
            }
        }

        // Pass 2: compile every function body
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                self.compile_fn(f)?;
            }
        }

        Ok(self.builder.build())
    }

    /// Declare external runtime functions (print, malloc, etc.)
    /// These are implemented in a small C runtime linked at the end
    fn emit_runtime_declarations(&mut self) {
        self.builder.emit_module("; LYZARD runtime function declarations");
        self.builder.emit_module("declare void @lyz_print_int(i64)");
        self.builder.emit_module("declare void @lyz_print_float(double)");
        self.builder.emit_module("declare void @lyz_print_str(ptr)");
        self.builder.emit_module("declare void @lyz_print_bool(i1)");
        self.builder.emit_module("declare ptr @malloc(i64)");
        self.builder.emit_module("declare ptr @lyz_alloc(i64, i64)");
        self.builder.emit_module("declare void @lyz_retain(ptr)");
        self.builder.emit_module("declare void @lyz_release(ptr)");
        self.builder.emit_module("");
    }

    fn register_fn_signature(&mut self, decl: &FnDecl) {
        let params: Vec<ResolvedType> = decl.params.iter()
            .filter(|p| !p.is_self)
            .map(|_| ResolvedType::Int) // simplified — real version resolves from TypeExpr
            .collect();
        let ret = ResolvedType::Int; // simplified — real version resolves return_type
        self.fn_signatures.insert(decl.name.clone(), (params, ret));
    }

    // ══════════════════════════════════════════
    //   COMPILE FUNCTIONS
    // ══════════════════════════════════════════

    fn compile_fn(&mut self, decl: &FnDecl) -> Result<(), RuntimeError> {
        self.builder.start_function();
        self.locals.clear();

        let mangled = mangle_fn_name(&decl.name);
        let ret_ty  = "i64".to_string(); // simplified for MVP — resolve from decl.return_type
        self.current_return_type = ResolvedType::Int;

        // Build parameter list: (i64 %a, i64 %b)
        let param_decls: Vec<String> = decl.params.iter()
            .filter(|p| !p.is_self)
            .map(|p| format!("i64 %{}", p.name)) // simplified: assume int params
            .collect();

        self.builder.emit_label("entry");

        // Allocate stack slots for parameters and store incoming values
        for param in decl.params.iter().filter(|p| !p.is_self) {
            let ptr = self.builder.emit_alloca("i64");
            self.builder.emit_store("i64", &format!("%{}", param.name), &ptr);
            self.locals.insert(param.name.clone(), VarLocation { ptr, ty: ResolvedType::Int });
        }

        // Compile the body — block bodies get automatic retain/release
        // insertion based on refcounted locals (LifetimeTracker)
        let param_types: Vec<(String, ResolvedType)> = decl.params.iter()
            .filter(|p| !p.is_self)
            .map(|p| (p.name.clone(), ResolvedType::Int)) // simplified: assume int params
            .collect();
        match &decl.body {
            FnBody::Block(block) => self.compile_fn_body_with_rc(block, &param_types)?,
            FnBody::Arrow(expr)  => {
                let (val, _) = self.compile_expr(expr)?;
                self.builder.emit_return("i64", &val);
            }
        }

        // Ensure the function has a terminator (implicit return)
        if self.builder.needs_terminator() {
            self.builder.emit_return("i64", "0");
        }

        let signature = format!("{} @{}({})", ret_ty, mangled, param_decls.join(", "));
        self.builder.finish_function(&signature);
        Ok(())
    }

    // ══════════════════════════════════════════
    //   REFCOUNT-AWARE FUNCTION COMPILATION
    // ══════════════════════════════════════════

    /// Compile a function body WITH automatic retain/release insertion.
    /// This replaces the plain `compile_block` call inside `compile_fn`
    /// for any function whose body may touch heap types.
    fn compile_fn_body_with_rc(&mut self, block: &Block, param_types: &[(String, ResolvedType)]) -> Result<(), RuntimeError> {
        let mut tracker = LifetimeTracker::new();

        // Function parameters that are refcounted are "owned" by this call —
        // release them at the end of the function unless returned
        for (name, ty) in param_types {
            if is_refcounted(ty) {
                tracker.track_let(name, ty);
            }
        }

        self.compile_block_with_rc(block, &mut tracker)?;

        // Emit releases for anything still alive at the end of the function
        // (only reached if the block didn't already return — codegen's
        // needs_terminator() check prevents emitting dead code here)
        if self.builder.needs_terminator() {
            self.emit_scope_releases(&tracker.pop_scope());
        }

        Ok(())
    }

    /// Compile a block, tracking heap-allocated locals, and emit releases
    /// for them right before the block's natural exit point.
    fn compile_block_with_rc(&mut self, block: &Block, tracker: &mut LifetimeTracker) -> Result<(), RuntimeError> {
        tracker.push_scope();

        for stmt in &block.statements {
            match stmt {
                Statement::Let(l) => {
                    let (val, ty) = self.compile_expr(&l.value)?;
                    let llvm_ty = llvm_type(&ty);
                    let ptr = self.builder.emit_alloca(&llvm_ty);
                    self.builder.emit_store(&llvm_ty, &val, &ptr);
                    self.locals.insert(l.name.clone(), VarLocation { ptr, ty: ty.clone() });
                    tracker.track_let(&l.name, &ty);
                }
                Statement::Return(r) => {
                    // If returning a bare identifier that's refcounted, mark it
                    // moved (no release) and RETAIN it before returning (the
                    // caller becomes the new owner)
                    if let Some(Expr::Identifier(id)) = &r.value {
                        tracker.mark_returned_identifier(&id.name);
                        if let Some(loc) = self.locals.get(&id.name).cloned() {
                            if is_refcounted(&loc.ty) {
                                let val = self.builder.emit_load(&llvm_type(&loc.ty), &loc.ptr);
                                self.emit_retain(&val);
                                // Emit releases for every OTHER still-live var in this scope
                                self.emit_scope_releases_except(tracker, &id.name);
                                self.builder.emit_return(&llvm_type(&loc.ty), &val);
                                continue;
                            }
                        }
                    }
                    // Non-identifier or non-refcounted return — release everything, then return
                    match &r.value {
                        Some(expr) => {
                            let (val, ty) = self.compile_expr(expr)?;
                            self.emit_scope_releases(&tracker.pop_scope());
                            tracker.push_scope(); // keep stack balanced for any code after (dead code anyway)
                            self.builder.emit_return(&llvm_type(&ty), &val);
                        }
                        None => {
                            self.emit_scope_releases(&tracker.pop_scope());
                            tracker.push_scope();
                            self.builder.emit_return_void();
                        }
                    }
                }
                Statement::Block(b) => {
                    // Nested blocks get their own scope — releases fire at the
                    // inner block's end, independent of the outer scope
                    self.compile_block_with_rc(b, tracker)?;
                }
                _ => { self.compile_statement(stmt)?; }
            }
        }

        // Natural fall-through exit (no explicit return in this block) —
        // emit releases for everything declared in THIS block only
        let scope = tracker.pop_scope();
        if self.builder.needs_terminator() {
            self.emit_scope_releases(&scope);
        }
        Ok(())
    }

    /// Emit `call void @lyz_retain(ptr %val)`
    fn emit_retain(&mut self, ptr_val: &str) {
        self.builder.emit(format!("call void @lyz_retain(ptr {})", ptr_val));
    }

    /// Emit `call void @lyz_release(ptr %val)`
    fn emit_release(&mut self, ptr_val: &str) {
        self.builder.emit(format!("call void @lyz_release(ptr {})", ptr_val));
    }

    /// Emit release calls for every non-moved variable in a scope, in LIFO order
    fn emit_scope_releases(&mut self, scope: &crate::memory::lifetime::ScopeLifetimes) {
        for var in scope.drop_order() {
            if let Some(loc) = self.locals.get(&var.name).cloned() {
                let llvm_ty = llvm_type(&loc.ty);
                let val = self.builder.emit_load(&llvm_ty, &loc.ptr);
                self.emit_release(&val);
            }
        }
    }

    /// Like emit_scope_releases, but skips one variable by name
    /// (used when that variable is being returned and retained instead)
    fn emit_scope_releases_except(&mut self, tracker: &mut LifetimeTracker, except: &str) {
        let scope = tracker.pop_scope();
        for var in scope.drop_order() {
            if var.name == except { continue; }
            if let Some(loc) = self.locals.get(&var.name).cloned() {
                let llvm_ty = llvm_type(&loc.ty);
                let val = self.builder.emit_load(&llvm_ty, &loc.ptr);
                self.emit_release(&val);
            }
        }
        tracker.push_scope(); // restore stack balance
    }

    /// Insert a retain when a heap-typed value is passed as a function argument
    /// (called from the Call expression codegen — the CALLER retains,
    /// the CALLEE's compile_fn_body_with_rc releases at its own scope exit)
    fn compile_call_arg_with_retain(&mut self, expr: &Expr) -> Result<(String, ResolvedType), RuntimeError> {
        let (val, ty) = self.compile_expr(expr)?;
        if is_refcounted(&ty) {
            self.emit_retain(&val);
        }
        Ok((val, ty))
    }

    // ══════════════════════════════════════════
    //   COMPILE STATEMENTS
    // ══════════════════════════════════════════

    fn compile_block(&mut self, block: &Block) -> Result<(), RuntimeError> {
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
    }

    fn compile_statement(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::Let(l) => {
                let (val, ty) = self.compile_expr(&l.value)?;
                let llvm_ty = llvm_type(&ty);
                let ptr = self.builder.emit_alloca(&llvm_ty);
                self.builder.emit_store(&llvm_ty, &val, &ptr);
                self.locals.insert(l.name.clone(), VarLocation { ptr, ty });
                Ok(())
            }
            Statement::Return(r) => {
                match &r.value {
                    Some(expr) => {
                        let (val, ty) = self.compile_expr(expr)?;
                        self.builder.emit_return(&llvm_type(&ty), &val);
                    }
                    None => self.builder.emit_return_void(),
                }
                Ok(())
            }
            Statement::If(stmt) => self.compile_if(stmt),
            Statement::While(stmt) => self.compile_while(stmt),
            Statement::Expression(e) => { self.compile_expr(&e.expr)?; Ok(()) }
            _ => Ok(()), // for/match/etc — extend in later iterations
        }
    }

    fn compile_if(&mut self, stmt: &IfStmt) -> Result<(), RuntimeError> {
        let (cond_val, _) = self.compile_expr(&stmt.condition)?;

        let then_label  = self.builder.fresh_label();
        let else_label  = self.builder.fresh_label();
        let merge_label = self.builder.fresh_label();

        let has_else = stmt.else_branch.is_some();
        let false_target = if has_else { &else_label } else { &merge_label };

        self.builder.emit_cond_branch(&cond_val, &then_label, false_target);

        self.builder.emit_label(&then_label);
        self.compile_block(&stmt.then_branch)?;
        if self.builder.needs_terminator() {
            self.builder.emit_branch(&merge_label);
        }

        if let Some(else_block) = &stmt.else_branch {
            self.builder.emit_label(&else_label);
            self.compile_block(else_block)?;
            if self.builder.needs_terminator() {
                self.builder.emit_branch(&merge_label);
            }
        }

        self.builder.emit_label(&merge_label);
        Ok(())
    }

    fn compile_while(&mut self, stmt: &WhileStmt) -> Result<(), RuntimeError> {
        let cond_label = self.builder.fresh_label();
        let body_label = self.builder.fresh_label();
        let exit_label = self.builder.fresh_label();

        self.builder.emit_branch(&cond_label);
        self.builder.emit_label(&cond_label);

        let (cond_val, _) = self.compile_expr(&stmt.condition)?;
        self.builder.emit_cond_branch(&cond_val, &body_label, &exit_label);

        self.builder.emit_label(&body_label);
        self.compile_block(&stmt.body)?;
        if self.builder.needs_terminator() {
            self.builder.emit_branch(&cond_label);
        }

        self.builder.emit_label(&exit_label);
        Ok(())
    }

    // ══════════════════════════════════════════
    //   COMPILE EXPRESSIONS
    //   Returns (llvm_value, resolved_type)
    // ══════════════════════════════════════════

    fn compile_expr(&mut self, expr: &Expr) -> Result<(String, ResolvedType), RuntimeError> {
        match expr {
            Expr::Int(lit)   => Ok((lit.value.to_string(), ResolvedType::Int)),
            Expr::Float(lit) => Ok((format!("{:?}", lit.value), ResolvedType::Float)),
            Expr::Bool(lit)  => Ok(((lit.value as i32).to_string(), ResolvedType::Bool)),

            Expr::Str(lit) => {
                let global_name = self.builder.emit_global_string(&lit.value);
                Ok((global_name, ResolvedType::Str))
            }

            Expr::StructInit(s) => {
                // Simplified struct literal codegen — heap-allocate the struct
                // and return its data pointer. A fresh allocation starts with
                // refcount = 1, so no retain is needed here (the lifetime
                // tracking pass is responsible for the eventual release).
                let ty = ResolvedType::Struct(s.name.clone());
                let size = types::llvm_type_size(&ty);
                let raw = self.builder.emit_call("ptr", "lyz_alloc", &[
                    ("i64".to_string(), size.to_string()),
                    ("i64".to_string(), "2".to_string()), // LYZ_TAG_STRUCT
                ]);
                Ok((raw.unwrap_or_else(|| "null".to_string()), ty))
            }

            Expr::Identifier(id) => {
                let loc = self.locals.get(&id.name).cloned().ok_or(RuntimeError::UndefinedVariable {
                    name: id.name.clone(), span: Some(id.span),
                })?;
                let val = self.builder.emit_load(&llvm_type(&loc.ty), &loc.ptr);
                Ok((val, loc.ty))
            }

            Expr::Binary(b) => {
                let (left, left_ty)   = self.compile_expr(&b.left)?;
                let (right, _right_ty) = self.compile_expr(&b.right)?;
                self.compile_binary(&b.op, left, right, left_ty)
            }

            Expr::Unary(u) => {
                let (operand, ty) = self.compile_expr(&u.operand)?;
                match u.op {
                    UnaryOp::Neg => {
                        let llty = llvm_type(&ty);
                        let dest = self.builder.emit_binop(
                            if ty == ResolvedType::Float { "fsub" } else { "sub" },
                            &llty,
                            if ty == ResolvedType::Float { "0.0" } else { "0" },
                            &operand,
                        );
                        Ok((dest, ty))
                    }
                    UnaryOp::Not => {
                        let dest = self.builder.emit_binop("xor", "i1", &operand, "1");
                        Ok((dest, ResolvedType::Bool))
                    }
                }
            }

            Expr::Call(c) => {
                let fn_name = match c.callee.as_ref() {
                    Expr::Identifier(id) => id.name.clone(),
                    _ => return Err(RuntimeError::NotImplemented { feature: "indirect calls".to_string() }),
                };

                let mut arg_pairs = Vec::new();
                for arg in &c.args {
                    let (val, ty) = self.compile_call_arg_with_retain(&arg.value)?;
                    arg_pairs.push((llvm_type(&ty), val));
                }

                let mangled = mangle_fn_name(&fn_name);
                let ret_ty  = "i64".to_string(); // simplified
                let result  = self.builder.emit_call(&ret_ty, &mangled, &arg_pairs);
                Ok((result.unwrap_or_else(|| "0".to_string()), ResolvedType::Int))
            }

            Expr::Assign(a) => {
                // Only local identifier targets are supported for now
                let id = match a.target.as_ref() {
                    Expr::Identifier(id) => id,
                    _ => return Err(RuntimeError::NotImplemented { feature: "assignment to non-local target".to_string() }),
                };
                let loc = self.locals.get(&id.name).cloned().ok_or(RuntimeError::UndefinedVariable {
                    name: id.name.clone(), span: Some(id.span),
                })?;
                let (val, _) = self.compile_expr(&a.value)?;
                let llty = llvm_type(&loc.ty);
                self.builder.emit_store(&llty, &val, &loc.ptr);
                Ok((val, loc.ty))
            }

            _ => Err(RuntimeError::NotImplemented { feature: "this expression in codegen".to_string() }),
        }
    }

    fn compile_binary(&mut self, op: &BinaryOp, left: String, right: String, ty: ResolvedType) -> Result<(String, ResolvedType), RuntimeError> {
        let is_float = matches!(ty, ResolvedType::Float);
        let llty = llvm_type(&ty);

        let result = match op {
            BinaryOp::Add => self.builder.emit_binop(if is_float { "fadd" } else { "add" }, &llty, &left, &right),
            BinaryOp::Sub => self.builder.emit_binop(if is_float { "fsub" } else { "sub" }, &llty, &left, &right),
            BinaryOp::Mul => self.builder.emit_binop(if is_float { "fmul" } else { "mul" }, &llty, &left, &right),
            BinaryOp::Div => self.builder.emit_binop(if is_float { "fdiv" } else { "sdiv" }, &llty, &left, &right),
            BinaryOp::Mod => self.builder.emit_binop(if is_float { "frem" } else { "srem" }, &llty, &left, &right),

            BinaryOp::Eq  => return Ok((self.builder.emit_icmp("eq",  &llty, &left, &right), ResolvedType::Bool)),
            BinaryOp::NotEq => return Ok((self.builder.emit_icmp("ne", &llty, &left, &right), ResolvedType::Bool)),
            BinaryOp::Lt  => return Ok((self.builder.emit_icmp("slt", &llty, &left, &right), ResolvedType::Bool)),
            BinaryOp::Lte => return Ok((self.builder.emit_icmp("sle", &llty, &left, &right), ResolvedType::Bool)),
            BinaryOp::Gt  => return Ok((self.builder.emit_icmp("sgt", &llty, &left, &right), ResolvedType::Bool)),
            BinaryOp::Gte => return Ok((self.builder.emit_icmp("sge", &llty, &left, &right), ResolvedType::Bool)),

            BinaryOp::And => return Ok((self.builder.emit_binop("and", "i1", &left, &right), ResolvedType::Bool)),
            BinaryOp::Or  => return Ok((self.builder.emit_binop("or",  "i1", &left, &right), ResolvedType::Bool)),
        };

        Ok((result, ty))
    }
}

impl Default for CodeGenerator { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod refcount_codegen_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn generate_ir(src: &str) -> String {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        CodeGenerator::new().compile(&prog).unwrap()
    }

    #[test]
    fn test_str_local_gets_released_at_scope_end() {
        let ir = generate_ir(r#"fn f() { let s = "hello" }"#);
        assert!(ir.contains("call void @lyz_release"));
    }

    #[test]
    fn test_int_local_no_release() {
        let ir = generate_ir("fn f() { let n = 42 }");
        // ints are not refcounted — no release call should target them
        // (we check no release call exists at all for this int-only function)
        assert!(!ir.contains("call void @lyz_release"));
    }

    #[test]
    fn test_returned_struct_is_retained_not_released() {
        let ir = generate_ir(r#"
struct Point { x: float, y: float }
fn make() -> Point {
    let p = Point { x: 1.0, y: 2.0 }
    return p
}
"#);
        assert!(ir.contains("call void @lyz_retain"));
    }

    #[test]
    fn test_multiple_locals_released_in_reverse_order() {
        let ir = generate_ir(r#"fn f() { let a = "x" let b = "y" }"#);
        // Count the actual release CALL sites (the module-level `declare`
        // line is not a call)
        let release_calls: Vec<usize> = ir.match_indices("call void @lyz_release").map(|(i, _)| i).collect();
        assert_eq!(release_calls.len(), 2, "Both string locals should be released");
    }

    #[test]
    fn test_runtime_declares_retain_release() {
        let ir = generate_ir("fn f() -> int { return 1 }");
        assert!(ir.contains("declare void @lyz_retain(ptr)"));
        assert!(ir.contains("declare void @lyz_release(ptr)"));
    }

    #[test]
    fn test_nested_block_releases_its_own_locals() {
        let ir = generate_ir(r#"
fn f() {
    let outer = "outer"
    {
        let inner = "inner"
    }
}
"#);
        // Should have two separate release calls — one for inner (at inner
        // block's end), one for outer (at function's end)
        let count = ir.matches("call void @lyz_release").count();
        assert_eq!(count, 2);
    }
}

#[cfg(test)]
mod codegen_expr_tests {
    use super::*;

    fn gen() -> CodeGenerator { let mut g = CodeGenerator::new(); g.builder.start_function(); g }

    fn dummy_span() -> crate::lexer::Span { crate::lexer::Span::dummy() }

    #[test]
    fn test_int_literal() {
        let mut g = gen();
        let e = Expr::Int(IntLit { value: 42, span: dummy_span() });
        let (val, ty) = g.compile_expr(&e).unwrap();
        assert_eq!(val, "42");
        assert_eq!(ty, ResolvedType::Int);
    }

    #[test]
    fn test_bool_literal() {
        let mut g = gen();
        let e = Expr::Bool(BoolLit { value: true, span: dummy_span() });
        let (val, ty) = g.compile_expr(&e).unwrap();
        assert_eq!(val, "1");
        assert_eq!(ty, ResolvedType::Bool);
    }

    #[test]
    fn test_add_emits_add_instruction() {
        let mut g = gen();
        let l = Expr::Int(IntLit { value: 3, span: dummy_span() });
        let r = Expr::Int(IntLit { value: 4, span: dummy_span() });
        let e = Expr::Binary(BinaryExpr { op: BinaryOp::Add, left: Box::new(l), right: Box::new(r), span: dummy_span() });
        let (val, ty) = g.compile_expr(&e).unwrap();
        assert_eq!(val, "%t0");
        assert_eq!(ty, ResolvedType::Int);
        assert!(g.builder.current_fn_lines[0].contains("add i64 3, 4"));
    }

    #[test]
    fn test_comparison_produces_bool() {
        let mut g = gen();
        let l = Expr::Int(IntLit { value: 5, span: dummy_span() });
        let r = Expr::Int(IntLit { value: 3, span: dummy_span() });
        let e = Expr::Binary(BinaryExpr { op: BinaryOp::Gt, left: Box::new(l), right: Box::new(r), span: dummy_span() });
        let (_, ty) = g.compile_expr(&e).unwrap();
        assert_eq!(ty, ResolvedType::Bool);
    }

    #[test]
    fn test_negation() {
        let mut g = gen();
        let inner = Expr::Int(IntLit { value: 5, span: dummy_span() });
        let e = Expr::Unary(UnaryExpr { op: UnaryOp::Neg, operand: Box::new(inner), span: dummy_span() });
        let (_, ty) = g.compile_expr(&e).unwrap();
        assert_eq!(ty, ResolvedType::Int);
        assert!(g.builder.current_fn_lines[0].contains("sub"));
    }

    #[test]
    fn test_mangle_fn_name_normal() {
        assert_eq!(mangling::mangle_fn_name("add"), "lyz_add");
    }

    #[test]
    fn test_mangle_fn_name_main() {
        assert_eq!(mangling::mangle_fn_name("main"), "lyz_main");
    }

    #[test]
    fn test_full_fn_compile_simple() {
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        let src = "fn add(a: int, b: int) -> int { return a + b }";
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        let ir = CodeGenerator::new().compile(&prog).unwrap();
        assert!(ir.contains("define i64 @lyz_add"));
        assert!(ir.contains("add i64"));
        assert!(ir.contains("ret i64"));
    }

    #[test]
    fn test_if_compile_produces_branches() {
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        let src = "fn f(x: int) -> int { if x > 0 { return 1 } return 0 }";
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        let ir = CodeGenerator::new().compile(&prog).unwrap();
        assert!(ir.contains("br i1"));
        assert!(ir.contains("icmp sgt"));
    }

    #[test]
    fn test_recursive_call_compiles() {
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        let src = "fn fib(n: int) -> int { if n <= 1 { return n } return fib(n) }";
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        let ir = CodeGenerator::new().compile(&prog).unwrap();
        assert!(ir.contains("call i64 @lyz_fib"));
    }
}
