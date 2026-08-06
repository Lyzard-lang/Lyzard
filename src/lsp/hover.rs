use serde_json::Value as Json;

use super::document::DocumentStore;

/// Return hover content for the document position. Placeholder — a real
/// implementation is added in a later phase.
pub fn handle_hover(_documents: &DocumentStore, _params: &Json) -> Json {
    Json::Null
}
