pub mod header;
pub mod lifetime;

use crate::parser::ast::StructDecl;
use crate::types::ResolvedType;
use std::collections::HashMap;

/// Describes which byte offsets within a struct hold refcounted (heap) pointers.
/// Generated once per struct definition, embedded as LLVM IR global data,
/// and consulted at runtime by `lyz_release_struct_contents`.
#[derive(Debug, Clone)]
pub struct StructDescriptor {
    pub name: String,
    /// Byte offset of each refcounted field, in declaration order
    pub refcounted_offsets: Vec<usize>,
    /// Total size of the struct's data (excluding the 16-byte header)
    pub total_size: usize,
}

pub struct MemoryManager {
    struct_descriptors: HashMap<String, StructDescriptor>,
}

impl MemoryManager {
    pub fn new() -> Self {
        MemoryManager {
            struct_descriptors: HashMap::new(),
        }
    }

    /// Build a descriptor for a struct decl, given each field's resolved type.
    /// Assumes 8-byte-aligned fields (true for all LYZARD primitive/pointer types
    /// on 64-bit targets — LLVM's default struct layout for our type set).
    pub fn register_struct(&mut self, decl: &StructDecl, field_types: &[ResolvedType]) {
        let mut offsets = Vec::new();
        let mut offset = 0usize;

        for (_field, ty) in decl.fields.iter().zip(field_types.iter()) {
            if lifetime::is_refcounted(ty) {
                offsets.push(offset);
            }
            offset += 8; // every LYZARD field is 8 bytes (i64, double, or ptr)
        }

        self.struct_descriptors.insert(
            decl.name.clone(),
            StructDescriptor {
                name: decl.name.clone(),
                refcounted_offsets: offsets,
                total_size: offset,
            },
        );
    }

    pub fn descriptor_for(&self, struct_name: &str) -> Option<&StructDescriptor> {
        self.struct_descriptors.get(struct_name)
    }

    /// Generate the LLVM IR global array describing a struct's refcounted
    /// field offsets, for consumption by the C runtime's generic destructor.
    /// Format: [ i64 count, i64 offset0, i64 offset1, ... ]
    pub fn emit_descriptor_global(&self, desc: &StructDescriptor) -> String {
        let mut values = vec![desc.refcounted_offsets.len().to_string()];
        values.extend(desc.refcounted_offsets.iter().map(|o| o.to_string()));
        let elements: Vec<String> = values.iter().map(|v| format!("i64 {}", v)).collect();
        format!(
            "@desc.{} = global [{} x i64] [{}]",
            desc.name,
            values.len(),
            elements.join(", ")
        )
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod descriptor_tests {
    use super::*;
    use crate::lexer::Span;
    use crate::parser::ast::*;

    fn dummy_span() -> Span {
        Span::dummy()
    }

    fn make_struct_decl(name: &str, field_names: &[&str]) -> StructDecl {
        StructDecl {
            name: name.to_string(),
            generics: vec![],
            fields: field_names
                .iter()
                .map(|n| StructField {
                    name: n.to_string(),
                    field_type: TypeExpr::Named(NamedType {
                        name: "str".to_string(),
                        span: dummy_span(),
                    }),
                    is_pub: false,
                    span: dummy_span(),
                })
                .collect(),
            is_pub: false,
            span: dummy_span(),
        }
    }

    #[test]
    fn test_register_struct_all_refcounted_fields() {
        let mut mgr = MemoryManager::new();
        let decl = make_struct_decl("Pair", &["a", "b"]);
        mgr.register_struct(&decl, &[ResolvedType::Str, ResolvedType::Str]);
        let desc = mgr.descriptor_for("Pair").unwrap();
        assert_eq!(desc.refcounted_offsets, vec![0, 8]);
    }

    #[test]
    fn test_register_struct_mixed_fields() {
        let mut mgr = MemoryManager::new();
        let decl = make_struct_decl("Point", &["x", "label"]);
        // x is int (offset 0, NOT refcounted), label is str (offset 8, refcounted)
        mgr.register_struct(&decl, &[ResolvedType::Int, ResolvedType::Str]);
        let desc = mgr.descriptor_for("Point").unwrap();
        assert_eq!(desc.refcounted_offsets, vec![8]);
    }

    #[test]
    fn test_register_struct_no_refcounted_fields() {
        let mut mgr = MemoryManager::new();
        let decl = make_struct_decl("Vec2", &["x", "y"]);
        mgr.register_struct(&decl, &[ResolvedType::Float, ResolvedType::Float]);
        let desc = mgr.descriptor_for("Vec2").unwrap();
        assert!(desc.refcounted_offsets.is_empty());
    }

    #[test]
    fn test_total_size_computed() {
        let mut mgr = MemoryManager::new();
        let decl = make_struct_decl("Triple", &["a", "b", "c"]);
        mgr.register_struct(
            &decl,
            &[ResolvedType::Int, ResolvedType::Int, ResolvedType::Int],
        );
        let desc = mgr.descriptor_for("Triple").unwrap();
        assert_eq!(desc.total_size, 24); // 3 fields x 8 bytes
    }

    #[test]
    fn test_emit_descriptor_global_format() {
        let mut mgr = MemoryManager::new();
        let decl = make_struct_decl("Pair", &["a", "b"]);
        mgr.register_struct(&decl, &[ResolvedType::Str, ResolvedType::Str]);
        let desc = mgr.descriptor_for("Pair").unwrap();
        let ir = mgr.emit_descriptor_global(desc);
        assert!(ir.contains("@desc.Pair"));
        assert!(ir.contains("i64 2")); // count = 2 refcounted fields
        assert!(ir.contains("i64 0")); // offset 0
        assert!(ir.contains("i64 8")); // offset 8
    }

    #[test]
    fn test_descriptor_not_found_for_unknown_struct() {
        let mgr = MemoryManager::new();
        assert!(mgr.descriptor_for("Nonexistent").is_none());
    }
}
