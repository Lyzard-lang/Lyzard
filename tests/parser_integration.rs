use lyzard::lexer::Lexer;
use lyzard::parser::error::ParseErrors;
use lyzard::parser::{ast, Parser};

fn parse(src: &str) -> (ast::Program, Vec<String>, ParseErrors) {
    let tokens = Lexer::tokenize(src, "integration_test.lyz").unwrap();
    let (program, errors) = Parser::new(tokens, "integration_test.lyz", src)
        .parse()
        .expect("parse should never fail fatally");
    let formatted = errors.format_all(src);
    (
        program,
        formatted
            .lines()
            .filter(|l| l.contains("error"))
            .map(str::to_owned)
            .collect(),
        errors,
    )
}

#[test]
fn test_full_program_parses() {
    let src = r#"
import std.io as io
import std.collections as collections

struct Point {
    x: float,
    y: float,
}

enum Shape {
    Circle(float),
    Line,
    Point
}

interface Area {
    fn area(self) -> float
}

impl Point for Area {
    fn area(self) -> float {
        return self.x * self.y
    }
}

fn distance(p1: Point, p2: Point) -> float {
    let dx = p2.x - p1.x
    let dy = p2.y - p1.y
    return (dx * dx + dy * dy)
}

fn classify(shape: Shape) -> string {
    match shape {
        Shape.Circle(r) -> "circle"
        Shape.Line -> "line"
        _ -> "point"
    }
}

fn main() {
    const K = 1.0
    let origin = Point { x: 0.0, y: 0.0 }
    let p = Point { x: 3.0, y: 4.0 }
    let d = distance(origin, p)
    if d > K {
        let s = classify(Shape.Circle(2.0))
        print(s)
    }
    let mut i = 0
    while i < 3 {
        i = i + 1
    }
    for n in range(0, 10) {
        print(n)
    }
    loop outer {
        break outer
    }
    spawn {
        io.println("hi")
    }
    let maybe = [1, 2, 3]
    let v = maybe[0] ?? 0
    let label = match p { x -> string(x) }
    print(label)
    return
}
"#;

    let (program, messages, errors) = parse(src);
    assert!(
        errors.is_empty(),
        "expected zero errors, got:\n{}",
        messages.join("\n")
    );

    // 2 imports, struct, enum, interface, impl, 3 fns
    assert_eq!(program.declarations.len(), 9);
}

#[test]
fn test_declaration_order_preserved() {
    let src = "import a\nstruct S {}\nfn f() {}\nlet x = 1\nenum E { A }\nconst C = 2\n";
    let (program, messages, errors) = parse(src);
    assert!(
        errors.is_empty(),
        "unexpected errors:\n{}",
        messages.join("\n")
    );

    use ast::Declaration;
    assert_eq!(program.declarations.len(), 6);
    assert!(matches!(program.declarations[0], Declaration::Import(_)));
    assert!(matches!(program.declarations[1], Declaration::Struct(_)));
    assert!(matches!(program.declarations[2], Declaration::Function(_)));
    assert!(matches!(program.declarations[3], Declaration::Let(_)));
    assert!(matches!(program.declarations[4], Declaration::Enum(_)));
    assert!(matches!(program.declarations[5], Declaration::Const(_)));
}

#[test]
fn test_module_nested_declarations() {
    let src = "module m {\n  fn helper() {}\n  struct Inner { a: int }\n  module deep { fn leaf() {} }\n}\n";
    let (program, messages, errors) = parse(src);
    assert!(
        errors.is_empty(),
        "unexpected errors:\n{}",
        messages.join("\n")
    );

    if let ast::Declaration::Module(m) = &program.declarations[0] {
        assert_eq!(m.name, "m");
        assert_eq!(m.body.len(), 3);
    } else {
        panic!("expected module declaration");
    }
}

#[test]
fn test_multiple_errors_recovered_per_run() {
    let src = "fn ok() { return 1 }\nfn a() { let x =\nfn b() {)\nlet y = 1 + \nfn c() {}\n";
    let (program, messages, errors) = parse(src);
    assert!(
        errors.len() >= 3,
        "expected at least 3 collected errors, got {}:\n{}",
        errors.len(),
        messages.join("\n")
    );
    assert!(
        program.declarations.len() >= 2,
        "should recover and keep parsing"
    );
}

#[test]
fn test_spans_are_contiguous_and_ordered() {
    let src = "fn f(a: int) -> int {\n  let b = a + 1\n  return b\n}\n";
    let (program, messages, errors) = parse(src);
    assert!(
        errors.is_empty(),
        "unexpected errors:\n{}",
        messages.join("\n")
    );

    let spans: Vec<_> = program.declarations.iter().map(|d| d.span()).collect();
    assert!(!spans.is_empty());
    for pair in spans.windows(2) {
        assert!(
            pair[0].start <= pair[1].start,
            "declarations out of order: {:?} vs {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn test_empty_and_whitespace_programs() {
    for src in ["", "  \n\n  \n", "-- just a comment\n"] {
        let (program, messages, errors) = parse(src);
        assert!(
            errors.is_empty(),
            "unexpected errors:\n{}",
            messages.join("\n")
        );
        assert!(program.declarations.is_empty());
    }
}
