use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::cons;
use crate::environment::Env;
use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::moofint::MoofInt;
use crate::value::{
    ClosureBody, MoofClass, MoofClosure, MoofRange, MoofTable, NativeFn, Value,
};

// ═══════════════════════════════════════════════════════════════════════
// Install everything
// ═══════════════════════════════════════════════════════════════════════

pub fn install(interp: &mut Interpreter) {
    install_globals(interp);
    install_object_methods(interp);
    install_symbol_methods(interp);
    install_integer_methods(interp);
    install_float_methods(interp);
    install_string_methods(interp);
    install_cons_methods(interp);
    install_table_methods(interp);
    install_bool_methods(interp);
    install_nil_methods(interp);
    install_closure_methods(interp);
    install_error_methods(interp);
    install_range_methods(interp);
}

// ═══════════════════════════════════════════════════════════════════════
// Registration helpers
// ═══════════════════════════════════════════════════════════════════════

fn register(interp: &mut Interpreter, name: &str, f: NativeFn) {
    let id = interp.symbols.intern(name);
    let closure = Value::Closure(Rc::new(MoofClosure {
        name: Some(id),
        params: vec![],
        rest_param: None,
        body: ClosureBody::Native(f),
        env: interp.global_env.clone(),
    }));
    interp.global_env.define(id, closure, false);
}

fn register_method(interp: &mut Interpreter, class: &Rc<RefCell<MoofClass>>, name: &str, f: NativeFn) {
    let id = interp.symbols.intern(name);
    let closure = Value::Closure(Rc::new(MoofClosure {
        name: Some(id),
        params: vec![],
        rest_param: None,
        body: ClosureBody::Native(f),
        env: interp.global_env.clone(),
    }));
    class.borrow_mut().add_method(id, closure);
}

// ═══════════════════════════════════════════════════════════════════════
// Numeric helpers
// ═══════════════════════════════════════════════════════════════════════

fn numeric_add(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(Value::Integer(x + y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
        (Value::Integer(x), Value::Float(y)) => Ok(Value::Float(x.to_f64() + y)),
        (Value::Float(x), Value::Integer(y)) => Ok(Value::Float(x + y.to_f64())),
        _ => Err(MoofError::type_error("+ expects numbers")),
    }
}

fn numeric_sub(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(Value::Integer(x - y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
        (Value::Integer(x), Value::Float(y)) => Ok(Value::Float(x.to_f64() - y)),
        (Value::Float(x), Value::Integer(y)) => Ok(Value::Float(x - y.to_f64())),
        _ => Err(MoofError::type_error("- expects numbers")),
    }
}

fn numeric_mul(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(Value::Integer(x * y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
        (Value::Integer(x), Value::Float(y)) => Ok(Value::Float(x.to_f64() * y)),
        (Value::Float(x), Value::Integer(y)) => Ok(Value::Float(x * y.to_f64())),
        _ => Err(MoofError::type_error("* expects numbers")),
    }
}

fn numeric_div(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => {
            if y.is_zero() {
                return Err(MoofError::runtime("Division by zero"));
            }
            match x.checked_div(y) {
                Some(r) => Ok(Value::Integer(r)),
                None => Ok(Value::Float(x.to_f64() / y.to_f64())),
            }
        }
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 {
                return Err(MoofError::runtime("Division by zero"));
            }
            Ok(Value::Float(x / y))
        }
        (Value::Integer(x), Value::Float(y)) => {
            if *y == 0.0 {
                return Err(MoofError::runtime("Division by zero"));
            }
            Ok(Value::Float(x.to_f64() / y))
        }
        (Value::Float(x), Value::Integer(y)) => {
            if y.is_zero() {
                return Err(MoofError::runtime("Division by zero"));
            }
            Ok(Value::Float(x / y.to_f64()))
        }
        _ => Err(MoofError::type_error("/ expects numbers")),
    }
}

fn numeric_rem(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => {
            if y.is_zero() {
                return Err(MoofError::runtime("Modulo by zero"));
            }
            match x.checked_rem(y) {
                Some(r) => Ok(Value::Integer(r)),
                None => Err(MoofError::runtime("Modulo overflow")),
            }
        }
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 {
                return Err(MoofError::runtime("Modulo by zero"));
            }
            Ok(Value::Float(x % y))
        }
        (Value::Integer(x), Value::Float(y)) => {
            if *y == 0.0 {
                return Err(MoofError::runtime("Modulo by zero"));
            }
            Ok(Value::Float(x.to_f64() % y))
        }
        (Value::Float(x), Value::Integer(y)) => {
            if y.is_zero() {
                return Err(MoofError::runtime("Modulo by zero"));
            }
            Ok(Value::Float(x % y.to_f64()))
        }
        _ => Err(MoofError::type_error("% expects numbers")),
    }
}

