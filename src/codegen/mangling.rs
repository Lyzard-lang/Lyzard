/// Mangle a LYZARD function name into a unique LLVM symbol name
/// Prevents collisions with LLVM/C reserved names (e.g. "main", "malloc")
pub fn mangle_fn_name(name: &str) -> String {
    if name == "main" {
        "lyz_main".to_string()
    } else {
        format!("lyz_{}", name)
    }
}

/// Mangle a struct name into an LLVM type name
pub fn mangle_struct_name(name: &str) -> String {
    format!("struct.{}", name)
}
