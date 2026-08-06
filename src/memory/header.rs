/// The size in bytes of the LyzHeader prefix on every heap allocation
/// refcount (i64, 8 bytes) + type_tag (i64, 8 bytes) = 16 bytes
pub const HEADER_SIZE: usize = 16;

/// Type tags — identify what kind of object a heap pointer refers to
/// so the destructor knows how to recursively free nested references
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Str = 0,
    Array = 1,
    Struct = 2, // generic struct — layout looked up via a separate descriptor table
}

impl TypeTag {
    pub fn as_i64(&self) -> i64 {
        *self as i64
    }

    pub fn from_i64(val: i64) -> Option<Self> {
        match val {
            0 => Some(Self::Str),
            1 => Some(Self::Array),
            2 => Some(Self::Struct),
            _ => None,
        }
    }
}

/// LLVM IR snippet: allocate `data_size` bytes of data plus the header,
/// initialize refcount=1 and the given type_tag, and return a pointer
/// to the START OF THE DATA (i.e. header_ptr + HEADER_SIZE).
///
/// This produces a fragment to be emitted via IrBuilder — the actual
/// register wiring happens in Phase 9 Task 902 (refcount.rs)
pub fn alloc_ir_template(data_size_expr: &str, type_tag: TypeTag) -> String {
    format!(
        "; lyz_alloc({} bytes, tag={:?})\n         %raw = call ptr @malloc(i64 add (i64 {}, i64 {}))\n         ; store refcount = 1 at offset 0\n         store i64 1, ptr %raw\n         %tag_ptr = getelementptr i8, ptr %raw, i64 8\n         store i64 {}, ptr %tag_ptr\n         %data_ptr = getelementptr i8, ptr %raw, i64 {}",
        data_size_expr, type_tag,
        data_size_expr, HEADER_SIZE,
        type_tag.as_i64(),
        HEADER_SIZE
    )
}

#[cfg(test)]
mod header_tests {
    use super::*;

    #[test]
    fn test_type_tag_roundtrip() {
        assert_eq!(TypeTag::from_i64(TypeTag::Str.as_i64()), Some(TypeTag::Str));
        assert_eq!(
            TypeTag::from_i64(TypeTag::Array.as_i64()),
            Some(TypeTag::Array)
        );
        assert_eq!(
            TypeTag::from_i64(TypeTag::Struct.as_i64()),
            Some(TypeTag::Struct)
        );
    }

    #[test]
    fn test_type_tag_invalid_returns_none() {
        assert_eq!(TypeTag::from_i64(999), None);
    }

    #[test]
    fn test_header_size_is_16_bytes() {
        // 2 x i64 = 16 bytes — must match the C struct layout exactly
        assert_eq!(HEADER_SIZE, 16);
    }

    #[test]
    fn test_alloc_ir_template_contains_malloc() {
        let ir = alloc_ir_template("24", TypeTag::Struct);
        assert!(ir.contains("call ptr @malloc"));
        assert!(ir.contains("store i64 1")); // initial refcount
    }

    #[test]
    fn test_alloc_ir_template_stores_correct_tag() {
        let ir = alloc_ir_template("8", TypeTag::Array);
        assert!(ir.contains(&format!("store i64 {}", TypeTag::Array.as_i64())));
    }

    #[test]
    fn test_alloc_ir_template_offsets_by_header_size() {
        let ir = alloc_ir_template("8", TypeTag::Str);
        assert!(ir.contains(&format!("i64 {}", HEADER_SIZE)));
    }
}
