use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::cons;
use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::moofint::MoofInt;
use crate::value::{MoofTable, NativeFn, Value};

/// Install all primitives into the interpreter's primitive registry.
pub fn install(interp: &mut Interpreter) {
    // ── Numeric (polymorphic) ──────────────────────────────────────
    reg(interp, "num_add", prim_num_add);
    reg(interp, "num_sub", prim_num_sub);
    reg(interp, "num_mul", prim_num_mul);
    reg(interp, "num_div", prim_num_div);
    reg(interp, "num_mod", prim_num_mod);
    reg(interp, "num_neg", prim_num_neg);
    reg(interp, "num_abs", prim_num_abs);
    reg(interp, "num_gt", prim_num_gt);
    reg(interp, "num_lt", prim_num_lt);
    reg(interp, "num_gte", prim_num_gte);
    reg(interp, "num_lte", prim_num_lte);
    reg(interp, "num_eq", prim_num_eq);
    reg(interp, "num_pow", prim_num_pow);
    reg(interp, "num_sqrt", prim_num_sqrt);

    // ── Integer-specific ───────────────────────────────────────────
    reg(interp, "int_to_float", prim_int_to_float);
    reg(interp, "int_gcd", prim_int_gcd);
    reg(interp, "int_bit_and", prim_int_bit_and);
    reg(interp, "int_bit_or", prim_int_bit_or);
    reg(interp, "int_bit_xor", prim_int_bit_xor);

    // ── Float-specific ─────────────────────────────────────────────
    reg(interp, "float_to_int", prim_float_to_int);
    reg(interp, "float_round", prim_float_round);
    reg(interp, "float_floor", prim_float_floor);
    reg(interp, "float_ceil", prim_float_ceil);
    reg(interp, "float_truncate", prim_float_truncate);
    reg(interp, "float_nan", prim_float_nan);
    reg(interp, "float_infinite", prim_float_infinite);
    reg(interp, "float_finite", prim_float_finite);

    // ── String ─────────────────────────────────────────────────────
    reg(interp, "str_length", prim_str_length);
    reg(interp, "str_at", prim_str_at);
    reg(interp, "str_slice", prim_str_slice);
    reg(interp, "str_concat", prim_str_concat);
    reg(interp, "str_split", prim_str_split);
    reg(interp, "str_contains", prim_str_contains);
    reg(interp, "str_starts_with", prim_str_starts_with);
    reg(interp, "str_ends_with", prim_str_ends_with);
    reg(interp, "str_replace", prim_str_replace);
    reg(interp, "str_trim", prim_str_trim);
    reg(interp, "str_upper", prim_str_upper);
    reg(interp, "str_lower", prim_str_lower);
    reg(interp, "str_chars", prim_str_chars);
    reg(interp, "str_bytes", prim_str_bytes);
    reg(interp, "str_to_int", prim_str_to_int);
    reg(interp, "str_to_float", prim_str_to_float);
    reg(interp, "str_index_of", prim_str_index_of);
    reg(interp, "str_repeat", prim_str_repeat);
    reg(interp, "str_capitalize", prim_str_capitalize);
    reg(interp, "str_reverse", prim_str_reverse);

    // ── Cons ───────────────────────────────────────────────────────
    reg(interp, "cons_car", prim_cons_car);
    reg(interp, "cons_cdr", prim_cons_cdr);
    reg(interp, "cons_length", prim_cons_length);

    // ── Table ──────────────────────────────────────────────────────
    reg(interp, "table_get", prim_table_get);
    reg(interp, "table_put", prim_table_put);
    reg(interp, "table_remove", prim_table_remove);
    reg(interp, "table_keys", prim_table_keys);
    reg(interp, "table_values", prim_table_values);
    reg(interp, "table_length", prim_table_length);
    reg(interp, "table_push", prim_table_push);
    reg(interp, "table_pop", prim_table_pop);
    reg(interp, "table_shift", prim_table_shift);
    reg(interp, "table_unshift", prim_table_unshift);
    reg(interp, "table_merge", prim_table_merge);
    reg(interp, "table_has_key", prim_table_has_key);

    // ── Object / Class ─────────────────────────────────────────────
    reg(interp, "obj_class_name", prim_obj_class_name);
    reg(interp, "obj_is_a", prim_obj_is_a);
    reg(interp, "obj_responds_to", prim_obj_responds_to);
    reg(interp, "obj_hash", prim_obj_hash);

    // ── Symbol ─────────────────────────────────────────────────────
    reg(interp, "sym_to_str", prim_sym_to_str);
    reg(interp, "str_to_sym", prim_str_to_sym);
    reg(interp, "sym_length", prim_sym_length);

    // ── I/O ────────────────────────────────────────────────────────
    reg(interp, "io_print", prim_io_print);
    reg(interp, "io_println", prim_io_println);
    reg(interp, "io_display", prim_io_display);
    reg(interp, "io_read_line", prim_io_read_line);
    reg(interp, "io_read_file", prim_io_read_file);
    reg(interp, "io_write_file", prim_io_write_file);
    reg(interp, "io_file_exists", prim_io_file_exists);

    // ── Control ────────────────────────────────────────────────────
    reg(interp, "error_raise", prim_error_raise);
    reg(interp, "process_exit", prim_process_exit);
    reg(interp, "time_now", prim_time_now);

    // ── Type introspection ─────────────────────────────────────────
    reg(interp, "type_of", prim_type_of);
    reg(interp, "closure_arity", prim_closure_arity);
    reg(interp, "identity_eq", prim_identity_eq);

    // ── Range ──────────────────────────────────────────────────────
    reg(interp, "range_start", prim_range_start);
    reg(interp, "range_end", prim_range_end);
    reg(interp, "range_step", prim_range_step);
    reg(interp, "range_length", prim_range_length);
    reg(interp, "range_contains", prim_range_contains);

    // ── Pretty-print ───────────────────────────────────────────────
    reg(interp, "pretty_print", prim_pretty_print);

    // ── Value display ──────────────────────────────────────────────
    reg(interp, "obj_to_s", prim_obj_to_s);
}

