use lyzard::analyzer::Analyzer;
use lyzard::interpreter::error::RuntimeError;
use lyzard::interpreter::Interpreter;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

/// Parse, analyze, and run a program end-to-end with output capture on.
fn run_ok(src: &str) -> Interpreter {
    let tokens = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, parse_errs) = Parser::new(tokens, "i.lyz", src).parse().unwrap();
    assert!(
        parse_errs.is_empty(),
        "Parse errors:\n{}",
        parse_errs.format_all(src)
    );
    let (errs, _) = Analyzer::new(src, "i.lyz").analyze(&prog);
    assert!(
        errs.is_empty(),
        "Semantic errors:\n{}",
        errs.format_all(src)
    );
    let mut i = Interpreter::new();
    i.capture_output = true;
    i.run(&prog).unwrap();
    i
}

/// Run a program that is expected to fail at runtime, returning the error.
fn run_err(src: &str) -> RuntimeError {
    let tokens = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, parse_errs) = Parser::new(tokens, "i.lyz", src).parse().unwrap();
    assert!(
        parse_errs.is_empty(),
        "Parse errors:\n{}",
        parse_errs.format_all(src)
    );
    let (errs, _) = Analyzer::new(src, "i.lyz").analyze(&prog);
    assert!(
        errs.is_empty(),
        "Semantic errors:\n{}",
        errs.format_all(src)
    );
    let mut i = Interpreter::new();
    i.capture_output = true;
    i.run(&prog).unwrap_err()
}

#[test]
fn test_hello_world() {
    let i = run_ok("println(\"Hello, world!\")");
    assert_eq!(i.output, vec!["Hello, world!"]);
}

#[test]
fn test_chained_calls() {
    let i = run_ok(
        r#"
println(toString(42))
println(parseInt("7") + 1)
println(parseFloat("2.5") * 2)
"#,
    );
    assert_eq!(i.output, vec!["42", "8", "5.0"]);
}

#[test]
fn test_user_function_and_return() {
    let i = run_ok(
        r#"
fn add(a, b) {
    return a + b
}
let result = add(3, 4)
println(result)
"#,
    );
    assert_eq!(i.output, vec!["7"]);
}

#[test]
fn test_recursion_fibonacci() {
    let i = run_ok(
        r#"
fn fib(n) {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
let result = fib(10)
println(result)
"#,
    );
    assert_eq!(i.output, vec!["55"]);
}

#[test]
fn test_match_expression() {
    let i = run_ok(
        r#"
fn classify(n) {
    let mut result = "?"
    match n {
        0 -> result = "zero"
        1 -> result = "one"
        x -> result = "many"
    }
    return result
}
println(classify(0))
println(classify(1))
println(classify(5))
"#,
    );
    assert_eq!(i.output, vec!["zero", "one", "many"]);
}

#[test]
fn test_while_loop() {
    let i = run_ok(
        r#"
let mut i = 0
let mut sum = 0
while i < 5 {
    sum = sum + i
    i = i + 1
}
println(sum)
"#,
    );
    assert_eq!(i.output, vec!["10"]);
}

#[test]
fn test_for_loop() {
    let i = run_ok(
        r#"
let mut total = 0
for i in 1..5 {
    total = total + i
}
println(total)
"#,
    );
    assert_eq!(i.output, vec!["10"]);
}

#[test]
fn test_loop_break_continue() {
    let i = run_ok(
        r#"
let mut count = 0
loop {
    count = count + 1
    if count == 2 {
        continue
    }
    if count >= 4 {
        break
    }
    println(count)
}
"#,
    );
    assert_eq!(i.output, vec!["1", "3"]);
}

#[test]
fn test_if_else() {
    let i = run_ok(
        r#"
fn sign(n) {
    if n > 0 {
        return "pos"
    } else {
        return "neg"
    }
}
println(sign(5))
println(sign(-5))
"#,
    );
    assert_eq!(i.output, vec!["pos", "neg"]);
}

#[test]
fn test_struct_field_and_method() {
    let i = run_ok(
        r#"
struct Point {
    x: int,
    y: int,
}

fn Point_length(p) {
    return p.x * p.x + p.y * p.y
}

let p = Point { x: 3, y: 4 }
let dist = p.length()
println(dist)
println(p.x)
"#,
    );
    assert_eq!(i.output, vec!["25", "3"]);
}

#[test]
fn test_array_operations() {
    let i = run_ok(
        r#"
let mut arr = [1, 2, 3]
arr = push(arr, 4)
println(len(arr))
println(arr[-1])
println(arr[0] + arr[1])
let last_item = pop(arr)
println(last_item)
println(arr.join("-"))
"#,
    );
    assert_eq!(i.output, vec!["4", "4", "3", "4", "1-2-3-4"]);
}

#[test]
fn test_complex_program() {
    let i = run_ok(
        r#"
fn is_prime(n) {
    if n < 2 {
        return false
    }
    let mut i = 2
    while i * i <= n {
        if n % i == 0 {
            return false
        }
        i = i + 1
    }
    return true
}

let mut primes = []
for i in 1..10 {
    if is_prime(i) {
        primes = push(primes, i)
    }
}
println(primes)
println(len(primes))
"#,
    );
    assert_eq!(i.output, vec!["[2, 3, 5, 7]", "4"]);
}

#[test]
fn test_runtime_error_index_out_of_bounds() {
    let err = run_err(
        r#"
let arr = [1, 2, 3]
let x = arr[5]
println(x)
"#,
    );
    assert!(matches!(err, RuntimeError::IndexOutOfBounds { .. }));
}
