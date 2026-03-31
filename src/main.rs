use moof::interpreter::Interpreter;
use moof::lexer::Lexer;
use moof::parser::Parser;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STDLIB: &str = include_str!("../stdlib/stdlib.moof");

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 1 {
        // REPL mode
        moof::repl::run_repl();
    } else if args[1] == "-v" || args[1] == "--version" {
        println!("Moof {VERSION}");
    } else if args[1] == "-h" || args[1] == "--help" {
        print_usage();
    } else if args[1] == "-e" {
        // Eval mode
        if args.len() < 3 {
            eprintln!("Error: -e requires an argument");
            std::process::exit(1);
        }
        run_source(&args[2], "<eval>", true);
    } else {
        // File mode
        let path = &args[1];
        match std::fs::read_to_string(path) {
            Ok(source) => run_source(&source, path, false),
            Err(e) => {
                eprintln!("Error: Cannot read '{}': {}", path, e);
                std::process::exit(1);
            }
        }
    }
}

fn print_usage() {
    println!("Usage: moof [options] [file.moof]");
    println!();
    println!("Options:");
    println!("  -e <expr>     Evaluate expression and print result");
    println!("  -v, --version Print version");
    println!("  -h, --help    Print this help");
    println!();
    println!("Examples:");
    println!("  moof                  Start the REPL");
    println!("  moof script.moof      Run a file");
    println!("  moof -e '(+ 1 2)'     Evaluate and print");
}

fn run_source(source: &str, filename: &str, print_result: bool) {
    let mut interp = Interpreter::new();
    load_stdlib(&mut interp);

    match interp.load_source(source, filename) {
        Ok(result) => {
            if print_result {
                match result {
                    moof::value::Value::Nil => {} // don't print nil for -e
                    val => println!("{}", val.inspect()),
                }
            }
        }
        Err(e) => {
            let kind_label = match e.kind {
                moof::error::ErrorKind::Syntax => "SyntaxError",
                moof::error::ErrorKind::Runtime => "RuntimeError",
                moof::error::ErrorKind::Name => "NameError",
                moof::error::ErrorKind::Message => "MessageError",
                moof::error::ErrorKind::Arity => "ArityError",
                moof::error::ErrorKind::Type => "TypeError",
                moof::error::ErrorKind::IO => "IOError",
            };

            eprintln!("\x1b[31m{kind_label}: {}\x1b[0m", e.message);

            if let Some(line_num) = e.line {
                let lines: Vec<&str> = source.lines().collect();
                if line_num > 0 && line_num <= lines.len() {
                    eprintln!(
                        "\x1b[2m  {} | {}\x1b[0m",
                        line_num,
                        lines[line_num - 1]
                    );
                    if let Some(col) = e.column {
                        if col > 0 {
                            let padding =
                                " ".repeat(col - 1 + format!("{line_num}").len() + 4);
                            eprintln!("\x1b[31m{padding}^\x1b[0m");
                        }
                    }
                }
            }

            std::process::exit(1);
        }
    }
}

fn load_stdlib(interp: &mut Interpreter) {
    if !STDLIB.trim().is_empty() {
        if let Err(e) = interp.load_source(STDLIB, "<stdlib>") {
            eprintln!(
                "\x1b[33mWarning: failed to load stdlib: {}\x1b[0m",
                e.message
            );
        }
    }
}