fn reg(interp: &mut Interpreter, name: &str, f: NativeFn) {
    let id = interp.symbols.intern(name);
    interp.primitive_registry.insert(id, f);
}

// ═══════════════════════════════════════════════════════════════════════
// Numeric helpers (shared with builtins.rs variadic globals)
// ═══════════════════════════════════════════════════════════════════════

pub fn numeric_add(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(Value::Integer(x + y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
        (Value::Integer(x), Value::Float(y)) => Ok(Value::Float(x.to_f64() + y)),
        (Value::Float(x), Value::Integer(y)) => Ok(Value::Float(x + y.to_f64())),
        _ => Err(MoofError::type_error("+ expects numbers")),
    }
}

pub fn numeric_sub(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(Value::Integer(x - y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
        (Value::Integer(x), Value::Float(y)) => Ok(Value::Float(x.to_f64() - y)),
        (Value::Float(x), Value::Integer(y)) => Ok(Value::Float(x - y.to_f64())),
        _ => Err(MoofError::type_error("- expects numbers")),
    }
}

pub fn numeric_mul(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(Value::Integer(x * y)),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
        (Value::Integer(x), Value::Float(y)) => Ok(Value::Float(x.to_f64() * y)),
        (Value::Float(x), Value::Integer(y)) => Ok(Value::Float(x * y.to_f64())),
        _ => Err(MoofError::type_error("* expects numbers")),
    }
}

pub fn numeric_div(a: &Value, b: &Value) -> Result<Value> {
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
            if *y == 0.0 { return Err(MoofError::runtime("Division by zero")); }
            Ok(Value::Float(x / y))
        }
        (Value::Integer(x), Value::Float(y)) => {
            if *y == 0.0 { return Err(MoofError::runtime("Division by zero")); }
            Ok(Value::Float(x.to_f64() / y))
        }
        (Value::Float(x), Value::Integer(y)) => {
            if y.is_zero() { return Err(MoofError::runtime("Division by zero")); }
            Ok(Value::Float(x / y.to_f64()))
        }
        _ => Err(MoofError::type_error("/ expects numbers")),
    }
}

