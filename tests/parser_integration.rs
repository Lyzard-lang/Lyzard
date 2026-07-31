use lyzard::lexer::Lexer;
use lyzard::parser::{ast::*, Parser};

fn parse_ok(src: &str) -> Program {
    let t = Lexer::tokenize(src, "i.lyz").unwrap();
    let (p, e) = Parser::new(t, "i.lyz", src).parse().unwrap();
    if !e.is_empty() {
        panic!("Errors:\n{}", e.format_all(src));
    }
    p
}

#[test]
fn test_full_lyzard_program() {
    let src = r#"
import std.io.{ print }

struct Vector { x: float, y: float }

enum Color { Red, Green, Blue, Custom(int, int, int) }

interface Drawable {
    fn draw(self) -> void
    fn area(self) -> float
}

impl Vector {
    fn length(self) -> float => self.x * self.x + self.y * self.y
    fn scale(self, f: float) -> Vector {
        return Vector { x: self.x * f, y: self.y * f }
    }
}

fn fibonacci(n: int) -> int {
    match n {
        0 -> 0
        1 -> 1
        _ -> fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn main() {
    let v  = Vector { x: 3.0, y: 4.0 }
    let len = v.length()
    print(len)
    for i in 0..10 {
        let sq = i * i
        if sq > 50 { break }
        print(sq)
    }
}
"#;
    let p = parse_ok(src);
    assert!(p
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Function(_))));
    assert!(p
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Struct(_))));
    assert!(p
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Enum(_))));
    assert!(p
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Interface(_))));
    assert!(p
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Impl(_))));
}

#[test]
fn test_error_recovery() {
    let src = "fn ok1() { return 1 }\nfn bad() { let x = \nfn ok2() { return 2 }";
    let t = Lexer::tokenize(src, "t.lyz").unwrap();
    let (p, e) = Parser::new(t, "t.lyz", src).parse().unwrap();
    assert!(!e.is_empty(), "Expected errors");
    let fn_count = p
        .declarations
        .iter()
        .filter(|d| matches!(d, Declaration::Function(_)))
        .count();
    assert!(fn_count >= 2, "Should recover and parse both valid fns");
}

#[test]
fn test_operator_precedence_in_code() {
    let src = "fn f() { let r = 1 + 2 * 3 - 4 / 2 }";
    let p = parse_ok(src);
    if let Declaration::Function(f) = &p.declarations[0] {
        if let FnBody::Block(b) = &f.body {
            assert!(
                matches!(&b.statements[0], Statement::Let(l) if matches!(&l.value, Expr::Binary(_)))
            );
        }
    }
}

#[test]
fn test_chained_calls() {
    let src = "fn f() { let r = arr.map(double).filter(isEven).first() }";
    let p = parse_ok(src);
    assert_eq!(p.declarations.len(), 1);
}

#[test]
fn test_nested_if_else_if() {
    let src = r#"fn classify(n: int) -> str {
        if n > 0 { return "pos" } else if n < 0 { return "neg" } else { return "zero" }
    }"#;
    let p = parse_ok(src);
    if let Declaration::Function(f) = &p.declarations[0] {
        if let FnBody::Block(b) = &f.body {
            if let Statement::If(i) = &b.statements[0] {
                assert_eq!(i.else_if_branches.len(), 1);
                assert!(i.else_branch.is_some());
            }
        }
    }
}

#[test]
fn test_complex_match() {
    let src = r#"fn f() {
        match shape {
            Shape.Circle(r)       -> 3.14 * r * r
            Shape.Rect(w, h)      -> w * h
            _                     -> 0.0
        }
    }"#;
    let p = parse_ok(src);
    if let Declaration::Function(f) = &p.declarations[0] {
        if let FnBody::Block(b) = &f.body {
            if let Statement::Match(m) = &b.statements[0] {
                assert_eq!(m.arms.len(), 3);
            }
        }
    }
}
