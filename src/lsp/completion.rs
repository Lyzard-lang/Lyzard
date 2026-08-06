use serde_json::{json, Value as Json};

use super::document::DocumentStore;

/// Return completion items for the document position. Placeholder — a real
/// implementation is added in a later phase.
pub fn handle_completion(_documents: &DocumentStore, _params: &Json) -> Json {
    json!({ "items": [] })
}