pub fn numeric_rem(a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => {
            if y.is_zero() { return Err(MoofError::runtime("Modulo by zero")); }
            match x.checked_rem(y) {
                Some(r) => Ok(Value::Integer(r)),
                None => Err(MoofError::runtime("Modulo overflow")),
            }
        }
        (Value::Float(x), Value::Float(y)) => {
            if *y == 0.0 { return Err(MoofError::runtime("Modulo by zero")); }
            Ok(Value::Float(x % y))
        }
        (Value::Integer(x), Value::Float(y)) => {
            if *y == 0.0 { return Err(MoofError::runtime("Modulo by zero")); }
            Ok(Value::Float(x.to_f64() % y))
        }
        (Value::Float(x), Value::Integer(y)) => {
            if y.is_zero() { return Err(MoofError::runtime("Modulo by zero")); }
            Ok(Value::Float(x % y.to_f64()))
        }
        _ => Err(MoofError::type_error("% expects numbers")),
    }
}

pub fn numeric_gt(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x > y),
        (Value::Float(x), Value::Float(y)) => Ok(x > y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() > *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x > y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

pub fn numeric_lt(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x < y),
        (Value::Float(x), Value::Float(y)) => Ok(x < y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() < *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x < y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

pub fn numeric_gte(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x >= y),
        (Value::Float(x), Value::Float(y)) => Ok(x >= y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() >= *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x >= y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

pub fn numeric_lte(a: &Value, b: &Value) -> Result<bool> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => Ok(x <= y),
        (Value::Float(x), Value::Float(y)) => Ok(x <= y),
        (Value::Integer(x), Value::Float(y)) => Ok(x.to_f64() <= *y),
        (Value::Float(x), Value::Integer(y)) => Ok(*x <= y.to_f64()),
        _ => Err(MoofError::type_error("Comparison expects numbers")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Numeric primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_num_add(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    numeric_add(&args[0], &args[1])
}

fn prim_num_sub(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    numeric_sub(&args[0], &args[1])
}

fn prim_num_mul(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    numeric_mul(&args[0], &args[1])
}

fn prim_num_div(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    numeric_div(&args[0], &args[1])
}

fn prim_num_mod(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    numeric_rem(&args[0], &args[1])
}

fn prim_num_neg(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Integer(n) => Ok(Value::Integer(-n)),
        Value::Float(n) => Ok(Value::Float(-n)),
        _ => Err(MoofError::type_error("neg expects a number")),
    }
}

fn prim_num_abs(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Integer(n) => Ok(Value::Integer(n.abs())),
        Value::Float(n) => Ok(Value::Float(n.abs())),
        _ => Err(MoofError::type_error("abs expects a number")),
    }
}

fn prim_num_gt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Bool(numeric_gt(&args[0], &args[1])?))
}

fn prim_num_lt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Bool(numeric_lt(&args[0], &args[1])?))
}

fn prim_num_gte(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Bool(numeric_gte(&args[0], &args[1])?))
}

fn prim_num_lte(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Bool(numeric_lte(&args[0], &args[1])?))
}

fn prim_num_eq(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Bool(args[0] == args[1]))
}

fn prim_num_pow(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match (&args[0], &args[1]) {
        (Value::Integer(base), Value::Integer(exp)) => {
            Ok(Value::Integer(base.pow(exp)))
        }
        (Value::Float(base), Value::Float(exp)) => Ok(Value::Float(base.powf(*exp))),
        (Value::Integer(base), Value::Float(exp)) => Ok(Value::Float(base.to_f64().powf(*exp))),
        (Value::Float(base), Value::Integer(exp)) => Ok(Value::Float(base.powf(exp.to_f64()))),
        _ => Err(MoofError::type_error("pow expects numbers")),
    }
}

