use std::cell::RefCell;
use std::rc::Rc;

use crate::cons;
use crate::environment::Env;
use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::moofint::MoofInt;
use crate::value::{
    ClosureBody, MoofClass, MoofClosure, MoofTable, NativeFn, Value,
};

// ═══════════════════════════════════════════════════════════════════════
// Install everything
// ═══════════════════════════════════════════════════════════════════════

pub fn install(interp: &mut Interpreter) {
    install_globals(interp);
    install_integer_methods(interp);
    install_float_methods(interp);
    install_string_methods(interp);
    install_cons_methods(interp);
    install_table_methods(interp);
    install_bool_methods(interp);
    install_nil_methods(interp);
    install_closure_methods(interp);
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

fn builtin_error(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    check_arity("error", 1, &args)?;
    Err(MoofError::runtime(format!("{}", args[0])))
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
            let end = args[0].as_int()?.to_i64()
                .ok_or_else(|| MoofError::type_error("range: integer too large"))?;
            (0i64, end, 1i64)
        }
        2 => {
            let start = args[0].as_int()?.to_i64()
                .ok_or_else(|| MoofError::type_error("range: integer too large"))?;
            let end = args[1].as_int()?.to_i64()
                .ok_or_else(|| MoofError::type_error("range: integer too large"))?;
            (start, end, 1i64)
        }
        3 => {
            let start = args[0].as_int()?.to_i64()
                .ok_or_else(|| MoofError::type_error("range: integer too large"))?;
            let end = args[1].as_int()?.to_i64()
                .ok_or_else(|| MoofError::type_error("range: integer too large"))?;
            let step = args[2].as_int()?.to_i64()
                .ok_or_else(|| MoofError::type_error("range: integer too large"))?;
            if step == 0 {
                return Err(MoofError::runtime("range: step cannot be zero"));
            }
            (start, end, step)
        }
        _ => return Err(MoofError::arity("1-3", args.len(), Some("range"))),
    };

    let mut items = Vec::new();
    let mut i = start;
    if step > 0 {
        while i < end {
            items.push(Value::Integer(MoofInt::from_i64(i)));
            i += step;
        }
    } else {
        while i > end {
            items.push(Value::Integer(MoofInt::from_i64(i)));
            i += step;
        }
    }
    Ok(Value::from_slice(&items))
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
}
