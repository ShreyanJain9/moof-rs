use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Install all builtin functions into the interpreter's global environment.
pub fn install(interp: &mut Interpreter) {
    let env = interp.global_env.clone();

    // ═══════════════════════════════════════════════════════════════
    // Arithmetic — variadic
    // ═══════════════════════════════════════════════════════════════

    env.define("+", Value::Builtin("+".into(), builtin_add), false);
    env.define("-", Value::Builtin("-".into(), builtin_sub), false);
    env.define("*", Value::Builtin("*".into(), builtin_mul), false);
    env.define("/", Value::Builtin("/".into(), builtin_div), false);
    env.define("%", Value::Builtin("%".into(), builtin_mod), false);

    // ═══════════════════════════════════════════════════════════════
    // Comparison
    // ═══════════════════════════════════════════════════════════════

    env.define(">",  Value::Builtin(">".into(),  builtin_gt), false);
    env.define("<",  Value::Builtin("<".into(),  builtin_lt), false);
    env.define(">=", Value::Builtin(">=".into(), builtin_gte), false);
    env.define("<=", Value::Builtin("<=".into(), builtin_lte), false);

    // ═══════════════════════════════════════════════════════════════
    // Equality
    // ═══════════════════════════════════════════════════════════════

    env.define("=",   Value::Builtin("=".into(),   builtin_eq), false);
    env.define("eq?", Value::Builtin("eq?".into(), builtin_eq_identity), false);

    // ═══════════════════════════════════════════════════════════════
    // Core data constructors
    // ═══════════════════════════════════════════════════════════════

    env.define("list", Value::Builtin("list".into(), builtin_list), false);
    env.define("cons", Value::Builtin("cons".into(), builtin_cons), false);
    env.define("car",  Value::Builtin("car".into(),  builtin_car), false);
    env.define("cdr",  Value::Builtin("cdr".into(),  builtin_cdr), false);

    // ═══════════════════════════════════════════════════════════════
    // I/O
    // ═══════════════════════════════════════════════════════════════

    env.define("print",   Value::Builtin("print".into(),   builtin_print), false);
    env.define("display", Value::Builtin("display".into(), builtin_display), false);
    env.define("format",  Value::Builtin("format".into(),  builtin_format), false);
    env.define("read-line", Value::Builtin("read-line".into(), builtin_read_line), false);

    // ═══════════════════════════════════════════════════════════════
    // File I/O
    // ═══════════════════════════════════════════════════════════════

    env.define("read-file",    Value::Builtin("read-file".into(),    builtin_read_file), false);
    env.define("write-file",   Value::Builtin("write-file".into(),   builtin_write_file), false);
    env.define("file-exists?", Value::Builtin("file-exists?".into(), builtin_file_exists), false);
    env.define("read-lines",   Value::Builtin("read-lines".into(),   builtin_read_lines), false);

    // ═══════════════════════════════════════════════════════════════
    // Message dispatch — bridge between () and []
    // ═══════════════════════════════════════════════════════════════

    env.define("__send", Value::Builtin("__send".into(), builtin_send), false);

    // ═══════════════════════════════════════════════════════════════
    // Apply
    // ═══════════════════════════════════════════════════════════════

    env.define("apply", Value::Builtin("apply".into(), builtin_apply), false);

    // ═══════════════════════════════════════════════════════════════
    // Error / control
    // ═══════════════════════════════════════════════════════════════

    env.define("error", Value::Builtin("error".into(), builtin_error), false);
    env.define("exit",  Value::Builtin("exit".into(),  builtin_exit), false);

    // ═══════════════════════════════════════════════════════════════
    // Type introspection
    // ═══════════════════════════════════════════════════════════════

    env.define("type-of",     Value::Builtin("type-of".into(),     builtin_type_of), false);
    env.define("implements?", Value::Builtin("implements?".into(), builtin_implements), false);

    // ═══════════════════════════════════════════════════════════════
    // Range
    // ═══════════════════════════════════════════════════════════════

    env.define("range", Value::Builtin("range".into(), builtin_range), false);

    // ═══════════════════════════════════════════════════════════════
    // Timing
    // ═══════════════════════════════════════════════════════════════

    env.define("time", Value::Builtin("time".into(), builtin_time), false);
}