fn numeric_gt(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x > y),
        (Value::Float(x), Value::Float(y)) => Ok(x > y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() > *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x > y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

fn numeric_lt(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x < y),
        (Value::Float(x), Value::Float(y)) => Ok(x < y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() < *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x < y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

fn numeric_gte(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x >= y),
        (Value::Float(x), Value::Float(y)) => Ok(x >= y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() >= *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x >= y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

fn numeric_lte(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x <= y),
        (Value::Float(x), Value::Float(y)) => Ok(x <= y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() <= *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x <= y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

fn check_arity(name: &str, expected: usize, args: &[Value]) -> Result<()> {
    if args.len() != expected {
        Err(MoofError::arity(&expected.to_string(), args.len(), Some(name)))
    } else {
        Ok(())
    }
}

fn check_min_arity(name: &str, min: usize, args: &[Value]) -> Result<()> {
    if args.len() < min {
        Err(MoofError::arity(&format!("{min}+"), args.len(), Some(name)))
    } else {
        Ok(())
    }
}

/// Invoke a closure value with the given args.
fn invoke(interp: &mut Interpreter, func: &Value, args: Vec<Value>) -> Result<Value> {
    interp.invoke(func.clone(), args)
}

// ═══════════════════════════════════════════════════════════════════════
// Global builtins
// ═══════════════════════════════════════════════════════════════════════

fn install_globals(interp: &mut Interpreter) {
    // Arithmetic
    register(interp, "+", builtin_add);
    register(interp, "-", builtin_sub);
    register(interp, "*", builtin_mul);
    register(interp, "/", builtin_div);
    register(interp, "%", builtin_mod);

    // Comparison
    register(interp, ">", builtin_gt);
    register(interp, "<", builtin_lt);
    register(interp, ">=", builtin_gte);
    register(interp, "<=", builtin_lte);

    // Equality
    register(interp, "=", builtin_eq);
    register(interp, "eq?", builtin_eq_identity);

    // List / cons ops
    register(interp, "cons", builtin_cons);
    register(interp, "car", builtin_car);
    register(interp, "cdr", builtin_cdr);
    register(interp, "list", builtin_list);

    // I/O
    register(interp, "print", builtin_print);
    register(interp, "display", builtin_display);
    register(interp, "format", builtin_format);
    register(interp, "read-line", builtin_read_line);

    // File I/O
    register(interp, "read-file", builtin_read_file);
    register(interp, "write-file", builtin_write_file);
    register(interp, "file-exists?", builtin_file_exists);
    register(interp, "read-lines", builtin_read_lines);

    // Apply
    register(interp, "apply", builtin_apply);

    // Control
    register(interp, "error", builtin_error);
    register(interp, "exit", builtin_exit);
    register(interp, "not", builtin_not);

    // Introspection
    register(interp, "type-of", builtin_type_of);
    register(interp, "range", builtin_range);

    // Timing
    register(interp, "time", builtin_time);

    // Internal helper for curry
    register(interp, "__curry_call", builtin_curry_call);
}

// ── Arithmetic ─────────────────────────────────────────────────────

fn builtin_add(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let mut result = Value::Integer(MoofInt::from(0i64));
    for arg in &args {
        result = numeric_add(&result, arg)?;
    }
    Ok(result)
}

fn builtin_sub(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_min_arity("-", 1, &args)?;
    if args.len() == 1 {
        return match &args[0] {
            Value::Integer(n) => Ok(Value::Integer(-n)),
            Value::Float(n) => Ok(Value::Float(-n)),
            _ => Err(MoofError::type_error("- expects numbers")),
        };
    }
    let mut result = args[0].clone();
    for arg in &args[1..] {
        result = numeric_sub(&result, arg)?;
    }
    Ok(result)
}

fn builtin_mul(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let mut result = Value::Integer(MoofInt::from(1i64));
    for arg in &args {
        result = numeric_mul(&result, arg)?;
    }
    Ok(result)
}

fn builtin_div(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_min_arity("/", 2, &args)?;
    let mut result = args[0].clone();
    for arg in &args[1..] {
        result = numeric_div(&result, arg)?;
    }
    Ok(result)
}

fn builtin_mod(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("%", 2, &args)?;
    numeric_rem(&args[0], &args[1])
}

// ── Comparison ─────────────────────────────────────────────────────

fn builtin_gt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity(">", 2, &args)?;
    Ok(Value::Bool(numeric_gt(&args[0], &args[1])?))
}

fn builtin_lt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("<", 2, &args)?;
    Ok(Value::Bool(numeric_lt(&args[0], &args[1])?))
}

fn builtin_gte(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity(">=", 2, &args)?;
    Ok(Value::Bool(numeric_gte(&args[0], &args[1])?))
}

fn builtin_lte(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("<=", 2, &args)?;
    Ok(Value::Bool(numeric_lte(&args[0], &args[1])?))
}

// ── Equality ───────────────────────────────────────────────────────

fn builtin_eq(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("=", 2, &args)?;
    Ok(Value::Bool(args[0] == args[1]))
}

fn builtin_eq_identity(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("eq?", 2, &args)?;
    let same = match (&args[0], &args[1]) {
        (Value::Cons(a), Value::Cons(b)) => Rc::ptr_eq(a, b),
        (Value::Table(a), Value::Table(b)) => Rc::ptr_eq(a, b),
        (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
        (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
        _ => args[0] == args[1],
    };
    Ok(Value::Bool(same))
}

// ── List / cons ops ────────────────────────────────────────────────

fn builtin_cons(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("cons", 2, &args)?;
    Ok(Value::cons(args[0].clone(), args[1].clone()))
}

fn builtin_car(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("car", 1, &args)?;
    args[0].car().cloned()
}

fn builtin_cdr(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("cdr", 1, &args)?;
    args[0].cdr().cloned()
}

fn builtin_list(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::from_slice(&args))
}

// ── I/O ────────────────────────────────────────────────────────────

fn builtin_print(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let output: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
    println!("{}", output.join(" "));
    Ok(Value::Nil)
}

fn builtin_display(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    use std::io::Write;
    let output: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
    print!("{}", output.join(" "));
    std::io::stdout().flush().ok();
    Ok(Value::Nil)
}

fn builtin_format(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_min_arity("format", 1, &args)?;
    let template = match &args[0] {
        Value::Str(s) => s.to_string(),
        other => format!("{other}"),
    };
    let values = &args[1..];
    let mut idx = 0;
    let mut result = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '~' && chars.peek() == Some(&'a') {
            chars.next();
            if idx < values.len() {
                result.push_str(&format!("{}", values[idx]));
                idx += 1;
            }
        } else {
            result.push(c);
        }
    }
    Ok(Value::Str(Rc::from(result.as_str())))
}

fn builtin_read_line(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    use std::io::{self, BufRead, Write};
    if args.len() == 1 {
        print!("{}", args[0]);
        io::stdout().flush().ok();
    }
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok();
    if line.ends_with('\n') {
        line.pop();
    }
    if line.ends_with('\r') {
        line.pop();
    }
    Ok(Value::Str(Rc::from(line.as_str())))
}

// ── File I/O ───────────────────────────────────────────────────────

fn builtin_read_file(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("read-file", 1, &args)?;
    let path = args[0].as_str()?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| MoofError::io(format!("read-file: {e}")))?;
    Ok(Value::Str(Rc::from(content.as_str())))
}

fn builtin_write_file(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("write-file", 2, &args)?;
    let path = args[0].as_str()?;
    let content = args[1].as_str()?;
    std::fs::write(path, content)
        .map_err(|e| MoofError::io(format!("write-file: {e}")))?;
    Ok(Value::Nil)
}

fn builtin_file_exists(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("file-exists?", 1, &args)?;
    let path = args[0].as_str()?;
    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

fn builtin_read_lines(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("read-lines", 1, &args)?;
    let path = args[0].as_str()?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| MoofError::io(format!("read-lines: {e}")))?;
    let lines: Vec<Value> = content
        .lines()
        .map(|l| Value::Str(Rc::from(l)))
        .collect();
    Ok(Value::from_slice(&lines))
}

// ── Apply ──────────────────────────────────────────────────────────

fn builtin_apply(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("apply", 2, &args)?;
    let func = &args[0];
    let func_args = args[1].to_vec()?;
    invoke(interp, func, func_args)
}

// ── Control ────────────────────────────────────────────────────────

fn builtin_error(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("error", 1, &args)?;
    let msg = format!("{}", args[0]);
    let cls = interp.runtime_error_class.clone();
    let obj = interp.make_error_object(&cls, &msg);
    Err(MoofError::runtime(&msg).with_object(obj))
}

fn builtin_exit(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let code = if args.len() == 1 {
        match &args[0] {
            Value::Integer(n) => n.to_i64().unwrap_or(0) as i32,
            _ => 0,
        }
    } else {
        0
    };
    std::process::exit(code);
}

fn builtin_not(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("not", 1, &args)?;
    Ok(Value::Bool(!args[0].is_truthy()))
}

// ── Introspection ──────────────────────────────────────────────────

fn builtin_type_of(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("type-of", 1, &args)?;
    Ok(Value::Str(Rc::from(args[0].type_name())))
}

fn builtin_range(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let (start, end, step) = match args.len() {
        1 => {
            let end = args[0].as_int()?.clone();
            (MoofInt::from_i64(0), end, MoofInt::from_i64(1))
        }
        2 => {
            let start = args[0].as_int()?.clone();
            let end = args[1].as_int()?.clone();
            (start, end, MoofInt::from_i64(1))
        }
        3 => {
            let start = args[0].as_int()?.clone();
            let end = args[1].as_int()?.clone();
            let step = args[2].as_int()?.clone();
            if step == MoofInt::from_i64(0) {
                return Err(MoofError::runtime("range: step cannot be zero"));
            }
            (start, end, step)
        }
        _ => return Err(MoofError::arity("1-3", args.len(), Some("range"))),
    };

    Ok(Value::Range(Rc::new(MoofRange { start, end, step })))
}

// ── Timing ─────────────────────────────────────────────────────────

fn builtin_time(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("time", 1, &args)?;
    let t0 = std::time::Instant::now();
    let result = invoke(interp, &args[0], vec![])?;
    let elapsed = t0.elapsed();
    println!("Elapsed: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    Ok(result)
}

// ── Internal: curry call helper ──────────────────────────────────────
// Called as (__curry_call func partial_list rest_list)
// Concatenates partial_list and rest_list, then invokes func with the combined args.
fn builtin_curry_call(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("__curry_call", 3, &args)?;
    let func = &args[0];
    let mut all_args = args[1].to_vec()?;
    let rest = args[2].to_vec()?;
    all_args.extend(rest);
    invoke(interp, func, all_args)
}

// ═══════════════════════════════════════════════════════════════════════
// Integer methods
// ═══════════════════════════════════════════════════════════════════════

fn install_integer_methods(interp: &mut Interpreter) {
    let class = interp.integer_class.clone();

    register_method(interp, &class, "abs", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Integer(n.abs()))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    register_method(interp, &class, "to_f", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Float(n.to_f64()))
    });

    register_method(interp, &class, "to_i", |_interp, args| {
        Ok(args[0].clone())
    });

    register_method(interp, &class, "zero?", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Bool(n.is_zero()))
    });

    register_method(interp, &class, "positive?", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Bool(n.is_positive()))
    });

    register_method(interp, &class, "negative?", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Bool(n.is_negative()))
    });

    register_method(interp, &class, "even?", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Bool(n.is_even()))
    });

    register_method(interp, &class, "odd?", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Bool(!n.is_even()))
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Integer")))
    });

    register_method(interp, &class, "sqrt", |_interp, args| {
        let n = args[0].as_int()?;
        Ok(Value::Float(n.to_f64().sqrt()))
    });

    register_method(interp, &class, "pow:", |_interp, args| {
        let base = args[0].as_int()?;
        match &args[1] {
            Value::Integer(exp) => Ok(Value::Integer(base.pow(exp))),
            Value::Float(exp) => Ok(Value::Float(base.to_f64().powf(*exp))),
            _ => Err(MoofError::type_error("pow: expects a number argument")),
        }
    });

    register_method(interp, &class, "max:", |_interp, args| {
        if numeric_gt(&args[0], &args[1])? { Ok(args[0].clone()) } else { Ok(args[1].clone()) }
    });

    register_method(interp, &class, "min:", |_interp, args| {
        if numeric_lt(&args[0], &args[1])? { Ok(args[0].clone()) } else { Ok(args[1].clone()) }
    });

    // Arithmetic as methods
    register_method(interp, &class, "+", |_interp, args| {
        numeric_add(&args[0], &args[1])
    });
    register_method(interp, &class, "-", |_interp, args| {
        numeric_sub(&args[0], &args[1])
    });
    register_method(interp, &class, "*", |_interp, args| {
        numeric_mul(&args[0], &args[1])
    });
    register_method(interp, &class, "/", |_interp, args| {
        numeric_div(&args[0], &args[1])
    });
    register_method(interp, &class, "%", |_interp, args| {
        numeric_rem(&args[0], &args[1])
    });

    // Comparison as methods
    register_method(interp, &class, ">", |_interp, args| {
        Ok(Value::Bool(numeric_gt(&args[0], &args[1])?))
    });
    register_method(interp, &class, "<", |_interp, args| {
        Ok(Value::Bool(numeric_lt(&args[0], &args[1])?))
    });
    register_method(interp, &class, ">=", |_interp, args| {
        Ok(Value::Bool(numeric_gte(&args[0], &args[1])?))
    });
    register_method(interp, &class, "<=", |_interp, args| {
        Ok(Value::Bool(numeric_lte(&args[0], &args[1])?))
    });

    // ── ** (alias for pow:) ──────────────────────────────────────────
    register_method(interp, &class, "**", |_interp, args| {
        let base = args[0].as_int()?;
        match &args[1] {
            Value::Integer(exp) => Ok(Value::Integer(base.pow(exp))),
            Value::Float(exp) => Ok(Value::Float(base.to_f64().powf(*exp))),
            _ => Err(MoofError::type_error("**: expects a number argument")),
        }
    });

    // ── gcd: ─────────────────────────────────────────────────────────
    register_method(interp, &class, "gcd:", |_interp, args| {
        let a = args[0].as_int()?;
        let b = args[1].as_int()?;
        Ok(Value::Integer(a.gcd(b)))
    });

    // ── lcm: ─────────────────────────────────────────────────────────
    register_method(interp, &class, "lcm:", |_interp, args| {
        let a = args[0].as_int()?;
        let b = args[1].as_int()?;
        let g = a.gcd(b);
        if g.is_zero() {
            Ok(Value::Integer(MoofInt::from_i64(0)))
        } else {
            // lcm = |a * b| / gcd(a, b)
            let product = a.clone() * b.clone();
            let lcm = product / g;
            Ok(Value::Integer(lcm.abs()))
        }
    });

    // ── times: ───────────────────────────────────────────────────────
    register_method(interp, &class, "times:", |interp, args| {
        let n = args[0].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("times: integer too large"))?;
        let func = &args[1];
        for i in 0..n {
            invoke(interp, func, vec![Value::Integer(MoofInt::from_i64(i))])?;
        }
        Ok(Value::Nil)
    });

    // ── upto: ────────────────────────────────────────────────────────
    register_method(interp, &class, "upto:", |_interp, args| {
        let start = args[0].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("upto: integer too large"))?;
        let end = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("upto: integer too large"))?;
        let items: Vec<Value> = (start..=end)
            .map(|i| Value::Integer(MoofInt::from_i64(i)))
            .collect();
        Ok(Value::from_slice(&items))
    });

    // ── downto: ──────────────────────────────────────────────────────
    register_method(interp, &class, "downto:", |_interp, args| {
        let start = args[0].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("downto: integer too large"))?;
        let end = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("downto: integer too large"))?;
        let items: Vec<Value> = (end..=start)
            .rev()
            .map(|i| Value::Integer(MoofInt::from_i64(i)))
            .collect();
        Ok(Value::from_slice(&items))
    });

    // ── between:and: ─────────────────────────────────────────────────
    register_method(interp, &class, "between:and:", |_interp, args| {
        let val = &args[0];
        let lo = &args[1];
        let hi = &args[2];
        Ok(Value::Bool(numeric_gte(val, lo)? && numeric_lte(val, hi)?))
    });

    // ── bit_and: ─────────────────────────────────────────────────────
    register_method(interp, &class, "bit_and:", |_interp, args| {
        let a = args[0].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("bit_and: only works on small integers"))?;
        let b = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("bit_and: only works on small integers"))?;
        Ok(Value::Integer(MoofInt::from_i64(a & b)))
    });

    // ── bit_or: ──────────────────────────────────────────────────────
    register_method(interp, &class, "bit_or:", |_interp, args| {
        let a = args[0].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("bit_or: only works on small integers"))?;
        let b = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("bit_or: only works on small integers"))?;
        Ok(Value::Integer(MoofInt::from_i64(a | b)))
    });

    // ── bit_xor: ─────────────────────────────────────────────────────
    register_method(interp, &class, "bit_xor:", |_interp, args| {
        let a = args[0].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("bit_xor: only works on small integers"))?;
        let b = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("bit_xor: only works on small integers"))?;
        Ok(Value::Integer(MoofInt::from_i64(a ^ b)))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Float methods
