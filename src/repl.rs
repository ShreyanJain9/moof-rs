use std::rc::Rc;
use std::time::Instant;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::FileHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::error::{ErrorKind, MoofError};
use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::moofint::MoofInt;
use crate::parser::Parser;
use crate::value::Value;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_FILE: &str = ".moof_history";
const MAX_HISTORY: usize = 1000;

const STDLIB: &str = include_str!("../stdlib/stdlib.moof");

const TIPS: &[&str] = &[
    "Try [\"hello\" uppercase] to send a message to a string",
    "Use ,help for a list of REPL commands",
    "Use ,doc map to see info about any binding",
    "_ always holds the last result",
    "Classes are open! Re-evaluate (class Foo ...) to add methods anytime",
    "Use ,methods String to see what messages a type responds to",
    "Use (-> value [step1] [step2]) for pipelines",
    "Use (match val (pattern body) ...) for pattern matching",
    "Use ,time (fib 30) to benchmark an expression",
    "(map f lst), (filter pred lst), (reduce f init lst) -- your functional toolkit",
];

const KEYWORDS: &[&str] = &[
    "define", "lambda", "fn", "if", "let", "do", "set!", "quote", "try", "catch",
    "cond", "and", "or", "class", "trait", "match", "type", "protocol",
    "extend", "defmacro", "module", "use", "require", "->",
];

const META_COMMANDS: &[&str] = &[
    ",help", ",quit", ",exit", ",version", ",env", ",type", ",doc",
    ",methods", ",classes", ",load", ",time", ",clear", ",reset",
];

const COMMON_SELECTORS: &[&str] = &[
    "length", "first", "last", "rest", "reverse", "sort", "flatten", "empty?",
    "map:", "filter:", "reduce:init:", "each:", "any:", "all:", "none:", "sortBy:",
    "push:", "prepend:", "at:", "contains:", "join:", "take:", "drop:", "zip:",
    "uppercase", "lowercase", "trim", "chars", "split:", "concat:",
    "to_s", "to_i", "to_f", "abs", "nil?", "class", "not",
    "pow:", "max:", "min:", "sqrt",
];

// ── Completion ──────────────────────────────────────────────────────

struct MoofHelper {
    completions: Vec<String>,
}

impl MoofHelper {
    fn new() -> Self {
        let mut completions = Vec::new();
        for kw in KEYWORDS {
            completions.push(kw.to_string());
        }
        for mc in META_COMMANDS {
            completions.push(mc.to_string());
        }
        for sel in COMMON_SELECTORS {
            completions.push(sel.to_string());
        }
        MoofHelper { completions }
    }

    fn refresh(&mut self, interp: &Interpreter) {
        self.completions.clear();
        for kw in KEYWORDS {
            self.completions.push(kw.to_string());
        }
        for mc in META_COMMANDS {
            self.completions.push(mc.to_string());
        }
        for sel in COMMON_SELECTORS {
            self.completions.push(sel.to_string());
        }
        // Add environment bindings (resolve SymIds to names)
        for (id, _) in interp.global_env.bindings() {
            if let Some(name) = interp.symbols.try_name(id) {
                let s = name.to_string();
                if !self.completions.contains(&s) {
                    self.completions.push(s);
                }
            }
        }
    }
}

impl Completer for MoofHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let start = line[..pos]
            .rfind(|c: char| c.is_whitespace() || "()[]{}".contains(c))
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &line[start..pos];

        if prefix.is_empty() {
            return Ok((start, vec![]));
        }

        let mut matches: Vec<Pair> = self
            .completions
            .iter()
            .filter(|c| c.starts_with(prefix))
            .map(|c| Pair {
                display: c.clone(),
                replacement: c.clone(),
            })
            .collect();
        matches.sort_by(|a, b| a.display.cmp(&b.display));
        matches.dedup_by(|a, b| a.display == b.display);

        Ok((start, matches))
    }
}

impl Hinter for MoofHelper {
    type Hint = String;
}
impl Highlighter for MoofHelper {}
impl Validator for MoofHelper {}
impl Helper for MoofHelper {}

// ── REPL ────────────────────────────────────────────────────────────

