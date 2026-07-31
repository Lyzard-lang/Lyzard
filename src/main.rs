use std::env;
use std::process;

use lyzard::analyzer::Analyzer;
use lyzard::interpreter::Interpreter;
use lyzard::lexer::Lexer;
use lyzard::parser::Parser;

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

    let tokens = match Lexer::tokenize(&source, path) {
        Ok(tokens) => tokens,
        Err(err) => {
            eprint!("{}", err.format(&source));
            process::exit(1);
        }
    };

    let (program, parse_errs) = match Parser::new(tokens, path, &source).parse() {
        Ok(result) => result,
        Err(err) => {
            eprint!("{}", err.format(&source));
            process::exit(1);
        }
    };
    if !parse_errs.is_empty() {
        eprint!("{}", parse_errs.format_all(&source));
        process::exit(1);
    }

    let (semantic_errs, _) = Analyzer::new(&source, path).analyze(&program);
    if !semantic_errs.is_empty() {
        eprint!("{}", semantic_errs.format_all(&source));
        process::exit(1);
    }

    match Interpreter::new().run(&program) {
        Ok(()) => {}
        Err(err) => {
            eprint!("{}", err.format(&source));
            process::exit(1);
        }
    }
}