/// Backward-compatible alias used by the old interpreter stub.
pub fn register_builtins(interp: &mut Interpreter) {
    install(interp);
}

// ── Helpers ─────────────────────────────────────────────────────

fn check_arity(name: &str, expected: usize, args: &[Value]) -> Result<()> {
    if args.len() != expected {
        Err(MoofError::arity(&expected.to_string(), args.len(), Some(name)))
    } else {
        Ok(())
    }
}

fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(n) => Some(*n as f64),
        Value::Float(n) => Some(*n),
        _ => None,
    }
}

fn is_float_pair(a: &Value, b: &Value) -> bool {
    matches!(a, Value::Float(_)) || matches!(b, Value::Float(_))
}

fn format_value(val: &Value) -> String {
    format!("{}", val)
}

fn call_func(interp: &mut Interpreter, func: &Value, args: Vec<Value>) -> Result<Value> {
    match func {
        Value::Function(f) => crate::interpreter::call_function(interp, f, args),
        Value::Builtin(_, f_ptr) => f_ptr(interp, args),
        _ => Err(MoofError::runtime(format!("Expected a function, got {}", func.type_name()))),
    }
}

// ── Arithmetic ──────────────────────────────────────────────────

fn builtin_add(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let mut acc_i: i64 = 0;
    let mut acc_f: f64 = 0.0;
    let mut is_float = false;
    for arg in &args {
        match arg {
            Value::Integer(n) => {
                if is_float { acc_f += *n as f64; } else { acc_i += n; }
            }
            Value::Float(n) => {
                if !is_float { acc_f = acc_i as f64; is_float = true; }
                acc_f += n;
            }
            _ => return Err(MoofError::runtime("+ requires numbers")),
        }
    }
    if is_float { Ok(Value::Float(acc_f)) } else { Ok(Value::Integer(acc_i)) }
}

fn builtin_sub(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if args.is_empty() {
        return Err(MoofError::arity("1+", 0, Some("-")));
    }
    if args.len() == 1 {
        return match &args[0] {
            Value::Integer(n) => Ok(Value::Integer(-n)),
            Value::Float(n) => Ok(Value::Float(-n)),
            _ => Err(MoofError::runtime("- requires numbers")),
        };
    }
    let first_f = to_f64(&args[0]).ok_or_else(|| MoofError::runtime("- requires numbers"))?;
    let mut result = first_f;
    let mut is_float = matches!(&args[0], Value::Float(_));
    for arg in &args[1..] {
        let v = to_f64(arg).ok_or_else(|| MoofError::runtime("- requires numbers"))?;
        result -= v;
        if matches!(arg, Value::Float(_)) { is_float = true; }
    }
    if is_float { Ok(Value::Float(result)) } else { Ok(Value::Integer(result as i64)) }
}

fn builtin_mul(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let mut acc_i: i64 = 1;
    let mut acc_f: f64 = 1.0;
    let mut is_float = false;
    for arg in &args {
        match arg {
            Value::Integer(n) => {
                if is_float { acc_f *= *n as f64; } else { acc_i *= n; }
            }
            Value::Float(n) => {
                if !is_float { acc_f = acc_i as f64; is_float = true; }
                acc_f *= n;
            }
            _ => return Err(MoofError::runtime("* requires numbers")),
        }
    }
    if is_float { Ok(Value::Float(acc_f)) } else { Ok(Value::Integer(acc_i)) }
}

