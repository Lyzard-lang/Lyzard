pub mod compiler;
pub mod disasm;
pub mod opcode;

use std::collections::HashMap;
use crate::interpreter::value::Value;
use crate::interpreter::error::RuntimeError;
use opcode::{Chunk, Opcode};

/// One active function call frame
struct CallFrame {
    /// The bytecode being executed
    chunk: Chunk,
    /// Instruction pointer — index into chunk.code
    ip: usize,
    /// Local variable slots
    locals: Vec<Value>,
}

impl CallFrame {
    fn new(chunk: Chunk, _arg_count: usize, args: Vec<Value>) -> Self {
        let mut locals = args;
        // Pad locals to ensure slots exist
        while locals.len() < 64 { locals.push(Value::Void); }
        CallFrame { chunk, ip: 0, locals }
    }

    fn read_op(&mut self) -> &Opcode {
        let op = &self.chunk.code[self.ip];
        self.ip += 1;
        op
    }
}

pub struct VM {
    /// Operand stack
    stack: Vec<Value>,
    /// Call stack (frames)
    frames: Vec<CallFrame>,
    /// Global variables
    globals: HashMap<String, Value>,
    /// Compiled function chunks
    functions: HashMap<String, Chunk>,
    /// Captured output (for testing)
    pub output: Vec<String>,
    pub capture_output: bool,
}

impl VM {
    pub fn new() -> Self {
        VM {
            stack: Vec::with_capacity(256),
            frames: Vec::with_capacity(64),
            globals: HashMap::new(),
            functions: HashMap::new(),
            output: Vec::new(),
            capture_output: false,
        }
    }

    /// Load compiled functions from the compiler
    pub fn load_functions(&mut self, functions: HashMap<String, Chunk>) {
        self.functions = functions;
    }

    /// Run a compiled chunk
    pub fn run(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        self.frames.push(CallFrame::new(chunk, 0, vec![]));
        self.execute()
    }

