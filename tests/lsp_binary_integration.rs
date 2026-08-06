use std::io::Write;
use std::process::{Command, Stdio};

fn framed(msg: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg).into_bytes()
}

#[test]
fn test_lyz_analyzer_handles_initialize_and_exit() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lyz-analyzer"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lyz-analyzer");

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(&framed(init)).unwrap();
        stdin.write_all(&framed(exit)).unwrap();
        stdin.flush().unwrap();
    }

    let output = child.wait_with_output().expect("wait for server");
    assert!(
        output.status.success(),
        "server exited with {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("capabilities"),
        "missing capabilities response: {}",
        stdout
    );
    assert!(
        stdout.contains("hoverProvider"),
        "missing hoverProvider: {}",
        stdout
    );
}

#[test]
fn test_lyz_analyzer_publishes_diagnostics_on_open() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lyz-analyzer"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lyz-analyzer");

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    let did_open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///t.lyz","text":"fn main() { let x = undeclared }","version":1}}}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(&framed(init)).unwrap();
        stdin.write_all(&framed(did_open)).unwrap();
        stdin.write_all(&framed(exit)).unwrap();
        stdin.flush().unwrap();
    }

    let output = child.wait_with_output().expect("wait for server");
    assert!(
        output.status.success(),
        "server exited with {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("publishDiagnostics"),
        "missing publishDiagnostics notification: {}",
        stdout
    );
    assert!(
        stdout.contains("undeclared"),
        "diagnostics should mention the undefined variable: {}",
        stdout
    );
}