fn prim_num_sqrt(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Integer(n) => Ok(Value::Float(n.to_f64().sqrt())),
        Value::Float(n) => Ok(Value::Float(n.sqrt())),
        _ => Err(MoofError::type_error("sqrt expects a number")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Integer-specific primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_int_to_float(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let n = args[0].as_int()?;
    Ok(Value::Float(n.to_f64()))
}

fn prim_int_gcd(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let a = args[0].as_int()?;
    let b = args[1].as_int()?;
    Ok(Value::Integer(a.gcd(b)))
}

fn prim_int_bit_and(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let a = args[0].as_int()?.to_i64().ok_or_else(|| MoofError::type_error("bit_and: integer too large"))?;
    let b = args[1].as_int()?.to_i64().ok_or_else(|| MoofError::type_error("bit_and: integer too large"))?;
    Ok(Value::Integer(MoofInt::from_i64(a & b)))
}

fn prim_int_bit_or(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let a = args[0].as_int()?.to_i64().ok_or_else(|| MoofError::type_error("bit_or: integer too large"))?;
    let b = args[1].as_int()?.to_i64().ok_or_else(|| MoofError::type_error("bit_or: integer too large"))?;
    Ok(Value::Integer(MoofInt::from_i64(a | b)))
}

fn prim_int_bit_xor(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let a = args[0].as_int()?.to_i64().ok_or_else(|| MoofError::type_error("bit_xor: integer too large"))?;
    let b = args[1].as_int()?.to_i64().ok_or_else(|| MoofError::type_error("bit_xor: integer too large"))?;
    Ok(Value::Integer(MoofInt::from_i64(a ^ b)))
}

// ═══════════════════════════════════════════════════════════════════════
// Float-specific primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_float_to_int(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Integer(MoofInt::from_i64(*n as i64))),
        _ => Err(MoofError::type_error("float_to_int expects a float")),
    }
}

fn prim_float_round(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Float(n.round())),
        _ => Err(MoofError::type_error("float_round expects a float")),
    }
}

fn prim_float_floor(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Float(n.floor())),
        _ => Err(MoofError::type_error("float_floor expects a float")),
    }
}

fn prim_float_ceil(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Float(n.ceil())),
        _ => Err(MoofError::type_error("float_ceil expects a float")),
    }
}

fn prim_float_truncate(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Float(n.trunc())),
        _ => Err(MoofError::type_error("float_truncate expects a float")),
    }
}

fn prim_float_nan(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Bool(n.is_nan())),
        _ => Ok(Value::Bool(false)),
    }
}

fn prim_float_infinite(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Bool(n.is_infinite())),
        _ => Ok(Value::Bool(false)),
    }
}

fn prim_float_finite(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    match &args[0] {
        Value::Float(n) => Ok(Value::Bool(n.is_finite())),
        _ => Ok(Value::Bool(true)),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// String primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_str_length(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    Ok(Value::Integer(MoofInt::from_i64(s.chars().count() as i64)))
}

fn prim_str_at(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let idx = args[1].as_int()?.to_i64().unwrap_or(-1);
    if idx < 0 {
        return Ok(Value::Nil);
    }
    match s.chars().nth(idx as usize) {
        Some(c) => Ok(Value::Str(Rc::from(c.to_string().as_str()))),
        None => Ok(Value::Nil),
    }
}

fn prim_str_slice(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let start = args[1].as_int()?.to_i64().unwrap_or(0) as usize;
    let len = args[2].as_int()?.to_i64().unwrap_or(0) as usize;
    let sliced: String = s.chars().skip(start).take(len).collect();
    Ok(Value::Str(Rc::from(sliced.as_str())))
}

fn prim_str_concat(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let a = args[0].as_str()?;
    let b = args[1].as_str()?;
    let mut result = String::with_capacity(a.len() + b.len());
    result.push_str(a);
    result.push_str(b);
    Ok(Value::Str(Rc::from(result.as_str())))
}

fn prim_str_split(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let delim = args[1].as_str()?;
    let parts: Vec<Value> = s.split(delim).map(|p| Value::Str(Rc::from(p))).collect();
    Ok(Value::from_slice(&parts))
}

fn prim_str_contains(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let sub = args[1].as_str()?;
    Ok(Value::Bool(s.contains(sub)))
}

fn prim_str_starts_with(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let prefix = args[1].as_str()?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

fn prim_str_ends_with(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let suffix = args[1].as_str()?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

fn prim_str_replace(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let from = args[1].as_str()?;
    let to = args[2].as_str()?;
    Ok(Value::Str(Rc::from(s.replace(from, to).as_str())))
}

fn prim_str_trim(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    Ok(Value::Str(Rc::from(s.trim())))
}

fn prim_str_upper(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    Ok(Value::Str(Rc::from(s.to_uppercase().as_str())))
}

fn prim_str_lower(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    Ok(Value::Str(Rc::from(s.to_lowercase().as_str())))
}

fn prim_str_chars(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let chars: Vec<Value> = s.chars().map(|c| Value::Str(Rc::from(c.to_string().as_str()))).collect();
    Ok(Value::from_slice(&chars))
}

fn prim_str_bytes(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let bytes: Vec<Value> = s.bytes().map(|b| Value::Integer(MoofInt::from_i64(b as i64))).collect();
    Ok(Value::from_slice(&bytes))
}

fn prim_str_to_int(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(Value::Integer(MoofInt::from_i64(n))),
        Err(_) => Ok(Value::Nil),
    }
}

fn prim_str_to_float(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    match s.trim().parse::<f64>() {
        Ok(n) => Ok(Value::Float(n)),
        Err(_) => Ok(Value::Nil),
    }
}

fn prim_str_index_of(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let sub = args[1].as_str()?;
    match s.find(sub) {
        Some(byte_idx) => {
            let char_idx = s[..byte_idx].chars().count();
            Ok(Value::Integer(MoofInt::from_i64(char_idx as i64)))
        }
        None => Ok(Value::Integer(MoofInt::from_i64(-1))),
    }
}

fn prim_str_repeat(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let n = args[1].as_int()?.to_i64().unwrap_or(0);
    if n <= 0 {
        return Ok(Value::Str(Rc::from("")));
    }
    Ok(Value::Str(Rc::from(s.repeat(n as usize).as_str())))
}

fn prim_str_capitalize(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    if s.is_empty() {
        return Ok(Value::Str(Rc::from("")));
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    let rest: String = chars.collect();
    Ok(Value::Str(Rc::from(format!("{first}{rest}").as_str())))
}

fn prim_str_reverse(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    Ok(Value::Str(Rc::from(s.chars().rev().collect::<String>().as_str())))
}

// ═══════════════════════════════════════════════════════════════════════
// Cons primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_cons_car(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    args[0].car().cloned()
}

fn prim_cons_cdr(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    args[0].cdr().cloned()
}

fn prim_cons_length(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Integer(MoofInt::from_i64(cons::cons_length(&args[0]) as i64)))
}

// ═══════════════════════════════════════════════════════════════════════
// Table primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_table_get(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        Ok(tbl.borrow().get(&args[1]).cloned().unwrap_or(Value::Nil))
    } else {
        Err(MoofError::type_error("table_get: expected a table"))
    }
}