    fn execute(&mut self) -> Result<Value, RuntimeError> {
        loop {
            // Borrow the current instruction without holding the frame ref
            let op = {
                let frame = self.frames.last_mut().unwrap();
                frame.read_op().clone()
            };

            match op {
                // ── PUSH CONSTANTS ──────────────────────────────
                Opcode::PushInt(n)   => self.stack.push(Value::Int(n)),
                Opcode::PushFloat(f) => self.stack.push(Value::Float(f)),
                Opcode::PushBool(b)  => self.stack.push(Value::Bool(b)),
                Opcode::PushNull     => self.stack.push(Value::Null),
                Opcode::PushVoid     => self.stack.push(Value::Void),
                Opcode::PushConst(i) => {
                    let val = self.frames.last().unwrap().chunk.constants[i].clone();
                    self.stack.push(val);
                }

                // ── ARITHMETIC ──────────────────────────────────
                Opcode::Add => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    self.stack.push(match (l, r) {
                        (Value::Int(a),   Value::Int(b))   => Value::Int(a + b),
                        (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                        (Value::Int(a),   Value::Float(b)) => Value::Float(a as f64 + b),
                        (Value::Float(a), Value::Int(b))   => Value::Float(a + b as f64),
                        (Value::Str(a),   Value::Str(b))   => Value::Str(a + &b),
                        (Value::Str(a),   other)           => Value::Str(a + &other.to_display_string()),
                        (l, r) => return Err(RuntimeError::TypeError {
                            expected: "compatible types for +".to_string(),
                            got: format!("{} and {}", l.type_name(), r.type_name()),
                        }),
                    });
                }

                Opcode::Sub => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    self.stack.push(match (l, r) {
                        (Value::Int(a),   Value::Int(b))   => Value::Int(a - b),
                        (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                        (Value::Int(a),   Value::Float(b)) => Value::Float(a as f64 - b),
                        (Value::Float(a), Value::Int(b))   => Value::Float(a - b as f64),
                        (l, r) => return Err(RuntimeError::TypeError {
                            expected: "numeric types for -".to_string(),
                            got: format!("{} and {}", l.type_name(), r.type_name()),
                        }),
                    });
                }

                Opcode::Mul => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    self.stack.push(match (l, r) {
                        (Value::Int(a),   Value::Int(b))   => Value::Int(a * b),
                        (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
                        (Value::Int(a),   Value::Float(b)) => Value::Float(a as f64 * b),
                        (Value::Float(a), Value::Int(b))   => Value::Float(a * b as f64),
                        (l, r) => return Err(RuntimeError::TypeError {
                            expected: "numeric types for *".to_string(),
                            got: format!("{} and {}", l.type_name(), r.type_name()),
                        }),
                    });
                }

                Opcode::Div => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    self.stack.push(match (l, r) {
                        (Value::Int(a),   Value::Int(b))   => {
                            if b == 0 { return Err(RuntimeError::DivisionByZero { span: None }); }
                            Value::Int(a / b)
                        }
                        (Value::Float(a), Value::Float(b)) => Value::Float(a / b),
                        (Value::Int(a),   Value::Float(b)) => Value::Float(a as f64 / b),
                        (Value::Float(a), Value::Int(b))   => {
                            if b == 0 { return Err(RuntimeError::DivisionByZero { span: None }); }
                            Value::Float(a / b as f64)
                        }
                        (l, r) => return Err(RuntimeError::TypeError {
                            expected: "numeric types for /".to_string(),
                            got: format!("{} and {}", l.type_name(), r.type_name()),
                        }),
                    });
                }

                Opcode::Mod => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    if let (Value::Int(a), Value::Int(b)) = (l, r) {
                        if b == 0 { return Err(RuntimeError::DivisionByZero { span: None }); }
                        self.stack.push(Value::Int(a % b));
                    }
                }

                Opcode::Negate => {
                    let val = self.pop()?;
                    self.stack.push(match val {
                        Value::Int(n)   => Value::Int(-n),
                        Value::Float(f) => Value::Float(-f),
                        other => return Err(RuntimeError::TypeError {
                            expected: "numeric type for negation".to_string(),
                            got: other.type_name().to_string(),
                        }),
                    });
                }

                // ── COMPARISON ──────────────────────────────────
                Opcode::Equal        => { let (r, l) = (self.pop()?, self.pop()?); self.stack.push(Value::Bool(l == r)); }
                Opcode::NotEqual     => { let (r, l) = (self.pop()?, self.pop()?); self.stack.push(Value::Bool(l != r)); }
                Opcode::Less         => { let (r, l) = (self.pop()?, self.pop()?); self.stack.push(Value::Bool(self.cmp_values(&l, &r) == std::cmp::Ordering::Less)); }
                Opcode::LessEqual    => { let (r, l) = (self.pop()?, self.pop()?); self.stack.push(Value::Bool(self.cmp_values(&l, &r) != std::cmp::Ordering::Greater)); }
                Opcode::Greater      => { let (r, l) = (self.pop()?, self.pop()?); self.stack.push(Value::Bool(self.cmp_values(&l, &r) == std::cmp::Ordering::Greater)); }
                Opcode::GreaterEqual => { let (r, l) = (self.pop()?, self.pop()?); self.stack.push(Value::Bool(self.cmp_values(&l, &r) != std::cmp::Ordering::Less)); }

                // ── LOGICAL ─────────────────────────────────────
                Opcode::And => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    self.stack.push(Value::Bool(l.is_truthy() && r.is_truthy()));
                }
                Opcode::Or => {
                    let r = self.pop()?;
                    let l = self.pop()?;
                    self.stack.push(Value::Bool(l.is_truthy() || r.is_truthy()));
                }
                Opcode::Not => {
                    let val = self.pop()?;
                    self.stack.push(Value::Bool(!val.is_truthy()));
                }

                // ── VARIABLES ───────────────────────────────────
                Opcode::LoadLocal(idx) => {
                    let val = self.frames.last().unwrap().locals[idx].clone();
                    self.stack.push(val);
                }
                Opcode::StoreLocal(idx) => {
                    let val = self.peek().clone();
                    let frame = self.frames.last_mut().unwrap();
                    while frame.locals.len() <= idx { frame.locals.push(Value::Void); }
                    frame.locals[idx] = val;
                    // Note: StoreLocal does NOT pop — value stays on stack for let declarations
                    // but in some contexts we pop separately
                    self.pop()?;
                }
                Opcode::LoadGlobal(name) => {
                    let val = self.globals.get(&name).cloned().unwrap_or(Value::Null);
                    // Check functions too
                    if matches!(val, Value::Null) {
                        if self.functions.contains_key(&name) {
                            self.stack.push(Value::Str(format!("<fn:{}>", name)));
                        } else {
                            self.stack.push(val);
                        }
                    } else {
                        self.stack.push(val);
                    }
                }
                Opcode::StoreGlobal(name) => {
                    let val = self.pop()?;
                    self.globals.insert(name, val);
                }
                Opcode::DefineGlobal(name) => {
                    let val = self.pop()?;
                    self.globals.insert(name, val);
                }
                Opcode::DefineLocal => {
                    // Value stays on stack, just marks it as a defined local
                }

