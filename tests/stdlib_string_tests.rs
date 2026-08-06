use lyzard::interpreter::Interpreter;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

fn run_with_stdlib(user_code: &str) -> Result<(), String> {
    let core = std::fs::read_to_string("std/core.lyz").unwrap();
    let string = std::fs::read_to_string("std/string.lyz").unwrap();
    let full_src = format!("{}\n{}\n{}", core, string, user_code);
    let tokens = Lexer::tokenize(&full_src, "test.lyz").map_err(|e| e.to_string())?;
    let (prog, errs) = Parser::new(tokens, "test.lyz", &full_src)
        .parse()
        .map_err(|e| e.to_string())?;
    if !errs.is_empty() {
        return Err(errs.format_all(&full_src));
    }
    Interpreter::new().run(&prog).map_err(|e| e.to_string())
}

#[test]
fn test_string_lyz_parses() {
    let src = std::fs::read_to_string("std/string.lyz").expect("must exist");
    let tokens = Lexer::tokenize(&src, "s.lyz").unwrap();
    let (_, errs) = Parser::new(tokens, "s.lyz", &src).parse().unwrap();
    assert!(errs.is_empty(), "{}", errs.format_all(&src));
}

#[test]
fn test_join_basic() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let parts = ["a", "b", "c"]
    let joined = join(parts, ", ")
    assert(joined == "a, b, c")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_repeat() {
    let r = run_with_stdlib(
        r#"
fn main() {
    assert(repeat("ab", 3) == "ababab")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_pad_left() {
    let r = run_with_stdlib(
        r#"
fn main() {
    assert(padLeft("5", 3, "0") == "005")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_index_of_found() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let idx = indexOf("hello world", "world").unwrap()
    assert(idx == 6)
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_index_of_not_found() {
    let r = run_with_stdlib(
        r#"
fn main() {
    assert(indexOf("hello", "xyz").isNone())
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_replace() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let s = replace("hello world", "world", "LYZARD")
    assert(s == "hello LYZARD")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_reverse() {
    let r = run_with_stdlib(
        r#"
fn main() {
    assert(reverse("hello") == "olleh")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_is_palindrome() {
    let r = run_with_stdlib(
        r#"
fn main() {
    assert(isPalindrome("racecar"))
    assert(!isPalindrome("hello"))
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_capitalize() {
    let r = run_with_stdlib(
        r#"
fn main() {
    assert(capitalize("lyzard") == "Lyzard")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}
