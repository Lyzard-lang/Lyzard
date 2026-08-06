use crate::interpreter::value::Value;

/// Every instruction the LYZARD VM can execute
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    // ── PUSH CONSTANTS ───────────────────────────────────────
    PushInt(i64),
    PushFloat(f64),
    PushBool(bool),
    PushNull,
    PushVoid,
    /// Push a constant from the constant pool by index
    PushConst(usize),

    // ── ARITHMETIC ───────────────────────────────────────────
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Negate, // unary -

    // ── COMPARISON ───────────────────────────────────────────
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,

    // ── LOGICAL ──────────────────────────────────────────────
    And,
    Or,
    Not, // unary !

    // ── LOCAL VARIABLES ──────────────────────────────────────
    LoadLocal(usize),  // push locals[idx] onto stack
    StoreLocal(usize), // pop stack → locals[idx]
    DefineLocal,       // define a new local (pushes the current top into slot)

    // ── GLOBAL VARIABLES ─────────────────────────────────────
    LoadGlobal(String),   // push globals[name] onto stack
    StoreGlobal(String),  // pop stack → globals[name]
    DefineGlobal(String), // define globals[name] = pop()

    // ── CONTROL FLOW ─────────────────────────────────────────
    Jump(usize),              // unconditional jump to instruction index
    JumpIfFalse(usize),       // jump if top of stack is falsy (does NOT pop)
    JumpIfFalseAndPop(usize), // jump if falsy AND pop

    // ── FUNCTION CALLS ───────────────────────────────────────
    Call(usize),        // call function with N args (fn is below args on stack)
    Return,             // return top of stack (or Void if empty)
    MakeClosure(usize), // wrap constant function with current env

    // ── BUILT-IN OPERATIONS ──────────────────────────────────
    Print,
    Len,
    Range, // pops end, pops start, pushes [int] array

    // ── ARRAYS ───────────────────────────────────────────────
    MakeArray(usize), // pops N values, builds array, pushes it
    IndexGet,         // pops index, pops array, pushes array[index]
    IndexSet,         // pops value, pops index, pops array, sets array[index]

    // ── STRUCTS ──────────────────────────────────────────────
    MakeStruct(String, usize), // struct name, field count — pops N (name,value) pairs
    GetField(String),          // pops struct, pushes struct.field
    SetField(String),          // pops value, pops struct, sets struct.field

    // ── STACK MANIPULATION ───────────────────────────────────
    Pop,  // discard top of stack
    Dup,  // duplicate top of stack
    Swap, // swap top two stack items

    // ── NULL COALESCING ──────────────────────────────────────
    NullCoalesce, // pops right, pops left: if left is null → right, else left

    // ── HALT ─────────────────────────────────────────────────
    Halt, // stop the VM
}

impl std::fmt::Display for Opcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PushInt(n) => write!(f, "PUSH_INT     {}", n),
            Self::PushFloat(v) => write!(f, "PUSH_FLOAT   {}", v),
            Self::PushBool(b) => write!(f, "PUSH_BOOL    {}", b),
            Self::PushNull => write!(f, "PUSH_NULL"),
            Self::PushVoid => write!(f, "PUSH_VOID"),
            Self::PushConst(i) => write!(f, "PUSH_CONST   {}", i),
            Self::Add => write!(f, "ADD"),
            Self::Sub => write!(f, "SUB"),
            Self::Mul => write!(f, "MUL"),
            Self::Div => write!(f, "DIV"),
            Self::Mod => write!(f, "MOD"),
            Self::Negate => write!(f, "NEGATE"),
            Self::Equal => write!(f, "EQUAL"),
            Self::NotEqual => write!(f, "NOT_EQUAL"),
            Self::Less => write!(f, "LESS"),
            Self::LessEqual => write!(f, "LESS_EQUAL"),
            Self::Greater => write!(f, "GREATER"),
            Self::GreaterEqual => write!(f, "GREATER_EQ"),
            Self::And => write!(f, "AND"),
            Self::Or => write!(f, "OR"),
            Self::Not => write!(f, "NOT"),
            Self::LoadLocal(i) => write!(f, "LOAD_LOCAL   {}", i),
            Self::StoreLocal(i) => write!(f, "STORE_LOCAL  {}", i),
            Self::DefineLocal => write!(f, "DEF_LOCAL"),
            Self::LoadGlobal(n) => write!(f, "LOAD_GLOBAL  {}", n),
            Self::StoreGlobal(n) => write!(f, "STORE_GLOBAL {}", n),
            Self::DefineGlobal(n) => write!(f, "DEF_GLOBAL   {}", n),
            Self::Jump(t) => write!(f, "JUMP         {}", t),
            Self::JumpIfFalse(t) => write!(f, "JUMP_FALSE   {}", t),
            Self::JumpIfFalseAndPop(t) => write!(f, "JUMP_FPOP    {}", t),
            Self::Call(n) => write!(f, "CALL         {}", n),
            Self::Return => write!(f, "RETURN"),
            Self::MakeClosure(i) => write!(f, "MAKE_CLOSURE {}", i),
            Self::Print => write!(f, "PRINT"),
            Self::Len => write!(f, "LEN"),
            Self::Range => write!(f, "RANGE"),
            Self::MakeArray(n) => write!(f, "MAKE_ARRAY   {}", n),
            Self::IndexGet => write!(f, "INDEX_GET"),
            Self::IndexSet => write!(f, "INDEX_SET"),
            Self::MakeStruct(n, c) => write!(f, "MAKE_STRUCT  {} {}", n, c),
            Self::GetField(n) => write!(f, "GET_FIELD    {}", n),
            Self::SetField(n) => write!(f, "SET_FIELD    {}", n),
            Self::Pop => write!(f, "POP"),
            Self::Dup => write!(f, "DUP"),
            Self::Swap => write!(f, "SWAP"),
            Self::NullCoalesce => write!(f, "NULL_COALESCE"),
            Self::Halt => write!(f, "HALT"),
        }
    }
}

