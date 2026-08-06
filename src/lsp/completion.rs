use serde_json::{json, Value as Json};

use crate::parser::ast::*;

use super::document::DocumentStore;

// LSP CompletionItemKind values (per spec — Function=3, Struct=7... etc)
const KIND_FUNCTION: i64 = 3;
const KIND_STRUCT: i64 = 7;
const KIND_ENUM: i64 = 13;
const KIND_KEYWORD: i64 = 14;

pub fn handle_completion(documents: &DocumentStore, params: &Json) -> Json {
    let uri = match params["textDocument"]["uri"].as_str() {
        Some(u) => u,
        None => return json!({ "items": [] }),
    };

    let doc = match documents.get(uri) {
        Some(d) => d,
        None => return json!({ "items": [] }),
    };
    let program = match &doc.analysis.program {
        Some(p) => p,
        None => return json!({ "items": [] }),
    };

    let mut items = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Function(f) => {
                items.push(json!({ "label": f.name, "kind": KIND_FUNCTION }))
            }
            Declaration::Struct(s) => items.push(json!({ "label": s.name, "kind": KIND_STRUCT })),
            Declaration::Enum(e) => items.push(json!({ "label": e.name, "kind": KIND_ENUM })),
            _ => {}
        }
    }

    for kw in [
        "let", "fn", "if", "else", "while", "for", "return", "match", "struct", "enum",
    ] {
        items.push(json!({ "label": kw, "kind": KIND_KEYWORD }));
    }

    for builtin in ["print", "println", "len", "range", "assert", "panic"] {
        items.push(json!({ "label": builtin, "kind": KIND_FUNCTION }));
    }

    json!({ "items": items })
}