fn builtin_div(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if args.len() < 2 {
        return Err(MoofError::arity("2+", args.len(), Some("/")));
    }
    let first_f = to_f64(&args[0]).ok_or_else(|| MoofError::runtime("/ requires numbers"))?;
    let mut result = first_f;
    let mut is_float = matches!(&args[0], Value::Float(_));
    for arg in &args[1..] {
        let v = to_f64(arg).ok_or_else(|| MoofError::runtime("/ requires numbers"))?;
        if v == 0.0 { return Err(MoofError::runtime("Division by zero")); }
        result /= v;
        if matches!(arg, Value::Float(_)) { is_float = true; }
    }
    if is_float { Ok(Value::Float(result)) } else { Ok(Value::Integer(result as i64)) }
}

fn builtin_mod(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("%", 2, &args)?;
    let a = to_f64(&args[0]).ok_or_else(|| MoofError::runtime("% requires numbers"))?;
    let b = to_f64(&args[1]).ok_or_else(|| MoofError::runtime("% requires numbers"))?;
    if b == 0.0 { return Err(MoofError::runtime("Modulo by zero")); }
    if is_float_pair(&args[0], &args[1]) {
        Ok(Value::Float(a % b))
    } else {
        Ok(Value::Integer((a as i64) % (b as i64)))
    }
}

// ── Comparison ──────────────────────────────────────────────────

fn builtin_gt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity(">", 2, &args)?;
    let a = to_f64(&args[0]).ok_or_else(|| MoofError::runtime("> requires numbers"))?;
    let b = to_f64(&args[1]).ok_or_else(|| MoofError::runtime("> requires numbers"))?;
    Ok(Value::Bool(a > b))
}

fn builtin_lt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("<", 2, &args)?;
    let a = to_f64(&args[0]).ok_or_else(|| MoofError::runtime("< requires numbers"))?;
    let b = to_f64(&args[1]).ok_or_else(|| MoofError::runtime("< requires numbers"))?;
    Ok(Value::Bool(a < b))
}

fn builtin_gte(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity(">=", 2, &args)?;
    let a = to_f64(&args[0]).ok_or_else(|| MoofError::runtime(">= requires numbers"))?;
    let b = to_f64(&args[1]).ok_or_else(|| MoofError::runtime(">= requires numbers"))?;
    Ok(Value::Bool(a >= b))
}

fn builtin_lte(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("<=", 2, &args)?;
    let a = to_f64(&args[0]).ok_or_else(|| MoofError::runtime("<= requires numbers"))?;
    let b = to_f64(&args[1]).ok_or_else(|| MoofError::runtime("<= requires numbers"))?;
    Ok(Value::Bool(a <= b))
}

// ── Equality ────────────────────────────────────────────────────

fn builtin_eq(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("=", 2, &args)?;
    Ok(Value::Bool(args[0] == args[1]))
}

fn builtin_eq_identity(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("eq?", 2, &args)?;
    // Identity comparison — for primitives, same as ==; for objects, pointer eq
    let same = match (&args[0], &args[1]) {
        (Value::Object(a), Value::Object(b)) => {
            std::ptr::eq(&*a.class as *const _, &*b.class as *const _) && a.fields == b.fields
        }
        _ => args[0] == args[1],
    };
    Ok(Value::Bool(same))
}

// ── Data constructors ───────────────────────────────────────────

fn builtin_list(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::List(args))
}

fn builtin_cons(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("cons", 2, &args)?;
    match &args[1] {
        Value::List(tail) => {
            let mut new_list = vec![args[0].clone()];
            new_list.extend(tail.clone());
            Ok(Value::List(new_list))
        }
        _ => Err(MoofError::runtime("cons: second argument must be a list")),
    }
}

fn builtin_car(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("car", 1, &args)?;
    match &args[0] {
        Value::List(items) => Ok(items.first().cloned().unwrap_or(Value::Nil)),
        _ => Err(MoofError::runtime("car: argument must be a list")),
    }
}

