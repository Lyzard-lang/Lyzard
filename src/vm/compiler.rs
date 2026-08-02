use std::collections::HashMap;
use crate::parser::ast::*;
use crate::interpreter::value::Value;
use crate::interpreter::error::RuntimeError;
use super::opcode::{Chunk, Opcode};

/// Compiler state for one function scope
#[derive(Debug)]
struct FnScope {
    /// Maps local variable name → slot index
    locals: Vec<(String, usize)>,
    /// Bytecode chunk being built
    chunk: Chunk,
}

impl FnScope {
    fn new(name: impl Into<String>) -> Self {
        FnScope { locals: Vec::new(), chunk: Chunk::new(name) }
    }

    /// Define a new local variable, return its slot index
    fn define_local(&mut self, name: String) -> usize {
        let idx = self.locals.len();
        self.locals.push((name, idx));
        idx
    }

    /// Look up a local variable by name, return its slot
    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.locals.iter().rev()
            .find(|(n, _)| n == name)
            .map(|(_, idx)| *idx)
    }

    fn emit(&mut self, op: Opcode, line: usize) -> usize {
        self.chunk.emit(op, line)
    }

    fn current_pos(&self) -> usize { self.chunk.current_pos() }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        self.chunk.patch_jump(idx, target);
    }
}