pub fn run_repl() {
    let helper = MoofHelper::new();
    let mut rl: Editor<MoofHelper, FileHistory> = Editor::with_config(
        rustyline::Config::builder()
            .max_history_size(MAX_HISTORY)
            .unwrap()
            .build(),
    )
    .expect("Failed to initialize line editor");
    rl.set_helper(Some(helper));

    let history_path = dirs_or_home().join(HISTORY_FILE);
    let history_str = history_path.to_string_lossy().to_string();
    let _ = rl.load_history(&history_str);

    let mut interp = Interpreter::new();
    load_stdlib(&mut interp);

    // Define _ as mutable last-result holder
    let underscore = interp.symbols.intern("_");
    interp.global_env.define(underscore, Value::Nil, true);

    // Refresh completions after stdlib load
    if let Some(h) = rl.helper_mut() {
        h.refresh(&interp);
    }

    let start_time = Instant::now();
    let mut eval_count: usize = 0;

    print_banner();

    let mut buffer = String::new();
    let mut last_line = String::new();

    loop {
        let prompt = if buffer.is_empty() {
            "\x1b[1m\x1b[36mmoof> \x1b[0m"
        } else {
            "\x1b[1m\x1b[36m ...> \x1b[0m"
        };

        match rl.readline(prompt) {
            Ok(line) => {
                let stripped = line.trim();

                if stripped.is_empty() && buffer.is_empty() {
                    continue;
                }

                // Dedup consecutive history entries
                if line != last_line {
                    let _ = rl.add_history_entry(&line);
                    last_line = line.clone();
                }

                // Meta commands only when not accumulating
                if buffer.is_empty() && stripped.starts_with(',') {
                    handle_meta_command(stripped, &mut interp);
                    if let Some(h) = rl.helper_mut() {
                        h.refresh(&interp);
                    }
                    continue;
                }

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

                let eval_start = Instant::now();
                match evaluate_input(&input, &mut interp) {
                    Ok(result) => {
                        eval_count += 1;
                        let _ = interp.global_env.set(underscore, result.clone());

                        let elapsed = eval_start.elapsed().as_secs_f64();
                        let inspected = result.inspect();
                        let type_name = result.type_name();

                        if elapsed >= 0.1 {
                            println!(
                                "\x1b[32m=> \x1b[0m{}\x1b[2m : {} [{}]\x1b[0m",
                                inspected,
                                type_name,
                                format_short_time(elapsed)
                            );
                        } else {
                            println!(
                                "\x1b[32m=> \x1b[0m{}\x1b[2m : {}\x1b[0m",
                                inspected, type_name
                            );
                        }
                    }
                    Err(e) => {
                        print_error(&e, &input);
                    }
                }

                // Refresh completions after eval
                if let Some(h) = rl.helper_mut() {
                    h.refresh(&interp);
                }
            }
            Err(ReadlineError::Interrupted) => {
                if !buffer.is_empty() {
                    buffer.clear();
                    println!("\x1b[2m(input cleared)\x1b[0m");
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(err) => {
                eprintln!("\x1b[31mReadline error: {err}\x1b[0m");
                break;
            }
        }
    }

    let _ = rl.save_history(&history_str);

    let elapsed = start_time.elapsed().as_secs_f64();
    let plural = if eval_count == 1 { "" } else { "s" };
    println!(
        "\x1b[2mSession: {eval_count} expression{plural} in {}. Goodbye!\x1b[0m",
        format_duration(elapsed)
    );
}

// ── Public evaluate API ─────────────────────────────────────────────

pub fn evaluate_source(source: &str, interp: &mut Interpreter) -> Result<Value, MoofError> {
    evaluate_input(source, interp)
}

// ── Meta Commands ───────────────────────────────────────────────────

fn handle_meta_command(input: &str, interp: &mut Interpreter) {
    let parts: Vec<&str> = input.splitn(2, char::is_whitespace).collect();
    let cmd = parts[0];
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd {
        ",help" | ",h" => cmd_help(),
        ",quit" | ",exit" | ",q" => std::process::exit(0),
        ",version" | ",v" => println!("\x1b[1m\x1b[36mMoof {VERSION}\x1b[0m"),
        ",env" | ",e" => cmd_env(interp, arg),
        ",type" | ",t" => cmd_type(interp, arg),
        ",doc" | ",d" => cmd_doc(interp, arg),
        ",methods" | ",m" => cmd_methods(interp, arg),
        ",classes" => cmd_classes(interp),
        ",load" => cmd_load(interp, arg),
        ",time" => cmd_time(interp, arg),
        ",clear" => print!("\x1b[2J\x1b[H"),
        ",reset" => cmd_reset(interp),
        _ => {
            println!("\x1b[33mUnknown command: {cmd}\x1b[0m");
            println!("\x1b[2mType ,help for available commands\x1b[0m");
        }
    }
}

fn cmd_help() {
    println!("\x1b[1mREPL Commands:\x1b[0m");
    println!();
    println!("  \x1b[36m,help\x1b[0m  \x1b[36m,h\x1b[0m       Show this help");
    println!("  \x1b[36m,env\x1b[0m   \x1b[36m,e\x1b[0m       Show environment bindings (,env filter)");
    println!("  \x1b[36m,type\x1b[0m  \x1b[36m,t\x1b[0m       Show the type of an expression");
    println!("  \x1b[36m,doc\x1b[0m   \x1b[36m,d\x1b[0m       Show documentation for a binding");
    println!("  \x1b[36m,methods\x1b[0m \x1b[36m,m\x1b[0m     List methods for a class");
    println!("  \x1b[36m,classes\x1b[0m         List all defined classes");
    println!("  \x1b[36m,load\x1b[0m            Load a .moof file");
    println!("  \x1b[36m,time\x1b[0m            Benchmark an expression");
    println!("  \x1b[36m,clear\x1b[0m           Clear the screen");
    println!("  \x1b[36m,reset\x1b[0m           Reset the interpreter");
    println!("  \x1b[36m,version\x1b[0m \x1b[36m,v\x1b[0m     Show Moof version");
    println!("  \x1b[36m,quit\x1b[0m  \x1b[36m,q\x1b[0m       Exit the REPL");
    println!();
    println!("\x1b[1mKeyboard shortcuts:\x1b[0m");
    println!("  \x1b[2mCtrl-D\x1b[0m   Exit    \x1b[2mCtrl-C\x1b[0m   Cancel input");
    println!("  \x1b[2mTab\x1b[0m      Complete    \x1b[2mUp/Down\x1b[0m  History");
}

fn cmd_env(interp: &Interpreter, filter: &str) {
    let bindings = interp.global_env.bindings();
    let mut entries: Vec<(String, Value)> = bindings
        .into_iter()
        .filter_map(|(id, val)| {
            let name = interp.symbols.try_name(id)?.to_string();
            Some((name, val))
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut functions = Vec::new();
    let mut values = Vec::new();

    for (name, val) in &entries {
        if name.starts_with("__") {
            continue;
        }
        if !filter.is_empty() && !name.contains(filter) {
            continue;
        }

        match val {
            Value::Closure(_) => functions.push(name.clone()),
            _ => values.push((name.clone(), val.clone())),
        }
    }

    if !values.is_empty() {
        println!("\x1b[1mValues:\x1b[0m");
        for (name, val) in &values {
            println!(
                "  \x1b[36m{name}\x1b[0m = {}\x1b[2m : {}\x1b[0m",
                val.inspect(),
                val.type_name()
            );
        }
    }
    if !functions.is_empty() {
        println!("\x1b[1mFunctions:\x1b[0m ({} total)", functions.len());
        print_columns(&functions, 4);
    }
}

fn cmd_type(interp: &mut Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,type <expr>\x1b[0m");
        return;
    }
    match evaluate_input(arg, interp) {
        Ok(val) => println!("\x1b[36m{}\x1b[0m", val.type_name()),
        Err(e) => print_error(&e, arg),
    }
}

fn cmd_doc(interp: &mut Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,doc <name>\x1b[0m");
        return;
    }

    // Try to look up by name in the global env
    let id = interp.symbols.intern(arg);
    match interp.global_env.get(id) {
        Ok(val) => {
            println!("\x1b[1m{arg}\x1b[0m : \x1b[36m{}\x1b[0m", val.type_name());
            match &val {
                Value::Closure(c) => {
                    let params: Vec<String> = c
                        .params
                        .iter()
                        .map(|&p| {
                            interp
                                .symbols
                                .try_name(p)
                                .unwrap_or("?")
                                .to_string()
                        })
                        .collect();
                    let rest = c
                        .rest_param
                        .map(|r| {
                            format!(
                                " . {}",
                                interp.symbols.try_name(r).unwrap_or("?")
                            )
                        })
                        .unwrap_or_default();
                    let name_str = c
                        .name
                        .map(|n| {
                            interp.symbols.try_name(n).unwrap_or("?").to_string()
                        })
                        .unwrap_or_else(|| arg.to_string());
                    println!("  \x1b[2m({} {}{})\x1b[0m", name_str, params.join(" "), rest);
                }
                other => {
                    println!("  {}", other.inspect());
                }
            }
        }
        Err(_) => {
            println!("\x1b[2mNothing found for '{arg}'\x1b[0m");
        }
    }
}

fn cmd_methods(interp: &mut Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,methods <ClassName>\x1b[0m");
        return;
    }

    let class_opt = interp.class_of_name(arg);
    match class_opt {
        Some(class_rc) => {
            let klass = class_rc.borrow();
            println!("\x1b[1m{arg}\x1b[0m methods:");

            let mut method_names: Vec<String> = klass
                .methods
                .keys()
                .filter_map(|&id| interp.symbols.try_name(id).map(|s| s.to_string()))
                .collect();
            method_names.sort();

            if method_names.is_empty() {
                println!("  \x1b[2m(no methods)\x1b[0m");
            } else {
                println!("  {}", method_names.join(", "));
            }

            // Walk superclass chain
            let mut sup = klass.superclass.clone();
            drop(klass);
            let mut inherited = Vec::new();
            while let Some(s) = sup {
                let sb = s.borrow();
                for &id in sb.methods.keys() {
                    if let Some(name) = interp.symbols.try_name(id) {
                        let name_s = name.to_string();
                        if !method_names.contains(&name_s) && !inherited.contains(&name_s) {
                            inherited.push(name_s);
                        }
                    }
                }
                sup = sb.superclass.clone();
            }

            if !inherited.is_empty() {
                inherited.sort();
                println!("  \x1b[2mInherited:\x1b[0m {}", inherited.join(", "));
            }
        }
        None => {
            println!("\x1b[2mNo class found: '{arg}'\x1b[0m");
        }
    }
}

fn cmd_classes(interp: &Interpreter) {
    let builtin_names = [
        "Integer", "Float", "String", "Cons", "Table", "Bool", "True", "False", "Nil", "Closure",
    ];

    let mut builtin_list = Vec::new();
    let mut user_list = Vec::new();

    for (name, class_rc) in interp.all_classes() {
        let klass = class_rc.borrow();
        let method_count = klass.methods.len();
        if builtin_names.contains(&name.as_str()) {
            if method_count > 0 {
                builtin_list.push(format!("{name} ({method_count} methods)"));
            } else {
                builtin_list.push(name);
            }
        } else {
            let field_count = klass.all_field_names().len();
            if field_count > 0 || method_count > 0 {
                user_list.push(format!("{name} ({field_count} fields, {method_count} methods)"));
            } else {
                user_list.push(name);
            }
        }
    }

    builtin_list.sort();
    user_list.sort();

    println!("\x1b[1mBuilt-in types:\x1b[0m {}", builtin_list.join(", "));
    if !user_list.is_empty() {
        println!("\x1b[1mUser classes:\x1b[0m");
        for c in &user_list {
            println!("  \x1b[36m{c}\x1b[0m");
        }
    }
}

fn cmd_load(interp: &mut Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,load <file.moof>\x1b[0m");
        return;
    }
    let path = if arg.ends_with(".moof") {
        arg.to_string()
    } else {
        format!("{arg}.moof")
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => match interp.load_source(&source, &path) {
            Ok(_) => println!("\x1b[32mLoaded {path}\x1b[0m"),
            Err(e) => print_error(&e, &source),
        },
        Err(e) => println!("\x1b[31mCannot read '{path}': {e}\x1b[0m"),
    }
}

fn cmd_time(interp: &mut Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,time <expr>\x1b[0m");
        return;
    }
    let start = Instant::now();
    match evaluate_input(arg, interp) {
        Ok(result) => {
            let elapsed = start.elapsed().as_secs_f64();
            println!(
                "\x1b[32m=> \x1b[0m{}\x1b[2m : {}\x1b[0m",
                result.inspect(),
                result.type_name()
            );
            println!("\x1b[2mTime: {}\x1b[0m", format_short_time(elapsed));
        }
        Err(e) => print_error(&e, arg),
    }
}

fn cmd_reset(interp: &mut Interpreter) {
    *interp = Interpreter::new();
    load_stdlib(interp);
    let underscore = interp.symbols.intern("_");
    interp.global_env.define(underscore, Value::Nil, true);
    println!("\x1b[32mInterpreter reset to fresh state\x1b[0m");
}

// ── Helpers ─────────────────────────────────────────────────────────

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
    let kind_label = match e.kind {
        ErrorKind::Syntax => "SyntaxError",
        ErrorKind::Runtime => "RuntimeError",
        ErrorKind::Name => "NameError",
        ErrorKind::Message => "MessageError",
        ErrorKind::Arity => "ArityError",
        ErrorKind::Type => "TypeError",
        ErrorKind::IO => "IOError",
    };
    eprintln!("\x1b[31m{kind_label}: {}\x1b[0m", e.message);

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
    let exprs = Parser::new(tokens, &mut interp.symbols).parse_program()?;
    interp.evaluate_program(&exprs)
}

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

fn format_short_time(seconds: f64) -> String {
    if seconds < 0.001 {
        format!("{:.0}us", seconds * 1_000_000.0)
    } else if seconds < 1.0 {
        format!("{:.1}ms", seconds * 1_000.0)
    } else {
        format!("{:.2}s", seconds)
    }
}

fn print_columns(items: &[String], cols: usize) {
    let max_width = items.iter().map(|s| s.len()).max().unwrap_or(10) + 2;
    for (i, item) in items.iter().enumerate() {
        print!("  {item:<width$}", width = max_width);
        if (i + 1) % cols == 0 {
            println!();
        }
    }
    if items.len() % cols != 0 {
        println!();
    }
}
