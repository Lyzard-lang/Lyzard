pub mod protocol;
pub mod document;
pub mod diagnostics;
pub mod hover;
pub mod completion;
pub mod definition;

use std::io::{BufRead, Write};

use serde_json::{json, Value as Json};

use protocol::{read_message, write_message, RpcResponse, RpcNotification, METHOD_NOT_FOUND};
use document::DocumentStore;

pub struct LanguageServer {
    documents: DocumentStore,
    shutting_down: bool,
}

impl LanguageServer {
    pub fn new() -> Self {
        LanguageServer {
            documents: DocumentStore::new(),
            shutting_down: false,
        }
    }

    /// Main loop: read messages from `reader`, write responses/notifications
    /// to `writer`, until the client sends `exit`.
    pub fn run<R: BufRead, W: Write>(&mut self, reader: &mut R, writer: &mut W) -> std::io::Result<()> {
        while let Some(message) = read_message(reader)? {
            if message.method == "exit" {
                break;
            }

            let response = self.handle_message(&message);
            if let Some(resp) = response {
                write_message(writer, &resp)?;
            }

            // After a change, proactively push fresh diagnostics for the
            // relevant document (this is a NOTIFICATION, sent alongside
            // any request/response above — LSP allows multiple messages
            // per client message when appropriate)
            if matches!(message.method.as_str(), "textDocument/didOpen" | "textDocument/didChange") {
                if let Some(notif) = self.build_diagnostics_notification(&message) {
                    write_message(writer, &notif)?;
                }
            }
        }
        Ok(())
    }

    /// Dispatch one message to the correct handler. Returns Some(response)
    /// for requests (which have an `id`), None for notifications.
    fn handle_message(&mut self, message: &protocol::RpcRequest) -> Option<RpcResponse> {
        let id = message.id.clone();

        match message.method.as_str() {
            "initialize" => id.map(|id| RpcResponse::success(id, self.capabilities())),

            "initialized" => None, // notification, no response

            "shutdown" => {
                self.shutting_down = true;
                id.map(|id| RpcResponse::success(id, Json::Null))
            }

            "textDocument/didOpen" => {
                self.on_did_open(&message.params);
                None
            }

            "textDocument/didChange" => {
                self.on_did_change(&message.params);
                None
            }

            "textDocument/didClose" => {
                self.on_did_close(&message.params);
                None
            }

            "textDocument/hover" => {
                let result = hover::handle_hover(&self.documents, &message.params);
                id.map(|id| RpcResponse::success(id, result))
            }

            "textDocument/completion" => {
                let result = completion::handle_completion(&self.documents, &message.params);
                id.map(|id| RpcResponse::success(id, result))
            }

            "textDocument/definition" => {
                let result = definition::handle_definition(&self.documents, &message.params);
                id.map(|id| RpcResponse::success(id, result))
            }

            unknown => id.map(|id| RpcResponse::error(
                id, METHOD_NOT_FOUND, format!("method not supported: {}", unknown)
            )),
        }
    }

    /// What the server advertises it can do — sent once, in response to `initialize`
    fn capabilities(&self) -> Json {
        json!({
            "capabilities": {
                "textDocumentSync": 1, // 1 = Full sync (whole document sent on every change)
                "hoverProvider": true,
                "completionProvider": { "triggerCharacters": ["."] },
                "definitionProvider": true,
            },
            "serverInfo": { "name": "lyz-analyzer", "version": "0.1.0" }
        })
    }

    fn on_did_open(&mut self, params: &Json) {
        if let (Some(uri), Some(text), Some(version)) = (
            params["textDocument"]["uri"].as_str(),
            params["textDocument"]["text"].as_str(),
            params["textDocument"]["version"].as_i64(),
        ) {
            self.documents.open(uri.to_string(), text.to_string(), version);
        }
    }

    fn on_did_change(&mut self, params: &Json) {
        let uri = params["textDocument"]["uri"].as_str();
        let version = params["textDocument"]["version"].as_i64();
        // Full-sync mode: contentChanges[0].text is the ENTIRE new document
        let new_text = params["contentChanges"][0]["text"].as_str();

        if let (Some(uri), Some(text), Some(version)) = (uri, new_text, version) {
            self.documents.update(uri, text.to_string(), version);
        }
    }

