use std::io;

use lyzard::lsp::LanguageServer;

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = LanguageServer::new();
    server.run(&mut stdin.lock(), &mut stdout.lock())
}