/// A chunk of bytecode — one compiled function or script
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The bytecode instructions
    pub code: Vec<Opcode>,
    /// String/large constants referenced by PushConst
    pub constants: Vec<Value>,
    /// Parallel array: code[i] was generated from source line lines[i]
    pub lines: Vec<usize>,
    /// Human-readable name for this chunk (fn name or "<script>")
    pub name: String,
}

impl Chunk {
    pub fn new(name: impl Into<String>) -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
            name: name.into(),
        }
    }

    /// Emit one opcode with its source line
    pub fn emit(&mut self, op: Opcode, line: usize) -> usize {
        self.code.push(op);
        self.lines.push(line);
        self.code.len() - 1 // return instruction index
    }

    /// Add a constant to the pool, return its index
    pub fn add_constant(&mut self, value: Value) -> usize {
        // Dedup strings for efficiency
        if let Value::Str(ref s) = value {
            for (i, c) in self.constants.iter().enumerate() {
                if let Value::Str(ref cs) = c {
                    if cs == s {
                        return i;
                    }
                }
            }
        }
        self.constants.push(value);
        self.constants.len() - 1
    }

    /// Emit a string push using the constant pool
    pub fn emit_str(&mut self, s: String, line: usize) -> usize {
        let idx = self.add_constant(Value::Str(s));
        self.emit(Opcode::PushConst(idx), line)
    }

    /// Patch a previously emitted jump instruction with its real target
    pub fn patch_jump(&mut self, jump_idx: usize, target: usize) {
        match &mut self.code[jump_idx] {
            Opcode::Jump(t) => *t = target,
            Opcode::JumpIfFalse(t) => *t = target,
            Opcode::JumpIfFalseAndPop(t) => *t = target,
            _ => panic!("patch_jump called on non-jump opcode at {}", jump_idx),
        }
    }

    /// Current instruction count (= next instruction's index)
    pub fn current_pos(&self) -> usize {
        self.code.len()
    }

    /// Get source line for instruction at index
    pub fn line_for(&self, idx: usize) -> usize {
        self.lines.get(idx).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod opcode_tests {
    use super::*;
    use crate::interpreter::value::Value;

    #[test]
    fn test_opcode_display_push_int() {
        assert_eq!(format!("{}", Opcode::PushInt(42)), "PUSH_INT     42");
    }
    #[test]
    fn test_opcode_display_add() {
        assert_eq!(format!("{}", Opcode::Add), "ADD");
    }
    #[test]
    fn test_opcode_display_load_local() {
        assert_eq!(format!("{}", Opcode::LoadLocal(0)), "LOAD_LOCAL   0");
    }
    #[test]
    fn test_opcode_display_jump() {
        assert_eq!(format!("{}", Opcode::Jump(99)), "JUMP         99");
    }

    #[test]
    fn test_chunk_emit() {
        let mut chunk = Chunk::new("test");
        let idx = chunk.emit(Opcode::PushInt(5), 1);
        assert_eq!(idx, 0);
        chunk.emit(Opcode::PushInt(3), 1);
        chunk.emit(Opcode::Add, 1);
        assert_eq!(chunk.code.len(), 3);
        assert_eq!(chunk.current_pos(), 3);
    }

    #[test]
    fn test_chunk_add_constant() {
        let mut chunk = Chunk::new("test");
        let i1 = chunk.add_constant(Value::Str("hello".to_string()));
        let i2 = chunk.add_constant(Value::Str("hello".to_string())); // dedup
        assert_eq!(i1, 0);
        assert_eq!(i1, i2, "Same string should deduplicate");
        let i3 = chunk.add_constant(Value::Str("world".to_string()));
        assert_eq!(i3, 1);
    }

    #[test]
    fn test_chunk_patch_jump() {
        let mut chunk = Chunk::new("test");
        let jump_idx = chunk.emit(Opcode::JumpIfFalse(0), 1); // placeholder target 0
        chunk.emit(Opcode::Print, 1);
        let target = chunk.current_pos();
        chunk.patch_jump(jump_idx, target);
        assert_eq!(chunk.code[jump_idx], Opcode::JumpIfFalse(2));
    }

    #[test]
    fn test_chunk_line_tracking() {
        let mut chunk = Chunk::new("test");
        chunk.emit(Opcode::PushInt(1), 5);
        chunk.emit(Opcode::PushInt(2), 7);
        chunk.emit(Opcode::Add, 7);
        assert_eq!(chunk.line_for(0), 5);
        assert_eq!(chunk.line_for(1), 7);
        assert_eq!(chunk.line_for(2), 7);
    }

    #[test]
    fn test_emit_str_dedup() {
        let mut chunk = Chunk::new("test");
        chunk.emit_str("hello".to_string(), 1);
        chunk.emit_str("hello".to_string(), 2);
        // Both instructions push from constant pool, but pool should have only 1 entry
        assert_eq!(chunk.constants.len(), 1);
    }
}
