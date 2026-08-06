/// Builds LLVM IR as text, managing SSA register and label naming
pub struct IrBuilder {
    /// The module-level output (type defs, global strings, function decls)
    module_lines: Vec<String>,
    /// The current function body being built
    pub current_fn_lines: Vec<String>,
    /// Counter for fresh SSA register names (%t0, %t1, ...)
    reg_counter: usize,
    /// Counter for fresh basic block labels (bb0, bb1, ...)
    label_counter: usize,
    /// Counter for global string constants (@.str.0, @.str.1, ...)
    string_counter: usize,
    /// Has the current basic block been terminated (br/ret)?
    /// LLVM requires every block end in exactly one terminator
    block_terminated: bool,
}

impl IrBuilder {
    pub fn new() -> Self {
        IrBuilder {
            module_lines: Vec::new(),
            current_fn_lines: Vec::new(),
            reg_counter: 0,
            label_counter: 0,
            string_counter: 0,
            block_terminated: false,
        }
    }

    // ── FRESH NAME GENERATION ────────────────────────────────

    /// Generate a fresh SSA register name: %t0, %t1, %t2, ...
    pub fn fresh_reg(&mut self) -> String {
        let name = format!("%t{}", self.reg_counter);
        self.reg_counter += 1;
        name
    }

    /// Generate a fresh basic block label: bb0, bb1, bb2, ...
    pub fn fresh_label(&mut self) -> String {
        let name = format!("bb{}", self.label_counter);
        self.label_counter += 1;
        name
    }

    /// Generate a fresh global string name: @.str.0, @.str.1, ...
    pub fn fresh_string_name(&mut self) -> String {
        let name = format!("@.str.{}", self.string_counter);
        self.string_counter += 1;
        name
    }

    // ── MODULE-LEVEL EMISSION ────────────────────────────────

    /// Emit a line at module scope (struct defs, globals, fn declarations)
    pub fn emit_module(&mut self, line: impl Into<String>) {
        self.module_lines.push(line.into());
    }

    /// Declare a global string constant, return a pointer to it
    /// e.g. emit_global_string("hello") -> "@.str.0"
    pub fn emit_global_string(&mut self, value: &str) -> String {
        let name = self.fresh_string_name();
        let escaped = escape_llvm_string(value);
        let len = value.len() + 1; // +1 for null terminator
        self.emit_module(format!(
            "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"",
            name, len, escaped
        ));
        name
    }

    // ── FUNCTION-LEVEL EMISSION ───────────────────────────────

    /// Start a new function body (resets counters, clears buffer)
    pub fn start_function(&mut self) {
        self.current_fn_lines.clear();
        self.reg_counter = 0;
        self.label_counter = 0;
        self.block_terminated = false;
    }

    /// Emit an instruction inside the current function
    /// Refuses to emit if the current block is already terminated
    /// (prevents invalid IR with unreachable code after ret/br)
    pub fn emit(&mut self, line: impl Into<String>) {
        if self.block_terminated {
            return;
        } // silently drop dead code
        self.current_fn_lines.push(format!("    {}", line.into()));
    }

    /// Emit a basic block label (starts a new block)
    pub fn emit_label(&mut self, label: &str) {
        self.current_fn_lines.push(format!("{}:", label));
        self.block_terminated = false;
    }

    /// Emit a terminator instruction (ret, br) — marks block as closed
    pub fn emit_terminator(&mut self, line: impl Into<String>) {
        if self.block_terminated {
            return;
        }
        self.current_fn_lines.push(format!("    {}", line.into()));
        self.block_terminated = true;
    }

    /// Is the current block missing a terminator? (needs implicit ret)
    pub fn needs_terminator(&self) -> bool {
        !self.block_terminated
    }

    // ── HIGH-LEVEL INSTRUCTION HELPERS ────────────────────────