// ═══════════════════════════════════════════════════════════════════════

fn install_float_methods(interp: &mut Interpreter) {
    let class = interp.float_class.clone();

    register_method(interp, &class, "abs", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Float(n.abs()))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    register_method(interp, &class, "to_f", |_interp, args| {
        Ok(args[0].clone())
    });

    register_method(interp, &class, "to_i", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Integer(MoofInt::from_i64(n as i64)))
    });

    register_method(interp, &class, "zero?", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Bool(n == 0.0))
    });

    register_method(interp, &class, "positive?", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Bool(n > 0.0))
    });

    register_method(interp, &class, "negative?", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Bool(n < 0.0))
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Float")))
    });

    register_method(interp, &class, "round", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Float(n.round()))
    });

    register_method(interp, &class, "floor", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Float(n.floor()))
    });

    register_method(interp, &class, "ceil", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Float(n.ceil()))
    });

    register_method(interp, &class, "sqrt", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Float(n.sqrt()))
    });

    register_method(interp, &class, "pow:", |_interp, args| {
        let base = args[0].as_float()?;
        let exp = args[1].as_number_f64()?;
        Ok(Value::Float(base.powf(exp)))
    });

    register_method(interp, &class, "max:", |_interp, args| {
        if numeric_gt(&args[0], &args[1])? { Ok(args[0].clone()) } else { Ok(args[1].clone()) }
    });

    register_method(interp, &class, "min:", |_interp, args| {
        if numeric_lt(&args[0], &args[1])? { Ok(args[0].clone()) } else { Ok(args[1].clone()) }
    });

    register_method(interp, &class, "nan?", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Bool(n.is_nan()))
    });

    register_method(interp, &class, "infinite?", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Bool(n.is_infinite()))
    });

    // Arithmetic as methods
    register_method(interp, &class, "+", |_interp, args| {
        numeric_add(&args[0], &args[1])
    });
    register_method(interp, &class, "-", |_interp, args| {
        numeric_sub(&args[0], &args[1])
    });
    register_method(interp, &class, "*", |_interp, args| {
        numeric_mul(&args[0], &args[1])
    });
    register_method(interp, &class, "/", |_interp, args| {
        numeric_div(&args[0], &args[1])
    });

    // Comparison as methods
    register_method(interp, &class, ">", |_interp, args| {
        Ok(Value::Bool(numeric_gt(&args[0], &args[1])?))
    });
    register_method(interp, &class, "<", |_interp, args| {
        Ok(Value::Bool(numeric_lt(&args[0], &args[1])?))
    });
    register_method(interp, &class, ">=", |_interp, args| {
        Ok(Value::Bool(numeric_gte(&args[0], &args[1])?))
    });
    register_method(interp, &class, "<=", |_interp, args| {
        Ok(Value::Bool(numeric_lte(&args[0], &args[1])?))
    });

    // ── ** (alias for pow:) ──────────────────────────────────────────
    register_method(interp, &class, "**", |_interp, args| {
        let base = args[0].as_float()?;
        let exp = args[1].as_number_f64()?;
        Ok(Value::Float(base.powf(exp)))
    });

    // ── truncate ─────────────────────────────────────────────────────
    register_method(interp, &class, "truncate", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Float(n.trunc()))
    });

    // ── finite? ──────────────────────────────────────────────────────
    register_method(interp, &class, "finite?", |_interp, args| {
        let n = args[0].as_float()?;
        Ok(Value::Bool(n.is_finite()))
    });

    // ── round: (round to N decimal places) ───────────────────────────
    register_method(interp, &class, "round:", |_interp, args| {
        let n = args[0].as_float()?;
        let precision = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("round: precision too large"))?;
        let factor = 10f64.powi(precision as i32);
        Ok(Value::Float((n * factor).round() / factor))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// String methods
// ═══════════════════════════════════════════════════════════════════════

fn install_string_methods(interp: &mut Interpreter) {
    let class = interp.string_class.clone();

    register_method(interp, &class, "length", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Integer(MoofInt::from_i64(s.len() as i64)))
    });

    register_method(interp, &class, "uppercase", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Str(Rc::from(s.to_uppercase().as_str())))
    });

    register_method(interp, &class, "lowercase", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Str(Rc::from(s.to_lowercase().as_str())))
    });

    register_method(interp, &class, "reverse", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Str(Rc::from(s.chars().rev().collect::<String>().as_str())))
    });

    register_method(interp, &class, "trim", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Str(Rc::from(s.trim())))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(args[0].clone())
    });

    register_method(interp, &class, "to_i", |_interp, args| {
        let s = args[0].as_str()?;
        match s.trim().parse::<i64>() {
            Ok(n) => Ok(Value::Integer(MoofInt::from_i64(n))),
            Err(_) => Err(MoofError::type_error(format!("Cannot convert '{}' to integer", s))),
        }
    });

    register_method(interp, &class, "to_f", |_interp, args| {
        let s = args[0].as_str()?;
        match s.trim().parse::<f64>() {
            Ok(n) => Ok(Value::Float(n)),
            Err(_) => Err(MoofError::type_error(format!("Cannot convert '{}' to float", s))),
        }
    });

    register_method(interp, &class, "chars", |_interp, args| {
        let s = args[0].as_str()?;
        let chars: Vec<Value> = s.chars().map(|c| Value::Str(Rc::from(c.to_string().as_str()))).collect();
        Ok(Value::from_slice(&chars))
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("String")))
    });

    register_method(interp, &class, "empty?", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Bool(s.is_empty()))
    });

    register_method(interp, &class, "at:", |_interp, args| {
        let s = args[0].as_str()?;
        let idx = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("at: index too large"))? as usize;
        match s.chars().nth(idx) {
            Some(c) => Ok(Value::Str(Rc::from(c.to_string().as_str()))),
            None => Ok(Value::Nil),
        }
    });

    register_method(interp, &class, "contains:", |_interp, args| {
        let s = args[0].as_str()?;
        let needle = args[1].as_str()?;
        Ok(Value::Bool(s.contains(needle)))
    });

    register_method(interp, &class, "starts_with:", |_interp, args| {
        let s = args[0].as_str()?;
        let prefix = args[1].as_str()?;
        Ok(Value::Bool(s.starts_with(prefix)))
    });

    register_method(interp, &class, "ends_with:", |_interp, args| {
        let s = args[0].as_str()?;
        let suffix = args[1].as_str()?;
        Ok(Value::Bool(s.ends_with(suffix)))
    });

    register_method(interp, &class, "split:", |_interp, args| {
        let s = args[0].as_str()?;
        let delim = args[1].as_str()?;
        let parts: Vec<Value> = s.split(delim).map(|p| Value::Str(Rc::from(p))).collect();
        Ok(Value::from_slice(&parts))
    });

    register_method(interp, &class, "concat:", |_interp, args| {
        let s = args[0].as_str()?;
        let other = args[1].as_str()?;
        let result = format!("{s}{other}");
        Ok(Value::Str(Rc::from(result.as_str())))
    });

    register_method(interp, &class, "replace_all:with:", |_interp, args| {
        let s = args[0].as_str()?;
        let from = args[1].as_str()?;
        let to = args[2].as_str()?;
        Ok(Value::Str(Rc::from(s.replace(from, to).as_str())))
    });

    register_method(interp, &class, "slice:length:", |_interp, args| {
        let s = args[0].as_str()?;
        let start = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("slice: index too large"))? as usize;
        let len = args[2].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("slice: length too large"))? as usize;
        let result: String = s.chars().skip(start).take(len).collect();
        Ok(Value::Str(Rc::from(result.as_str())))
    });

    // ── capitalize ───────────────────────────────────────────────────
    register_method(interp, &class, "capitalize", |_interp, args| {
        let s = args[0].as_str()?;
        let mut chars = s.chars();
        let result = match chars.next() {
            None => String::new(),
            Some(first) => {
                let mut r = first.to_uppercase().to_string();
                r.extend(chars.map(|c| c.to_lowercase().next().unwrap_or(c)));
                r
            }
        };
        Ok(Value::Str(Rc::from(result.as_str())))
    });

    // ── strip (alias for trim) ───────────────────────────────────────
    register_method(interp, &class, "strip", |_interp, args| {
        let s = args[0].as_str()?;
        Ok(Value::Str(Rc::from(s.trim())))
    });

    // ── index_of: ────────────────────────────────────────────────────
    register_method(interp, &class, "index_of:", |_interp, args| {
        let s = args[0].as_str()?;
        let needle = args[1].as_str()?;
        match s.find(needle) {
            Some(byte_idx) => {
                // Convert byte index to char index
                let char_idx = s[..byte_idx].chars().count() as i64;
                Ok(Value::Integer(MoofInt::from_i64(char_idx)))
            }
            None => Ok(Value::Integer(MoofInt::from_i64(-1))),
        }
    });

    // ── to_sym ───────────────────────────────────────────────────────
    register_method(interp, &class, "to_sym", |interp, args| {
        let s = args[0].as_str()?;
        let id = interp.symbols.intern(s);
        Ok(Value::Symbol(id))
    });

    // ── each_char: ───────────────────────────────────────────────────
    register_method(interp, &class, "each_char:", |interp, args| {
        let s = args[0].as_str()?.to_string();
        let func = &args[1];
        for c in s.chars() {
            invoke(interp, func, vec![Value::Str(Rc::from(c.to_string().as_str()))])?;
        }
        Ok(Value::Nil)
    });

    // ── each_line: ───────────────────────────────────────────────────
    register_method(interp, &class, "each_line:", |interp, args| {
        let s = args[0].as_str()?.to_string();
        let func = &args[1];
        for line in s.split('\n') {
            invoke(interp, func, vec![Value::Str(Rc::from(line))])?;
        }
        Ok(Value::Nil)
    });

    // ── * (repeat string N times) ────────────────────────────────────
    register_method(interp, &class, "*", |_interp, args| {
        let s = args[0].as_str()?;
        let n = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("*: count too large"))? as usize;
        Ok(Value::Str(Rc::from(s.repeat(n).as_str())))
    });

    // ── bytes ────────────────────────────────────────────────────────
    register_method(interp, &class, "bytes", |_interp, args| {
        let s = args[0].as_str()?;
        let byte_vals: Vec<Value> = s.bytes()
            .map(|b| Value::Integer(MoofInt::from_i64(b as i64)))
            .collect();
        Ok(Value::from_slice(&byte_vals))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Cons (list) methods
// ═══════════════════════════════════════════════════════════════════════

fn install_cons_methods(interp: &mut Interpreter) {
    let class = interp.cons_class.clone();

    register_method(interp, &class, "car", |_interp, args| {
        args[0].car().cloned()
    });

    register_method(interp, &class, "cdr", |_interp, args| {
        args[0].cdr().cloned()
    });

    register_method(interp, &class, "empty?", |_interp, _args| {
        Ok(Value::Bool(false)) // a cons cell is never empty
    });

    register_method(interp, &class, "length", |_interp, args| {
        let len = cons::cons_length(&args[0]);
        Ok(Value::Integer(MoofInt::from_i64(len as i64)))
    });

    register_method(interp, &class, "first", |_interp, args| {
        args[0].car().cloned()
    });

    register_method(interp, &class, "rest", |_interp, args| {
        args[0].cdr().cloned()
    });

    register_method(interp, &class, "last", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        Ok(items.last().cloned().unwrap_or(Value::Nil))
    });

    register_method(interp, &class, "reverse", |_interp, args| {
        let mut items = cons::cons_to_vec(&args[0]);
        items.reverse();
        Ok(cons::vec_to_cons(&items))
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Cons")))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    register_method(interp, &class, "map:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            results.push(invoke(interp, func, vec![item])?);
        }
        Ok(cons::vec_to_cons(&results))
    });

    register_method(interp, &class, "filter:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        let mut results = Vec::new();
        for item in items {
            let keep = invoke(interp, func, vec![item.clone()])?;
            if keep.is_truthy() {
                results.push(item);
            }
        }
        Ok(cons::vec_to_cons(&results))
    });

    register_method(interp, &class, "each:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        for item in items {
            invoke(interp, func, vec![item])?;
        }
        Ok(Value::Nil)
    });

    register_method(interp, &class, "any:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        for item in items {
            if invoke(interp, func, vec![item])?.is_truthy() {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "all:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        for item in items {
            if !invoke(interp, func, vec![item])?.is_truthy() {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    });

    register_method(interp, &class, "none:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        for item in items {
            if invoke(interp, func, vec![item])?.is_truthy() {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    });

    register_method(interp, &class, "at:", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let idx = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("at: index too large"))? as usize;
        Ok(items.get(idx).cloned().unwrap_or(Value::Nil))
    });

    register_method(interp, &class, "contains:", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let needle = &args[1];
        Ok(Value::Bool(items.iter().any(|v| v == needle)))
    });

    register_method(interp, &class, "push:", |_interp, args| {
        let mut items = cons::cons_to_vec(&args[0]);
        items.push(args[1].clone());
        Ok(cons::vec_to_cons(&items))
    });

    register_method(interp, &class, "flatten", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let mut flat = Vec::new();
        for item in items {
            if item.is_cons() {
                flat.extend(cons::cons_to_vec(&item));
            } else {
                flat.push(item);
            }
        }
        Ok(cons::vec_to_cons(&flat))
    });

    register_method(interp, &class, "sort", |_interp, args| {
        let mut items = cons::cons_to_vec(&args[0]);
        items.sort_by(|a, b| {
            match (a.as_number_f64(), b.as_number_f64()) {
                (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => std::cmp::Ordering::Equal,
            }
        });
        Ok(cons::vec_to_cons(&items))
    });

    register_method(interp, &class, "take:", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let n = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("take: count too large"))? as usize;
        let taken: Vec<Value> = items.into_iter().take(n).collect();
        Ok(cons::vec_to_cons(&taken))
    });

    register_method(interp, &class, "drop:", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let n = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("drop: count too large"))? as usize;
        let dropped: Vec<Value> = items.into_iter().skip(n).collect();
        Ok(cons::vec_to_cons(&dropped))
    });

    register_method(interp, &class, "zip:", |_interp, args| {
        let items_a = cons::cons_to_vec(&args[0]);
        let items_b = cons::cons_to_vec(&args[1]);
        let pairs: Vec<Value> = items_a
            .into_iter()
            .zip(items_b)
            .map(|(a, b)| Value::cons(a, Value::cons(b, Value::Nil)))
            .collect();
        Ok(cons::vec_to_cons(&pairs))
    });

    register_method(interp, &class, "join:", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let sep = args[1].as_str()?;
        let strings: Vec<String> = items.iter().map(|v| format!("{v}")).collect();
        Ok(Value::Str(Rc::from(strings.join(sep).as_str())))
    });

    register_method(interp, &class, "reduce:init:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        let mut acc = args[2].clone();
        for item in items {
            acc = invoke(interp, func, vec![acc, item])?;
        }
        Ok(acc)
    });

    // ── sort_by: ─────────────────────────────────────────────────────
    register_method(interp, &class, "sort_by:", |interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let func = &args[1];
        // Compute keys for each element
        let mut keyed: Vec<(Value, Value)> = Vec::with_capacity(items.len());
        for item in items {
            let key = invoke(interp, func, vec![item.clone()])?;
            keyed.push((key, item));
        }
        keyed.sort_by(|(ka, _), (kb, _)| {
            match (ka.as_number_f64(), kb.as_number_f64()) {
                (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => {
                    // Fall back to string comparison
                    format!("{ka}").cmp(&format!("{kb}"))
                }
            }
        });
        let sorted: Vec<Value> = keyed.into_iter().map(|(_, v)| v).collect();
        Ok(cons::vec_to_cons(&sorted))
    });

    // ── uniq ─────────────────────────────────────────────────────────
    register_method(interp, &class, "uniq", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let mut seen = Vec::new();
        let mut result = Vec::new();
        for item in items {
            if !seen.iter().any(|s: &Value| s == &item) {
                seen.push(item.clone());
                result.push(item);
            }
        }
        Ok(cons::vec_to_cons(&result))
    });

    // ── nth: (alias for at:) ─────────────────────────────────────────
    register_method(interp, &class, "nth:", |_interp, args| {
        let items = cons::cons_to_vec(&args[0]);
        let idx = args[1].as_int()?.to_i64()
            .ok_or_else(|| MoofError::type_error("nth: index too large"))? as usize;
        Ok(items.get(idx).cloned().unwrap_or(Value::Nil))
    });

    // ── append: ──────────────────────────────────────────────────────
    register_method(interp, &class, "append:", |_interp, args| {
        let mut items = cons::cons_to_vec(&args[0]);
        let other = cons::cons_to_vec(&args[1]);
        items.extend(other);
        Ok(cons::vec_to_cons(&items))
    });

    // ── cons: (prepend an element) ───────────────────────────────────
    register_method(interp, &class, "cons:", |_interp, args| {
        Ok(Value::cons(args[1].clone(), args[0].clone()))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Table methods
// ═══════════════════════════════════════════════════════════════════════

fn install_table_methods(interp: &mut Interpreter) {
    let class = interp.table_class.clone();

    register_method(interp, &class, "length", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => Ok(Value::Integer(MoofInt::from_i64(tbl.borrow().len() as i64))),
            _ => Err(MoofError::type_error("length: expected Table")),
        }
    });

    register_method(interp, &class, "empty?", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => Ok(Value::Bool(tbl.borrow().is_empty())),
            _ => Err(MoofError::type_error("empty?: expected Table")),
        }
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Table")))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    register_method(interp, &class, "keys", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let keys = tbl.borrow().keys();
                Ok(Value::from_slice(&keys))
            }
            _ => Err(MoofError::type_error("keys: expected Table")),
        }
    });

    register_method(interp, &class, "values", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let vals: Vec<Value> = tbl.borrow().values().into_iter().cloned().collect();
                Ok(Value::from_slice(&vals))
            }
            _ => Err(MoofError::type_error("values: expected Table")),
        }
    });

    register_method(interp, &class, "at:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                Ok(tbl.borrow().get(&args[1]).cloned().unwrap_or(Value::Nil))
            }
            _ => Err(MoofError::type_error("at: expected Table")),
        }
    });

    register_method(interp, &class, "get:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                Ok(tbl.borrow().get(&args[1]).cloned().unwrap_or(Value::Nil))
            }
            _ => Err(MoofError::type_error("get: expected Table")),
        }
    });

    register_method(interp, &class, "put:value:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                tbl.borrow_mut().set(args[1].clone(), args[2].clone());
                Ok(args[0].clone())
            }
            _ => Err(MoofError::type_error("put:value: expected Table")),
        }
    });

    register_method(interp, &class, "set:to:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                tbl.borrow_mut().set(args[1].clone(), args[2].clone());
                Ok(args[0].clone())
            }
            _ => Err(MoofError::type_error("set:to: expected Table")),
        }
    });

    register_method(interp, &class, "remove:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let key = args[1].as_str()?;
                tbl.borrow_mut().hash.shift_remove(key);
                Ok(args[0].clone())
            }
            _ => Err(MoofError::type_error("remove: expected Table")),
        }
    });

    register_method(interp, &class, "has_key:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                Ok(Value::Bool(tbl.borrow().get(&args[1]).is_some()))
            }
            _ => Err(MoofError::type_error("has_key: expected Table")),
        }
    });

    register_method(interp, &class, "merge:", |_interp, args| {
        match (&args[0], &args[1]) {
            (Value::Table(a), Value::Table(b)) => {
                let mut new_tbl = MoofTable::new();
                {
                    let a_ref = a.borrow();
                    for v in &a_ref.array {
                        new_tbl.array.push(v.clone());
                    }
                    for (k, v) in &a_ref.hash {
                        new_tbl.hash.insert(k.clone(), v.clone());
                    }
                }
                {
                    let b_ref = b.borrow();
                    for (k, v) in &b_ref.hash {
                        new_tbl.hash.insert(k.clone(), v.clone());
                    }
                }
                Ok(Value::Table(Rc::new(RefCell::new(new_tbl))))
            }
            _ => Err(MoofError::type_error("merge: expects two Tables")),
        }
    });

    register_method(interp, &class, "push:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                tbl.borrow_mut().array.push(args[1].clone());
                Ok(args[0].clone())
            }
            _ => Err(MoofError::type_error("push: expected Table")),
        }
    });

    register_method(interp, &class, "pop", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                Ok(tbl.borrow_mut().array.pop().unwrap_or(Value::Nil))
            }
            _ => Err(MoofError::type_error("pop: expected Table")),
        }
    });

    register_method(interp, &class, "first", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                Ok(tbl.borrow().array.first().cloned().unwrap_or(Value::Nil))
            }
            _ => Err(MoofError::type_error("first: expected Table")),
        }
    });

    register_method(interp, &class, "last", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                Ok(tbl.borrow().array.last().cloned().unwrap_or(Value::Nil))
            }
            _ => Err(MoofError::type_error("last: expected Table")),
        }
    });

    register_method(interp, &class, "each:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let entries: Vec<(String, Value)> = tbl
                    .borrow()
                    .hash
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (k, v) in entries {
                    invoke(interp, func, vec![Value::Str(Rc::from(k.as_str())), v])?;
                }
                let array: Vec<Value> = tbl.borrow().array.clone();
                for (i, v) in array.into_iter().enumerate() {
                    invoke(interp, func, vec![Value::Integer(MoofInt::from_i64(i as i64)), v])?;
                }
                Ok(Value::Nil)
            }
            _ => Err(MoofError::type_error("each: expected Table")),
        }
    });

    register_method(interp, &class, "map:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let mut new_tbl = MoofTable::new();
                {
                    let t = tbl.borrow();
                    for v in &t.array {
                        new_tbl.array.push(invoke(interp, func, vec![v.clone()])?);
                    }
                    for (k, v) in &t.hash {
                        let result = invoke(interp, func, vec![Value::Str(Rc::from(k.as_str())), v.clone()])?;
                        new_tbl.hash.insert(k.clone(), result);
                    }
                }
                Ok(Value::Table(Rc::new(RefCell::new(new_tbl))))
            }
            _ => Err(MoofError::type_error("map: expected Table")),
        }
    });

    register_method(interp, &class, "filter:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let mut new_tbl = MoofTable::new();
                {
                    let t = tbl.borrow();
                    for v in &t.array {
                        if invoke(interp, func, vec![v.clone()])?.is_truthy() {
                            new_tbl.array.push(v.clone());
                        }
                    }
                    for (k, v) in &t.hash {
                        if invoke(interp, func, vec![Value::Str(Rc::from(k.as_str())), v.clone()])?.is_truthy() {
                            new_tbl.hash.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(Value::Table(Rc::new(RefCell::new(new_tbl))))
            }
            _ => Err(MoofError::type_error("filter: expected Table")),
        }
    });

    // ── shift ────────────────────────────────────────────────────────
    register_method(interp, &class, "shift", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let mut t = tbl.borrow_mut();
                if t.array.is_empty() {
                    Ok(Value::Nil)
                } else {
                    Ok(t.array.remove(0))
                }
            }
            _ => Err(MoofError::type_error("shift: expected Table")),
        }
    });

    // ── unshift: ─────────────────────────────────────────────────────
    register_method(interp, &class, "unshift:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                tbl.borrow_mut().array.insert(0, args[1].clone());
                Ok(args[0].clone())
            }
            _ => Err(MoofError::type_error("unshift: expected Table")),
        }
    });

    // ── compact ──────────────────────────────────────────────────────
    register_method(interp, &class, "compact", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let mut new_tbl = MoofTable::new();
                {
                    let t = tbl.borrow();
                    for v in &t.array {
                        if !v.is_nil() {
                            new_tbl.array.push(v.clone());
                        }
                    }
                    for (k, v) in &t.hash {
                        if !v.is_nil() {
                            new_tbl.hash.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(Value::Table(Rc::new(RefCell::new(new_tbl))))
            }
            _ => Err(MoofError::type_error("compact: expected Table")),
        }
    });

    // ── find: ────────────────────────────────────────────────────────
    register_method(interp, &class, "find:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let array: Vec<Value> = tbl.borrow().array.clone();
                for v in array {
                    if invoke(interp, func, vec![v.clone()])?.is_truthy() {
                        return Ok(v);
                    }
                }
                Ok(Value::Nil)
            }
            _ => Err(MoofError::type_error("find: expected Table")),
        }
    });

    // ── count: ───────────────────────────────────────────────────────
    register_method(interp, &class, "count:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let array: Vec<Value> = tbl.borrow().array.clone();
                let mut count = 0i64;
                for v in array {
                    if invoke(interp, func, vec![v])?.is_truthy() {
                        count += 1;
                    }
                }
                Ok(Value::Integer(MoofInt::from_i64(count)))
            }
            _ => Err(MoofError::type_error("count: expected Table")),
        }
    });

    // ── entries ──────────────────────────────────────────────────────
    register_method(interp, &class, "entries", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let t = tbl.borrow();
                let mut pairs = Vec::new();
                for (k, v) in &t.hash {
                    let pair = Value::from_slice(&[
                        Value::Str(Rc::from(k.as_str())),
                        v.clone(),
                    ]);
                    pairs.push(pair);
                }
                Ok(Value::from_slice(&pairs))
            }
            _ => Err(MoofError::type_error("entries: expected Table")),
        }
    });

    // ── has_value: ───────────────────────────────────────────────────
    register_method(interp, &class, "has_value:", |_interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let t = tbl.borrow();
                let needle = &args[1];
                for v in &t.array {
                    if v == needle {
                        return Ok(Value::Bool(true));
                    }
                }
                for v in t.hash.values() {
                    if v == needle {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            _ => Err(MoofError::type_error("has_value: expected Table")),
        }
    });

    // ── each_pair: ───────────────────────────────────────────────────
    register_method(interp, &class, "each_pair:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let entries: Vec<(String, Value)> = tbl
                    .borrow()
                    .hash
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (k, v) in entries {
                    invoke(interp, func, vec![Value::Str(Rc::from(k.as_str())), v])?;
                }
                Ok(Value::Nil)
            }
            _ => Err(MoofError::type_error("each_pair: expected Table")),
        }
    });

    // ── each_with_index: ─────────────────────────────────────────────
    register_method(interp, &class, "each_with_index:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let array: Vec<Value> = tbl.borrow().array.clone();
                for (i, v) in array.into_iter().enumerate() {
                    invoke(interp, func, vec![v, Value::Integer(MoofInt::from_i64(i as i64))])?;
                }
                Ok(Value::Nil)
            }
            _ => Err(MoofError::type_error("each_with_index: expected Table")),
        }
    });

    // ── select: (alias for filter on hash, predicate receives key, value) ──
    register_method(interp, &class, "select:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let mut new_tbl = MoofTable::new();
                {
                    let t = tbl.borrow();
                    for v in &t.array {
                        if invoke(interp, func, vec![v.clone()])?.is_truthy() {
                            new_tbl.array.push(v.clone());
                        }
                    }
                    for (k, v) in &t.hash {
                        if invoke(interp, func, vec![Value::Str(Rc::from(k.as_str())), v.clone()])?.is_truthy() {
                            new_tbl.hash.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(Value::Table(Rc::new(RefCell::new(new_tbl))))
            }
            _ => Err(MoofError::type_error("select: expected Table")),
        }
    });

    // ── reject: (opposite of select) ─────────────────────────────────
    register_method(interp, &class, "reject:", |interp, args| {
        match &args[0] {
            Value::Table(tbl) => {
                let func = &args[1];
                let mut new_tbl = MoofTable::new();
                {
                    let t = tbl.borrow();
                    for v in &t.array {
                        if !invoke(interp, func, vec![v.clone()])?.is_truthy() {
                            new_tbl.array.push(v.clone());
                        }
                    }
                    for (k, v) in &t.hash {
                        if !invoke(interp, func, vec![Value::Str(Rc::from(k.as_str())), v.clone()])?.is_truthy() {
                            new_tbl.hash.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(Value::Table(Rc::new(RefCell::new(new_tbl))))
            }
            _ => Err(MoofError::type_error("reject: expected Table")),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Bool methods (true_class and false_class)
// ═══════════════════════════════════════════════════════════════════════

fn install_bool_methods(interp: &mut Interpreter) {
    for class in [interp.true_class.clone(), interp.false_class.clone()] {
        register_method(interp, &class, "not", |_interp, args| {
            Ok(Value::Bool(!args[0].is_truthy()))
        });

        register_method(interp, &class, "to_s", |_interp, args| {
            Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
        });

        register_method(interp, &class, "nil?", |_interp, _args| {
            Ok(Value::Bool(false))
        });

        register_method(interp, &class, "class", |_interp, _args| {
            Ok(Value::Str(Rc::from("Bool")))
        });

        register_method(interp, &class, "and:", |_interp, args| {
            if args[0].is_truthy() {
                Ok(args[1].clone())
            } else {
                Ok(args[0].clone())
            }
        });

        register_method(interp, &class, "or:", |_interp, args| {
            if args[0].is_truthy() {
                Ok(args[0].clone())
            } else {
                Ok(args[1].clone())
            }
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Nil methods
// ═══════════════════════════════════════════════════════════════════════

fn install_nil_methods(interp: &mut Interpreter) {
    let class = interp.nil_class.clone();

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(true))
    });

    register_method(interp, &class, "empty?", |_interp, _args| {
        Ok(Value::Bool(true)) // nil is the empty list
    });

    register_method(interp, &class, "length", |_interp, _args| {
        Ok(Value::Integer(MoofInt::from_i64(0)))
    });

    register_method(interp, &class, "to_s", |_interp, _args| {
        Ok(Value::Str(Rc::from("nil")))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Nil")))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Closure methods
// ═══════════════════════════════════════════════════════════════════════

fn install_closure_methods(interp: &mut Interpreter) {
    let class = interp.closure_class.clone();

    register_method(interp, &class, "call:", |interp, args| {
        let func = &args[0];
        let call_args = args[1].to_vec()?;
        invoke(interp, func, call_args)
    });

    register_method(interp, &class, "arity", |_interp, args| {
        match &args[0] {
            Value::Closure(c) => Ok(Value::Integer(MoofInt::from_i64(c.params.len() as i64))),
            _ => Err(MoofError::type_error("arity: expected Closure")),
        }
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Closure")))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    // ── curry: (partial application) ─────────────────────────────────
    // Returns a new closure that captures the given partial args and,
    // when called with remaining args, invokes the original function
    // with partial_args ++ new_args.
    register_method(interp, &class, "curry:", |interp, args| {
        let func = args[0].clone();
        let partial_args: Vec<Value> = args[1..].to_vec();

        // Intern symbols we'll use in the env and AST body
        let fn_sym = interp.symbols.intern("__curry_fn");
        let partial_sym = interp.symbols.intern("__curry_partial");
        let rest_sym = interp.symbols.intern("__curry_rest");
        let curry_call_sym = interp.symbols.intern("__curry_call");

        // Build a closure env that captures the original function and partial args
        let curry_env = interp.global_env.child();
        curry_env.define(fn_sym, func, false);
        curry_env.define(partial_sym, Value::from_slice(&partial_args), false);

        // Build the AST body: (__curry_call __curry_fn __curry_partial __curry_rest)
        let body = Value::from_slice(&[
            Value::Symbol(curry_call_sym),
            Value::Symbol(fn_sym),
            Value::Symbol(partial_sym),
            Value::Symbol(rest_sym),
        ]);

        Ok(Value::Closure(Rc::new(MoofClosure {
            name: Some(interp.symbols.intern("<curried>")),
            params: vec![],
            rest_param: Some(rest_sym),
            body: ClosureBody::Expr(body),
            env: curry_env,
        })))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Object methods (inherited by all types via superclass chain)
// ═══════════════════════════════════════════════════════════════════════

fn install_object_methods(interp: &mut Interpreter) {
    let class = interp.object_class.clone();

    register_method(interp, &class, "class", |interp, args| {
        let cls = interp.class_of(&args[0]);
        let name_id = cls.borrow().name;
        Ok(Value::Str(Rc::from(interp.symbols.name(name_id))))
    });

    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    register_method(interp, &class, "inspect", |_interp, args| {
        Ok(Value::Str(Rc::from(args[0].inspect().as_str())))
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "is_a:", |interp, args| {
        let target_name = match &args[1] {
            Value::Str(s) => s.to_string(),
            Value::Symbol(id) => interp.symbols.name(*id).to_string(),
            _ => return Err(MoofError::type_error("is_a: expects a class name (String or Symbol)")),
        };
        let mut current = interp.class_of(&args[0]);
        loop {
            let name_id = current.borrow().name;
            if interp.symbols.name(name_id) == target_name {
                return Ok(Value::Bool(true));
            }
            let sup = current.borrow().superclass.clone();
            match sup {
                Some(parent) => current = parent,
                None => return Ok(Value::Bool(false)),
            }
        }
    });

    register_method(interp, &class, "responds_to:", |interp, args| {
        let selector_name = match &args[1] {
            Value::Str(s) => s.to_string(),
            Value::Symbol(id) => interp.symbols.name(*id).to_string(),
            _ => return Err(MoofError::type_error("responds_to: expects a selector name (String or Symbol)")),
        };
        let selector_id = interp.symbols.intern(&selector_name);
        let cls = interp.class_of(&args[0]);
        Ok(Value::Bool(cls.borrow().lookup(selector_id).is_some()))
    });

    register_method(interp, &class, "==", |_interp, args| {
        Ok(Value::Bool(args[0] == args[1]))
    });

    register_method(interp, &class, "!=", |_interp, args| {
        Ok(Value::Bool(args[0] != args[1]))
    });

    register_method(interp, &class, "hash", |_interp, args| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        args[0].hash(&mut hasher);
        let h = hasher.finish() as i64;
        Ok(Value::Integer(MoofInt::from_i64(h)))
    });

    register_method(interp, &class, "send:", |interp, args| {
        let receiver = args[0].clone();
        let selector_name = match &args[1] {
            Value::Str(s) => s.to_string(),
            Value::Symbol(id) => interp.symbols.name(*id).to_string(),
            _ => return Err(MoofError::type_error("send: expects a selector name (String or Symbol)")),
        };
        let selector_id = interp.symbols.intern(&selector_name);
        let msg_args = if args.len() > 2 {
            args[2..].to_vec()
        } else {
            vec![]
        };
        interp.send_message(receiver, selector_id, msg_args)
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Symbol methods
// ═══════════════════════════════════════════════════════════════════════

fn install_symbol_methods(interp: &mut Interpreter) {
    let class = interp.symbol_class.clone();

    register_method(interp, &class, "to_s", |interp, args| {
        let id = args[0].as_symbol()?;
        Ok(Value::Str(Rc::from(interp.symbols.name(id))))
    });

    register_method(interp, &class, "to_sym", |_interp, args| {
        Ok(args[0].clone())
    });

    register_method(interp, &class, "inspect", |interp, args| {
        let id = args[0].as_symbol()?;
        Ok(Value::Str(Rc::from(format!(":{}", interp.symbols.name(id)).as_str())))
    });

    register_method(interp, &class, "length", |interp, args| {
        let id = args[0].as_symbol()?;
        let len = interp.symbols.name(id).len() as i64;
        Ok(Value::Integer(MoofInt::from_i64(len)))
    });

    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });

    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Symbol")))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Error methods
// ═══════════════════════════════════════════════════════════════════════

fn install_error_methods(interp: &mut Interpreter) {
    let class = interp.error_class.clone();

    // message — return the message field (field 0)
    register_method(interp, &class, "message", |_interp, args| {
        if let Value::Object(ref obj) = args[0] {
            let obj = obj.borrow();
            if let Some(msg) = obj.fields.get(0) {
                return Ok(msg.clone());
            }
        }
        Ok(Value::Str(Rc::from("")))
    });

    // to_s — format as "ClassName: message"
    register_method(interp, &class, "to_s", |interp, args| {
        if let Value::Object(ref obj) = args[0] {
            let obj = obj.borrow();
            let class_name = {
                let cls = obj.class.borrow();
                interp.symbols.name(cls.name).to_string()
            };
            let msg = obj.fields.get(0)
                .map(|v| format!("{}", v))
                .unwrap_or_default();
            Ok(Value::Str(Rc::from(format!("{}: {}", class_name, msg).as_str())))
        } else {
            Ok(Value::Str(Rc::from("Error")))
        }
    });

    // inspect — same as to_s
    register_method(interp, &class, "inspect", |interp, args| {
        if let Value::Object(ref obj) = args[0] {
            let obj = obj.borrow();
            let class_name = {
                let cls = obj.class.borrow();
                interp.symbols.name(cls.name).to_string()
            };
            let msg = obj.fields.get(0)
                .map(|v| format!("{}", v))
                .unwrap_or_default();
            Ok(Value::Str(Rc::from(format!("{}: {}", class_name, msg).as_str())))
        } else {
            Ok(Value::Str(Rc::from("Error")))
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Range methods
// ═══════════════════════════════════════════════════════════════════════

/// Helper: iterate a MoofRange, calling f for each element.
fn range_foreach(r: &MoofRange, mut f: impl FnMut(i64) -> Result<()>) -> Result<()> {
    let start = r.start.to_i64().ok_or_else(|| MoofError::type_error("range: integer too large"))?;
    let end = r.end.to_i64().ok_or_else(|| MoofError::type_error("range: integer too large"))?;
    let step = r.step.to_i64().ok_or_else(|| MoofError::type_error("range: integer too large"))?;
    let mut i = start;
    if step > 0 {
        while i < end {
            f(i)?;
            i += step;
        }
    } else if step < 0 {
        while i > end {
            f(i)?;
            i += step;
        }
    }
    Ok(())
}

fn install_range_methods(interp: &mut Interpreter) {
    let class = interp.range_class.clone();

    // each: — iterate, calling closure for each value
    register_method(interp, &class, "each:", |interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("each: expects Range receiver")),
        };
        let func = &args[1];
        range_foreach(&r, |i| {
            interp.invoke(func.clone(), vec![Value::Integer(MoofInt::from_i64(i))])?;
            Ok(())
        })?;
        Ok(Value::Nil)
    });

    // map: — collect results of applying closure to each value
    register_method(interp, &class, "map:", |interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("map: expects Range receiver")),
        };
        let func = &args[1];
        let mut results = Vec::new();
        range_foreach(&r, |i| {
            let val = interp.invoke(func.clone(), vec![Value::Integer(MoofInt::from_i64(i))])?;
            results.push(val);
            Ok(())
        })?;
        Ok(cons::vec_to_cons(&results))
    });

    // filter: — collect values matching predicate
    register_method(interp, &class, "filter:", |interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("filter: expects Range receiver")),
        };
        let func = &args[1];
        let mut results = Vec::new();
        range_foreach(&r, |i| {
            let v = Value::Integer(MoofInt::from_i64(i));
            let test = interp.invoke(func.clone(), vec![v.clone()])?;
            if test.is_truthy() {
                results.push(v);
            }
            Ok(())
        })?;
        Ok(cons::vec_to_cons(&results))
    });

    // to_list — convert to cons list (eager)
    register_method(interp, &class, "to_list", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("to_list expects Range receiver")),
        };
        let mut items = Vec::new();
        range_foreach(&r, |i| {
            items.push(Value::Integer(MoofInt::from_i64(i)));
            Ok(())
        })?;
        Ok(cons::vec_to_cons(&items))
    });

    // contains: — check if value is in range
    register_method(interp, &class, "contains:", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("contains: expects Range receiver")),
        };
        let val = args[1].as_int()?;
        let v = val.to_i64().ok_or_else(|| MoofError::type_error("contains: integer too large"))?;
        let start = r.start.to_i64().unwrap_or(0);
        let end = r.end.to_i64().unwrap_or(0);
        let step = r.step.to_i64().unwrap_or(1);
        let in_range = if step > 0 {
            v >= start && v < end && (v - start) % step == 0
        } else if step < 0 {
            v <= start && v > end && (start - v) % (-step) == 0
        } else {
            false
        };
        Ok(Value::Bool(in_range))
    });

    // size / length — number of elements
    register_method(interp, &class, "size", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("size expects Range receiver")),
        };
        Ok(Value::Integer(MoofInt::from_i64(r.len())))
    });

    register_method(interp, &class, "length", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("length expects Range receiver")),
        };
        Ok(Value::Integer(MoofInt::from_i64(r.len())))
    });

    // first — start value
    register_method(interp, &class, "first", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("first expects Range receiver")),
        };
        Ok(Value::Integer(r.start.clone()))
    });

    // last — end - step value (last element in the range)
    register_method(interp, &class, "last", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("last expects Range receiver")),
        };
        let len = r.len();
        if len == 0 {
            return Ok(Value::Nil);
        }
        let start = r.start.to_i64().unwrap_or(0);
        let step = r.step.to_i64().unwrap_or(1);
        let last = start + step * (len - 1);
        Ok(Value::Integer(MoofInt::from_i64(last)))
    });

    // reverse — Range with reversed direction
    register_method(interp, &class, "reverse", |_interp, args| {
        let r = match &args[0] {
            Value::Range(r) => r.clone(),
            _ => return Err(MoofError::type_error("reverse expects Range receiver")),
        };
        let len = r.len();
        if len == 0 {
            return Ok(Value::Range(Rc::new(MoofRange {
                start: r.end.clone(),
                end: r.start.clone(),
                step: MoofInt::from_i64(-r.step.to_i64().unwrap_or(1)),
            })));
        }
        let start = r.start.to_i64().unwrap_or(0);
        let step = r.step.to_i64().unwrap_or(1);
        let last = start + step * (len - 1);
        let new_end = start - step; // exclusive end of reversed
        Ok(Value::Range(Rc::new(MoofRange {
            start: MoofInt::from_i64(last),
            end: MoofInt::from_i64(new_end),
            step: MoofInt::from_i64(-step),
        })))
    });

    // to_s — display format
    register_method(interp, &class, "to_s", |_interp, args| {
        Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
    });

    // class — "Range"
    register_method(interp, &class, "class", |_interp, _args| {
        Ok(Value::Str(Rc::from("Range")))
    });

    // nil? — false
    register_method(interp, &class, "nil?", |_interp, _args| {
        Ok(Value::Bool(false))
    });
}
