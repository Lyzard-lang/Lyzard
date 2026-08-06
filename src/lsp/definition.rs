use serde_json::Value as Json;

use super::document::DocumentStore;

/// Return the definition location for the document position. Placeholder —
/// a real implementation is added in a later phase.
pub fn handle_definition(_documents: &DocumentStore, _params: &Json) -> Json {
    Json::Null
}