    /// Emit: %dest = add i64 %a, %b  — returns dest register
    pub fn emit_binop(&mut self, op: &str, ty: &str, left: &str, right: &str) -> String {
        let dest = self.fresh_reg();
        self.emit(format!("{} = {} {} {}, {}", dest, op, ty, left, right));
        dest
    }

    /// Emit: %dest = icmp sgt i64 %a, %b
    pub fn emit_icmp(&mut self, cond: &str, ty: &str, left: &str, right: &str) -> String {
        let dest = self.fresh_reg();
        self.emit(format!(
            "{} = icmp {} {} {}, {}",
            dest, cond, ty, left, right
        ));
        dest
    }

    /// Emit: %dest = fcmp ogt double %a, %b (for floats)
    pub fn emit_fcmp(&mut self, cond: &str, ty: &str, left: &str, right: &str) -> String {
        let dest = self.fresh_reg();
        self.emit(format!(
            "{} = fcmp {} {} {}, {}",
            dest, cond, ty, left, right
        ));
        dest
    }

    /// Emit: br i1 %cond, label %then, label %else
    pub fn emit_cond_branch(&mut self, cond: &str, then_label: &str, else_label: &str) {
        self.emit_terminator(format!(
            "br i1 {}, label %{}, label %{}",
            cond, then_label, else_label
        ));
    }

    /// Emit: br label %target
    pub fn emit_branch(&mut self, target: &str) {
        self.emit_terminator(format!("br label %{}", target));
    }

    /// Emit: ret TYPE VALUE
    pub fn emit_return(&mut self, ty: &str, value: &str) {
        self.emit_terminator(format!("ret {} {}", ty, value));
    }

    /// Emit: ret void
    pub fn emit_return_void(&mut self) {
        self.emit_terminator("ret void".to_string());
    }

    /// Emit: %dest = call TYPE @fnname(ARGS)
    pub fn emit_call(
        &mut self,
        ret_ty: &str,
        fn_name: &str,
        args: &[(String, String)],
    ) -> Option<String> {
        let args_str = args
            .iter()
            .map(|(ty, val)| format!("{} {}", ty, val))
            .collect::<Vec<_>>()
            .join(", ");

        if ret_ty == "void" {
            self.emit(format!("call void @{}({})", fn_name, args_str));
            None
        } else {
            let dest = self.fresh_reg();
            self.emit(format!(
                "{} = call {} @{}({})",
                dest, ret_ty, fn_name, args_str
            ));
            Some(dest)
        }
    }

    /// Emit: %dest = alloca TYPE  (stack allocation)
    pub fn emit_alloca(&mut self, ty: &str) -> String {
        let dest = self.fresh_reg();
        self.emit(format!("{} = alloca {}", dest, ty));
        dest
    }

    /// Emit: store TYPE VALUE, ptr DEST
    pub fn emit_store(&mut self, ty: &str, value: &str, dest: &str) {
        self.emit(format!("store {} {}, ptr {}", ty, value, dest));
    }

    /// Emit: %dest = load TYPE, ptr SRC
    pub fn emit_load(&mut self, ty: &str, src: &str) -> String {
        let dest = self.fresh_reg();
        self.emit(format!("{} = load {}, ptr {}", dest, ty, src));
        dest
    }

    /// Finish the current function, wrap it in a `define` block, add to module
    pub fn finish_function(&mut self, signature: &str) {
        self.module_lines.push(format!("define {} {{", signature));
        for line in &self.current_fn_lines {
            self.module_lines.push(line.clone());
        }
        self.module_lines.push("}".to_string());
        self.module_lines.push(String::new()); // blank line separator
    }

    /// Return the complete assembled module as a string
    pub fn build(self) -> String {
        self.module_lines.join("\n")
    }
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape a string for LLVM's c"..." string literal syntax
fn escape_llvm_string(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        match byte {
            b'\\' => out.push_str("\\5C"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7E => out.push(byte as char), // printable ASCII
            _ => out.push_str(&format!("\\{:02X}", byte)),
        }
    }
    out
}

#[cfg(test)]
mod ir_builder_tests {
    use super::*;

