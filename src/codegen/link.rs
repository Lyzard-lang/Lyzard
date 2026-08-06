use crate::interpreter::error::RuntimeError;
use std::path::Path;
use std::process::Command;

pub struct LinkOptions {
    pub output_path: String,
    pub optimize: bool, // -O2 vs -O0
    pub keep_ir: bool,  // keep the .ll file for debugging
}

impl Default for LinkOptions {
    fn default() -> Self {
        LinkOptions {
            output_path: "a.out".to_string(),
            optimize: true,
            keep_ir: false,
        }
    }
}

/// Take generated LLVM IR text and produce a native executable
pub fn compile_to_binary(ir_text: &str, opts: &LinkOptions) -> Result<(), RuntimeError> {
    let ir_path = format!("{}.ll", opts.output_path);
    let obj_path = format!("{}.o", opts.output_path);

    // Step 1: write the LLVM IR to a .ll file
    std::fs::write(&ir_path, ir_text).map_err(|e| RuntimeError::NotImplemented {
        feature: format!("failed to write IR file: {}", e),
    })?;

    // Step 2: compile .ll -> .o using llc
    let opt_flag = if opts.optimize { "-O2" } else { "-O0" };
    let llc_status = Command::new("llc")
        .args(["-filetype=obj", opt_flag, &ir_path, "-o", &obj_path])
        .status()
        .map_err(|e| RuntimeError::NotImplemented {
            feature: format!("llc not found — install LLVM: {}", e),
        })?;

    if !llc_status.success() {
        return Err(RuntimeError::NotImplemented {
            feature: "llc compilation failed — check generated IR for errors".to_string(),
        });
    }

    // Step 3: link .o + runtime.c -> executable using clang
    let runtime_path = find_runtime_source()?;
    let clang_status = Command::new("clang")
        .args([&obj_path, &runtime_path, "-o", &opts.output_path])
        .status()
        .map_err(|e| RuntimeError::NotImplemented {
            feature: format!("clang not found — install LLVM/clang: {}", e),
        })?;

    if !clang_status.success() {
        return Err(RuntimeError::NotImplemented {
            feature: "clang linking failed".to_string(),
        });
    }

    // Step 4: cleanup intermediate files
    let _ = std::fs::remove_file(&obj_path);
    if !opts.keep_ir {
        let _ = std::fs::remove_file(&ir_path);
    }

    Ok(())
}

fn find_runtime_source() -> Result<String, RuntimeError> {
    let candidates = ["runtime/lyz_runtime.c", "./runtime/lyz_runtime.c"];
    for path in candidates {
        if Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }
    Err(RuntimeError::NotImplemented {
        feature: "runtime/lyz_runtime.c not found — run from project root".to_string(),
    })
}

#[cfg(test)]
mod link_tests {
    use super::*;

    #[test]
    fn test_link_options_default() {
        let opts = LinkOptions::default();
        assert_eq!(opts.output_path, "a.out");
        assert!(opts.optimize);
        assert!(!opts.keep_ir);
    }

    #[test]
    fn test_find_runtime_source_missing() {
        // In a fresh test env without runtime/ dir, this should error gracefully
        // (Test just verifies it does not panic)
        let _ = find_runtime_source();
    }

    // NOTE: Full compile_to_binary() tests require llc/clang installed.
    // These are best run as a separate integration test gated behind
    // a feature flag or CI step that has LLVM available.
}
