use serde_json::{json, Value as Json};

use super::document::{DiagnosticSeverity, DocumentDiagnostic};

/// Convert backend-agnostic document diagnostics into the LSP wire format
/// (range, severity, message) for textDocument/publishDiagnostics.
pub fn to_lsp_diagnostics(diagnostics: &[DocumentDiagnostic]) -> Vec<Json> {
    diagnostics
        .iter()
        .map(|d| {
            json!({
                "range": {
                    "start": { "line": d.line.saturating_sub(1), "character": d.col.saturating_sub(1) },
                    "end": { "line": d.end_line.saturating_sub(1), "character": d.end_col.saturating_sub(1) },
                },
                "severity": severity_number(d.severity),
                "message": d.message,
            })
        })
        .collect()
}

fn severity_number(severity: DiagnosticSeverity) -> i64 {
    match severity {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Information => 3,
        DiagnosticSeverity::Hint => 4,
    }
}
