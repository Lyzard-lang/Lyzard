use lyzard::lexer::{Lexer, TokenKind};

#[test]
fn test_full_program() {
    let src = r#"
-- LYZARD integration test program

struct Point {
    x: float,
    y: float,
}

fn distance(p1: Point, p2: Point) -> float {
    let dx = p2.x - p1.x
    let dy = p2.y - p1.y
    return dx * dx + dy * dy
}

fn main() {
    let origin = Point { x: 0.0, y: 0.0 }
    let p = Point { x: 3.0, y: 4.0 }
    let d = distance(origin, p)
    print(d)
}
"#;

    let result = Lexer::tokenize(src, "integration_test.lyz");
    assert!(result.is_ok(), "Lexer error: {}", result.unwrap_err());

    let tokens = result.unwrap();

    // Verify key tokens present
    let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();

    assert!(kinds.iter().any(|k| **k == TokenKind::Struct));
    assert!(kinds.iter().any(|k| **k == TokenKind::Fn));
    assert!(kinds.iter().any(|k| **k == TokenKind::Return));
    assert!(kinds.iter().any(|k| matches!(k, TokenKind::Identifier(n) if n.as_ref() == "distance")));
    assert!(kinds.iter().any(|k| matches!(k, TokenKind::FloatLiteral(f) if *f == 3.0)));
    assert!(**kinds.last().unwrap() == TokenKind::EOF);
}

#[test]
fn test_error_message_quality() {
    let src = "let x = @bad_char";
    let err = Lexer::tokenize(src, "test.lyz").unwrap_err();
    let formatted = err.format(src);

    // Must contain all key parts of a good error message
    assert!(formatted.contains("\u{1F98E}"), "Missing LYZARD emoji");
    assert!(formatted.contains("@"), "Missing the bad character");
    assert!(formatted.contains("Hint:"), "Missing hint");
    assert!(formatted.contains("test.lyz"), "Missing filename");
    assert!(formatted.contains('^'), "Missing source pointer");
}

#[test]
fn test_spans_are_accurate() {
    let src = "let x = 42";
    let tokens = Lexer::tokenize(src, "test.lyz").unwrap();

    // "let" starts at byte 0
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 3);

    // "x" starts at byte 4
    assert_eq!(tokens[1].span.start, 4);
    assert_eq!(tokens[1].span.end, 5);

    // "42" starts at byte 8
    assert_eq!(tokens[3].span.start, 8);
    assert_eq!(tokens[3].span.end, 10);
}

#[test]
fn test_line_numbers_accurate() {
    let src = "let x = 1\nlet y = 2\nlet z = 3";
    let tokens = Lexer::tokenize(src, "test.lyz").unwrap();

    let lets: Vec<_> = tokens.iter().filter(|t| t.kind == TokenKind::Let).collect();
    assert_eq!(lets.len(), 3);
    assert_eq!(lets[0].span.line, 1);
    assert_eq!(lets[1].span.line, 2);
    assert_eq!(lets[2].span.line, 3);
}