/// The bytecode compiler
pub struct Compiler {
    /// Stack of function scopes (innermost last)
    scopes: Vec<FnScope>,
    /// Compiled function chunks by name (for global fn lookup)
    functions: HashMap<String, Chunk>,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            scopes: vec![FnScope::new("<script>")],
            functions: HashMap::new(),
        }
    }

    /// Compile a full program into a main chunk
    pub fn compile(mut self, program: &Program) -> Result<Chunk, RuntimeError> {
        // First pass: compile all top-level functions
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                self.compile_fn(f)?;
            }
        }

        // Second pass: compile top-level statements
        for decl in &program.declarations {
            if !matches!(decl, Declaration::Function(_)) {
                self.compile_declaration(decl)?;
            }
        }

        self.emit(Opcode::Halt, 0);
        Ok(self.scopes.remove(0).chunk)
    }

    // ════════════════════════════════════
    //   SCOPE HELPERS
    // ════════════════════════════════════

    fn emit(&mut self, op: Opcode, line: usize) -> usize {
        self.scopes.last_mut().unwrap().emit(op, line)
    }

    fn current_pos(&self) -> usize {
        self.scopes.last().unwrap().current_pos()
    }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        self.scopes.last_mut().unwrap().patch_jump(idx, target);
    }

    fn emit_str(&mut self, s: String, line: usize) -> usize {
        let idx = self.scopes.last_mut().unwrap().chunk.add_constant(Value::Str(s));
        self.emit(Opcode::PushConst(idx), line)
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.scopes.last().unwrap().resolve_local(name)
    }

    fn define_local(&mut self, name: String) -> usize {
        self.scopes.last_mut().unwrap().define_local(name)
    }

    // ════════════════════════════════════
    //   COMPILE FUNCTIONS
    // ════════════════════════════════════

    fn compile_fn(&mut self, decl: &FnDecl) -> Result<(), RuntimeError> {
        // Push a new scope for this function
        self.scopes.push(FnScope::new(decl.name.clone()));

        // Register params as local slots 0, 1, 2...
        for param in &decl.params {
            if !param.is_self {
                self.define_local(param.name.clone());
            }
        }

        // Compile body
        match &decl.body {
            FnBody::Block(block) => self.compile_block(block)?,
            FnBody::Arrow(expr)  => {
                self.compile_expr(expr)?;
                self.emit(Opcode::Return, expr.span().line);
            }
        }

        // Always emit return at end (for void functions)
        self.emit(Opcode::PushVoid, 0);
        self.emit(Opcode::Return, 0);

        // Pop the scope and save the compiled chunk
        let fn_scope = self.scopes.pop().unwrap();
        let _ = self.scopes.last_mut().unwrap()
            .chunk.add_constant(Value::Str(format!("<fn:{}>", decl.name)));
        // Store the compiled chunk for later use by the VM
        self.functions.insert(decl.name.clone(), fn_scope.chunk);
        Ok(())
    }

    // ════════════════════════════════════
    //   COMPILE DECLARATIONS
    // ════════════════════════════════════

    fn compile_declaration(&mut self, decl: &Declaration) -> Result<(), RuntimeError> {
        match decl {
            Declaration::Let(l)       => self.compile_let(l),
            Declaration::Const(c)     => self.compile_const(c),
            Declaration::Statement(s) => self.compile_statement(s),
            Declaration::Function(_)  => Ok(()), // already compiled in first pass
            _                         => Ok(()),
        }
    }

    fn compile_let(&mut self, decl: &LetDecl) -> Result<(), RuntimeError> {
        let line = decl.span.line;
        self.compile_expr(&decl.value)?;
        if self.scopes.len() > 1 {
            // Inside a function — local variable
            let slot = self.define_local(decl.name.clone());
            self.emit(Opcode::StoreLocal(slot), line);
        } else {
            // Top-level — global variable
            self.emit(Opcode::DefineGlobal(decl.name.clone()), line);
        }
        Ok(())
    }

    fn compile_const(&mut self, decl: &ConstDecl) -> Result<(), RuntimeError> {
        let line = decl.span.line;
        self.compile_expr(&decl.value)?;
        self.emit(Opcode::DefineGlobal(decl.name.clone()), line);
        Ok(())
    }

    // ════════════════════════════════════
    //   COMPILE STATEMENTS
    // ════════════════════════════════════

    fn compile_statement(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::Let(l)        => self.compile_let(l),
            Statement::Const(c)      => self.compile_const(c),
            Statement::Return(r)     => self.compile_return(r),
            Statement::If(i)         => self.compile_if(i),
            Statement::While(w)      => self.compile_while(w),
            Statement::For(f)        => self.compile_for(f),
            Statement::Loop(l)       => self.compile_loop_stmt(l),
            Statement::Match(m)      => self.compile_match(m),
            Statement::Block(b)      => self.compile_block(b),
            Statement::Expression(e) => {
                self.compile_expr(&e.expr)?;
                self.emit(Opcode::Pop, e.span.line); // discard expression value
                Ok(())
            }
            Statement::Break(_)    => {
                // Emit placeholder — compiler needs to patch this later
                self.emit(Opcode::Jump(usize::MAX), 0);
                Ok(())
            }
            Statement::Continue(_) => {
                self.emit(Opcode::Jump(usize::MAX), 0);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn compile_block(&mut self, block: &Block) -> Result<(), RuntimeError> {
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
    }

    fn compile_return(&mut self, stmt: &ReturnStmt) -> Result<(), RuntimeError> {
        let line = stmt.span.line;
        match &stmt.value {
            Some(expr) => self.compile_expr(expr)?,
            None       => { self.emit(Opcode::PushVoid, line); }
        }
        self.emit(Opcode::Return, line);
        Ok(())
    }

    fn compile_if(&mut self, stmt: &IfStmt) -> Result<(), RuntimeError> {
        let line = stmt.span.line;

        // Compile condition
        self.compile_expr(&stmt.condition)?;

        // Emit jump-if-false (placeholder target)
        let jump_idx = self.emit(Opcode::JumpIfFalseAndPop(0), line);

        // Compile then branch
        self.compile_block(&stmt.then_branch)?;

        // Handle else if / else
        if stmt.else_if_branches.is_empty() && stmt.else_branch.is_none() {
            // No else — patch jump to after then
            let target = self.current_pos();
            self.patch_jump(jump_idx, target);
        } else {
            // Jump over else branch at end of then
            let skip_else_idx = self.emit(Opcode::Jump(0), line);
            let else_start = self.current_pos();
            self.patch_jump(jump_idx, else_start);

            // Compile else if branches
            for branch in &stmt.else_if_branches {
                self.compile_expr(&branch.condition)?;
                let branch_jump = self.emit(Opcode::JumpIfFalseAndPop(0), branch.span.line);
                self.compile_block(&branch.body)?;
                let branch_skip = self.emit(Opcode::Jump(0), branch.span.line);
                self.patch_jump(branch_jump, self.current_pos());
                // patch branch_skip after all branches compiled...
                // simplified: patch immediately to after block
                self.patch_jump(branch_skip, self.current_pos());
            }

            // Compile else
            if let Some(else_block) = &stmt.else_branch {
                self.compile_block(else_block)?;
            }

            let after_else = self.current_pos();
            self.patch_jump(skip_else_idx, after_else);
        }

        Ok(())
    }

    fn compile_while(&mut self, stmt: &WhileStmt) -> Result<(), RuntimeError> {
        let line = stmt.span.line;
        let loop_start = self.current_pos();

        // Compile condition
        self.compile_expr(&stmt.condition)?;
        let exit_jump = self.emit(Opcode::JumpIfFalseAndPop(0), line);

        // Compile body
        self.compile_block(&stmt.body)?;

        // Jump back to loop start
        self.emit(Opcode::Jump(loop_start), line);

        // Patch the exit jump
        self.patch_jump(exit_jump, self.current_pos());
        Ok(())
    }

    fn compile_for(&mut self, stmt: &ForStmt) -> Result<(), RuntimeError> {
        let line = stmt.span.line;

        // Compile iterable
        self.compile_expr(&stmt.iterable)?;

        // We implement for-loop using an internal index counter
        // Store the array in a hidden local
        let arr_slot = self.define_local(format!("__arr_{}", stmt.variable));
        self.emit(Opcode::StoreLocal(arr_slot), line);

        // Store index counter (starts at 0)
        self.emit(Opcode::PushInt(0), line);
        let idx_slot = self.define_local(format!("__idx_{}", stmt.variable));
        self.emit(Opcode::StoreLocal(idx_slot), line);

        // Loop header: check idx < arr.len()
        let loop_start = self.current_pos();
        self.emit(Opcode::LoadLocal(arr_slot), line);
        self.emit(Opcode::Len, line);
        self.emit(Opcode::LoadLocal(idx_slot), line);
        self.emit(Opcode::Greater, line);  // len > idx → continue
        let exit_jump = self.emit(Opcode::JumpIfFalseAndPop(0), line);

        // Load current element: arr[idx]
        self.emit(Opcode::LoadLocal(arr_slot), line);
        self.emit(Opcode::LoadLocal(idx_slot), line);
        self.emit(Opcode::IndexGet, line);

        // Store in loop variable
        let var_slot = self.define_local(stmt.variable.clone());
        self.emit(Opcode::StoreLocal(var_slot), line);

        // Compile body
        for s in &stmt.body.statements {
            self.compile_statement(s)?;
        }

        // Increment index
        self.emit(Opcode::LoadLocal(idx_slot), line);
        self.emit(Opcode::PushInt(1), line);
        self.emit(Opcode::Add, line);
        self.emit(Opcode::StoreLocal(idx_slot), line);

        // Jump back to header
        self.emit(Opcode::Jump(loop_start), line);

        // Patch exit
        self.patch_jump(exit_jump, self.current_pos());
        Ok(())
    }

    fn compile_loop_stmt(&mut self, stmt: &LoopStmt) -> Result<(), RuntimeError> {
        let line = stmt.span.line;
        let loop_start = self.current_pos();
        self.compile_block(&stmt.body)?;
        self.emit(Opcode::Jump(loop_start), line);
        Ok(())
    }

    fn compile_match(&mut self, stmt: &MatchStmt) -> Result<(), RuntimeError> {
        let line = stmt.span.line;
        self.compile_expr(&stmt.subject)?;

        let mut end_jumps = Vec::new();

        for arm in &stmt.arms {
            match &arm.pattern {
                Pattern::Wildcard(_) => {
                    // Always matches — just compile body
                    self.emit(Opcode::Pop, line);
                    match &arm.body {
                        MatchBody::Expr(e)  => { self.compile_expr(e)?; self.emit(Opcode::Pop, line); }
                        MatchBody::Block(b) => self.compile_block(b)?,
                    }
                    break; // wildcard must be last
                }
                Pattern::Literal(lit) => {
                    // Dup subject, push pattern value, compare
                    self.emit(Opcode::Dup, line);
                    self.compile_literal_pattern(&lit.value, line)?;
                    self.emit(Opcode::Equal, line);
                    let skip = self.emit(Opcode::JumpIfFalseAndPop(0), line);

                    // Match! pop the duped subject, compile body
                    self.emit(Opcode::Pop, line);
                    match &arm.body {
                        MatchBody::Expr(e)  => { self.compile_expr(e)?; self.emit(Opcode::Pop, line); }
                        MatchBody::Block(b) => self.compile_block(b)?,
                    }
                    let end_jump = self.emit(Opcode::Jump(0), line);
                    end_jumps.push(end_jump);
                    self.patch_jump(skip, self.current_pos());
                }
                Pattern::Binding(b) => {
                    // Bind the value to a local variable
                    let slot = self.define_local(b.name.clone());
                    self.emit(Opcode::StoreLocal(slot), line);
                    match &arm.body {
                        MatchBody::Expr(e)  => { self.compile_expr(e)?; self.emit(Opcode::Pop, line); }
                        MatchBody::Block(b) => self.compile_block(b)?,
                    }
                    break; // binding always matches
                }
                _ => {
                    self.emit(Opcode::Pop, line);
                }
            }
        }

        // Patch all end jumps to after the match
        let after = self.current_pos();
        for idx in end_jumps {
            self.patch_jump(idx, after);
        }

        Ok(())
    }

    fn compile_literal_pattern(&mut self, val: &LiteralValue, line: usize) -> Result<(), RuntimeError> {
        match val {
            LiteralValue::Int(n)    => { self.emit(Opcode::PushInt(*n), line); }
            LiteralValue::Float(f)  => { self.emit(Opcode::PushFloat(*f), line); }
            LiteralValue::Bool(b)   => { self.emit(Opcode::PushBool(*b), line); }
            LiteralValue::Str(s)    => { self.emit_str(s.clone(), line); }
            LiteralValue::Null      => { self.emit(Opcode::PushNull, line); }
            LiteralValue::Char(c)   => {
                let idx = self.scopes.last_mut().unwrap().chunk
                    .add_constant(Value::Char(*c));
                self.emit(Opcode::PushConst(idx), line);
            }
        }
        Ok(())
    }

    // ════════════════════════════════════
    //   COMPILE EXPRESSIONS
    // ════════════════════════════════════

    pub fn compile_expr(&mut self, expr: &Expr) -> Result<(), RuntimeError> {
        let line = expr.span().line;

        match expr {
            Expr::Int(lit)   => { self.emit(Opcode::PushInt(lit.value), line); }
            Expr::Float(lit) => { self.emit(Opcode::PushFloat(lit.value), line); }
            Expr::Str(lit)   => { self.emit_str(lit.value.clone(), line); }
            Expr::Bool(lit)  => { self.emit(Opcode::PushBool(lit.value), line); }
            Expr::Null(_)    => { self.emit(Opcode::PushNull, line); }

            Expr::Identifier(id) => {
                if let Some(slot) = self.resolve_local(&id.name) {
                    self.emit(Opcode::LoadLocal(slot), line);
                } else {
                    self.emit(Opcode::LoadGlobal(id.name.clone()), line);
                }
            }

            Expr::Binary(b) => {
                self.compile_expr(&b.left)?;
                self.compile_expr(&b.right)?;
                let op = match b.op {
                    BinaryOp::Add    => Opcode::Add,
                    BinaryOp::Sub    => Opcode::Sub,
                    BinaryOp::Mul    => Opcode::Mul,
                    BinaryOp::Div    => Opcode::Div,
                    BinaryOp::Mod    => Opcode::Mod,
                    BinaryOp::Eq     => Opcode::Equal,
                    BinaryOp::NotEq  => Opcode::NotEqual,
                    BinaryOp::Lt     => Opcode::Less,
                    BinaryOp::Lte    => Opcode::LessEqual,
                    BinaryOp::Gt     => Opcode::Greater,
                    BinaryOp::Gte    => Opcode::GreaterEqual,
                    BinaryOp::And    => Opcode::And,
                    BinaryOp::Or     => Opcode::Or,
                };
                self.emit(op, line);
            }

            Expr::Unary(u) => {
                self.compile_expr(&u.operand)?;
                let op = match u.op {
                    UnaryOp::Neg => Opcode::Negate,
                    UnaryOp::Not => Opcode::Not,
                };
                self.emit(op, line);
            }

            Expr::Call(c) => {
                // Push function
                self.compile_expr(&c.callee)?;
                // Push args left to right
                for arg in &c.args {
                    self.compile_expr(&arg.value)?;
                }
                self.emit(Opcode::Call(c.args.len()), line);
            }

            Expr::Array(arr) => {
                for elem in &arr.elements {
                    self.compile_expr(elem)?;
                }
                self.emit(Opcode::MakeArray(arr.elements.len()), line);
            }

            Expr::StructInit(s) => {
                for (fname, val) in &s.fields {
                    self.emit_str(fname.clone(), line);
                    self.compile_expr(val)?;
                }
                self.emit(Opcode::MakeStruct(s.name.clone(), s.fields.len()), line);
            }

            Expr::Field(f) => {
                self.compile_expr(&f.object)?;
                self.emit(Opcode::GetField(f.field.clone()), line);
            }

            Expr::Index(i) => {
                self.compile_expr(&i.object)?;
                self.compile_expr(&i.index)?;
                self.emit(Opcode::IndexGet, line);
            }

            Expr::Assign(a) => {
                self.compile_expr(&a.value)?;
                match a.target.as_ref() {
                    Expr::Identifier(id) => {
                        if let Some(slot) = self.resolve_local(&id.name) {
                            self.emit(Opcode::Dup, line); // keep value on stack
                            self.emit(Opcode::StoreLocal(slot), line);
                        } else {
                            self.emit(Opcode::Dup, line);
                            self.emit(Opcode::StoreGlobal(id.name.clone()), line);
                        }
                    }
                    Expr::Field(f) => {
                        self.compile_expr(&f.object)?;
                        self.emit(Opcode::Swap, line);
                        self.emit(Opcode::SetField(f.field.clone()), line);
                    }
                    Expr::Index(i) => {
                        self.compile_expr(&i.object)?;
                        self.compile_expr(&i.index)?;
                        self.emit(Opcode::IndexSet, line);
                    }
                    _ => {}
                }
            }

            Expr::Range(r) => {
                self.compile_expr(&r.start)?;
                self.compile_expr(&r.end)?;
                if r.inclusive {
                    // For inclusive: push end+1, then call Range
                    self.emit(Opcode::PushInt(1), line);
                    self.emit(Opcode::Add, line);
                }
                self.emit(Opcode::Range, line);
            }

            Expr::NullCoalesce(n) => {
                self.compile_expr(&n.left)?;
                self.compile_expr(&n.right)?;
                self.emit(Opcode::NullCoalesce, line);
            }

            Expr::Block(b) => self.compile_block(b)?,

            _ => {} // If/Match as expressions handled later
        }

        Ok(())
    }

    /// Return the compiled functions map (for the VM to load)
    pub fn take_functions(self) -> HashMap<String, Chunk> {
        self.functions
    }
}

impl Default for Compiler { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod compiler_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile_chunk(src: &str) -> Chunk {
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();
        Compiler::new().compile(&prog).unwrap()
    }

    #[test]
    fn test_compile_int_literal() {
        let chunk = compile_chunk("42");
        assert!(chunk.code.contains(&Opcode::PushInt(42)));
    }

    #[test]
    fn test_compile_add() {
        let chunk = compile_chunk("1 + 2");
        assert!(chunk.code.contains(&Opcode::PushInt(1)));
        assert!(chunk.code.contains(&Opcode::PushInt(2)));
        assert!(chunk.code.contains(&Opcode::Add));
    }

    #[test]
    fn test_compile_string() {
        let chunk = compile_chunk(r#""hello""#);
        assert!(chunk.code.iter().any(|op| matches!(op, Opcode::PushConst(_))));
        assert!(chunk.constants.iter().any(|c| matches!(c, Value::Str(s) if s == "hello")));
    }

    #[test]
    fn test_compile_let_global() {
        let chunk = compile_chunk("let x = 42");
        assert!(chunk.code.contains(&Opcode::PushInt(42)));
        assert!(chunk.code.contains(&Opcode::DefineGlobal("x".to_string())));
    }

    #[test]
    fn test_compile_if_has_jump() {
        let chunk = compile_chunk("if true { }");
        assert!(chunk.code.iter().any(|op| matches!(op, Opcode::JumpIfFalseAndPop(_))));
    }

    #[test]
    fn test_compile_while_has_backward_jump() {
        let chunk = compile_chunk("while false { }");
        // Should have a Jump that goes backward (to loop start)
        let jumps: Vec<usize> = chunk.code.iter().filter_map(|op| {
            if let Opcode::Jump(t) = op { Some(*t) } else { None }
        }).collect();
        // The backward jump should exist (target < some instruction index)
        assert!(!jumps.is_empty());
    }

    #[test]
    fn test_compile_array_literal() {
        let chunk = compile_chunk("[1, 2, 3]");
        assert!(chunk.code.contains(&Opcode::MakeArray(3)));
    }

    #[test]
    fn test_compile_ends_with_halt() {
        let chunk = compile_chunk("let x = 1");
        assert_eq!(chunk.code.last(), Some(&Opcode::Halt));
    }

    #[test]
    fn test_compile_negation() {
        let chunk = compile_chunk("-5");
        assert!(chunk.code.contains(&Opcode::PushInt(5)));
        assert!(chunk.code.contains(&Opcode::Negate));
    }

    #[test]
    fn test_compile_range() {
        let chunk = compile_chunk("0..10");
        assert!(chunk.code.contains(&Opcode::Range));
    }
}
