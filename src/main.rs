use std::env;
use std::process;

use lyzard::lexer::Lexer;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 || args[1] != "run" {
        eprintln!("usage: lyzard run <file.lyz>");
        process::exit(2);
    }

    let path = &args[2];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("error: could not read '{}': {}", path, err);
            process::exit(2);
        }
    };

    match Lexer::tokenize(&source, path) {
        Ok(tokens) => {
            for token in &tokens {
                println!(
                    "{:>4}:{:<3} {:>3} bytes  {:?}",
                    token.span.line,
                    token.span.col,
                    token.span.len(),
                    token.kind
                );
            }
            println!("{} tokens", tokens.len());
        }
        Err(err) => {
            eprint!("{}", err.format(&source));
            process::exit(1);
        }
    }
}
