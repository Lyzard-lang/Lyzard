use lyzard::analyzer::Analyzer;
use lyzard::interpreter::Interpreter;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;
use lyzard::types::TypeChecker;

/// Loads std/core.lyz + a user snippet, runs the full pipeline
fn run_with_stdlib(user_code: &str) -> Result<(), String> {
    let core_src = std::fs::read_to_string("std/core.lyz")
        .map_err(|e| format!("Could not read std/core.lyz: {}", e))?;
    let full_src = format!("{}\n{}", core_src, user_code);

    let tokens = Lexer::tokenize(&full_src, "test.lyz").map_err(|e| e.to_string())?;
    let (prog, parse_errs) = Parser::new(tokens, "test.lyz", &full_src)
        .parse()
        .map_err(|e| e.to_string())?;
    if !parse_errs.is_empty() {
        return Err(parse_errs.format_all(&full_src));
    }

    let (analysis_errs, _) = Analyzer::new(&full_src, "test.lyz").analyze(&prog);
    if !analysis_errs.is_empty() {
        return Err(analysis_errs.format_all(&full_src));
    }

    let type_errs = TypeChecker::new(&full_src, "test.lyz").check(&prog);
    if !type_errs.is_empty() {
        return Err(type_errs.format_all(&full_src));
    }

    Interpreter::new().run(&prog).map_err(|e| e.to_string())
}

#[test]
fn test_core_lyz_parses_without_errors() {
    let src = std::fs::read_to_string("std/core.lyz").expect("std/core.lyz must exist");
    let tokens = Lexer::tokenize(&src, "core.lyz").unwrap();
    let (_, errs) = Parser::new(tokens, "core.lyz", &src).parse().unwrap();
    assert!(
        errs.is_empty(),
        "std/core.lyz has parse errors:\n{}",
        errs.format_all(&src)
    );
}

#[test]
fn test_option_some_is_some() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let x = Option.Some(42)
    assert(x.isSome())
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}

#[test]
fn test_option_none_is_none() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let x: Option<int> = Option.None
    assert(x.isNone())
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}

#[test]
fn test_option_unwrap_or_with_some() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let x = Option.Some(5)
    let v = x.unwrapOr(0)
    assert(v == 5)
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}

#[test]
fn test_option_unwrap_or_with_none() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let x: Option<int> = Option.None
    let v = x.unwrapOr(99)
    assert(v == 99)
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}

#[test]
fn test_result_ok_is_ok() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let r: Result<int, str> = Result.Ok(10)
    assert(r.isOk())
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}

#[test]
fn test_result_err_is_err() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let r: Result<int, str> = Result.Err("failed")
    assert(r.isErr())
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}

#[test]
fn test_result_unwrap_or_with_err() {
    let result = run_with_stdlib(
        r#"
fn main() {
    let r: Result<int, str> = Result.Err("nope")
    let v = r.unwrapOr(-1)
    assert(v == -1)
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result);
}