fn prim_table_put(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        tbl.borrow_mut().set(args[1].clone(), args[2].clone());
        Ok(args[0].clone())
    } else {
        Err(MoofError::type_error("table_put: expected a table"))
    }
}

fn prim_table_remove(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        let key = args[1].as_str()?;
        tbl.borrow_mut().hash.shift_remove(key);
        Ok(args[0].clone())
    } else {
        Err(MoofError::type_error("table_remove: expected a table"))
    }
}

fn prim_table_keys(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        let keys = tbl.borrow().keys();
        Ok(Value::from_slice(&keys))
    } else {
        Err(MoofError::type_error("table_keys: expected a table"))
    }
}

fn prim_table_values(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        let tbl = tbl.borrow();
        let vals: Vec<Value> = tbl.values().into_iter().cloned().collect();
        Ok(Value::from_slice(&vals))
    } else {
        Err(MoofError::type_error("table_values: expected a table"))
    }
}

fn prim_table_length(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        Ok(Value::Integer(MoofInt::from_i64(tbl.borrow().len() as i64)))
    } else {
        Err(MoofError::type_error("table_length: expected a table"))
    }
}

fn prim_table_push(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        tbl.borrow_mut().array.push(args[1].clone());
        Ok(args[0].clone())
    } else {
        Err(MoofError::type_error("table_push: expected a table"))
    }
}

fn prim_table_pop(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        let val = tbl.borrow_mut().array.pop().unwrap_or(Value::Nil);
        Ok(val)
    } else {
        Err(MoofError::type_error("table_pop: expected a table"))
    }
}

fn prim_table_shift(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        let mut t = tbl.borrow_mut();
        if t.array.is_empty() {
            Ok(Value::Nil)
        } else {
            Ok(t.array.remove(0))
        }
    } else {
        Err(MoofError::type_error("table_shift: expected a table"))
    }
}

fn prim_table_unshift(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        tbl.borrow_mut().array.insert(0, args[1].clone());
        Ok(args[0].clone())
    } else {
        Err(MoofError::type_error("table_unshift: expected a table"))
    }
}

