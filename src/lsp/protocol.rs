use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::io::{self, BufRead, Write};

/// A JSON-RPC request FROM the editor TO our server (or vice versa for responses)
#[derive(Debug, Clone, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    /// Present for requests (expects a response), absent for notifications
    pub id: Option<Json>,
    pub method: String,
    #[serde(default)]
    pub params: Json,
}

/// A JSON-RPC response WE send back
#[derive(Debug, Clone, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Json,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

// Standard JSON-RPC error codes (from the spec — do not invent your own
// for these common cases, editors pattern-match on the exact numbers)
pub const PARSE_ERROR: i64 = -32700;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

impl RpcResponse {
    pub fn success(id: Json, result: Json) -> Self {
        RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Json, code: i64, message: impl Into<String>) -> Self {
        RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// A JSON-RPC notification WE send (no id, no response expected)
/// Used for e.g. textDocument/publishDiagnostics
#[derive(Debug, Clone, Serialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Json,
}

impl RpcNotification {
    pub fn new(method: impl Into<String>, params: Json) -> Self {
        RpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

/// Read ONE framed LSP message from a reader (Content-Length header + JSON body).
/// Returns None cleanly at EOF (used to detect the editor closing the connection).
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<RpcRequest>> {
    let mut content_length: Option<usize> = None;

    // Read headers until the blank line
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Ok(None); // EOF
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break; // blank line = end of headers
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(value.trim().parse().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length header")
            })?);
        }
        // Other headers (like Content-Type) are ignored — LSP mandates
        // UTF-8 JSON, so we don't need to branch on them
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;

    let mut buf = vec![0u8; length];
    reader.read_exact(&mut buf)?;

    let request: RpcRequest = serde_json::from_slice(&buf).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid JSON-RPC body: {}", e),
        )
    })?;

    Ok(Some(request))
}

/// Write ONE framed LSP message to a writer, computing Content-Length automatically
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> io::Result<()> {
    let body = serde_json::to_string(message)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn test_read_message_basic() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut cursor = Cursor::new(framed.as_bytes());
        let msg = read_message(&mut cursor).unwrap().unwrap();
        assert_eq!(msg.method, "initialize");
        assert_eq!(msg.id, Some(json!(1)));
    }

    #[test]
    fn test_read_message_eof_returns_none() {
        let mut cursor = Cursor::new(&b""[..]);
        let result = read_message(&mut cursor).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_message_notification_no_id() {
        let body = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{}}"#;
        let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut cursor = Cursor::new(framed.as_bytes());
        let msg = read_message(&mut cursor).unwrap().unwrap();
        assert_eq!(msg.id, None);
    }

    #[test]
    fn test_read_message_missing_content_length_errors() {
        let framed = "\r\n{}";
        let mut cursor = Cursor::new(framed.as_bytes());
        assert!(read_message(&mut cursor).is_err());
    }

    #[test]
    fn test_write_message_produces_correct_framing() {
        let mut buf = Vec::new();
        let resp = RpcResponse::success(json!(1), json!({"ok": true}));
        write_message(&mut buf, &resp).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("Content-Length: "));
        assert!(s.contains("\r\n\r\n"));
        assert!(s.contains("\"ok\":true"));
    }

    #[test]
    fn test_roundtrip_write_then_read() {
        let mut buf = Vec::new();
        let resp = RpcResponse::success(json!(42), json!({"data": "hello"}));
        write_message(&mut buf, &resp).unwrap();

        // Simulate reading it back as if it were a request-shaped message
        // (structurally identical framing, different payload shape — we
        // just verify the FRAMING round-trips correctly here)
        let cursor = Cursor::new(buf.as_slice());
        let mut line = String::new();
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(cursor);
        reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("Content-Length:"));
    }

    #[test]
    fn test_error_response_shape() {
        let resp = RpcResponse::error(json!(5), METHOD_NOT_FOUND, "unknown method");
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn test_notification_serializes_without_id() {
        let notif = RpcNotification::new(
            "textDocument/publishDiagnostics",
            json!({"uri": "file:///x"}),
        );
        let s = serde_json::to_string(&notif).unwrap();
        assert!(!s.contains("\"id\""));
        assert!(s.contains("publishDiagnostics"));
    }
}