                // ── CONTROL FLOW ────────────────────────────────
                Opcode::Jump(target) => {
                    self.frames.last_mut().unwrap().ip = target;
                }
                Opcode::JumpIfFalse(target) => {
                    if !self.peek().is_truthy() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }
                Opcode::JumpIfFalseAndPop(target) => {
                    let val = self.pop()?;
                    if !val.is_truthy() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }

                // ── FUNCTION CALLS ──────────────────────────────
                Opcode::Call(arg_count) => {
                    // Stack: [fn_ref, arg0, arg1, ..., argN]
                    // fn_ref is below args
                    let fn_name_val = self.stack[self.stack.len() - arg_count - 1].clone();

                    // Pop all args
                    let mut args: Vec<Value> = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let _fn_ref = self.pop()?; // pop the function ref

                    // Look up the function by name
                    let fn_name = match &fn_name_val {
                        Value::Str(s) if s.starts_with("<fn:") => {
                            s[4..s.len()-1].to_string()
                        }
                        _ => {
                            // Might be a builtin
                            self.call_builtin_by_name(&fn_name_val, args)?;
                            continue;
                        }
                    };

                    if let Some(fn_chunk) = self.functions.get(&fn_name).cloned() {
                        if self.frames.len() >= 1000 {
                            return Err(RuntimeError::StackOverflow { fn_name });
                        }
                        self.frames.push(CallFrame::new(fn_chunk, arg_count, args));
                    } else {
                        // Try builtin
                        self.call_builtin(&fn_name, args)?;
                    }
                }

                Opcode::Return => {
                    let return_val = if self.stack.is_empty() {
                        Value::Void
                    } else {
                        self.pop()?
                    };
                    self.frames.pop();
                    if self.frames.is_empty() {
                        return Ok(return_val);
                    }
                    self.stack.push(return_val);
                }

                // ── BUILT-INS ───────────────────────────────────
                Opcode::Print => {
                    let val = self.pop()?;
                    let msg = val.to_display_string();
                    if self.capture_output {
                        self.output.push(msg);
                    } else {
                        print!("{}", val);
                    }
                }

                Opcode::Len => {
                    let val = self.pop()?;
                    let len = match &val {
                        Value::Str(s)   => s.chars().count() as i64,
                        Value::Array(a) => a.len() as i64,
                        other => return Err(RuntimeError::TypeError {
                            expected: "str or array".to_string(),
                            got: other.type_name().to_string(),
                        }),
                    };
                    self.stack.push(Value::Int(len));
                }

                Opcode::Range => {
                    let end   = self.pop()?.as_int()?;
                    let start = self.pop()?.as_int()?;
                    let arr: Vec<Value> = (start..end).map(Value::Int).collect();
                    self.stack.push(Value::Array(arr));
                }

                // ── ARRAYS ──────────────────────────────────────
                Opcode::MakeArray(n) => {
                    let mut elems: Vec<Value> = (0..n).map(|_| self.pop().unwrap_or(Value::Void)).collect();
                    elems.reverse();
                    self.stack.push(Value::Array(elems));
                }

