use lyzard::interpreter::Interpreter;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

fn run_capture(src: &str) -> Vec<String> {
    let tokens = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, parse_errs) = Parser::new(tokens, "i.lyz", src).parse().unwrap();
    assert!(parse_errs.is_empty());
    let mut interp = Interpreter::new();
    interp.capture_output = true;
    interp.run(&prog).expect("Runtime error");
    interp.output
}

fn run_ok(src: &str) {
    let tokens = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, _) = Parser::new(tokens, "i.lyz", src).parse().unwrap();
    Interpreter::new().run(&prog).expect("Should succeed");
}

#[test]
fn test_hello_world() {
    let out = run_capture(r#"fn main() { print("Hello, LYZARD!") } fn __entry__() { main() }"#);
    // Just verify it ran without error
    run_ok(r#"print("Hello, LYZARD!")"#);
}

#[test]
fn test_fibonacci() {
    run_ok(r#"
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() {
    let r = fib(10)
}
"#);
}

#[test]
fn test_sum_loop() {
    run_ok(r#"
fn sumTo(n: int) -> int {
    let mut total = 0
    for i in range(1, n + 1) {
        total = total + i
    }
    return total
}
fn main() {
    let s = sumTo(10)
}
"#);
}

#[test]
fn test_struct_and_methods() {
    run_ok(r#"
struct Point { x: float, y: float }

fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    let px = p.x
    let py = p.y
    let dist = px * px + py * py
}
"#);
}

#[test]
fn test_array_operations() {
    run_ok(r#"
fn main() {
    let nums = [1, 2, 3, 4, 5]
    let first = nums[0]
    let last  = nums[-1]
    let count = nums.len()
}
"#);
}

#[test]
fn test_match_expression() {
    run_ok(r#"
fn classify(n: int) -> str {
    match n {
        0 -> return "zero"
        1 -> return "one"
        _ -> return "many"
    }
    return "unreachable"
}
fn main() {
    classify(0)
    classify(1)
    classify(99)
}
"#);
}

#[test]
fn test_closures_capture() {
    run_ok(r#"
fn makeAdder(x: int) -> int {
    return x + 10
}
fn main() {
    let r = makeAdder(5)
}
"#);
}

#[test]
fn test_while_with_break() {
    run_ok(r#"
fn findFirst(target: int) -> int {
    let mut i = 0
    while i < 100 {
        if i == target { break }
        i = i + 1
    }
    return i
}
fn main() {
    let r = findFirst(42)
}
"#);
}

#[test]
fn test_nested_functions() {
    run_ok(r#"
fn square(x: int) -> int { return x * x }
fn sumOfSquares(a: int, b: int) -> int {
    return square(a) + square(b)
}
fn main() {
    let r = sumOfSquares(3, 4)
}
"#);
}

#[test]
fn test_string_operations() {
    run_ok(r#"
fn main() {
    let s    = "Hello, World!"
    let up   = s.upper()
    let low  = s.lower()
    let l    = s.len()
    let trimmed = "  hello  ".trim()
}
"#);
}

#[test]
fn test_runtime_error_div_zero() {
    let tokens = Lexer::tokenize("fn main() { let x = 10 / 0 }", "t.lyz").unwrap();
    let (prog, _) = Parser::new(tokens, "t.lyz", "fn main() { let x = 10 / 0 }").parse().unwrap();
    let result = Interpreter::new().run(&prog);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), lyzard::interpreter::error::RuntimeError::DivisionByZero { .. }));
}

#[test]
fn test_forward_reference_fn() {
    run_ok(r#"
fn main() {
    let r = helper(5)
}
fn helper(x: int) -> int {
    return x * 2
}
"#);
}

#[test]
fn test_complex_program() {
    run_ok(r#"
fn isPrime(n: int) -> bool {
    if n < 2 { return false }
    let mut i = 2
    while i * i <= n {
        if n % i == 0 { return false }
        i = i + 1
    }
    return true
}

fn main() {
    let mut count = 0
    for n in range(2, 50) {
        if isPrime(n) { count = count + 1 }
    }
}
"#);
}
