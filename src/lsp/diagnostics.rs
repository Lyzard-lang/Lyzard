use serde_json::{json, Value as Json};

use super::document::{DiagnosticSeverity, DocumentDiagnostic};

/// Convert one internal severity to the LSP-spec integer code
fn severity_to_lsp(sev: DiagnosticSeverity) -> i64 {
    match sev {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Information => 3,
        DiagnosticSeverity::Hint => 4,
    }
}

/// Convert our 1-indexed (line, col) to LSP's 0-indexed Position JSON
fn position_json(line: usize, col: usize) -> Json {
    json!({
        "line": line.saturating_sub(1),
        "character": col.saturating_sub(1),
    })
}

/// Convert ONE internal diagnostic to the exact LSP wire format
pub fn to_lsp_diagnostic(d: &DocumentDiagnostic) -> Json {
    json!({
        "range": {
            "start": position_json(d.line, d.col),
            "end":   position_json(d.end_line, d.end_col),
        },
        "severity": severity_to_lsp(d.severity),
        "message": d.message,
        "source": "lyzard",
    })
}

/// Convert a whole list — used when publishing diagnostics for a document
pub fn to_lsp_diagnostics(diagnostics: &[DocumentDiagnostic]) -> Vec<Json> {
    diagnostics.iter().map(to_lsp_diagnostic).collect()
}

#[cfg(test)]
mod diagnostics_tests {
    use super::super::document::{DiagnosticSeverity, DocumentDiagnostic};
    use super::*;

    fn sample_diag() -> DocumentDiagnostic {
        DocumentDiagnostic {
            line: 5,
            col: 10,
            end_line: 5,
            end_col: 20,
            message: "Undefined variable: 'foo'".to_string(),
            severity: DiagnosticSeverity::Error,
        }
    }

    #[test]
    fn test_severity_error_is_1() {
        assert_eq!(severity_to_lsp(DiagnosticSeverity::Error), 1);
    }
    #[test]
    fn test_severity_warning_is_2() {
        assert_eq!(severity_to_lsp(DiagnosticSeverity::Warning), 2);
    }

    #[test]
    fn test_position_converts_to_zero_indexed() {
        let pos = position_json(5, 10);
        assert_eq!(pos["line"], 4);
        assert_eq!(pos["character"], 9);
    }

    #[test]
    fn test_position_line_one_becomes_zero() {
        let pos = position_json(1, 1);
        assert_eq!(pos["line"], 0);
        assert_eq!(pos["character"], 0);
    }

    #[test]
    fn test_to_lsp_diagnostic_has_correct_shape() {
        let json = to_lsp_diagnostic(&sample_diag());
        assert_eq!(json["range"]["start"]["line"], 4);
        assert_eq!(json["range"]["start"]["character"], 9);
        assert_eq!(json["severity"], 1);
        assert_eq!(json["message"], "Undefined variable: 'foo'");
        assert_eq!(json["source"], "lyzard");
    }

    #[test]
    fn test_to_lsp_diagnostics_converts_all() {
        let diags = vec![sample_diag(), sample_diag()];
        let result = to_lsp_diagnostics(&diags);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_empty_diagnostics_list_produces_empty_array() {
        let result = to_lsp_diagnostics(&[]);
        assert!(result.is_empty());
    }
}