                Opcode::IndexGet => {
                    let idx = self.pop()?;
                    let obj = self.pop()?;
                    match (obj, idx) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let idx = if i < 0 { (arr.len() as i64 + i) as usize } else { i as usize };
                            self.stack.push(arr.get(idx).cloned().ok_or(RuntimeError::IndexOutOfBounds { index: i, length: arr.len(), span: None })?);
                        }
                        (Value::Str(s), Value::Int(i)) => {
                            let chars: Vec<char> = s.chars().collect();
                            let idx = if i < 0 { (chars.len() as i64 + i) as usize } else { i as usize };
                            self.stack.push(chars.get(idx).map(|c| Value::Char(*c)).ok_or(RuntimeError::IndexOutOfBounds { index: i, length: chars.len(), span: None })?);
                        }
                        (other, _) => return Err(RuntimeError::NotIndexable { type_name: other.type_name().to_string(), span: None }),
                    }
                }

                Opcode::IndexSet => {
                    let idx = self.pop()?;
                    let obj = self.pop()?;
                    let val = self.pop()?;
                    if let (Value::Array(mut arr), Value::Int(i)) = (obj, idx) {
                        let idx = i as usize;
                        if idx >= arr.len() {
                            return Err(RuntimeError::IndexOutOfBounds { index: i, length: arr.len(), span: None });
                        }
                        arr[idx] = val;
                        self.stack.push(Value::Array(arr));
                    }
                }

                // ── STRUCTS ─────────────────────────────────────
                Opcode::MakeStruct(name, field_count) => {
                    let mut fields = std::collections::HashMap::new();
                    for _ in 0..field_count {
                        let val  = self.pop()?;
                        let key  = self.pop()?;
                        if let Value::Str(k) = key {
                            fields.insert(k, val);
                        }
                    }
                    self.stack.push(Value::Struct { name, fields });
                }

                Opcode::GetField(field_name) => {
                    let obj = self.pop()?;
                    match obj {
                        Value::Struct { name, fields } => {
                            let val = fields.get(&field_name).cloned().ok_or(RuntimeError::FieldNotFound {
                                struct_name: name, field: field_name, span: None,
                            })?;
                            self.stack.push(val);
                        }
                        other => return Err(RuntimeError::TypeError {
                            expected: "struct".to_string(),
                            got: other.type_name().to_string(),
                        }),
                    }
                }

                Opcode::SetField(field_name) => {
                    let new_val = self.pop()?;
                    let obj     = self.pop()?;
                    if let Value::Struct { name, mut fields } = obj {
                        fields.insert(field_name, new_val);
                        self.stack.push(Value::Struct { name, fields });
                    }
                }

                // ── STACK OPS ───────────────────────────────────
                Opcode::Pop  => { self.pop()?; }
                Opcode::Dup  => { let val = self.peek().clone(); self.stack.push(val); }
                Opcode::Swap => {
                    let len = self.stack.len();
                    if len >= 2 { self.stack.swap(len - 1, len - 2); }
                }

                Opcode::NullCoalesce => {
                    let right = self.pop()?;
                    let left  = self.pop()?;
                    self.stack.push(match left {
                        Value::Null => right,
                        other       => other,
                    });
                }

                Opcode::Halt => return Ok(self.stack.pop().unwrap_or(Value::Void)),

                _ => {
                    return Err(RuntimeError::NotImplemented {
                        feature: format!("{}", op),
                    });
                }
            }
        }
    }

    // ── HELPERS ─────────────────────────────────────────────────

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack.pop().ok_or(RuntimeError::TypeError {
            expected: "value on stack".to_string(),
            got: "empty stack".to_string(),
        })
    }

    fn peek(&self) -> &Value {
        self.stack.last().unwrap_or(&Value::Void)
    }

    fn cmp_values(&self, a: &Value, b: &Value) -> std::cmp::Ordering {
        match (a, b) {
            (Value::Int(x),   Value::Int(y))   => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Str(x),   Value::Str(y))   => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        }
    }

    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<(), RuntimeError> {
        use crate::interpreter::builtins::all_builtins;
        let result = match all_builtins().iter().find(|(n, _, _)| *n == name) {
            Some((_, _, func)) => {
                if self.capture_output && (name == "print" || name == "println") {
                    let msg = args.first().map(|v| v.to_display_string()).unwrap_or_default();
                    self.output.push(msg);
                    return Ok(());
                }
                func(args)?
            }
            None => return Err(RuntimeError::UndefinedVariable { name: name.to_string(), span: None }),
        };
        self.stack.push(result);
        Ok(())
    }

    fn call_builtin_by_name(&mut self, val: &Value, args: Vec<Value>) -> Result<(), RuntimeError> {
        match val {
            Value::Builtin { name, func } => {
                if self.capture_output && (*name == "print" || *name == "println") {
                    let msg = args.first().map(|v| v.to_display_string()).unwrap_or_default();
                    self.output.push(msg);
                    return Ok(());
                }
                let result = func(args)?;
                self.stack.push(result);
                Ok(())
            }
            _ => Err(RuntimeError::NotCallable { type_name: val.type_name().to_string(), span: None }),
        }
    }
}

impl Default for VM { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod vm_tests {
    use super::*;
    use super::opcode::{Chunk, Opcode};
    use crate::interpreter::value::Value;

    fn run_chunk(ops: Vec<(Opcode, usize)>) -> Value {
        let mut chunk = Chunk::new("test");
        for (op, line) in ops { chunk.emit(op, line); }
        VM::new().run(chunk).unwrap()
    }