fn builtin_cdr(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("cdr", 1, &args)?;
    match &args[0] {
        Value::List(items) => {
            if items.len() <= 1 {
                Ok(Value::List(vec![]))
            } else {
                Ok(Value::List(items[1..].to_vec()))
            }
        }
        _ => Err(MoofError::runtime("cdr: argument must be a list")),
    }
}

// ── I/O ─────────────────────────────────────────────────────────

fn builtin_print(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let output: Vec<String> = args.iter().map(|a| format_value(a)).collect();
    println!("{}", output.join(" "));
    Ok(Value::Nil)
}

fn builtin_display(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let output: Vec<String> = args.iter().map(|a| format_value(a)).collect();
    print!("{}", output.join(" "));
    Ok(Value::Nil)
}

fn builtin_format(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if args.is_empty() {
        return Err(MoofError::arity("1+", 0, Some("format")));
    }
    let template = match &args[0] {
        Value::Str(s) => s.clone(),
        other => format!("{}", other),
    };
    let values = &args[1..];
    let mut idx = 0;
    let mut result = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '~' && chars.peek() == Some(&'a') {
            chars.next(); // consume 'a'
            if idx < values.len() {
                result.push_str(&format_value(&values[idx]));
                idx += 1;
            }
        } else {
            result.push(c);
        }
    }
    Ok(Value::Str(result))
}

fn builtin_read_line(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    use std::io::{self, Write, BufRead};
    if args.len() == 1 {
        print!("{}", format_value(&args[0]));
        io::stdout().flush().ok();
    }
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok();
    if line.ends_with('\n') { line.pop(); }
    if line.ends_with('\r') { line.pop(); }
    Ok(Value::Str(line))
}

// ── File I/O ────────────────────────────────────────────────────

fn builtin_read_file(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("read-file", 1, &args)?;
    match &args[0] {
        Value::Str(path) => {
            let content = std::fs::read_to_string(path)
                .map_err(|e| MoofError::runtime(format!("read-file: {}", e)))?;
            Ok(Value::Str(content))
        }
        _ => Err(MoofError::runtime("read-file: argument must be a string")),
    }
}

fn builtin_write_file(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("write-file", 2, &args)?;
    match (&args[0], &args[1]) {
        (Value::Str(path), Value::Str(content)) => {
            std::fs::write(path, content)
                .map_err(|e| MoofError::runtime(format!("write-file: {}", e)))?;
            Ok(Value::Nil)
        }
        _ => Err(MoofError::runtime("write-file: expects two strings")),
    }
}

fn builtin_file_exists(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("file-exists?", 1, &args)?;
    match &args[0] {
        Value::Str(path) => Ok(Value::Bool(std::path::Path::new(path).exists())),
        _ => Err(MoofError::runtime("file-exists?: argument must be a string")),
    }
}

fn builtin_read_lines(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("read-lines", 1, &args)?;
    match &args[0] {
        Value::Str(path) => {
            let content = std::fs::read_to_string(path)
                .map_err(|e| MoofError::runtime(format!("read-lines: {}", e)))?;
            let lines: Vec<Value> = content.lines().map(|l| Value::Str(l.to_string())).collect();
            Ok(Value::List(lines))
        }
        _ => Err(MoofError::runtime("read-lines: argument must be a string")),
    }
}

// ── Message dispatch ────────────────────────────────────────────

fn builtin_send(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if args.len() < 2 {
        return Err(MoofError::arity("2+", args.len(), Some("__send")));
    }
    let receiver = args[0].clone();
    let selector = match &args[1] {
        Value::Str(s) => s.clone(),
        Value::Symbol(s) => s.clone(),
        _ => return Err(MoofError::runtime("__send: selector must be a string or symbol")),
    };
    let msg_args = args[2..].to_vec();
    crate::dispatcher::send_message(interp, receiver, &selector, msg_args)
}

// ── Apply ───────────────────────────────────────────────────────

fn builtin_apply(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("apply", 2, &args)?;
    let func = &args[0];
    let func_args = match &args[1] {
        Value::List(items) => items.clone(),
        _ => return Err(MoofError::runtime("apply: second argument must be a list")),
    };
    call_func(interp, func, func_args)
}

