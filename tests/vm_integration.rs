use lyzard::lexer::Lexer;
use lyzard::parser::Parser;
use lyzard::vm::{VM, compiler::Compiler};

fn run_ok(src: &str) -> lyzard::interpreter::value::Value {
    let t = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, _) = Parser::new(t, "i.lyz", src).parse().unwrap();
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&prog).unwrap();
    let functions = compiler.take_functions();
    let mut vm = VM::new();
    vm.load_functions(functions);
    vm.run(chunk).unwrap()
}

fn run_capture(src: &str) -> Vec<String> {
    let t = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, _) = Parser::new(t, "i.lyz", src).parse().unwrap();
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&prog).unwrap();
    let functions = compiler.take_functions();
    let mut vm = VM::new();
    vm.capture_output = true;
    vm.load_functions(functions);
    vm.run(chunk).unwrap();
    vm.output
}

#[test]
fn test_arithmetic() {
    let t = Lexer::tokenize("5 + 3 * 2", "i.lyz").unwrap();
    let (prog, _) = Parser::new(t, "i.lyz", "5 + 3 * 2").parse().unwrap();
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&prog).unwrap();
    let mut vm = VM::new();
    vm.load_functions(compiler.take_functions());
    // Just verify it runs without error
    assert!(vm.run(chunk).is_ok());
}

#[test]
fn test_print_captured() {
    let out = run_capture("print(42)");
    assert!(out.contains(&"42".to_string()));
}

#[test]
fn test_let_and_use() {
    // let x = 10 then use x
    run_ok("let x = 10
let y = x + 5");
}

#[test]
fn test_array_operations() {
    run_ok(r#"
let nums = [1, 2, 3]
let first = nums[0]
let last  = nums[-1]
"#);
}

#[test]
fn test_string_concat() {
    run_ok(r#"let s = "hello" + " " + "world""#);
}

#[test]
fn test_comparison_ops() {
    run_ok("let r = 5 > 3");
    run_ok("let r = 5 == 5");
    run_ok("let r = 3 != 5");
}

#[test]
fn test_while_loop() {
    run_ok(r#"
fn f() {
    let mut i = 0
    while i < 5 {
        i = i + 1
    }
}
"#);
}

#[test]
fn test_for_loop() {
    run_ok(r#"
fn f() {
    let mut sum = 0
    for i in [1, 2, 3, 4, 5] {
        sum = sum + i
    }
}
"#);
}

#[test]
fn test_if_else() {
    run_ok(r#"
fn abs_val(n: int) -> int {
    if n < 0 { return n * -1 }
    else { return n }
}
fn main() { abs_val(-5) abs_val(5) }
"#);
}

#[test]
fn test_recursive_fibonacci() {
    run_ok(r#"
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() { fib(10) }
"#);
}

#[test]
fn test_struct() {
    run_ok(r#"
struct Point { x: float, y: float }
fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    let px = p.x
}
"#);
}

#[test]
fn test_forward_reference() {
    run_ok(r#"
fn main() { let r = helper(5) }
fn helper(x: int) -> int { return x * 2 }
"#);
}

#[test]
fn test_range_opcode() {
    run_ok("let r = 0..5");
}

#[test]
fn test_null_coalesce() {
    run_ok("let x = null ?? 42");
}

#[test]
fn test_div_by_zero_error() {
    use lyzard::interpreter::error::RuntimeError;
    let t = Lexer::tokenize("let r = 10 / 0", "t.lyz").unwrap();
    let (prog, _) = Parser::new(t, "t.lyz", "let r = 10 / 0").parse().unwrap();
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&prog).unwrap();
    let mut vm = VM::new();
    vm.load_functions(compiler.take_functions());
    let err = vm.run(chunk).unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero { .. }));
}
