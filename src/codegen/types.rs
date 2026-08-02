use crate::types::ResolvedType;

/// Maps a LYZARD ResolvedType to its LLVM IR type string
pub fn llvm_type(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Int      => "i64".to_string(),
        ResolvedType::Float    => "double".to_string(),
        ResolvedType::Bool     => "i1".to_string(),
        ResolvedType::Char     => "i32".to_string(),      // Unicode codepoint
        ResolvedType::Void     => "void".to_string(),
        ResolvedType::Never    => "void".to_string(),      // functions that never return
        ResolvedType::Str      => "ptr".to_string(),        // %LyzStr*
        ResolvedType::Array(_) => "ptr".to_string(),        // %LyzArray*
        ResolvedType::Struct(name)   => format!("%struct.{}", name),
        ResolvedType::Enum(name)     => format!("%enum.{}", name),
        ResolvedType::Optional(inner) => {
            // Optional<T> is represented as { i1 has_value, T value }
            format!("{{ i1, {} }}", llvm_type(inner))
        }
        ResolvedType::Tuple(types) => {
            let parts: Vec<String> = types.iter().map(llvm_type).collect();
            format!("{{ {} }}", parts.join(", "))
        }
        ResolvedType::Generic { .. } => "ptr".to_string(), // Result<T,E>, Vec<T>, etc — heap allocated
        ResolvedType::Function { params, return_type } => {
            let param_strs: Vec<String> = params.iter().map(llvm_type).collect();
            format!("{} ({})*", llvm_type(return_type), param_strs.join(", "))
        }
        ResolvedType::TypeParam(_) => "ptr".to_string(), // generics erased to pointers (type erasure)
        ResolvedType::Unknown | ResolvedType::Error => "ptr".to_string(), // should never codegen
    }
}

/// Is this type passed by value (register) or by reference (pointer)?
/// LLVM calling conventions: primitives by value, compound types by pointer
pub fn is_pass_by_value(ty: &ResolvedType) -> bool {
    matches!(ty,
        ResolvedType::Int | ResolvedType::Float | ResolvedType::Bool |
        ResolvedType::Char | ResolvedType::Void
    )
}

/// The LLVM default value for a type (used for uninitialized variables)
pub fn llvm_zero_value(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Int    => "0".to_string(),
        ResolvedType::Float  => "0.0".to_string(),
        ResolvedType::Bool   => "0".to_string(),
        ResolvedType::Char   => "0".to_string(),
        _ => "null".to_string(), // pointers default to null
    }
}

/// Size in bytes of a type (for malloc calls)
pub fn llvm_type_size(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Int | ResolvedType::Float => 8,
        ResolvedType::Bool | ResolvedType::Char  => 1,
        ResolvedType::Void                        => 0,
        _                                          => 8, // pointer size
    }
}

#[cfg(test)]
mod llvm_type_tests {
    use super::*;

    #[test]
    fn test_int_type()    { assert_eq!(llvm_type(&ResolvedType::Int), "i64"); }
    #[test]
    fn test_float_type()  { assert_eq!(llvm_type(&ResolvedType::Float), "double"); }
    #[test]
    fn test_bool_type()   { assert_eq!(llvm_type(&ResolvedType::Bool), "i1"); }
    #[test]
    fn test_void_type()   { assert_eq!(llvm_type(&ResolvedType::Void), "void"); }
    #[test]
    fn test_str_type()    { assert_eq!(llvm_type(&ResolvedType::Str), "ptr"); }
    #[test]
    fn test_array_type()  { assert_eq!(llvm_type(&ResolvedType::Array(Box::new(ResolvedType::Int))), "ptr"); }
    #[test]
    fn test_struct_type() { assert_eq!(llvm_type(&ResolvedType::Struct("Point".to_string())), "%struct.Point"); }

    #[test]
    fn test_optional_type() {
        let t = ResolvedType::Optional(Box::new(ResolvedType::Int));
        assert_eq!(llvm_type(&t), "{ i1, i64 }");
    }

    #[test]
    fn test_function_type() {
        let t = ResolvedType::Function {
            params: vec![ResolvedType::Int, ResolvedType::Int],
            return_type: Box::new(ResolvedType::Int),
        };
        assert_eq!(llvm_type(&t), "i64 (i64, i64)*");
    }

    #[test]
    fn test_pass_by_value_primitives() {
        assert!(is_pass_by_value(&ResolvedType::Int));
        assert!(is_pass_by_value(&ResolvedType::Float));
        assert!(is_pass_by_value(&ResolvedType::Bool));
    }

    #[test]
    fn test_pass_by_value_compound_false() {
        assert!(!is_pass_by_value(&ResolvedType::Str));
        assert!(!is_pass_by_value(&ResolvedType::Struct("Point".to_string())));
    }

    #[test]
    fn test_zero_value_int()   { assert_eq!(llvm_zero_value(&ResolvedType::Int), "0"); }
    #[test]
    fn test_zero_value_float() { assert_eq!(llvm_zero_value(&ResolvedType::Float), "0.0"); }
    #[test]
    fn test_zero_value_ptr()   { assert_eq!(llvm_zero_value(&ResolvedType::Str), "null"); }

    #[test]
    fn test_type_size_int()   { assert_eq!(llvm_type_size(&ResolvedType::Int), 8); }
    #[test]
    fn test_type_size_bool()  { assert_eq!(llvm_type_size(&ResolvedType::Bool), 1); }
}