fn prim_table_merge(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let (Value::Table(a), Value::Table(b)) = (&args[0], &args[1]) {
        let mut new_tbl = MoofTable::new();
        {
            let a_ref = a.borrow();
            for v in &a_ref.array { new_tbl.array.push(v.clone()); }
            for (k, v) in &a_ref.hash { new_tbl.hash.insert(k.clone(), v.clone()); }
        }
        {
            let b_ref = b.borrow();
            for (k, v) in &b_ref.hash { new_tbl.hash.insert(k.clone(), v.clone()); }
            new_tbl.array.extend(b_ref.array.iter().cloned());
        }
        Ok(Value::Table(Rc::new(std::cell::RefCell::new(new_tbl))))
    } else {
        Err(MoofError::type_error("table_merge: expected two tables"))
    }
}

fn prim_table_has_key(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Table(tbl) = &args[0] {
        Ok(Value::Bool(tbl.borrow().get(&args[1]).is_some()))
    } else {
        Err(MoofError::type_error("table_has_key: expected a table"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Object / Class primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_obj_class_name(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let name = match &args[0] {
        Value::Integer(_) => "Integer",
        Value::Float(_) => "Float",
        Value::Bool(true) => "TrueClass",
        Value::Bool(false) => "FalseClass",
        Value::Nil => "NilClass",
        Value::Str(_) => "String",
        Value::Symbol(_) => "Symbol",
        Value::Cons(_) => "Cons",
        Value::Table(_) => "Table",
        Value::Closure(_) => "Closure",
        Value::Range(_) => "Range",
        Value::Object(obj) => {
            let obj = obj.borrow();
            let class = obj.class.borrow();
            let name = interp.symbols.name(class.name).to_string();
            return Ok(Value::Str(Rc::from(name.as_str())));
        }
    };
    Ok(Value::Str(Rc::from(name)))
}

fn prim_obj_is_a(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let class_name = args[1].as_str()?;
    let type_name = args[0].type_name();

    // Direct match
    if type_name == class_name {
        return Ok(Value::Bool(true));
    }

    // Check superclass chain for objects
    if let Value::Object(obj) = &args[0] {
        let obj = obj.borrow();
        let mut current = Some(obj.class.clone());
        while let Some(cls) = current {
            let cls_borrow = cls.borrow();
            let name = interp.symbols.name(cls_borrow.name);
            if name == class_name {
                return Ok(Value::Bool(true));
            }
            current = cls_borrow.superclass.clone();
        }
    }

    // Check broad categories
    match class_name {
        "Numeric" => Ok(Value::Bool(matches!(args[0], Value::Integer(_) | Value::Float(_)))),
        "Object" => Ok(Value::Bool(true)), // everything is an Object
        "Bool" => Ok(Value::Bool(matches!(args[0], Value::Bool(_)))),
        _ => Ok(Value::Bool(false)),
    }
}

fn prim_obj_responds_to(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let selector = args[1].as_str()?;
    let sel_id = interp.symbols.intern(selector);
    let class = interp.class_of(&args[0]);
    let found = class.borrow().lookup(sel_id).is_some();
    Ok(Value::Bool(found))
}

fn prim_obj_hash(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    args[0].hash(&mut hasher);
    let h = hasher.finish();
    Ok(Value::Integer(MoofInt::from_i64(h as i64)))
}

// ═══════════════════════════════════════════════════════════════════════
// Symbol primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_sym_to_str(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Symbol(id) = &args[0] {
        let name = interp.symbols.name(*id).to_string();
        Ok(Value::Str(Rc::from(name.as_str())))
    } else {
        Err(MoofError::type_error("sym_to_str: expected a symbol"))
    }
}

fn prim_str_to_sym(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let s = args[0].as_str()?;
    let id = interp.symbols.intern(s);
    Ok(Value::Symbol(id))
}

fn prim_sym_length(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Symbol(id) = &args[0] {
        let name = interp.symbols.name(*id);
        Ok(Value::Integer(MoofInt::from_i64(name.len() as i64)))
    } else {
        Err(MoofError::type_error("sym_length: expected a symbol"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I/O primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_io_print(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    print!("{}", args[0]);
    Ok(Value::Nil)
}

fn prim_io_println(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    println!("{}", args[0]);
    Ok(Value::Nil)
}

fn prim_io_display(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    use std::io::Write;
    print!("{}", args[0]);
    std::io::stdout().flush().ok();
    Ok(Value::Nil)
}

fn prim_io_read_line(_interp: &mut Interpreter, _args: Vec<Value>) -> Result<Value> {
    use std::io::{self, BufRead};
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok();
    if line.ends_with('\n') { line.pop(); }
    if line.ends_with('\r') { line.pop(); }
    Ok(Value::Str(Rc::from(line.as_str())))
}

fn prim_io_read_file(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let path = args[0].as_str()?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| MoofError::io(format!("read-file: {e}")))?;
    Ok(Value::Str(Rc::from(content.as_str())))
}

fn prim_io_write_file(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let path = args[0].as_str()?;
    let content = args[1].as_str()?;
    std::fs::write(path, content)
        .map_err(|e| MoofError::io(format!("write-file: {e}")))?;
    Ok(Value::Nil)
}

fn prim_io_file_exists(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let path = args[0].as_str()?;
    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

// ═══════════════════════════════════════════════════════════════════════
// Control primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_error_raise(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let msg = format!("{}", args[0]);
    let cls = interp.runtime_error_class.clone();
    let obj = interp.make_error_object(&cls, &msg);
    Err(MoofError::runtime(&msg).with_object(obj))
}

fn prim_process_exit(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let code = if !args.is_empty() {
        match &args[0] {
            Value::Integer(n) => n.to_i64().unwrap_or(0) as i32,
            _ => 0,
        }
    } else {
        0
    };
    std::process::exit(code);
}

fn prim_time_now(_interp: &mut Interpreter, _args: Vec<Value>) -> Result<Value> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    Ok(Value::Float(duration.as_secs_f64()))
}

// ═══════════════════════════════════════════════════════════════════════
// Type introspection primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_type_of(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Str(Rc::from(args[0].type_name())))
}

fn prim_closure_arity(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Closure(c) = &args[0] {
        let arity = c.params.len();
        Ok(Value::Integer(MoofInt::from_i64(arity as i64)))
    } else {
        Err(MoofError::type_error("closure_arity: expected a closure"))
    }
}

fn prim_identity_eq(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let same = match (&args[0], &args[1]) {
        (Value::Cons(a), Value::Cons(b)) => Rc::ptr_eq(a, b),
        (Value::Table(a), Value::Table(b)) => Rc::ptr_eq(a, b),
        (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
        (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
        _ => args[0] == args[1],
    };
    Ok(Value::Bool(same))
}

// ═══════════════════════════════════════════════════════════════════════
// Range primitives
// ═══════════════════════════════════════════════════════════════════════

fn prim_range_start(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Range(r) = &args[0] {
        Ok(Value::Integer(r.start.clone()))
    } else {
        Err(MoofError::type_error("range_start: expected a range"))
    }
}

fn prim_range_end(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Range(r) = &args[0] {
        Ok(Value::Integer(r.end.clone()))
    } else {
        Err(MoofError::type_error("range_end: expected a range"))
    }
}

fn prim_range_step(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Range(r) = &args[0] {
        Ok(Value::Integer(r.step.clone()))
    } else {
        Err(MoofError::type_error("range_step: expected a range"))
    }
}

fn prim_range_length(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Range(r) = &args[0] {
        Ok(Value::Integer(MoofInt::from_i64(r.len())))
    } else {
        Err(MoofError::type_error("range_length: expected a range"))
    }
}

fn prim_range_contains(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    if let Value::Range(r) = &args[0] {
        if let Value::Integer(n) = &args[1] {
            let in_range = if r.step > MoofInt::from_i64(0) {
                *n >= r.start && *n < r.end
            } else {
                *n <= r.start && *n > r.end
            };
            Ok(Value::Bool(in_range))
        } else {
            Ok(Value::Bool(false))
        }
    } else {
        Err(MoofError::type_error("range_contains: expected a range"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Value display
// ═══════════════════════════════════════════════════════════════════════

fn prim_obj_to_s(_interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    Ok(Value::Str(Rc::from(format!("{}", args[0]).as_str())))
}

fn prim_pretty_print(interp: &mut Interpreter, args: Vec<Value>) -> Result<Value> {
    let result = crate::pretty::pretty_print(&args[0], &interp.symbols, &interp.known);
    Ok(Value::Str(Rc::from(result.as_str())))
}
