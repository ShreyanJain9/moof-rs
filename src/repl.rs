use std::time::Instant;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::FileHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::normalizer::normalize;
use crate::value::Value;
use crate::error::MoofError;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const HISTORY_FILE: &str = ".moof_history";
const MAX_HISTORY: usize = 1000;

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
    "define", "lambda", "if", "let", "do", "set!", "quote", "try", "catch",
    "cond", "and", "or", "class", "trait", "match", "type", "protocol",
    "extend", "defmacro", "module", "use", "require", "->",
];

const META_COMMANDS: &[&str] = &[
    ",help", ",quit", ",exit", ",version", ",env", ",type", ",doc",
    ",methods", ",classes", ",protocols", ",ast", ",load", ",time",
    ",clear", ",reset",
];

const COMMON_SELECTORS: &[&str] = &[
    "length", "first", "last", "rest", "reverse", "sort", "flatten", "empty?",
    "map:", "filter:", "reduce:init:", "each:", "any:", "all:", "none:", "sortBy:",
    "push:", "prepend:", "at:", "contains:", "join:", "take:", "drop:", "zip:",
    "uppercase", "lowercase", "trim", "chars", "split:", "concat:",
    "to_s", "to_i", "to_f", "abs", "nil?", "class", "not",
    "pow:", "max:", "min:", "sqrt",
];

const STDLIB: &str = include_str!("../stdlib/stdlib.moof");

// ── Completion ──────────────────────────────────────────────────────

struct MoofHelper {
    /// Cached completions, rebuilt after each eval.
    completions: Vec<String>,
}

impl MoofHelper {
    fn new() -> Self {
        let mut completions = Vec::new();
        for kw in KEYWORDS { completions.push(kw.to_string()); }
        for mc in META_COMMANDS { completions.push(mc.to_string()); }
        for sel in COMMON_SELECTORS { completions.push(sel.to_string()); }
        MoofHelper { completions }
    }

