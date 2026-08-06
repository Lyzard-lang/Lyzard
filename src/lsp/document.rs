use std::collections::HashMap;

use crate::analyzer::Analyzer;
use crate::lexer::Lexer;
use crate::parser::{ast::Program, Parser};
use crate::types::TypeChecker;

/// The full analysis result for one document, refreshed on every edit
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub program: Option<Program>, // None if lexing/parsing failed entirely
    pub diagnostics: Vec<DocumentDiagnostic>,
}

/// A single problem found in the document, in a backend-agnostic form
/// (converted to the LSP wire format separately in Task 1204)
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentDiagnostic {
    pub line: usize, // 1-indexed, matching our Span convention
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// One open file, tracked live
pub struct Document {
    pub uri: String,
    pub text: String,
    pub version: i64, // LSP version counter — editors increment this per edit
    pub analysis: AnalysisResult,
}

impl Document {
    pub fn new(uri: String, text: String, version: i64) -> Self {
        let analysis = Self::analyze(&text, &uri);
        Document {
            uri,
            text,
            version,
            analysis,
        }
    }

    /// Replace the FULL document content (full-sync mode) and re-analyze
    pub fn update(&mut self, new_text: String, new_version: i64) {
        self.text = new_text;
        self.version = new_version;
        self.analysis = Self::analyze(&self.text, &self.uri);
    }

    /// Run the full Phase 1-5 pipeline and collect every diagnostic
    fn analyze(text: &str, uri: &str) -> AnalysisResult {
        let mut diagnostics = Vec::new();

        let tokens = match Lexer::tokenize(text, uri) {
            Ok(t) => t,
            Err(e) => {
                diagnostics.push(lex_error_to_diagnostic(&e));
                return AnalysisResult {
                    program: None,
                    diagnostics,
                };
            }
        };

        let (program, parse_errs) = match Parser::new(tokens, uri, text).parse() {
            Ok(result) => result,
            Err(fatal) => {
                diagnostics.push(parse_error_to_diagnostic(&fatal));
                return AnalysisResult {
                    program: None,
                    diagnostics,
                };
            }
        };
        for err in &parse_errs.0 {
            diagnostics.push(parse_error_to_diagnostic(err));
        }

        let (analysis_errs, _symbols) = Analyzer::new(text, uri).analyze(&program);
        for err in &analysis_errs.0 {
            diagnostics.push(semantic_error_to_diagnostic(err));
        }

        let type_errs = TypeChecker::new(text, uri).check(&program);
        for err in &type_errs.0 {
            diagnostics.push(type_error_to_diagnostic(err));
        }

        AnalysisResult {
            program: Some(program),
            diagnostics,
        }
    }
}

fn lex_error_to_diagnostic(e: &crate::lexer::LexError) -> DocumentDiagnostic {
    let span = e.span();
    DocumentDiagnostic {
        line: span.line,
        col: span.col,
        end_line: span.line,
        end_col: span.col + span.len().max(1),
        message: e.to_string(),
        severity: DiagnosticSeverity::Error,
    }
}

fn parse_error_to_diagnostic(e: &crate::parser::error::ParseError) -> DocumentDiagnostic {
    let span = e.span();
    DocumentDiagnostic {
        line: span.line,
        col: span.col,
        end_line: span.line,
        end_col: span.col + span.len().max(1),
        message: e.to_string(),
        severity: DiagnosticSeverity::Error,
    }
}

fn semantic_error_to_diagnostic(e: &crate::analyzer::error::SemanticError) -> DocumentDiagnostic {
    let span = e.span();
    DocumentDiagnostic {
        line: span.line,
        col: span.col,
        end_line: span.line,
        end_col: span.col + span.len().max(1),
        message: e.to_string(),
        severity: DiagnosticSeverity::Error,
    }
}

fn type_error_to_diagnostic(e: &crate::types::error::TypeError) -> DocumentDiagnostic {
    let span = e.span();
    DocumentDiagnostic {
        line: span.line,
        col: span.col,
        end_line: span.line,
        end_col: span.col + span.len().max(1),
        message: e.to_string(),
        severity: DiagnosticSeverity::Error,
    }
}

/// Tracks every currently-open document, keyed by its URI
#[derive(Default)]
pub struct DocumentStore {
    documents: HashMap<String, Document>,
}

impl DocumentStore {
    pub fn new() -> Self {
        DocumentStore {
            documents: HashMap::new(),
        }
    }

    pub fn open(&mut self, uri: String, text: String, version: i64) {
        self.documents
            .insert(uri.clone(), Document::new(uri, text, version));
    }

    pub fn update(&mut self, uri: &str, new_text: String, new_version: i64) {
        if let Some(doc) = self.documents.get_mut(uri) {
            doc.update(new_text, new_version);
        }
    }

    pub fn close(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.documents.get(uri)
    }

    pub fn open_count(&self) -> usize {
        self.documents.len()
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;

    #[test]
    fn test_new_document_valid_program_no_diagnostics() {
        let doc = Document::new(
            "file:///t.lyz".to_string(),
            "fn main() { let x = 1 }".to_string(),
            1,
        );
        assert!(doc.analysis.diagnostics.is_empty());
        assert!(doc.analysis.program.is_some());
    }

    #[test]
    fn test_new_document_undefined_var_produces_diagnostic() {
        let doc = Document::new(
            "file:///t.lyz".to_string(),
            "fn main() { let x = undeclared }".to_string(),
            1,
        );
        assert!(!doc.analysis.diagnostics.is_empty());
    }

    #[test]
    fn test_update_replaces_content_and_reanalyzes() {
        let mut doc = Document::new(
            "file:///t.lyz".to_string(),
            "fn main() { let x = undeclared }".to_string(),
            1,
        );
        assert!(!doc.analysis.diagnostics.is_empty());

        doc.update("fn main() { let x = 42 }".to_string(), 2);
        assert!(doc.analysis.diagnostics.is_empty());
        assert_eq!(doc.version, 2);
    }

    #[test]
    fn test_type_error_produces_diagnostic() {
        let doc = Document::new(
            "file:///t.lyz".to_string(),
            "let x: int = \"hello\"".to_string(),
            1,
        );
        assert!(doc
            .analysis
            .diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error));
    }

    #[test]
    fn test_diagnostic_has_correct_line_number() {
        let src = "fn main() {\n    let x = undeclared\n}";
        let doc = Document::new("file:///t.lyz".to_string(), src.to_string(), 1);
        assert!(doc.analysis.diagnostics.iter().any(|d| d.line == 2));
    }

    #[test]
    fn test_store_open_and_get() {
        let mut store = DocumentStore::new();
        store.open("file:///a.lyz".to_string(), "fn f() {}".to_string(), 1);
        assert!(store.get("file:///a.lyz").is_some());
        assert_eq!(store.open_count(), 1);
    }

    #[test]
    fn test_store_close_removes_document() {
        let mut store = DocumentStore::new();
        store.open("file:///a.lyz".to_string(), "fn f() {}".to_string(), 1);
        store.close("file:///a.lyz");
        assert!(store.get("file:///a.lyz").is_none());
        assert_eq!(store.open_count(), 0);
    }

    #[test]
    fn test_store_update_reanalyzes_correct_document() {
        let mut store = DocumentStore::new();
        store.open(
            "file:///a.lyz".to_string(),
            "let x = undeclared".to_string(),
            1,
        );
        store.update("file:///a.lyz", "let x = 42".to_string(), 2);
        let doc = store.get("file:///a.lyz").unwrap();
        assert!(doc.analysis.diagnostics.is_empty());
    }

    #[test]
    fn test_get_nonexistent_document_none() {
        let store = DocumentStore::new();
        assert!(store.get("file:///missing.lyz").is_none());
    }
}
