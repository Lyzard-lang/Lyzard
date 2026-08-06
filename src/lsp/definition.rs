use serde_json::{json, Value as Json};

use crate::lexer::Span;
use crate::parser::ast::*;

use super::document::DocumentStore;

pub fn handle_definition(documents: &DocumentStore, params: &Json) -> Json {
    let uri = match params["textDocument"]["uri"].as_str() {
        Some(u) => u,
        None => return Json::Null,
    };
    let doc = match documents.get(uri) {
        Some(d) => d,
        None => return Json::Null,
    };
    let program = match &doc.analysis.program {
        Some(p) => p,
        None => return Json::Null,
    };

    // For this MVP: identify the symbol name under the cursor by re-using
    // the SAME hover position lookup for identifiers, then find where
    // THAT name was declared at the top level.
    let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1;
    let character = params["position"]["character"].as_u64().unwrap_or(0) as usize + 1;

    let symbol_name = find_identifier_at(program, line, character);
    let name = match symbol_name {
        Some(n) => n,
        None => return Json::Null,
    };

    for decl in &program.declarations {
        let (decl_name, decl_span): (&str, Span) = match decl {
            Declaration::Function(f) => (&f.name, f.span),
            Declaration::Struct(s) => (&s.name, s.span),
            Declaration::Enum(e) => (&e.name, e.span),
            _ => continue,
        };
        if decl_name == name {
            return json!({
                "uri": uri,
                "range": {
                    "start": { "line": decl_span.line.saturating_sub(1), "character": decl_span.col.saturating_sub(1) },
                    "end":   { "line": decl_span.line.saturating_sub(1), "character": decl_span.col.saturating_sub(1) + decl_name.len() },
                }
            });
        }
    }

    Json::Null
}

fn find_identifier_at(program: &Program, line: usize, col: usize) -> Option<String> {
    for decl in &program.declarations {
        if let Declaration::Function(f) = decl {
            if let FnBody::Block(block) = &f.body {
                if let Some(name) = find_identifier_in_block(block, line, col) {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn find_identifier_in_block(block: &Block, line: usize, col: usize) -> Option<String> {
    for stmt in &block.statements {
        if let Statement::Expression(e) = stmt {
            if let Some(name) = find_identifier_in_expr(&e.expr, line, col) {
                return Some(name);
            }
        }
        if let Statement::Let(l) = stmt {
            if let Some(name) = find_identifier_in_expr(&l.value, line, col) {
                return Some(name);
            }
        }
    }
    None
}

fn find_identifier_in_expr(expr: &Expr, line: usize, col: usize) -> Option<String> {
    match expr {
        Expr::Identifier(id)
            if id.span.line == line && col >= id.span.col && col <= id.span.col + id.name.len() =>
        {
            Some(id.name.clone())
        }
        Expr::Call(c) => {
            if let Expr::Identifier(id) = c.callee.as_ref() {
                if id.span.line == line && col >= id.span.col && col <= id.span.col + id.name.len()
                {
                    return Some(id.name.clone());
                }
            }
            for arg in &c.args {
                if let Some(n) = find_identifier_in_expr(&arg.value, line, col) {
                    return Some(n);
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod completion_definition_tests {
    use super::super::completion::handle_completion;
    use super::super::document::DocumentStore;
    use super::*;
    use serde_json::json;

    fn setup(src: &str) -> DocumentStore {
        let mut store = DocumentStore::new();
        store.open("file:///t.lyz".to_string(), src.to_string(), 1);
        store
    }

    #[test]
    fn test_completion_includes_user_function() {
        let store = setup("fn myCustomFn() {}");
        let result = handle_completion(
            &store,
            &json!({ "textDocument": { "uri": "file:///t.lyz" } }),
        );
        let items = result["items"].as_array().unwrap();
        assert!(items.iter().any(|i| i["label"] == "myCustomFn"));
    }

    #[test]
    fn test_completion_includes_struct() {
        let store = setup("struct Point { x: float, y: float }");
        let result = handle_completion(
            &store,
            &json!({ "textDocument": { "uri": "file:///t.lyz" } }),
        );
        let items = result["items"].as_array().unwrap();
        assert!(items.iter().any(|i| i["label"] == "Point"));
    }

    #[test]
    fn test_completion_includes_builtins() {
        let store = setup("fn f() {}");
        let result = handle_completion(
            &store,
            &json!({ "textDocument": { "uri": "file:///t.lyz" } }),
        );
        let items = result["items"].as_array().unwrap();
        assert!(items.iter().any(|i| i["label"] == "print"));
    }

    #[test]
    fn test_completion_includes_keywords() {
        let store = setup("fn f() {}");
        let result = handle_completion(
            &store,
            &json!({ "textDocument": { "uri": "file:///t.lyz" } }),
        );
        let items = result["items"].as_array().unwrap();
        assert!(items.iter().any(|i| i["label"] == "match"));
    }

    #[test]
    fn test_completion_missing_document_empty_list() {
        let store = DocumentStore::new();
        let result = handle_completion(
            &store,
            &json!({ "textDocument": { "uri": "file:///missing.lyz" } }),
        );
        assert!(result["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_definition_finds_function_declaration() {
        let store = setup("fn compute() -> int { return 1 }\nfn main() { compute() }");
        // "compute()" appears on line 2 (1-indexed), roughly column 13
        let result = handle_definition(
            &store,
            &json!({
                "textDocument": { "uri": "file:///t.lyz" },
                "position": { "line": 1, "character": 12 },
            }),
        );
        assert_ne!(result, Json::Null);
        assert_eq!(result["uri"], "file:///t.lyz");
    }

    #[test]
    fn test_definition_missing_document_null() {
        let store = DocumentStore::new();
        let result = handle_definition(
            &store,
            &json!({
                "textDocument": { "uri": "file:///missing.lyz" },
                "position": { "line": 0, "character": 0 },
            }),
        );
        assert_eq!(result, Json::Null);
    }

    #[test]
    fn test_definition_no_symbol_at_position_null() {
        let store = setup("fn f() {}");
        let result = handle_definition(
            &store,
            &json!({
                "textDocument": { "uri": "file:///t.lyz" },
                "position": { "line": 10, "character": 10 },
            }),
        );
        assert_eq!(result, Json::Null);
    }

    #[test]
    fn test_definition_finds_struct_declaration() {
        let store = setup("struct Point { x: float, y: float }\nfn main() { let p = Point }");
        // "Point" appears on line 2 (1-indexed), roughly column 21
        let result = handle_definition(
            &store,
            &json!({
                "textDocument": { "uri": "file:///t.lyz" },
                "position": { "line": 1, "character": 20 },
            }),
        );
        assert_ne!(result, Json::Null);
        assert_eq!(result["uri"], "file:///t.lyz");
    }
}
