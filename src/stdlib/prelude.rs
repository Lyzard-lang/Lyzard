/// High-level convenience wrapper: load the prelude and a user file,
/// and run the FULL compilation pipeline (lex -> parse -> analyze ->
/// type-check) in one call. Used by the `lyzard run`/`lyzard build` CLI.
use crate::lexer::Lexer;
use crate::parser::{Parser, ast::Program};
use crate::analyzer::Analyzer;
use crate::types::TypeChecker;
use super::PreludeLoader;

pub struct CompiledProgram {
    pub program: Program,
    pub full_source: String,
}

pub fn compile_with_prelude(
    user_source: &str,
    user_filename: &str,
    std_dir: &str,
) -> Result<CompiledProgram, String> {
    let prelude = PreludeLoader::load(std_dir)?;
    let full_source = prelude.build_full_source(user_source, user_filename);

    let tokens = Lexer::tokenize(&full_source, user_filename)
        .map_err(|e| e.format(&full_source))?;

    let (program, parse_errs) = Parser::new(tokens, user_filename, &full_source)
        .parse()
        .map_err(|e| e.format(&full_source))?;
    if !parse_errs.is_empty() {
        return Err(parse_errs.format_all(&full_source));
    }

    let (analysis_errs, _symbols) = Analyzer::new(&full_source, user_filename).analyze(&program);
    if !analysis_errs.is_empty() {
        return Err(analysis_errs.format_all(&full_source));
    }

    let type_errs = TypeChecker::new(&full_source, user_filename).check(&program);
    if !type_errs.is_empty() {
        return Err(type_errs.format_all(&full_source));
    }

    Ok(CompiledProgram { program, full_source })
}
