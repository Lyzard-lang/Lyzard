use serde_json::{json, Value as Json};

use crate::lexer::Span;
use crate::parser::ast::*;

use super::document::DocumentStore;

/// Handle a textDocument/hover request. Returns LSP's Hover result shape,
/// or JSON null if there's nothing to show (no document, no symbol here, etc).
pub fn handle_hover(documents: &DocumentStore, params: &Json) -> Json {
    let uri = match params["textDocument"]["uri"].as_str() {
        Some(u) => u,
        None => return Json::Null,
    };
    let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1; // LSP 0-indexed -> ours 1-indexed
    let character = params["position"]["character"].as_u64().unwrap_or(0) as usize + 1;

    let doc = match documents.get(uri) {
        Some(d) => d,
        None => return Json::Null,
    };
    let program = match &doc.analysis.program {
        Some(p) => p,
        None => return Json::Null,
    };

    match find_hover_info(program, line, character) {
        Some(info) => json!({
            "contents": { "kind": "markdown", "value": format!("```lyzard\n{}\n```", info) }
        }),
        None => Json::Null,
    }
}

/// Does this span contain the given (line, col) position?
fn span_contains(span: &Span, line: usize, col: usize) -> bool {
    // Simplified single-line containment check — sufficient for MVP since
    // most hoverable tokens (identifiers, literals) don't span multiple
    // lines; multi-line expression hover is a natural follow-up extension.
    span.line == line && col >= span.col && col <= span.col + span.len().max(1)
}

/// Walk the program looking for the most specific hoverable info at this position
fn find_hover_info(program: &Program, line: usize, col: usize) -> Option<String> {
    for decl in &program.declarations {
        if let Declaration::Function(f) = decl {
            if let Some(info) = hover_in_fn(f, line, col) {
                return Some(info);
            }
        }
    }
    None
}

fn hover_in_fn(f: &FnDecl, line: usize, col: usize) -> Option<String> {
    // The most specific node wins: check the body first so hovering a
    // binding inside the function (possibly on a later line) shows the
    // binding's info rather than the enclosing signature.
    if let FnBody::Block(block) = &f.body {
        if let Some(info) = hover_in_block(block, line, col) {
            return Some(info);
        }
    }

    // Otherwise, hovering the function's own signature (its span lives on
    // the header line) shows the full signature — (Using the function's
    // overall span as an approximation for the name token position — a
    // refinement would track the name's own Span separately in the AST for
    // pixel-perfect accuracy.)
    if span_contains(&f.span, line, col) {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_to_string(p.param_type.as_ref())))
            .collect();
        let ret = f
            .return_type
            .as_ref()
            .map(type_expr_to_string)
            .unwrap_or_else(|| "void".to_string());
        return Some(format!("fn {}({}) -> {}", f.name, params.join(", "), ret));
    }
    None
}

fn hover_in_block(block: &Block, line: usize, col: usize) -> Option<String> {
    for stmt in &block.statements {
        if let Statement::Let(l) = stmt {
            if span_contains(&l.span, line, col) {
                let ty = l
                    .type_annotation
                    .as_ref()
                    .map(type_expr_to_string)
                    .unwrap_or_else(|| "inferred".to_string());
                return Some(format!("let {}: {}", l.name, ty));
            }
        }
        if let Statement::Expression(e) = stmt {
            if let Expr::Identifier(id) = &e.expr {
                if span_contains(&id.span, line, col) {
                    return Some(format!("{} (identifier)", id.name));
                }
            }
        }
    }
    None
}

fn type_to_string(t: Option<&TypeExpr>) -> String {
    t.map(type_expr_to_string)
        .unwrap_or_else(|| "unknown".to_string())
}

fn type_expr_to_string(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Named(n) => n.name.clone(),
        TypeExpr::Optional(inner, _) => format!("{}?", type_expr_to_string(inner)),
        TypeExpr::Array(inner, _) => format!("[{}]", type_expr_to_string(inner)),
        TypeExpr::Generic(g) => format!(
            "{}<{}>",
            g.name,
            g.args
                .iter()
                .map(type_expr_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => "?".to_string(),
    }
}

#[cfg(test)]
mod hover_tests {
    use super::super::document::DocumentStore;
    use super::*;
    use serde_json::json;

    fn hover_at(src: &str, line: u64, character: u64) -> Json {
        let mut store = DocumentStore::new();
        store.open("file:///t.lyz".to_string(), src.to_string(), 1);
        handle_hover(
            &store,
            &json!({
                "textDocument": { "uri": "file:///t.lyz" },
                "position": { "line": line, "character": character },
            }),
        )
    }

    #[test]
    fn test_hover_on_function_signature() {
        // "fn add(a: int, b: int) -> int { return a + b }"
        //  0123456
        let result = hover_at("fn add(a: int, b: int) -> int { return a + b }", 0, 4);
        assert_ne!(result, Json::Null);
    }

    #[test]
    fn test_hover_returns_markdown_shape() {
        let result = hover_at("fn add(a: int, b: int) -> int { return a + b }", 0, 4);
        assert_eq!(result["contents"]["kind"], "markdown");
        assert!(result["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("fn add"));
    }

    #[test]
    fn test_hover_on_let_declaration() {
        let src = "fn f() {\n    let x: int = 5\n}";
        // "    let x: int = 5" — line 1 (0-indexed), col of 'x' is around 8
        let result = hover_at(src, 1, 9);
        assert_ne!(result, Json::Null);
    }

    #[test]
    fn test_hover_missing_document_returns_null() {
        let store = DocumentStore::new();
        let result = handle_hover(
            &store,
            &json!({
                "textDocument": { "uri": "file:///missing.lyz" },
                "position": { "line": 0, "character": 0 },
            }),
        );
        assert_eq!(result, Json::Null);
    }

    #[test]
    fn test_hover_on_empty_position_returns_null() {
        let result = hover_at("fn f() {}", 5, 5); // way outside any span
        assert_eq!(result, Json::Null);
    }

    #[test]
    fn test_span_contains_basic() {
        let span = crate::lexer::Span::new(0, 5, 3, 10);
        assert!(span_contains(&span, 3, 10));
        assert!(span_contains(&span, 3, 12));
        assert!(!span_contains(&span, 4, 10)); // wrong line
        assert!(!span_contains(&span, 3, 5)); // before the span starts
    }

    #[test]
    fn test_type_expr_to_string_named() {
        let t = TypeExpr::Named(NamedType {
            name: "int".to_string(),
            span: crate::lexer::Span::dummy(),
        });
        assert_eq!(type_expr_to_string(&t), "int");
    }

    #[test]
    fn test_type_expr_to_string_optional() {
        let inner = TypeExpr::Named(NamedType {
            name: "str".to_string(),
            span: crate::lexer::Span::dummy(),
        });
        let t = TypeExpr::Optional(Box::new(inner), crate::lexer::Span::dummy());
        assert_eq!(type_expr_to_string(&t), "str?");
    }
}