// ── Error / control ─────────────────────────────────────────────

fn builtin_error(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("error", 1, &args)?;
    Err(MoofError::runtime(format!("{}", args[0])))
}

fn builtin_exit(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let code = if args.len() == 1 {
        match &args[0] {
            Value::Integer(n) => *n as i32,
            _ => 0,
        }
    } else {
        0
    };
    std::process::exit(code);
}

// ── Type introspection ──────────────────────────────────────────

fn builtin_type_of(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("type-of", 1, &args)?;
    Ok(Value::Str(args[0].type_name().to_string()))
}

fn builtin_implements(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("implements?", 2, &args)?;
    let protocol = match &args[1] {
        Value::Protocol(p) => p,
        _ => return Err(MoofError::runtime("implements?: second argument must be a Protocol")),
    };
    // Check if the value responds to all selectors in the protocol
    let obj = &args[0];
    for selector in &protocol.selectors {
        // Try dispatching a pseudo-check. We check via the class registry or object methods.
        let responds = match obj {
            Value::Object(o) => o.class.borrow().lookup(selector).is_some(),
            _ => {
                // For built-in types, check if the dispatcher would handle it
                let class_name = match obj {
                    Value::Integer(_) => Some("Integer"),
                    Value::Float(_) => Some("Float"),
                    Value::Str(_) => Some("String"),
                    Value::List(_) => Some("List"),
                    Value::Map(_) => Some("Map"),
                    Value::Bool(_) => Some("Bool"),
                    Value::Nil => Some("Nil"),
                    Value::Function(_) | Value::Builtin(_, _) => Some("Function"),
                    _ => None,
                };
                if let Some(cn) = class_name {
                    if let Some(klass_rc) = interp.class_registry.get(cn) {
                        klass_rc.borrow().lookup(selector).is_some()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        };
        if !responds {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

// ── Range ───────────────────────────────────────────────────────

fn builtin_range(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match args.len() {
        1 => {
            let end = match &args[0] {
                Value::Integer(n) => *n,
                _ => return Err(MoofError::runtime("range: arguments must be integers")),
            };
            let items: Vec<Value> = (0..end).map(Value::Integer).collect();
            Ok(Value::List(items))
        }
        2 => {
            let start = match &args[0] {
                Value::Integer(n) => *n,
                _ => return Err(MoofError::runtime("range: arguments must be integers")),
            };
            let end = match &args[1] {
                Value::Integer(n) => *n,
                _ => return Err(MoofError::runtime("range: arguments must be integers")),
            };
            let items: Vec<Value> = (start..end).map(Value::Integer).collect();
            Ok(Value::List(items))
        }
        3 => {
            let start = match &args[0] {
                Value::Integer(n) => *n,
                _ => return Err(MoofError::runtime("range: arguments must be integers")),
            };
            let end = match &args[1] {
                Value::Integer(n) => *n,
                _ => return Err(MoofError::runtime("range: arguments must be integers")),
            };
            let step = match &args[2] {
                Value::Integer(n) => *n,
                _ => return Err(MoofError::runtime("range: arguments must be integers")),
            };
            if step == 0 { return Err(MoofError::runtime("range: step cannot be zero")); }
            let mut items = Vec::new();
            let mut i = start;
            if step > 0 {
                while i < end { items.push(Value::Integer(i)); i += step; }
            } else {
                while i > end { items.push(Value::Integer(i)); i += step; }
            }
            Ok(Value::List(items))
        }
        _ => Err(MoofError::arity("1-3", args.len(), Some("range"))),
    }
}

// ── Timing ──────────────────────────────────────────────────────

fn builtin_time(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("time", 1, &args)?;
    let t0 = std::time::Instant::now();
    let result = call_func(interp, &args[0], vec![])?;
    let elapsed = t0.elapsed();
    println!("Elapsed: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    Ok(result)
}
