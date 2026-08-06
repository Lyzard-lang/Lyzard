use lyzard::lexer::Lexer;
use lyzard::parser::Parser;
use lyzard::types::{error::TypeError, TypeChecker};

fn check_ok(src: &str) {
    let t = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, errs) = Parser::new(t, "i.lyz", src).parse().unwrap();
    assert!(errs.is_empty(), "Parse errors");
    let type_errs = TypeChecker::new(src, "i.lyz").check(&prog);
    assert!(
        type_errs.is_empty(),
        "Type errors:\n{}",
        type_errs.format_all(src)
    );
}

fn check_err(src: &str) -> Vec<TypeError> {
    let t = Lexer::tokenize(src, "i.lyz").unwrap();
    let (prog, _) = Parser::new(t, "i.lyz", src).parse().unwrap();
    TypeChecker::new(src, "i.lyz").check(&prog).0
}

#[test]
fn test_valid_full_program() {
    check_ok(
        r#"
struct Vector { x: float, y: float }


fn dot(a: Vector, b: Vector) -> float {
    return a.x * b.x + a.y * b.y
}


fn main() {
    let v1 = Vector { x: 1.0, y: 2.0 }
    let v2 = Vector { x: 3.0, y: 4.0 }
    let d: float = dot(v1, v2)
}
"#,
    );
}

#[test]
fn test_type_mismatch_in_let() {
    let errs = check_err("let x: int = \"hello\"");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::TypeMismatch { .. }));
}

#[test]
fn test_wrong_return_type() {
    let errs = check_err("fn f() -> int { return \"oops\" }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::TypeMismatch { .. }));
}

#[test]
fn test_arithmetic_type_error() {
    let errs = check_err("fn f() { let r = true + 1 }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::InvalidOperation { .. }));
}

#[test]
fn test_if_non_bool_condition() {
    let errs = check_err("fn f() { if 42 { } }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::NonBoolCondition { .. }));
}

#[test]
fn test_struct_field_type_wrong() {
    let errs = check_err("struct P { x: float }\nfn f() { let p = P { x: \"oops\" } }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::TypeMismatch { .. }));
}

#[test]
fn test_unknown_struct_field() {
    let errs = check_err("struct P { x: float }\nfn f() { let p = P { x: 1.0 } let r = p.z }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::UnknownStructField { field, .. } if field == "z"));
}

#[test]
fn test_call_not_a_function() {
    let errs = check_err("fn f() { let x = 42 x() }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::NotAFunction { .. }));
}

#[test]
fn test_call_wrong_arg_type() {
    let src = "fn add(a: int, b: int) -> int { return a + b }\nfn f() { add(1, \"x\") }";
    let errs = check_err(src);
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::ArgumentTypeMismatch { .. }));
}

#[test]
fn test_index_non_array() {
    let errs = check_err("fn f() { let x = 42 let r = x[0] }");
    assert!(!errs.is_empty());
    assert!(matches!(&errs[0], TypeError::IndexOnNonArray { .. }));
}

#[test]
fn test_forward_reference_ok() {
    check_ok(
        r#"
fn main() { let r = helper(5) }
fn helper(x: int) -> int { return x * 2 }
"#,
    );
}

#[test]
fn test_int_to_float_coercion_ok() {
    check_ok("let x: float = 42");
}

#[test]
fn test_error_message_quality() {
    let src = "fn f() -> int { return \"bad\" }";
    let errs = check_err(src);
    let formatted = errs[0].format(src);
    assert!(formatted.contains("🦎"), "Missing emoji");
    assert!(formatted.contains("^"), "Missing pointer");
    assert!(formatted.contains("Hint:"), "Missing hint");
}

#[test]
fn test_multiple_errors_reported() {
    let src = "fn f() { if 42 { } while 99 { } }";
    let errs = check_err(src);
    assert!(errs.len() >= 2, "Should report both errors");
}
