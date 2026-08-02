pub mod link;
pub mod llvm_ir;
pub mod mangling;
pub mod types;

use std::collections::HashMap;
use crate::parser::ast::*;
use crate::types::ResolvedType;
use crate::interpreter::error::RuntimeError;
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

        // Compile the body
        match &decl.body {
            FnBody::Block(block) => self.compile_block(block)?,
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
                    let (val, ty) = self.compile_expr(&arg.value)?;
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
