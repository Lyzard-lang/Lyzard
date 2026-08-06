pub mod prelude;

/// Which stdlib files are auto-imported into every LYZARD program,
/// in dependency order (core.lyz has no dependencies, collections.lyz
/// depends on core.lyz's Option<T>, etc.)
pub const PRELUDE_FILES: &[&str] = &["std/core.lyz", "std/collections.lyz", "std/string.lyz"];

/// Loads the prelude (the auto-imported stdlib) from disk and prepends it
/// to every user program before the normal Phase 1-8 pipeline runs.
#[derive(Debug)]
pub struct PreludeLoader {
    /// Cached, already-read source of each prelude file
    sources: Vec<(String, String)>, // (filename, content)
}

impl PreludeLoader {
    /// Load all prelude files from disk, in dependency order.
    /// Returns an error listing any file that could not be read.
    pub fn load(std_dir: &str) -> Result<Self, String> {
        let mut sources = Vec::new();
        for file in PRELUDE_FILES {
            let path = format!(
                "{}/{}",
                std_dir.trim_end_matches('/'),
                file.trim_start_matches("std/")
            );
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to load prelude file '{}': {}", path, e))?;
            sources.push((file.to_string(), content));
        }
        Ok(PreludeLoader { sources })
    }

    /// Build the final source text: prelude files concatenated, THEN the
    /// user's program. A comment marker separates each section for
    /// readable error messages (line numbers still map correctly since
    /// we track exact byte offsets via the lexer's Span system).
    pub fn build_full_source(&self, user_source: &str, user_filename: &str) -> String {
        let mut full = String::new();
        for (name, content) in &self.sources {
            full.push_str(&format!("-- === prelude: {} ===\n", name));
            full.push_str(content);
            full.push('\n');
        }
        full.push_str(&format!("-- === user program: {} ===\n", user_filename));
        full.push_str(user_source);
        full
    }

    /// Total number of prelude files successfully loaded
    pub fn file_count(&self) -> usize {
        self.sources.len()
    }

    /// Is a given name defined by the prelude? (best-effort heuristic —
    /// checks for `struct NAME`, `enum NAME`, or `fn NAME` in the source;
    /// good enough for helpful "did you forget to..." error hints, not
    /// used for actual compilation correctness)
    pub fn defines(&self, name: &str) -> bool {
        let patterns = [
            format!("struct {}", name),
            format!("enum {}", name),
            format!("fn {}(", name),
            format!("pub fn {}(", name),
        ];
        self.sources
            .iter()
            .any(|(_, content)| patterns.iter().any(|p| content.contains(p.as_str())))
    }
}

#[cfg(test)]
mod prelude_tests {
    use super::*;

    #[test]
    fn test_load_all_prelude_files() {
        let loader = PreludeLoader::load("std").expect("std/ directory must exist with all files");
        assert_eq!(loader.file_count(), 3);
    }

    #[test]
    fn test_load_missing_directory_errors() {
        let result = PreludeLoader::load("nonexistent_dir_xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_full_source_includes_all_sections() {
        let loader = PreludeLoader::load("std").unwrap();
        let full = loader.build_full_source("fn main() {}", "user.lyz");
        assert!(full.contains("prelude: std/core.lyz"));
        assert!(full.contains("prelude: std/collections.lyz"));
        assert!(full.contains("prelude: std/string.lyz"));
        assert!(full.contains("user program: user.lyz"));
        assert!(full.contains("fn main() {}"));
    }

    #[test]
    fn test_prelude_order_core_before_collections() {
        let loader = PreludeLoader::load("std").unwrap();
        let full = loader.build_full_source("", "u.lyz");
        let core_pos = full.find("prelude: std/core.lyz").unwrap();
        let coll_pos = full.find("prelude: std/collections.lyz").unwrap();
        assert!(
            core_pos < coll_pos,
            "core.lyz must come before collections.lyz (dependency order)"
        );
    }

    #[test]
    fn test_defines_option() {
        let loader = PreludeLoader::load("std").unwrap();
        assert!(loader.defines("Option"));
    }

    #[test]
    fn test_defines_list() {
        let loader = PreludeLoader::load("std").unwrap();
        assert!(loader.defines("List"));
    }

    #[test]
    fn test_defines_nonexistent_false() {
        let loader = PreludeLoader::load("std").unwrap();
        assert!(!loader.defines("TotallyMadeUpTypeName"));
    }
}