    #[test]
    fn test_push_int()   { let v = run_chunk(vec![(Opcode::PushInt(42), 1), (Opcode::Halt, 1)]); assert_eq!(v, Value::Int(42)); }
    #[test]
    fn test_add_ints()   { let v = run_chunk(vec![(Opcode::PushInt(3),1),(Opcode::PushInt(4),1),(Opcode::Add,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Int(7)); }
    #[test]
    fn test_sub_ints()   { let v = run_chunk(vec![(Opcode::PushInt(10),1),(Opcode::PushInt(3),1),(Opcode::Sub,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Int(7)); }
    #[test]
    fn test_mul_ints()   { let v = run_chunk(vec![(Opcode::PushInt(3),1),(Opcode::PushInt(4),1),(Opcode::Mul,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Int(12)); }
    #[test]
    fn test_div_ints()   { let v = run_chunk(vec![(Opcode::PushInt(10),1),(Opcode::PushInt(2),1),(Opcode::Div,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Int(5)); }
    #[test]
    fn test_div_zero_err(){ let mut chunk = Chunk::new("t"); chunk.emit(Opcode::PushInt(5),1); chunk.emit(Opcode::PushInt(0),1); chunk.emit(Opcode::Div,1); assert!(VM::new().run(chunk).is_err()); }
    #[test]
    fn test_negate()     { let v = run_chunk(vec![(Opcode::PushInt(5),1),(Opcode::Negate,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Int(-5)); }
    #[test]
    fn test_equal_true() { let v = run_chunk(vec![(Opcode::PushInt(5),1),(Opcode::PushInt(5),1),(Opcode::Equal,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Bool(true)); }
    #[test]
    fn test_equal_false(){ let v = run_chunk(vec![(Opcode::PushInt(5),1),(Opcode::PushInt(6),1),(Opcode::Equal,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Bool(false)); }
    #[test]
    fn test_less()       { let v = run_chunk(vec![(Opcode::PushInt(3),1),(Opcode::PushInt(5),1),(Opcode::Less,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Bool(true)); }
    #[test]
    fn test_not_true()   { let v = run_chunk(vec![(Opcode::PushBool(true),1),(Opcode::Not,1),(Opcode::Halt,1)]); assert_eq!(v, Value::Bool(false)); }
    #[test]
    fn test_local_vars() {
        let mut chunk = Chunk::new("t");
        chunk.emit(Opcode::PushInt(42), 1);
        chunk.emit(Opcode::StoreLocal(0), 1);
        chunk.emit(Opcode::LoadLocal(0), 1);
        chunk.emit(Opcode::Halt, 1);
        assert_eq!(VM::new().run(chunk).unwrap(), Value::Int(42));
    }
    #[test]
    fn test_jump_unconditional() {
        let mut chunk = Chunk::new("t");
        chunk.emit(Opcode::Jump(3), 1);       // skip PushInt(99)
        chunk.emit(Opcode::PushInt(99), 1);    // never reached
        chunk.emit(Opcode::PushInt(42), 1);   // this runs after jump — wrong index
        chunk.emit(Opcode::PushInt(42), 1);   // instruction 3
        chunk.emit(Opcode::Halt, 1);
        let result = VM::new().run(chunk).unwrap();
        assert_eq!(result, Value::Int(42));
    }
    #[test]
    fn test_jump_if_false() {
        let mut chunk = Chunk::new("t");
        chunk.emit(Opcode::PushBool(false), 1);
        chunk.emit(Opcode::JumpIfFalseAndPop(3), 1);
        chunk.emit(Opcode::PushInt(1), 1);      // skipped
        chunk.emit(Opcode::PushInt(99), 1);     // reached
        chunk.emit(Opcode::Halt, 1);
        assert_eq!(VM::new().run(chunk).unwrap(), Value::Int(99));
    }
    #[test]
    fn test_make_array() {
        let mut chunk = Chunk::new("t");
        chunk.emit(Opcode::PushInt(1), 1);
        chunk.emit(Opcode::PushInt(2), 1);
        chunk.emit(Opcode::PushInt(3), 1);
        chunk.emit(Opcode::MakeArray(3), 1);
        chunk.emit(Opcode::Halt, 1);
        let v = VM::new().run(chunk).unwrap();
        assert_eq!(v, Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
    }
    #[test]
    fn test_index_get() {
        let mut chunk = Chunk::new("t");
        chunk.emit(Opcode::PushInt(10), 1);
        chunk.emit(Opcode::PushInt(20), 1);
        chunk.emit(Opcode::MakeArray(2), 1);
        chunk.emit(Opcode::PushInt(1), 1); // index 1
        chunk.emit(Opcode::IndexGet, 1);
        chunk.emit(Opcode::Halt, 1);
        assert_eq!(VM::new().run(chunk).unwrap(), Value::Int(20));
    }
    #[test]
    fn test_dup() {
        let mut chunk = Chunk::new("t");
        chunk.emit(Opcode::PushInt(5), 1);
        chunk.emit(Opcode::Dup, 1);
        chunk.emit(Opcode::Add, 1); // 5 + 5 = 10
        chunk.emit(Opcode::Halt, 1);
        assert_eq!(VM::new().run(chunk).unwrap(), Value::Int(10));
    }
}
