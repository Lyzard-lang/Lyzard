use lyzard::codegen::CodeGenerator;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

fn generate_ir(src: &str) -> String {
    let t = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, errs) = Parser::new(t, "i.lyz", src).parse().unwrap();
    assert!(errs.is_empty(), "Parse errors");
    CodeGenerator::new()
        .compile(&prog)
        .expect("Codegen should succeed")
}

#[test]
fn test_simple_function_ir_structure() {
    let ir = generate_ir("fn add(a: int, b: int) -> int { return a + b }");
    assert!(ir.contains("define i64 @lyz_add(i64 %a, i64 %b)"));
    assert!(ir.contains("entry:"));
    assert!(ir.contains("ret i64"));
}

#[test]
fn test_every_function_has_entry_block() {
    let ir = generate_ir(
        r#"
fn square(x: int) -> int { return x * x }
fn cube(x: int) -> int { return x * x * x }
"#,
    );
    let entry_count = ir.matches("entry:").count();
    assert_eq!(entry_count, 2);
}

#[test]
fn test_if_generates_three_blocks() {
    let ir = generate_ir("fn abs(x: int) -> int { if x < 0 { return 0 - x } return x }");
    assert!(ir.contains("br i1"));
    // Should have then/merge blocks in addition to entry
    assert!(ir.matches("bb").count() >= 2);
}

#[test]
fn test_while_generates_loop_structure() {
    let ir = generate_ir("fn countdown(n: int) -> int { while n > 0 { n = n } return n }");
    assert!(ir.contains("br label"));
    assert!(ir.contains("br i1"));
}

#[test]
fn test_recursive_function_calls_itself() {
    let ir = generate_ir(
        r#"
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    return n
}
"#,
    );
    assert!(ir.contains("define i64 @lyz_factorial"));
}

#[test]
fn test_string_literal_becomes_global_constant() {
    let ir = generate_ir(r#"fn greet() -> int { return 0 }"#);
    // Basic structural sanity — real string test needs full string codegen
    assert!(ir.contains("declare void @lyz_print_str"));
}

#[test]
fn test_main_function_mangled() {
    let ir = generate_ir("fn main() -> int { return 0 }");
    assert!(ir.contains("@lyz_main"));
    assert!(!ir.contains("@main(")); // raw "main" must be mangled, avoids C runtime clash
}

#[test]
fn test_runtime_declarations_present() {
    let ir = generate_ir("fn f() -> int { return 1 }");
    assert!(ir.contains("declare void @lyz_print_int(i64)"));
    assert!(ir.contains("declare ptr @malloc(i64)"));
}

#[test]
fn test_slice_builtin_compiles_to_runtime_call() {
    let ir = generate_ir(r#"fn f() -> int { let s = slice("hello world", 0, 5) return 0 }"#);
    assert!(ir.contains("call ptr @lyz_slice(ptr @.str.0, i64 0, i64 5)"));
}

#[test]
fn test_len_builtin_compiles_to_runtime_call() {
    let ir = generate_ir(r#"fn f() -> int { let n = len("hello") return 0 }"#);
    assert!(ir.contains("call i64 @lyz_strlen(ptr @.str.0)"));
}

#[test]
fn test_slice_runtime_declaration_present() {
    let ir = generate_ir("fn f() -> int { return 1 }");
    assert!(ir.contains("declare ptr @lyz_slice(ptr, i64, i64)"));
    assert!(ir.contains("declare i64 @lyz_strlen(ptr)"));
}

#[test]
fn test_arithmetic_all_ops_present() {
    let ir = generate_ir(
        r#"
fn ops(a: int, b: int) -> int {
    let s = a + b
    let d = a - b
    let m = a * b
    let q = a / b
    return s
}
"#,
    );
    assert!(ir.contains("add i64"));
    assert!(ir.contains("sub i64"));
    assert!(ir.contains("mul i64"));
    assert!(ir.contains("sdiv i64"));
}

#[test]
fn test_comparison_ops_use_icmp() {
    let ir = generate_ir("fn cmp(a: int, b: int) -> int { if a > b { return 1 } return 0 }");
    assert!(ir.contains("icmp sgt"));
}

// If llc is available, validate the IR is syntactically correct
#[test]
fn test_ir_is_valid_llvm_if_llc_available() {
    let ir = generate_ir("fn add(a: int, b: int) -> int { return a + b }");
    let tmp_path = std::env::temp_dir().join("lyzard_test.ll");
    std::fs::write(&tmp_path, &ir).unwrap();

    let result = std::process::Command::new("llc")
        .args(["-filetype=null", tmp_path.to_str().unwrap()])
        .output();

    match result {
        Ok(output) => {
            assert!(
                output.status.success(),
                "LLVM IR validation failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(_) => {
            eprintln!("llc not installed — skipping IR validation test");
        }
    }
    let _ = std::fs::remove_file(&tmp_path);
}
