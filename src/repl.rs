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
use crate::parser::Parser;
use crate::value::Value;

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
    "Lists are cons cells: (car (list 1 2 3)) \u{2192} 1, (cdr (list 1 2 3)) \u{2192} (2 3)",
    "Numbers never overflow: (* 999999999999999999 999999999999999999)",
    "(range 1 10) creates a lazy range -- use [r to_list] to materialize",
    "(try (error \"boom\") (catch e [e class])) \u{2192} \"RuntimeError\"",
];

const KEYWORDS: &[&str] = &[
    "define", "lambda", "fn", "if", "let", "do", "set!", "quote", "try", "catch",
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

// ── REPL display via message sends ────────────────────────────────

/// Display a value for the REPL by sending the `inspect` message.
fn repl_inspect(val: &Value, interp: &mut crate::interpreter::Interpreter) -> String {
    // For strings, quote them
    if let Value::Str(s) = val {
        return format!("{:?}", &**s);
    }
    // For nil, just show "nil"
    if matches!(val, Value::Nil) {
        return "nil".to_string();
    }
    // Try sending `inspect` message
    let inspect_sel = interp.symbols.intern("inspect");
    match interp.send_message(val.clone(), inspect_sel, vec![]) {
        Ok(Value::Str(s)) => s.to_string(),
        Ok(other) => format!("{}", other),
        Err(_) => interp.display_value(val),
    }
}

/// Get the type name for REPL display by sending `class` then `name`.
fn repl_type_name(val: &Value, interp: &mut crate::interpreter::Interpreter) -> String {
    let class_sel = interp.symbols.intern("class");
    match interp.send_message(val.clone(), class_sel, vec![]) {
        Ok(class_obj) => {
            let name_sel = interp.symbols.intern("name");
            match interp.send_message(class_obj, name_sel, vec![]) {
                Ok(Value::Str(s)) => s.to_string(),
                _ => val.type_name().to_string(),
            }
        }
        Err(_) => val.type_name().to_string(),
    }
}

// ── Symbol-resolving display ───────────────────────────────────────

fn display_value(val: &Value, interp: &Interpreter) -> String {
    match val {
        Value::Symbol(id) => interp
            .symbols
            .try_name(*id)
            .unwrap_or("?")
            .to_string(),
        Value::Cons(_) => display_cons(val, interp),
        Value::Table(tbl) => {
            let tbl = tbl.borrow();
            let has_array = !tbl.array.is_empty();
            let has_hash = !tbl.hash.is_empty();
            let mut out = String::from("{");
            let mut first = true;
            if has_array {
                for v in &tbl.array {
                    if !first {
                        out.push_str(", ");
                    }
                    first = false;
                    out.push_str(&display_value(v, interp));
                }
            }
            if has_hash {
                for (k, v) in &tbl.hash {
                    if !first {
                        out.push_str(", ");
                    }
                    first = false;
                    out.push_str(k);
                    out.push_str(": ");
                    out.push_str(&display_value(v, interp));
                }
            }
            out.push('}');
            out
        }
        Value::Object(obj) => {
            let obj = obj.borrow();
            let class = obj.class.borrow();
            let class_name = interp
                .symbols
                .try_name(class.name)
                .unwrap_or("?");
            let field_names = class.all_field_names();
            if field_names.is_empty() {
                format!("#<{class_name}>")
            } else {
                let fields: Vec<String> = field_names
                    .iter()
                    .enumerate()
                    .map(|(i, &fid)| {
                        let fname = interp.symbols.try_name(fid).unwrap_or("?");
                        let fval = obj
                            .fields
                            .get(i)
                            .map(|v| display_value(v, interp))
                            .unwrap_or_else(|| "nil".to_string());
                        format!("{fname}: {fval}")
                    })
                    .collect();
                format!("#<{class_name} {}>", fields.join(", "))
            }
        }
        Value::Closure(c) => {
            let arity = c.params.len();
            if let Some(name_id) = c.name {
                let name = interp.symbols.try_name(name_id).unwrap_or("?");
                format!("<{name}/{arity}>")
            } else {
                format!("<lambda/{arity}>")
            }
        }
        Value::Str(s) => format!("{:?}", &**s),
        _ => val.inspect(),
    }
}

fn display_cons(val: &Value, interp: &Interpreter) -> String {
    let mut out = String::from("(");
    let mut current = val;
    let mut first = true;

    loop {
        match current {
            Value::Cons(cell) => {
                if !first {
                    out.push(' ');
                }
                first = false;
                out.push_str(&display_value(&cell.car, interp));
                current = &cell.cdr;
            }
            Value::Nil => break,
            other => {
                out.push_str(" . ");
                out.push_str(&display_value(other, interp));
                break;
            }
        }
    }

    out.push(')');
    out
}

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
        // Add class names
        for (name, _) in interp.all_classes() {
            if !self.completions.contains(&name) {
                self.completions.push(name);
            }
        }
        // Add protocol names
        for &id in interp.protocol_registry.keys() {
            if let Some(name) = interp.symbols.try_name(id) {
                let s = name.to_string();
                if !self.completions.contains(&s) {
                    self.completions.push(s);
                }
            }
        }
        // Add trait names
        for &id in interp.trait_registry.keys() {
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
                        let inspected = repl_inspect(&result, &mut interp);
                        let type_name = repl_type_name(&result, &mut interp);

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
        ",protocols" => cmd_protocols(interp),
        ",ast" => cmd_ast(interp, arg),
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
    println!("  \x1b[36m,doc\x1b[0m   \x1b[36m,d\x1b[0m       Show documentation for a binding, class, or protocol");
    println!("  \x1b[36m,methods\x1b[0m \x1b[36m,m\x1b[0m     List methods for a class");
    println!("  \x1b[36m,classes\x1b[0m         List all defined classes");
    println!("  \x1b[36m,protocols\x1b[0m       List all registered protocols");
    println!("  \x1b[36m,ast\x1b[0m             Show parsed (desugared) form of an expression");
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
    let mut classes = Vec::new();

    // Collect class names for identification
    let class_names: Vec<String> = interp
        .all_classes()
        .iter()
        .map(|(name, _)| name.clone())
        .collect();

    for (name, val) in &entries {
        if name.starts_with("__") {
            continue;
        }
        if !filter.is_empty() && !name.contains(filter) {
            continue;
        }

        match val {
            Value::Closure(_) => functions.push(name.clone()),
            Value::Object(obj) => {
                let obj = obj.borrow();
                let cname = interp
                    .symbols
                    .try_name(obj.class.borrow().name)
                    .unwrap_or("")
                    .to_string();
                // Objects whose class is a metaclass are class bindings
                if obj.class.borrow().is_meta || class_names.contains(&name.to_string()) {
                    classes.push(name.clone());
                } else {
                    values.push((name.clone(), val.clone(), cname));
                }
            }
            _ => values.push((name.clone(), val.clone(), val.type_name().to_string())),
        }
    }

    // Macros from macro_registry
    let mut macros: Vec<String> = interp
        .macro_registry
        .keys()
        .filter_map(|&id| {
            let name = interp.symbols.try_name(id)?.to_string();
            if !filter.is_empty() && !name.contains(filter) {
                return None;
            }
            Some(name)
        })
        .collect();
    macros.sort();

    // Types (ADTs) from type_registry
    let mut types: Vec<String> = interp
        .type_registry
        .iter()
        .filter_map(|(&id, variants)| {
            let name = interp.symbols.try_name(id)?.to_string();
            if !filter.is_empty() && !name.contains(filter) {
                return None;
            }
            let variant_names: Vec<String> = variants
                .iter()
                .filter_map(|&vid| interp.symbols.try_name(vid).map(|s| s.to_string()))
                .collect();
            Some(format!("{name}: {}", variant_names.join(" | ")))
        })
        .collect();
    types.sort();

    // Protocols from protocol_registry
    let mut protocols: Vec<String> = interp
        .protocol_registry
        .iter()
        .filter_map(|(&id, _selectors)| {
            let name = interp.symbols.try_name(id)?.to_string();
            if !filter.is_empty() && !name.contains(filter) {
                return None;
            }
            Some(name)
        })
        .collect();
    protocols.sort();

    if !values.is_empty() {
        println!("\x1b[1mValues:\x1b[0m");
        for (name, val, type_name) in &values {
            println!(
                "  \x1b[36m{name}\x1b[0m = {}\x1b[2m : {}\x1b[0m",
                display_value(val, interp),
                type_name
            );
        }
    }
    if !functions.is_empty() {
        println!("\x1b[1mFunctions:\x1b[0m ({} total)", functions.len());
        print_columns(&functions, 4);
    }
    if !classes.is_empty() {
        println!("\x1b[1mClasses:\x1b[0m");
        print_columns(&classes, 4);
    }
    if !protocols.is_empty() {
        println!("\x1b[1mProtocols:\x1b[0m");
        print_columns(&protocols, 4);
    }
    if !macros.is_empty() {
        println!("\x1b[1mMacros:\x1b[0m");
        print_columns(&macros, 4);
    }
    if !types.is_empty() {
        println!("\x1b[1mTypes:\x1b[0m");
        for t in &types {
            println!("  \x1b[36m{t}\x1b[0m");
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

    // 1. Check if it's a class
    if let Some(class_rc) = interp.class_of_name(arg) {
        let klass = class_rc.borrow();
        let superclass_name = klass
            .superclass
            .as_ref()
            .map(|s| {
                interp
                    .symbols
                    .try_name(s.borrow().name)
                    .unwrap_or("?")
                    .to_string()
            })
            .unwrap_or_else(|| "(none)".to_string());

        let field_names: Vec<String> = klass
            .field_names
            .iter()
            .filter_map(|&fid| interp.symbols.try_name(fid).map(|s| s.to_string()))
            .collect();

        let mut own_methods: Vec<String> = klass
            .methods
            .keys()
            .filter_map(|&id| interp.symbols.try_name(id).map(|s| s.to_string()))
            .collect();
        own_methods.sort();

        // Walk superclass chain for inherited methods
        let mut sup = klass.superclass.clone();
        drop(klass);
        let mut inherited = Vec::new();
        while let Some(s) = sup {
            let sb = s.borrow();
            for &id in sb.methods.keys() {
                if let Some(name) = interp.symbols.try_name(id) {
                    let name_s = name.to_string();
                    if !own_methods.contains(&name_s) && !inherited.contains(&name_s) {
                        inherited.push(name_s);
                    }
                }
            }
            sup = sb.superclass.clone();
        }
        inherited.sort();

        println!("\x1b[1m{arg}\x1b[0m : \x1b[36mClass\x1b[0m");
        if !field_names.is_empty() {
            println!("  Fields: {}", field_names.join(", "));
        }
        println!("  Extends: {superclass_name}");
        if !own_methods.is_empty() {
            println!(
                "  Methods: {} ({} own)",
                own_methods.join(", "),
                own_methods.len()
            );
        } else {
            println!("  Methods: (none)");
        }
        if !inherited.is_empty() {
            println!(
                "  \x1b[2mInherited: {} (from {superclass_name})\x1b[0m",
                inherited.join(", ")
            );
        }
        return;
    }

    // 2. Check protocol registry
    let proto_id = interp.symbols.intern(arg);
    if let Some(selectors) = interp.protocol_registry.get(&proto_id) {
        let selector_names: Vec<String> = selectors
            .iter()
            .filter_map(|&sid| interp.symbols.try_name(sid).map(|s| s.to_string()))
            .collect();
        println!("\x1b[1m{arg}\x1b[0m : \x1b[36mProtocol\x1b[0m");
        println!("  Required selectors: {}", selector_names.join(", "));
        return;
    }

    // 3. Check trait registry
    if let Some(methods) = interp.trait_registry.get(&proto_id) {
        let method_names: Vec<String> = methods
            .keys()
            .filter_map(|&mid| interp.symbols.try_name(mid).map(|s| s.to_string()))
            .collect();
        println!("\x1b[1m{arg}\x1b[0m : \x1b[36mTrait\x1b[0m");
        println!("  Methods: {}", method_names.join(", "));
        return;
    }

    // 4. Fall back to env lookup (current behavior for functions/values)
    match interp.global_env.get(proto_id) {
        Ok(val) => {
            println!(
                "\x1b[1m{arg}\x1b[0m : \x1b[36m{}\x1b[0m",
                val.type_name()
            );
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
                    println!(
                        "  \x1b[2m({} {}{})\x1b[0m",
                        name_str,
                        params.join(" "),
                        rest
                    );
                }
                other => {
                    println!("  {}", display_value(other, interp));
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
        "Object", "Class", "Numeric", "Integer", "Float", "String", "Symbol",
        "Cons", "Table", "Closure", "Range", "Bool", "TrueClass", "FalseClass",
        "NilClass",
    ];
    let error_names = [
        "Error", "SyntaxError", "RuntimeError", "NameError", "TypeError",
        "ArityError", "MessageError", "IOError",
    ];

    let mut builtin_list = Vec::new();
    let mut error_list = Vec::new();
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
        } else if error_names.contains(&name.as_str()) {
            error_list.push(name);
        } else {
            let field_count = klass.all_field_names().len();
            if field_count > 0 || method_count > 0 {
                user_list.push(format!(
                    "{name} ({field_count} fields, {method_count} methods)"
                ));
            } else {
                user_list.push(name);
            }
        }
    }

    builtin_list.sort();
    error_list.sort();
    user_list.sort();

    println!("\x1b[1mBuilt-in types:\x1b[0m");
    println!("  {}", builtin_list.join(", "));

    if !error_list.is_empty() {
        println!();
        println!("\x1b[1mError hierarchy:\x1b[0m");
        println!(
            "  Error > SyntaxError, RuntimeError > {}",
            error_list
                .iter()
                .filter(|n| *n != "Error" && *n != "SyntaxError" && *n != "RuntimeError")
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // ADTs from type_registry
    let mut adts: Vec<String> = interp
        .type_registry
        .iter()
        .filter_map(|(&id, variants)| {
            let name = interp.symbols.try_name(id)?.to_string();
            let variant_names: Vec<String> = variants
                .iter()
                .filter_map(|&vid| interp.symbols.try_name(vid).map(|s| s.to_string()))
                .collect();
            Some(format!("{name}: {}", variant_names.join(" | ")))
        })
        .collect();
    adts.sort();

    if !adts.is_empty() {
        println!();
        println!("\x1b[1mADTs:\x1b[0m");
        for adt in &adts {
            println!("  {adt}");
        }
    }

    if !user_list.is_empty() {
        println!();
        println!("\x1b[1mUser classes:\x1b[0m");
        for c in &user_list {
            println!("  \x1b[36m{c}\x1b[0m");
        }
    }
}

fn cmd_protocols(interp: &Interpreter) {
    if interp.protocol_registry.is_empty() {
        println!("\x1b[2m(no protocols registered)\x1b[0m");
        return;
    }

    let mut entries: Vec<(String, Vec<String>)> = interp
        .protocol_registry
        .iter()
        .filter_map(|(&id, selectors)| {
            let name = interp.symbols.try_name(id)?.to_string();
            let sel_names: Vec<String> = selectors
                .iter()
                .filter_map(|&sid| interp.symbols.try_name(sid).map(|s| s.to_string()))
                .collect();
            Some((name, sel_names))
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    println!("\x1b[1mProtocols:\x1b[0m");
    for (name, selectors) in &entries {
        println!("  \x1b[36m{name}\x1b[0m: {}", selectors.join(", "));
    }
}

fn cmd_ast(interp: &mut Interpreter, arg: &str) {
    if arg.is_empty() {
        println!("\x1b[2mUsage: ,ast <expr>\x1b[0m");
        return;
    }
    match Lexer::new(arg).tokenize() {
        Ok(tokens) => match Parser::new(tokens, &mut interp.symbols).parse_program() {
            Ok(exprs) => {
                for expr in &exprs {
                    println!("{}", display_value(expr, interp));
                }
            }
            Err(e) => print_error(&e, arg),
        },
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
                display_value(&result, interp),
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
    if let Err(e) = interp.load_prelude() {
        eprintln!(
            "\x1b[33mWarning: failed to load stdlib: {}\x1b[0m",
            e.message
        );
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