    fn on_did_close(&mut self, params: &Json) {
        if let Some(uri) = params["textDocument"]["uri"].as_str() {
            self.documents.close(uri);
        }
    }

    fn build_diagnostics_notification(&self, message: &protocol::RpcRequest) -> Option<RpcNotification> {
        let uri = message.params["textDocument"]["uri"].as_str()?;
        let doc = self.documents.get(uri)?;
        let diags = diagnostics::to_lsp_diagnostics(&doc.analysis.diagnostics);
        Some(RpcNotification::new("textDocument/publishDiagnostics", json!({
            "uri": uri,
            "diagnostics": diags,
        })))
    }
}

impl Default for LanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use protocol::RpcRequest;
    use serde_json::json;

    fn make_request(id: Option<i64>, method: &str, params: Json) -> RpcRequest {
        RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.map(|i| json!(i)),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn test_initialize_returns_capabilities() {
        let mut server = LanguageServer::new();
        let req = make_request(Some(1), "initialize", json!({}));
        let resp = server.handle_message(&req).unwrap();
        let result = resp.result.unwrap();
        assert!(result["capabilities"]["hoverProvider"].as_bool().unwrap());
    }

    #[test]
    fn test_initialized_notification_no_response() {
        let mut server = LanguageServer::new();
        let req = make_request(None, "initialized", json!({}));
        assert!(server.handle_message(&req).is_none());
    }

    #[test]
    fn test_shutdown_sets_flag_and_responds() {
        let mut server = LanguageServer::new();
        let req = make_request(Some(2), "shutdown", json!({}));
        let resp = server.handle_message(&req);
        assert!(resp.is_some());
        assert!(server.shutting_down);
    }

    #[test]
    fn test_unknown_method_returns_error() {
        let mut server = LanguageServer::new();
        let req = make_request(Some(3), "totally/madeUp", json!({}));
        let resp = server.handle_message(&req).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, protocol::METHOD_NOT_FOUND);
    }

    #[test]
    fn test_did_open_registers_document() {
        let mut server = LanguageServer::new();
        let req = make_request(None, "textDocument/didOpen", json!({
            "textDocument": { "uri": "file:///t.lyz", "text": "fn f() {}", "version": 1 }
        }));
        server.handle_message(&req);
        assert!(server.documents.get("file:///t.lyz").is_some());
    }

    #[test]
    fn test_did_change_updates_document() {
        let mut server = LanguageServer::new();
        server.handle_message(&make_request(None, "textDocument/didOpen", json!({
            "textDocument": { "uri": "file:///t.lyz", "text": "let x = undeclared", "version": 1 }
        })));
        server.handle_message(&make_request(None, "textDocument/didChange", json!({
            "textDocument": { "uri": "file:///t.lyz", "version": 2 },
            "contentChanges": [{ "text": "let x = 42" }]
        })));
        let doc = server.documents.get("file:///t.lyz").unwrap();
        assert_eq!(doc.version, 2);
        assert!(doc.analysis.diagnostics.is_empty());
    }

    #[test]
    fn test_did_close_removes_document() {
        let mut server = LanguageServer::new();
        server.handle_message(&make_request(None, "textDocument/didOpen", json!({
            "textDocument": { "uri": "file:///t.lyz", "text": "fn f() {}", "version": 1 }
        })));
        server.handle_message(&make_request(None, "textDocument/didClose", json!({
            "textDocument": { "uri": "file:///t.lyz" }
        })));
        assert!(server.documents.get("file:///t.lyz").is_none());
    }

    #[test]
    fn test_full_run_loop_processes_and_exits() {
        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
        let input = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            init.len(), init, exit.len(), exit
        );
        let mut reader = std::io::BufReader::new(input.as_bytes());
        let mut output = Vec::new();
        let mut server = LanguageServer::new();
        server.run(&mut reader, &mut output).unwrap();
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains("capabilities"));
    }
}
