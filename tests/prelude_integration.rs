// tests/prelude_integration.rs
use lyzard::stdlib::prelude::compile_with_prelude;

#[test]
fn test_user_program_uses_option_without_importing() {
    let user_code = r#"
fn main() {
    let x = Option.Some(42)
    let y = x.unwrapOr(0)
    assert(y == 42)
}
"#;
    let result = compile_with_prelude(user_code, "main.lyz", "std");
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}

#[test]
fn test_user_program_uses_list_and_string_helpers_together() {
    let user_code = r#"
fn main() {
    let names = List.new().push("alice").push("bob")
    let joined = join(names.toArray(), ", ")
    assert(joined == "alice, bob")
}
"#;
    let result = compile_with_prelude(user_code, "main.lyz", "std");
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
}