    fn refresh(&mut self, interp: &Interpreter) {
        self.completions.clear();
        for kw in KEYWORDS { self.completions.push(kw.to_string()); }
        for mc in META_COMMANDS { self.completions.push(mc.to_string()); }
        for sel in COMMON_SELECTORS { self.completions.push(sel.to_string()); }
        // Add environment bindings
        for (name, _) in interp.global_env.bindings() {
            if !self.completions.contains(&name) {
                self.completions.push(name);
            }
        }
        // Add class names
        for name in interp.class_registry.keys() {
            if !self.completions.contains(name) {
                self.completions.push(name.clone());
            }
        }
        // Add protocol names
        for name in interp.protocol_registry.keys() {
            if !self.completions.contains(name) {
                self.completions.push(name.clone());
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
        // Find the word being completed
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
    let mut rl: Editor<MoofHelper, FileHistory> =
        Editor::with_config(rustyline::Config::builder().max_history_size(MAX_HISTORY).unwrap().build())
            .expect("Failed to initialize line editor");
    rl.set_helper(Some(helper));

    let history_path = dirs_or_home().join(HISTORY_FILE);
    let history_str = history_path.to_string_lossy().to_string();
    let _ = rl.load_history(&history_str);

    let mut interp = Interpreter::new();
    load_stdlib(&mut interp);
    interp.global_env.define("_", Value::Nil, true);

    // Refresh completions after stdlib load
    if let Some(h) = rl.helper_mut() { h.refresh(&interp); }

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
                    if let Some(h) = rl.helper_mut() { h.refresh(&interp); }
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
                        interp.global_env.set("_", result.clone()).ok();

                        let elapsed = eval_start.elapsed().as_secs_f64();
                        let inspected = result.inspect();
                        let type_name = result.type_name();

                        if elapsed >= 0.1 {
                            println!(
                                "\x1b[32m=> \x1b[0m{}\x1b[2m : {} [{}]\x1b[0m",
                                inspected, type_name, format_short_time(elapsed)
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
                if let Some(h) = rl.helper_mut() { h.refresh(&interp); }
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
        ",protocols" => cmd_protocols(interp),
        ",ast" => cmd_ast(arg),
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
    println!("  \x1b[36m,methods\x1b[0m \x1b[36m,m\x1b[0m     List methods for a class or value");
    println!("  \x1b[36m,classes\x1b[0m         List all defined classes");
    println!("  \x1b[36m,protocols\x1b[0m       List all defined protocols");
    println!("  \x1b[36m,ast\x1b[0m             Show parsed & normalized AST");
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
    let mut sorted: Vec<_> = bindings.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    // Group by type
    let mut functions = Vec::new();
    let mut classes = Vec::new();
    let mut protocols = Vec::new();
    let mut macros = Vec::new();
    let mut values = Vec::new();

    for (name, val) in &sorted {
        // Skip internals
        if name.starts_with("__") { continue; }
        // Apply filter
        if !filter.is_empty() && !name.contains(filter) { continue; }

        match val {
            Value::Function(_) | Value::Builtin(_, _) => functions.push((name, val)),
            Value::Class(_) => classes.push((name, val)),
            Value::Protocol(_) => protocols.push((name, val)),
            Value::Macro(_) => macros.push((name, val)),
            _ => values.push((name, val)),
        }
    }

    if !values.is_empty() {
        println!("\x1b[1mValues:\x1b[0m");
        for (name, val) in &values {
            println!("  \x1b[36m{name}\x1b[0m = {}\x1b[2m : {}\x1b[0m", val.inspect(), val.type_name());
        }
    }
    if !functions.is_empty() {
        println!("\x1b[1mFunctions:\x1b[0m ({} total)", functions.len());
        let display: Vec<String> = functions.iter().map(|(n, _)| n.to_string()).collect();
        print_columns(&display, 4);
    }
    if !classes.is_empty() {
        println!("\x1b[1mClasses:\x1b[0m");
        for (name, _) in &classes {
            println!("  \x1b[36m{name}\x1b[0m");
        }
    }
    if !protocols.is_empty() {
        println!("\x1b[1mProtocols:\x1b[0m");
        for (name, _) in &protocols {
            println!("  \x1b[36m{name}\x1b[0m");
        }
    }
    if !macros.is_empty() {
        println!("\x1b[1mMacros:\x1b[0m");
        for (name, _) in &macros {
            println!("  \x1b[36m{name}\x1b[0m");
        }
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

    // Check class registry
    if let Some(klass_rc) = interp.class_registry.get(arg) {
        let klass = klass_rc.borrow();
        println!("\x1b[1m{}\x1b[0m : \x1b[36mClass\x1b[0m", klass.name);
        let fields = klass.all_fields();
        if !fields.is_empty() {
            println!("  \x1b[2mFields:\x1b[0m {}", fields.join(", "));
        }
        if let Some(ref sup) = klass.superclass {
            println!("  \x1b[2mExtends:\x1b[0m {}", sup.borrow().name);
        }
        let methods: Vec<String> = klass.methods.keys().cloned().collect();
        if !methods.is_empty() {
            let mut sorted = methods;
            sorted.sort();
            println!("  \x1b[2mMethods:\x1b[0m {}", sorted.join(", "));
        }
        return;
    }

    // Check protocol registry
    if let Some(proto) = interp.protocol_registry.get(arg) {
        println!("\x1b[1m{}\x1b[0m : \x1b[36mProtocol\x1b[0m", proto.name);
        println!("  \x1b[2mSelectors:\x1b[0m {}", proto.selectors.join(", "));
        if !proto.default_methods.is_empty() {
            let defaults: Vec<String> = proto.default_methods.keys().cloned().collect();
            println!("  \x1b[2mDefault methods:\x1b[0m {}", defaults.join(", "));
        }
        return;
    }

    // Check bindings
    match interp.global_env.get(arg) {
        Ok(val) => {
            println!("\x1b[1m{arg}\x1b[0m : \x1b[36m{}\x1b[0m", val.type_name());
            match &val {
                Value::Function(f) => {
                    let params = f.params.join(" ");
                    let rest = f.rest_param.as_ref().map(|r| format!(" . {r}")).unwrap_or_default();
                    println!("  \x1b[2m({arg} {params}{rest})\x1b[0m");
                }
                Value::Builtin(name, _) => {
                    println!("  \x1b[2m<builtin {name}>\x1b[0m");
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

fn cmd_methods(interp: &Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,methods <ClassName>\x1b[0m");
        return;
    }

    let class_name = match arg {
        "Integer" | "Float" | "String" | "List" | "Map" | "Bool" | "Nil" | "Function" => arg.to_string(),
        _ => arg.to_string(),
    };

    if let Some(klass_rc) = interp.class_registry.get(&class_name) {
        let klass = klass_rc.borrow();
        println!("\x1b[1m{}\x1b[0m methods:", class_name);

        let mut builtins = Vec::new();
        let mut user_defined = Vec::new();

        for (name, method) in &klass.methods {
            match method {
                crate::value::Method::Builtin(_, _) => builtins.push(name.clone()),
                crate::value::Method::UserDefined(_) => user_defined.push(name.clone()),
            }
        }

        // Walk superclass chain
        let mut sup = klass.superclass.clone();
        drop(klass);
        let mut inherited = Vec::new();
        while let Some(s) = sup {
            let sb = s.borrow();
            for name in sb.methods.keys() {
                if !builtins.contains(name) && !user_defined.contains(name) && !inherited.contains(name) {
                    inherited.push(name.clone());
                }
            }
            sup = sb.superclass.clone();
        }

        builtins.sort();
        user_defined.sort();
        inherited.sort();

        if !builtins.is_empty() {
            println!("  \x1b[2mBuilt-in:\x1b[0m {}", builtins.join(", "));
        }
        if !user_defined.is_empty() {
            println!("  \x1b[2mUser-defined:\x1b[0m {}", user_defined.join(", "));
        }
        if !inherited.is_empty() {
            println!("  \x1b[2mInherited:\x1b[0m {}", inherited.join(", "));
        }
        if builtins.is_empty() && user_defined.is_empty() && inherited.is_empty() {
            println!("  \x1b[2m(no methods)\x1b[0m");
        }
    } else {
        println!("\x1b[2mNo class found: '{arg}'\x1b[0m");
    }
}

fn cmd_classes(interp: &Interpreter) {
    let mut builtin_types = Vec::new();
    let mut user_classes = Vec::new();
    let mut adt_variants = Vec::new();

    let adt_names: Vec<String> = interp.type_registry.values().flatten().cloned().collect();

    for (name, klass_rc) in &interp.class_registry {
        let klass = klass_rc.borrow();
        let is_builtin = matches!(
            name.as_str(),
            "Integer" | "Float" | "String" | "List" | "Map" | "Bool" | "Nil" | "Function"
        );

        if is_builtin {
            let user_methods: Vec<String> = klass.methods.iter()
                .filter(|(_, m)| matches!(m, crate::value::Method::UserDefined(_)))
                .map(|(n, _)| n.clone())
                .collect();
            if user_methods.is_empty() {
                builtin_types.push(name.clone());
            } else {
                builtin_types.push(format!("{name} (+{})", user_methods.len()));
            }
        } else if adt_names.contains(name) {
            adt_variants.push(name.clone());
        } else {
            let field_count = klass.all_fields().len();
            let method_count = klass.methods.len();
            if field_count > 0 || method_count > 0 {
                user_classes.push(format!("{name} ({field_count} fields, {method_count} methods)"));
            } else {
                user_classes.push(name.clone());
            }
        }
    }

    builtin_types.sort();
    user_classes.sort();
    adt_variants.sort();

    println!("\x1b[1mBuilt-in types:\x1b[0m {}", builtin_types.join(", "));
    if !user_classes.is_empty() {
        println!("\x1b[1mUser classes:\x1b[0m");
        for c in &user_classes {
            println!("  \x1b[36m{c}\x1b[0m");
        }
    }
    if !adt_variants.is_empty() {
        // Group by type
        for (type_name, variants) in &interp.type_registry {
            println!("\x1b[1mADT {type_name}:\x1b[0m {}", variants.join(" | "));
        }
    }
}

fn cmd_protocols(interp: &Interpreter) {
    if interp.protocol_registry.is_empty() {
        println!("\x1b[2mNo protocols defined\x1b[0m");
        return;
    }
    for (name, proto) in &interp.protocol_registry {
        let has_defaults = if proto.default_methods.is_empty() { "" } else { " (has defaults)" };
        println!(
            "\x1b[1m{name}\x1b[0m\x1b[2m{has_defaults}\x1b[0m: {}",
            proto.selectors.join(", ")
        );
    }
}

fn cmd_ast(arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,ast <expr>\x1b[0m");
        return;
    }
    match Lexer::new(arg).tokenize() {
        Ok(tokens) => {
            match Parser::new(tokens).parse_program() {
                Ok(program) => {
                    println!("\x1b[1mParsed:\x1b[0m");
                    for expr in &program.expressions {
                        println!("  {:?}", expr);
                    }
                    let normalized = normalize(program);
                    println!("\x1b[1mNormalized:\x1b[0m");
                    for expr in &normalized.expressions {
                        println!("  {:?}", expr);
                    }
                }
                Err(e) => print_error(&e, arg),
            }
        }
        Err(e) => print_error(&e, arg),
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
        Ok(source) => {
            match interp.load_source(&source, &path) {
                Ok(_) => println!("\x1b[32mLoaded {path}\x1b[0m"),
                Err(e) => print_error(&e, &source),
            }
        }
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
                result.inspect(), result.type_name()
            );
            println!("\x1b[2mTime: {}\x1b[0m", format_short_time(elapsed));
        }
        Err(e) => print_error(&e, arg),
    }
}

fn cmd_reset(interp: &mut Interpreter) {
    *interp = Interpreter::new();
    load_stdlib(interp);
    interp.global_env.define("_", Value::Nil, true);
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
            eprintln!("\x1b[33mWarning: failed to load stdlib: {}\x1b[0m", e.message);
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
        crate::error::ErrorKind::Syntax => "SyntaxError",
        crate::error::ErrorKind::Runtime => "RuntimeError",
        crate::error::ErrorKind::Name => "NameError",
        crate::error::ErrorKind::Message => "MessageError",
        crate::error::ErrorKind::Arity => "ArityError",
        crate::error::ErrorKind::ImmutableBinding => "ImmutableError",
        crate::error::ErrorKind::Type => "TypeError",
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
    let program = Parser::new(tokens).parse_program()?;
    let normalized = normalize(program);
    interp.evaluate(&normalized)
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
                block_depth += 1; i += 2; continue;
            } else if ch == '|' && nch == Some('#') {
                block_depth -= 1;
                if block_depth == 0 { in_block_comment = false; }
                i += 2; continue;
            }
            i += 1; continue;
        }
        if in_line_comment {
            if ch == '\n' { in_line_comment = false; }
            i += 1; continue;
        }
        if escape { escape = false; i += 1; continue; }
        if in_string {
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            i += 1; continue;
        }

        match ch {
            ';' => in_line_comment = true,
            '#' if nch == Some('|') => { in_block_comment = true; block_depth = 1; i += 2; continue; }
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

    if in_string || in_block_comment { return false; }
    depth_paren <= 0 && depth_bracket <= 0 && depth_brace <= 0
}

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 { format!("{:.1}s", seconds) }
    else if seconds < 3600.0 { format!("{:.1}m", seconds / 60.0) }
    else { format!("{:.1}h", seconds / 3600.0) }
}

fn format_short_time(seconds: f64) -> String {
    if seconds < 0.001 { format!("{:.0}µs", seconds * 1_000_000.0) }
    else if seconds < 1.0 { format!("{:.1}ms", seconds * 1_000.0) }
    else { format!("{:.2}s", seconds) }
}

fn print_columns(items: &[String], cols: usize) {
    let max_width = items.iter().map(|s| s.len()).max().unwrap_or(10) + 2;
    for (i, item) in items.iter().enumerate() {
        print!("  {item:<width$}", width = max_width);
        if (i + 1) % cols == 0 { println!(); }
    }
    if items.len() % cols != 0 { println!(); }
}
