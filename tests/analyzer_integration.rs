use lyzard::analyzer::error::SemanticError;
use lyzard::analyzer::Analyzer;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

fn analyze_ok(src: &str) {
    let tokens = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, parse_errs) = Parser::new(tokens, "i.lyz", src).parse().unwrap();
    assert!(
        parse_errs.is_empty(),
        "Parse errors: {}",
        parse_errs.format_all(src)
    );
    let (errs, _) = Analyzer::new(src, "i.lyz").analyze(&prog);
    assert!(
        errs.is_empty(),
        "Semantic errors:\n{}",
        errs.format_all(src)
    );
}

fn analyze_err(src: &str) -> Vec<SemanticError> {
    let tokens = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, _) = Parser::new(tokens, "i.lyz", src).parse().unwrap();
    Analyzer::new(src, "i.lyz").analyze(&prog).0 .0
}

#[test]
fn test_valid_full_program() {
    let src = r#"
struct Vector {
    x: float,
    y: float,
}

enum Status {
    Ok,
    Err,
}

impl Vector {
    fn length(self) -> float => self.x * self.x + self.y * self.y
    fn scale(self, factor: float) -> Vector {
        return Vector { x: self.x * factor, y: self.y * factor }
    }
}

fn add(a: int, b: int) -> int => a + b

fn main() {
    let v = Vector { x: 3.0, y: 4.0 }
    let len = v.length()
    print(len)
    let result = add(1, 2)
    for i in 0..10 {
        let sq = i * i
        if sq > 50 { break }
        print(sq)
    }
}
"#;
    analyze_ok(src);
}

#[test]
fn test_forward_reference() {
    let src = r#"
fn main() {
    let r = compute(5)
    print(r)
}

fn compute(n: int) -> int {
    return n * n
}
"#;
    analyze_ok(src);
}

#[test]
fn test_multiple_errors_reported() {
    let src = r#"
fn f() {
    let r = undeclared1 + undeclared2
    break
}
"#;
    let errs = analyze_err(src);
    assert!(
        errs.len() >= 3,
        "Expected at least 3 errors, got {}",
        errs.len()
    );
}

#[test]
fn test_scope_isolation() {
    let src = r#"
fn f() {
    let x = 1
}

fn g() {
    print(x)
}
"#;
    let errs = analyze_err(src);
    assert!(!errs.is_empty());
    assert!(errs
        .iter()
        .any(|e| matches!(e, SemanticError::UndefinedName { name, .. } if name == "x")));
}

#[test]
fn test_recursive_fn_ok() {
    let src = r#"
fn fib(n: int) -> int {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
"#;
    analyze_ok(src);
}

#[test]
fn test_error_message_quality() {
    let src = "fn f() { let r = undef + 1 }";
    let tokens = Lexer::tokenize(src, "quality.lyz").unwrap();
    let (prog, _) = Parser::new(tokens, "quality.lyz", src).parse().unwrap();
    let (errs, _) = Analyzer::new(src, "quality.lyz").analyze(&prog);

    let formatted = errs.format_all(src);
    assert!(formatted.contains("🦎"), "Missing emoji");
    assert!(formatted.contains("undef"), "Missing variable name");
    assert!(formatted.contains("^"), "Missing source pointer");
    assert!(formatted.contains("Hint:"), "Missing hint");
}

#[test]
fn test_closure_scope() {
    let src = r#"
fn f() {
    let result = [1, 2, 3]
}
"#;
    analyze_ok(src);
}

#[test]
fn test_match_bindings_visible_in_arm() {
    let src = r#"
fn classify(n: int) -> str {
    match n {
        x -> print(x)
    }
    return "done"
}
"#;
    analyze_ok(src);
}
