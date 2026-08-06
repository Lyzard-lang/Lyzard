use lyzard::interpreter::Interpreter;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

fn run_with_stdlib(user_code: &str) -> Result<(), String> {
    let core = std::fs::read_to_string("std/core.lyz").unwrap();
    let collections = std::fs::read_to_string("std/collections.lyz").unwrap();
    let full_src = format!("{}\n{}\n{}", core, collections, user_code);

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
fn test_collections_lyz_parses() {
    let src = std::fs::read_to_string("std/collections.lyz").expect("must exist");
    let tokens = Lexer::tokenize(&src, "c.lyz").unwrap();
    let (_, errs) = Parser::new(tokens, "c.lyz", &src).parse().unwrap();
    assert!(errs.is_empty(), "{}", errs.format_all(&src));
}

#[test]
fn test_list_new_is_empty() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let l: List<int> = List.new()
    assert(l.isEmpty())
    assert(l.len() == 0)
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_list_push_grows_length() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let l = List.new()
    let l2 = l.push(10)
    let l3 = l2.push(20)
    assert(l3.len() == 2)
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_list_get_valid_index() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let l = List.new().push(100).push(200)
    let v = l.get(1).unwrap()
    assert(v == 200)
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_list_get_out_of_bounds_returns_none() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let l = List.new().push(1)
    let v = l.get(99)
    assert(v.isNone())
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_list_contains() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let l = List.new().push(1).push(2).push(3)
    assert(l.contains(2))
    assert(!l.contains(99))
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_map_set_and_get() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let m = Map.new()
    let m2 = m.set("name", "LYZARD")
    let v = m2.get("name").unwrap()
    assert(v == "LYZARD")
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_map_overwrite_existing_key() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let m = Map.new().set("x", 1).set("x", 2)
    assert(m.len() == 1)
    assert(m.get("x").unwrap() == 2)
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}

#[test]
fn test_set_add_and_contains() {
    let r = run_with_stdlib(
        r#"
fn main() {
    let s = Set.new().add(1).add(2).add(1)
    assert(s.len() == 2)
    assert(s.contains(1))
    assert(!s.contains(99))
}
"#,
    );
    assert!(r.is_ok(), "{:?}", r);
}
