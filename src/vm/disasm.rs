use super::opcode::Chunk;

/// Print a chunk's bytecode in human-readable format
pub fn disassemble(chunk: &Chunk) -> String {
    let mut out = String::new();
    out.push_str(&format!("=== {} ===\n", chunk.name));

    if !chunk.constants.is_empty() {
        out.push_str("-- Constants --\n");
        for (i, c) in chunk.constants.iter().enumerate() {
            out.push_str(&format!("  [{:3}] {}\n", i, c));
        }
    }

    out.push_str("-- Code --\n");
    let mut prev_line = 0;
    for (idx, op) in chunk.code.iter().enumerate() {
        let line = chunk.line_for(idx);
        let line_str = if line != prev_line {
            prev_line = line;
            format!("{:4}", line)
        } else {
            "   |".to_string()
        };
        out.push_str(&format!("  {:4}  {}  {}\n", idx, line_str, op));
    }

    out
}

/// Print disassembly directly to stdout (for debugging)
pub fn print_chunk(chunk: &Chunk) {
    print!("{}", disassemble(chunk));
}

#[cfg(test)]
mod disasm_tests {
    use super::*;
    use crate::vm::opcode::{Chunk, Opcode};
    use crate::interpreter::value::Value;

    #[test]
    fn test_disassemble_basic() {
        let mut chunk = Chunk::new("test");
        chunk.emit(Opcode::PushInt(5), 1);
        chunk.emit(Opcode::PushInt(3), 1);
        chunk.emit(Opcode::Add, 1);
        chunk.emit(Opcode::Halt, 1);
        let out = disassemble(&chunk);
        assert!(out.contains("=== test ==="));
        assert!(out.contains("PUSH_INT"));
        assert!(out.contains("ADD"));
        assert!(out.contains("HALT"));
    }

    #[test]
    fn test_disassemble_shows_constants() {
        let mut chunk = Chunk::new("test");
        chunk.add_constant(Value::Str("hello".to_string()));
        chunk.emit(Opcode::PushConst(0), 1);
        let out = disassemble(&chunk);
        assert!(out.contains("Constants"));
        assert!(out.contains("hello"));
    }

    #[test]
    fn test_disassemble_line_numbers() {
        let mut chunk = Chunk::new("test");
        chunk.emit(Opcode::PushInt(1), 1);
        chunk.emit(Opcode::PushInt(2), 3);
        chunk.emit(Opcode::Add,        3);
        let out = disassemble(&chunk);
        // Line 1 appears once, then | for same-line continuation
        assert!(out.contains("   1"));
        assert!(out.contains("   3"));
        assert!(out.contains("   |"));
    }
}