    #[test]
    fn test_fresh_reg_increments() {
        let mut b = IrBuilder::new();
        assert_eq!(b.fresh_reg(), "%t0");
        assert_eq!(b.fresh_reg(), "%t1");
        assert_eq!(b.fresh_reg(), "%t2");
    }

    #[test]
    fn test_fresh_label_increments() {
        let mut b = IrBuilder::new();
        assert_eq!(b.fresh_label(), "bb0");
        assert_eq!(b.fresh_label(), "bb1");
    }

    #[test]
    fn test_start_function_resets_counters() {
        let mut b = IrBuilder::new();
        b.fresh_reg();
        b.fresh_reg();
        b.start_function();
        assert_eq!(b.fresh_reg(), "%t0"); // reset!
    }

    #[test]
    fn test_emit_binop() {
        let mut b = IrBuilder::new();
        b.start_function();
        let dest = b.emit_binop("add", "i64", "1", "2");
        assert_eq!(dest, "%t0");
        assert!(b.current_fn_lines[0].contains("add i64 1, 2"));
    }

    #[test]
    fn test_emit_return_terminates_block() {
        let mut b = IrBuilder::new();
        b.start_function();
        assert!(b.needs_terminator());
        b.emit_return("i64", "42");
        assert!(!b.needs_terminator());
    }

    #[test]
    fn test_dead_code_after_terminator_dropped() {
        let mut b = IrBuilder::new();
        b.start_function();
        b.emit_return("i64", "42");
        let lines_before = b.current_fn_lines.len();
        b.emit("this should be dropped".to_string());
        assert_eq!(
            b.current_fn_lines.len(),
            lines_before,
            "Dead code should not be emitted"
        );
    }

    #[test]
    fn test_emit_label_resets_terminated_flag() {
        let mut b = IrBuilder::new();
        b.start_function();
        b.emit_return("i64", "1");
        assert!(!b.needs_terminator());
        b.emit_label("bb1");
        assert!(b.needs_terminator(), "New block should need a terminator");
    }

    #[test]
    fn test_emit_call_with_return() {
        let mut b = IrBuilder::new();
        b.start_function();
        let args = vec![
            ("i64".to_string(), "3".to_string()),
            ("i64".to_string(), "4".to_string()),
        ];
        let dest = b.emit_call("i64", "lyz_add", &args);
        assert_eq!(dest, Some("%t0".to_string()));
        assert!(b.current_fn_lines[0].contains("call i64 @lyz_add"));
    }

    #[test]
    fn test_emit_call_void_no_return() {
        let mut b = IrBuilder::new();
        b.start_function();
        let dest = b.emit_call("void", "lyz_print", &[("i64".to_string(), "5".to_string())]);
        assert_eq!(dest, None);
    }

    #[test]
    fn test_global_string_escaping() {
        let mut b = IrBuilder::new();
        let name = b.emit_global_string("hello");
        assert_eq!(name, "@.str.0");
        assert!(b.module_lines[0].contains("hello"));
    }

    #[test]
    fn test_alloca_store_load_roundtrip() {
        let mut b = IrBuilder::new();
        b.start_function();
        let ptr = b.emit_alloca("i64"); // %t0
        b.emit_store("i64", "42", &ptr);
        let loaded = b.emit_load("i64", &ptr); // %t1
        assert_eq!(loaded, "%t1");
    }

    #[test]
    fn test_finish_function_wraps_correctly() {
        let mut b = IrBuilder::new();
        b.start_function();
        b.emit_return("i64", "42");
        b.finish_function("i64 @lyz_main()");
        let output = b.build();
        assert!(output.contains("define i64 @lyz_main()"));
        assert!(output.contains("ret i64 42"));
        assert!(output.contains("}"));
    }
}
