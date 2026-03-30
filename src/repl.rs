use std::time::Instant;

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::Editor;

use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::normalizer::normalize;
use crate::value::Value;
use crate::error::MoofError;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HISTORY_FILE: &str = ".moof_history";

const TIPS: &[&str] = &[
    "Try [\"hello\" uppercase] to send a message to a string",
    "Use ,help for a list of REPL commands",
    "Use (define x 42) to bind a value, then just type x",
    "_ always holds the last result",
    "Classes are open! Re-evaluate (class Foo ...) to add methods anytime",
    "(+ 1 2) calls the + function with two arguments",
    "Use (-> value step1 step2) for pipelines",
    "Use (match val (pattern body) ...) for pattern matching",
    "Use Ctrl-D to exit the REPL",
    "(map f lst), (filter pred lst), (reduce f init lst) -- your functional toolkit",
];

const STDLIB: &str = include_str!("../stdlib/stdlib.moof");

/// Run the interactive REPL.
pub fn run_repl() {
    let mut rl: Editor<(), FileHistory> = Editor::new().expect("Failed to initialize line editor");

    let history_path = dirs_or_home().join(HISTORY_FILE);
    let history_str = history_path.to_string_lossy().to_string();
    let _ = rl.load_history(&history_str);

    let mut interp = Interpreter::new();
    load_stdlib(&mut interp);

    // Define _ as nil initially
    interp.env.define("_", Value::Nil, true);

    let start_time = Instant::now();
    let mut eval_count: usize = 0;

    print_banner();

    let mut buffer = String::new();

    loop {
        let prompt = if buffer.is_empty() {
            "\x1b[1m\x1b[36mmoof> \x1b[0m"
        } else {
            "\x1b[1m\x1b[36m ...> \x1b[0m"
        };

        match rl.readline(prompt) {
            Ok(line) => {
                let stripped = line.trim();

                // Skip empty lines when no buffer
                if stripped.is_empty() && buffer.is_empty() {
                    continue;
                }

                // Meta commands -- only when not accumulating multi-line
                if buffer.is_empty() && stripped.starts_with(',') {
                    let _ = rl.add_history_entry(&line);
                    handle_meta_command(stripped, &mut interp);
                    continue;
                }

                let _ = rl.add_history_entry(&line);

                buffer.push_str(&line);
                buffer.push('\n');

                if !is_balanced(&buffer) {
                    continue;
                }

                let input = buffer.trim().to_string();
                buffer.clear();

                if input.is_empty() {
                    continue;
                }

                match evaluate_input(&input, &mut interp) {
                    Ok(result) => {
                        eval_count += 1;
                        // Store in _
                        interp.env.set("_", result.clone()).ok();

                        // Print result with type hint
                        let inspected = result.inspect();
                        let type_name = result.type_name();
                        println!(
                            "\x1b[32m=> \x1b[0m{}\x1b[2m : {}\x1b[0m",
                            inspected, type_name
                        );
                    }
                    Err(e) => {
                        print_error(&e, &input);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C: clear buffer
                if !buffer.is_empty() {
                    buffer.clear();
                    println!("\x1b[2m(input cleared)\x1b[0m");
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D: exit
                println!();
                break;
            }
            Err(err) => {
                eprintln!("\x1b[31mReadline error: {err}\x1b[0m");
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_str);

    // Farewell
    let elapsed = start_time.elapsed().as_secs_f64();
    let plural = if eval_count == 1 { "" } else { "s" };
    println!(
        "\x1b[2mSession: {eval_count} expression{plural} in {}. Goodbye!\x1b[0m",
        format_duration(elapsed)
    );
}

/// Evaluate a source string, returning the result or error.
pub fn evaluate_source(source: &str, interp: &mut Interpreter) -> Result<Value, MoofError> {
    evaluate_input(source, interp)
}

// ── Internal helpers ──────────────────────────────────────────────

fn dirs_or_home() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
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

fn print_banner() {
    let tip_idx = {
        // Simple deterministic "random" based on time
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        (secs as usize) % TIPS.len()
    };

    println!();
    println!("\x1b[1m\x1b[36m  Moof {VERSION}\x1b[0m");
    println!("\x1b[2m  A Lisp with Smalltalk-style message passing\x1b[0m");
    println!();
    println!("\x1b[2m  Type ,help for commands, Ctrl-D to exit\x1b[0m");
    println!("\x1b[2m  Tip: {}\x1b[0m", TIPS[tip_idx]);
    println!();
}

fn print_error(e: &MoofError, source: &str) {
    // Error kind label
    let kind_label = match e.kind {
        crate::error::ErrorKind::Syntax => "SyntaxError",
        crate::error::ErrorKind::Runtime => "RuntimeError",
        crate::error::ErrorKind::Name => "NameError",
        crate::error::ErrorKind::Message => "MessageError",
        crate::error::ErrorKind::Arity => "ArityError",
        crate::error::ErrorKind::ImmutableBinding => "ImmutableError",
        crate::error::ErrorKind::Type => "TypeError",
    };

    eprintln!("\x1b[31m{kind_label}: {}\x1b[0m", e.message);

    // Show source context if we have a line number
    if let Some(line_num) = e.line {
        let lines: Vec<&str> = source.lines().collect();
        if line_num > 0 && line_num <= lines.len() {
            let line_text = lines[line_num - 1];
            eprintln!("\x1b[2m  {line_num} | {line_text}\x1b[0m");

            if let Some(col) = e.column {
                if col > 0 {
                    let padding = " ".repeat(col - 1 + format!("{line_num}").len() + 4);
                    eprintln!("\x1b[31m{padding}^\x1b[0m");
                }
            }
        }
    }
}

fn evaluate_input(input: &str, interp: &mut Interpreter) -> Result<Value, MoofError> {
    let tokens = Lexer::new(input).tokenize()?;
    let program = Parser::new(tokens).parse_program()?;
    let normalized = normalize(program);
    interp.evaluate(&normalized)
}

fn handle_meta_command(input: &str, interp: &mut Interpreter) {
    let parts: Vec<&str> = input.splitn(2, char::is_whitespace).collect();
    let cmd = parts[0];
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd {
        ",help" | ",h" => {
            println!("\x1b[1mREPL Commands:\x1b[0m");
            println!("  \x1b[36m,help\x1b[0m       Show this help");
            println!("  \x1b[36m,quit\x1b[0m       Exit the REPL");
            println!("  \x1b[36m,exit\x1b[0m       Exit the REPL");
            println!("  \x1b[36m,version\x1b[0m    Show Moof version");
            println!("  \x1b[36m,env\x1b[0m        Show current environment bindings");
            println!("  \x1b[36m,type <expr>\x1b[0m Show the type of an expression");
            println!("  \x1b[36m,doc <name>\x1b[0m  Show documentation for a binding");
            println!("  \x1b[36m,classes\x1b[0m    List defined classes");
        }
        ",quit" | ",exit" | ",q" => {
            std::process::exit(0);
        }
        ",version" | ",v" => {
            println!("\x1b[1m\x1b[36mMoof {VERSION}\x1b[0m");
        }
        ",env" | ",e" => {
            println!("\x1b[2m(environment listing not yet implemented)\x1b[0m");
        }
        ",type" | ",t" => {
            if arg.is_empty() {
                println!("\x1b[2mUsage: ,type <expr>\x1b[0m");
            } else {
                match evaluate_input(arg, interp) {
                    Ok(val) => {
                        println!("\x1b[36m{}\x1b[0m", val.type_name());
                    }
                    Err(e) => print_error(&e, arg),
                }
            }
        }
        ",doc" | ",d" => {
            if arg.is_empty() {
                println!("\x1b[2mUsage: ,doc <name>\x1b[0m");
            } else {
                match interp.env.get(arg) {
                    Ok(val) => {
                        println!("\x1b[1m{arg}\x1b[0m : \x1b[36m{}\x1b[0m", val.type_name());
                        println!("  {}", val);
                    }
                    Err(_) => {
                        println!("\x1b[2mNo binding found for '{arg}'\x1b[0m");
                    }
                }
            }
        }
        ",classes" => {
            println!("\x1b[2m(class listing not yet implemented)\x1b[0m");
        }
        _ => {
            println!("\x1b[33mUnknown command: {cmd}\x1b[0m");
            println!("\x1b[2mType ,help for available commands\x1b[0m");
        }
    }
}

/// Check whether the input has balanced delimiters.
/// Returns true if the input is complete (balanced or over-closed).
fn is_balanced(source: &str) -> bool {
    let mut depth_paren: i32 = 0;
    let mut depth_bracket: i32 = 0;
    let mut depth_brace: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut block_depth: i32 = 0;

    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        let nch = chars.get(i + 1).copied();

        if in_block_comment {
            if ch == '#' && nch == Some('|') {
                block_depth += 1;
                i += 2;
                continue;
            } else if ch == '|' && nch == Some('#') {
                block_depth -= 1;
                if block_depth == 0 {
                    in_block_comment = false;
                }
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if escape {
            escape = false;
            i += 1;
            continue;
        }

        if in_string {
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }

        match ch {
            ';' => in_line_comment = true,
            '#' if nch == Some('|') => {
                in_block_comment = true;
                block_depth = 1;
                i += 2;
                continue;
            }
            '"' => in_string = true,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            _ => {}
        }
        i += 1;
    }

    // Still in string or block comment means not balanced
    if in_string || in_block_comment {
        return false;
    }

    depth_paren <= 0 && depth_bracket <= 0 && depth_brace <= 0
}

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{:.1}s", seconds)
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}
