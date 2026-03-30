use std::cmp::Ordering;

use indexmap::IndexMap;

use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::value::Value;

// ── Helpers ──────────────────────────────────────────────────────────

fn check_args(selector: &str, args: &[Value], expected: usize) -> Result<()> {
    if args.len() != expected {
        Err(MoofError::arity(&expected.to_string(), args.len(), Some(selector)))
    } else {
        Ok(())
    }
}

/// Promote two numeric values to (left, right, is_float).
fn promote(a: &Value, b: &Value) -> Result<(f64, f64, bool)> {
    let is_float = matches!(a, Value::Float(_)) || matches!(b, Value::Float(_));
    let left = a.as_number()?;
    let right = b.as_number()?;
    Ok((left, right, is_float))
}

fn num_binop(recv: &Value, op: &str, arg: &Value) -> Result<Value> {
    let (l, r, is_float) = promote(recv, arg)?;
    match op {
        "+" => {
            if is_float { Ok(Value::Float(l + r)) }
            else { Ok(Value::Integer(recv.as_int()? + arg.as_int()?)) }
        }
        "-" => {
            if is_float { Ok(Value::Float(l - r)) }
            else { Ok(Value::Integer(recv.as_int()? - arg.as_int()?)) }
        }
        "*" => {
            if is_float { Ok(Value::Float(l * r)) }
            else { Ok(Value::Integer(recv.as_int()? * arg.as_int()?)) }
        }
        "/" => {
            if r == 0.0 {
                return Err(MoofError::runtime("Division by zero"));
            }
            if is_float {
                Ok(Value::Float(l / r))
            } else {
                let a = recv.as_int()?;
                let b = arg.as_int()?;
                if a % b == 0 {
                    Ok(Value::Integer(a / b))
                } else {
                    Ok(Value::Float(l / r))
                }
            }
        }
        "%" => {
            if r == 0.0 {
                return Err(MoofError::runtime("Modulo by zero"));
            }
            if is_float { Ok(Value::Float(l % r)) }
            else { Ok(Value::Integer(recv.as_int()? % arg.as_int()?)) }
        }
        _ => Err(MoofError::runtime(format!("Unknown operator: {op}")))
    }
}

fn num_cmp(recv: &Value, op: &str, arg: &Value) -> Result<Value> {
    let l = recv.as_number()?;
    let r = arg.as_number()?;
    let result = match op {
        ">" => l > r,
        "<" => l < r,
        ">=" => l >= r,
        "<=" => l <= r,
        _ => return Err(MoofError::runtime(format!("Unknown comparator: {op}")))
    };
    Ok(Value::Bool(result))
}

fn value_cmp(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Integer(x), Value::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Float(x), Value::Integer(y)) => x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

fn flatten_into(val: Value, result: &mut Vec<Value>) {
    match val {
        Value::List(items) => {
            for item in items {
                flatten_into(item, result);
            }
        }
        other => result.push(other),
    }
}

// ══════════════════════════════════════════════════════════════════════
// Public entry point
// ══════════════════════════════════════════════════════════════════════

pub fn install(interp: &mut Interpreter) {
    register_integer(interp);
    register_float(interp);
    register_string(interp);
    register_list(interp);
    register_map(interp);
    register_function(interp);
    register_bool(interp);
    register_nil(interp);
}

// ══════════════════════════════════════════════════════════════════════
// Integer
// ══════════════════════════════════════════════════════════════════════

fn register_integer(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("Integer").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("abs", int_abs);
    k.register_builtin("to_s", num_to_s);
    k.register_builtin("to_f", num_to_f);
    k.register_builtin("to_i", num_to_i);
    k.register_builtin("zero?", num_zero);
    k.register_builtin("positive?", num_positive);
    k.register_builtin("negative?", num_negative);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", int_class);
    k.register_builtin("sqrt", num_sqrt);
    k.register_builtin("pow:", num_pow);
    k.register_builtin("max:", num_max);
    k.register_builtin("min:", num_min);
    k.register_builtin("+", num_add);
    k.register_builtin("-", num_sub);
    k.register_builtin("*", num_mul);
    k.register_builtin("/", num_div);
    k.register_builtin("%", num_mod);
    k.register_builtin(">", num_gt);
    k.register_builtin("<", num_lt);
    k.register_builtin(">=", num_gte);
    k.register_builtin("<=", num_lte);
}

// ══════════════════════════════════════════════════════════════════════
// Float
// ══════════════════════════════════════════════════════════════════════

fn register_float(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("Float").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("abs", float_abs);
    k.register_builtin("to_s", num_to_s);
    k.register_builtin("to_f", num_to_f);
    k.register_builtin("to_i", num_to_i);
    k.register_builtin("zero?", num_zero);
    k.register_builtin("positive?", num_positive);
    k.register_builtin("negative?", num_negative);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", float_class);
    k.register_builtin("sqrt", num_sqrt);
    k.register_builtin("pow:", num_pow);
    k.register_builtin("max:", num_max);
    k.register_builtin("min:", num_min);
    k.register_builtin("+", num_add);
    k.register_builtin("-", num_sub);
    k.register_builtin("*", num_mul);
    k.register_builtin("/", num_div);
    k.register_builtin("%", num_mod);
    k.register_builtin(">", num_gt);
    k.register_builtin("<", num_lt);
    k.register_builtin(">=", num_gte);
    k.register_builtin("<=", num_lte);
}

// ── Number methods (shared) ──────────────────────────────────────────

fn int_abs(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("abs", &args, 0)?;
    Ok(Value::Integer(recv.as_int()?.abs()))
}

fn float_abs(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("abs", &args, 0)?;
    Ok(Value::Float(recv.as_float()?.abs()))
}

fn num_to_s(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_s", &args, 0)?;
    Ok(Value::Str(format!("{}", recv)))
}

fn num_to_f(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_f", &args, 0)?;
    Ok(Value::Float(recv.as_number()?))
}

fn num_to_i(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_i", &args, 0)?;
    match &recv {
        Value::Integer(_) => Ok(recv),
        Value::Float(f) => Ok(Value::Integer(*f as i64)),
        _ => Err(MoofError::runtime("to_i requires a number"))
    }
}

fn num_zero(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("zero?", &args, 0)?;
    Ok(Value::Bool(recv.as_number()? == 0.0))
}

fn num_positive(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("positive?", &args, 0)?;
    Ok(Value::Bool(recv.as_number()? > 0.0))
}

fn num_negative(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("negative?", &args, 0)?;
    Ok(Value::Bool(recv.as_number()? < 0.0))
}

fn int_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("Integer".to_string()))
}

fn float_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("Float".to_string()))
}

fn num_sqrt(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("sqrt", &args, 0)?;
    let n = recv.as_number()?;
    if n < 0.0 {
        return Err(MoofError::runtime("Cannot take sqrt of negative number"));
    }
    let root = n.sqrt();
    if let Value::Integer(i) = &recv {
        let iroot = root as i64;
        if iroot * iroot == *i {
            return Ok(Value::Integer(iroot));
        }
    }
    Ok(Value::Float(root))
}

fn num_pow(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("pow:", &args, 1)?;
    let (base, exp, is_float) = promote(&recv, &args[0])?;
    if is_float {
        Ok(Value::Float(base.powf(exp)))
    } else {
        let b = recv.as_int()?;
        let e = args[0].as_int()?;
        if e < 0 {
            Ok(Value::Float(base.powf(exp)))
        } else {
            Ok(Value::Integer(b.pow(e as u32)))
        }
    }
}

fn num_max(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("max:", &args, 1)?;
    let (l, r, is_float) = promote(&recv, &args[0])?;
    if l >= r {
        if is_float { Ok(Value::Float(l)) } else { Ok(recv) }
    } else if is_float {
        Ok(Value::Float(r))
    } else {
        Ok(args[0].clone())
    }
}

fn num_min(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("min:", &args, 1)?;
    let (l, r, is_float) = promote(&recv, &args[0])?;
    if l <= r {
        if is_float { Ok(Value::Float(l)) } else { Ok(recv) }
    } else if is_float {
        Ok(Value::Float(r))
    } else {
        Ok(args[0].clone())
    }
}

fn num_add(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("+", &args, 1)?;
    num_binop(&recv, "+", &args[0])
}

fn num_sub(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("-", &args, 1)?;
    num_binop(&recv, "-", &args[0])
}

fn num_mul(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("*", &args, 1)?;
    num_binop(&recv, "*", &args[0])
}

fn num_div(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("/", &args, 1)?;
    num_binop(&recv, "/", &args[0])
}

fn num_mod(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("%", &args, 1)?;
    num_binop(&recv, "%", &args[0])
}

fn num_gt(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args(">", &args, 1)?;
    num_cmp(&recv, ">", &args[0])
}

fn num_lt(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("<", &args, 1)?;
    num_cmp(&recv, "<", &args[0])
}

fn num_gte(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args(">=", &args, 1)?;
    num_cmp(&recv, ">=", &args[0])
}

fn num_lte(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("<=", &args, 1)?;
    num_cmp(&recv, "<=", &args[0])
}

// ══════════════════════════════════════════════════════════════════════
// Common
// ══════════════════════════════════════════════════════════════════════

fn common_nil_false(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("nil?", &args, 0)?;
    Ok(Value::Bool(false))
}

// ══════════════════════════════════════════════════════════════════════
// String
// ══════════════════════════════════════════════════════════════════════

fn register_string(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("String").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("length", str_length);
    k.register_builtin("uppercase", str_uppercase);
    k.register_builtin("lowercase", str_lowercase);
    k.register_builtin("reverse", str_reverse);
    k.register_builtin("to_s", str_to_s);
    k.register_builtin("to_i", str_to_i);
    k.register_builtin("to_f", str_to_f);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", str_class);
    k.register_builtin("chars", str_chars);
    k.register_builtin("trim", str_trim);
    k.register_builtin("at:", str_at);
    k.register_builtin("contains:", str_contains);
    k.register_builtin("startsWith:", str_starts_with);
    k.register_builtin("endsWith:", str_ends_with);
    k.register_builtin("replaceAll:with:", str_replace_all_with);
    k.register_builtin("split:", str_split);
    k.register_builtin("concat:", str_concat);
    k.register_builtin("slice:length:", str_slice_length);
}

fn str_length(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("length", &args, 0)?;
    Ok(Value::Integer(recv.as_str()?.chars().count() as i64))
}

fn str_uppercase(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("uppercase", &args, 0)?;
    Ok(Value::Str(recv.as_str()?.to_uppercase()))
}

fn str_lowercase(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("lowercase", &args, 0)?;
    Ok(Value::Str(recv.as_str()?.to_lowercase()))
}

fn str_reverse(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("reverse", &args, 0)?;
    Ok(Value::Str(recv.as_str()?.chars().rev().collect()))
}

fn str_to_s(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_s", &args, 0)?;
    Ok(recv)
}

fn str_to_i(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_i", &args, 0)?;
    let s = recv.as_str()?;
    Ok(Value::Integer(s.parse::<i64>().unwrap_or(0)))
}

fn str_to_f(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_f", &args, 0)?;
    let s = recv.as_str()?;
    Ok(Value::Float(s.parse::<f64>().unwrap_or(0.0)))
}

fn str_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("String".to_string()))
}

fn str_chars(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("chars", &args, 0)?;
    let chars: Vec<Value> = recv.as_str()?.chars().map(|c| Value::Str(c.to_string())).collect();
    Ok(Value::List(chars))
}

fn str_trim(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("trim", &args, 0)?;
    Ok(Value::Str(recv.as_str()?.trim().to_string()))
}

fn str_at(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("at:", &args, 1)?;
    let idx = args[0].as_int()?;
    let s = recv.as_str()?;
    let len = s.chars().count() as i64;
    let actual = if idx < 0 { idx + len } else { idx };
    if actual < 0 || actual >= len {
        Ok(Value::Nil)
    } else {
        Ok(Value::Str(s.chars().nth(actual as usize).unwrap().to_string()))
    }
}

fn str_contains(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("contains:", &args, 1)?;
    let needle = args[0].as_str()?;
    Ok(Value::Bool(recv.as_str()?.contains(needle)))
}

fn str_starts_with(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("startsWith:", &args, 1)?;
    let prefix = args[0].as_str()?;
    Ok(Value::Bool(recv.as_str()?.starts_with(prefix)))
}

fn str_ends_with(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("endsWith:", &args, 1)?;
    let suffix = args[0].as_str()?;
    Ok(Value::Bool(recv.as_str()?.ends_with(suffix)))
}

fn str_replace_all_with(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("replaceAll:with:", &args, 2)?;
    let from = args[0].as_str()?;
    let to = args[1].as_str()?;
    Ok(Value::Str(recv.as_str()?.replace(from, to)))
}

fn str_split(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("split:", &args, 1)?;
    let sep = args[0].as_str()?;
    let parts: Vec<Value> = recv.as_str()?.split(sep).map(|s| Value::Str(s.to_string())).collect();
    Ok(Value::List(parts))
}

fn str_concat(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("concat:", &args, 1)?;
    let s = recv.as_str()?;
    Ok(Value::Str(format!("{}{}", s, args[0])))
}

fn str_slice_length(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("slice:length:", &args, 2)?;
    let start = args[0].as_int()? as usize;
    let len = args[1].as_int()? as usize;
    let s = recv.as_str()?;
    let result: String = s.chars().skip(start).take(len).collect();
    Ok(Value::Str(result))
}

// ══════════════════════════════════════════════════════════════════════
// List
// ══════════════════════════════════════════════════════════════════════

fn register_list(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("List").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("length", list_length);
    k.register_builtin("count", list_length);
    k.register_builtin("first", list_first);
    k.register_builtin("last", list_last);
    k.register_builtin("reverse", list_reverse);
    k.register_builtin("empty?", list_empty);
    k.register_builtin("to_s", list_to_s);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", list_class);
    k.register_builtin("rest", list_rest);
    k.register_builtin("sort", list_sort);
    k.register_builtin("uniq", list_uniq);
    k.register_builtin("flatten", list_flatten);
    k.register_builtin("at:", list_at);
    k.register_builtin("push:", list_push);
    k.register_builtin("prepend:", list_prepend);
    k.register_builtin("contains:", list_contains);
    k.register_builtin("join:", list_join);
    k.register_builtin("indexOf:", list_index_of);
    k.register_builtin("take:", list_take);
    k.register_builtin("drop:", list_drop);
    k.register_builtin("zip:", list_zip);
    k.register_builtin("map:", list_map);
    k.register_builtin("filter:", list_filter);
    k.register_builtin("reduce:init:", list_reduce_init);
    k.register_builtin("each:", list_each);
    k.register_builtin("any:", list_any);
    k.register_builtin("all:", list_all);
    k.register_builtin("none?", list_none);
    k.register_builtin("sortBy:", list_sort_by);
}

fn list_length(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("length", &args, 0)?;
    Ok(Value::Integer(recv.as_list()?.len() as i64))
}

fn list_first(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("first", &args, 0)?;
    Ok(recv.as_list()?.first().cloned().unwrap_or(Value::Nil))
}

fn list_last(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("last", &args, 0)?;
    Ok(recv.as_list()?.last().cloned().unwrap_or(Value::Nil))
}

fn list_reverse(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("reverse", &args, 0)?;
    let mut items = recv.into_list()?;
    items.reverse();
    Ok(Value::List(items))
}

fn list_empty(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("empty?", &args, 0)?;
    Ok(Value::Bool(recv.as_list()?.is_empty()))
}

fn list_to_s(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_s", &args, 0)?;
    Ok(Value::Str(format!("{}", recv)))
}

fn list_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("List".to_string()))
}

fn list_rest(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("rest", &args, 0)?;
    let items = recv.into_list()?;
    if items.is_empty() {
        Ok(Value::List(vec![]))
    } else {
        Ok(Value::List(items[1..].to_vec()))
    }
}

fn list_sort(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("sort", &args, 0)?;
    let mut items = recv.into_list()?;
    items.sort_by(value_cmp);
    Ok(Value::List(items))
}

fn list_uniq(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("uniq", &args, 0)?;
    let items = recv.into_list()?;
    let mut seen = Vec::new();
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    Ok(Value::List(seen))
}

fn list_flatten(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("flatten", &args, 0)?;
    let items = recv.into_list()?;
    let mut result = Vec::new();
    for item in items {
        flatten_into(item, &mut result);
    }
    Ok(Value::List(result))
}

fn list_at(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("at:", &args, 1)?;
    let idx = args[0].as_int()?;
    let items = recv.as_list()?;
    let len = items.len() as i64;
    let actual = if idx < 0 { idx + len } else { idx };
    if actual < 0 || actual >= len {
        Ok(Value::Nil)
    } else {
        Ok(items[actual as usize].clone())
    }
}

fn list_push(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("push:", &args, 1)?;
    let mut items = recv.into_list()?;
    items.push(args.into_iter().next().unwrap());
    Ok(Value::List(items))
}

fn list_prepend(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("prepend:", &args, 1)?;
    let mut items = recv.into_list()?;
    items.insert(0, args.into_iter().next().unwrap());
    Ok(Value::List(items))
}

fn list_contains(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("contains:", &args, 1)?;
    let items = recv.as_list()?;
    Ok(Value::Bool(items.contains(&args[0])))
}

fn list_join(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("join:", &args, 1)?;
    let sep = args[0].as_str()?;
    let items = recv.as_list()?;
    let parts: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
    Ok(Value::Str(parts.join(sep)))
}

fn list_index_of(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("indexOf:", &args, 1)?;
    let items = recv.as_list()?;
    let idx = items.iter().position(|v| v == &args[0]);
    Ok(Value::Integer(idx.map(|i| i as i64).unwrap_or(-1)))
}

fn list_take(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("take:", &args, 1)?;
    let n = args[0].as_int()? as usize;
    let items = recv.into_list()?;
    Ok(Value::List(items.into_iter().take(n).collect()))
}

fn list_drop(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("drop:", &args, 1)?;
    let n = args[0].as_int()? as usize;
    let items = recv.into_list()?;
    Ok(Value::List(items.into_iter().skip(n).collect()))
}

fn list_zip(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("zip:", &args, 1)?;
    let left = recv.into_list()?;
    let right = args.into_iter().next().unwrap().into_list()?;
    let pairs: Vec<Value> = left.into_iter().zip(right.into_iter())
        .map(|(a, b)| Value::List(vec![a, b]))
        .collect();
    Ok(Value::List(pairs))
}

fn list_map(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("map:", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        result.push(interp.call_value(&func, vec![item])?);
    }
    Ok(Value::List(result))
}

fn list_filter(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("filter:", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    let mut result = Vec::new();
    for item in items {
        let keep = interp.call_value(&func, vec![item.clone()])?;
        if keep.is_truthy() {
            result.push(item);
        }
    }
    Ok(Value::List(result))
}

fn list_reduce_init(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("reduce:init:", &args, 2)?;
    let mut args_iter = args.into_iter();
    let func = args_iter.next().unwrap();
    let mut acc = args_iter.next().unwrap();
    let items = recv.into_list()?;
    for item in items {
        acc = interp.call_value(&func, vec![acc, item])?;
    }
    Ok(acc)
}

fn list_each(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("each:", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    for item in items {
        interp.call_value(&func, vec![item])?;
    }
    Ok(Value::Nil)
}

fn list_any(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("any:", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    for item in items {
        let result = interp.call_value(&func, vec![item])?;
        if result.is_truthy() {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

fn list_all(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("all:", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    for item in items {
        let result = interp.call_value(&func, vec![item])?;
        if !result.is_truthy() {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn list_none(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("none?", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    for item in items {
        let result = interp.call_value(&func, vec![item])?;
        if result.is_truthy() {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn list_sort_by(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("sortBy:", &args, 1)?;
    let func = args.into_iter().next().unwrap();
    let items = recv.into_list()?;
    // Compute keys for each element.
    let mut keyed: Vec<(Value, Value)> = Vec::with_capacity(items.len());
    for item in items {
        let key = interp.call_value(&func, vec![item.clone()])?;
        keyed.push((key, item));
    }
    keyed.sort_by(|(ka, _), (kb, _)| value_cmp(ka, kb));
    let sorted: Vec<Value> = keyed.into_iter().map(|(_, v)| v).collect();
    Ok(Value::List(sorted))
}

// ══════════════════════════════════════════════════════════════════════
// Map
// ══════════════════════════════════════════════════════════════════════

fn register_map(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("Map").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("keys", map_keys);
    k.register_builtin("values", map_values);
    k.register_builtin("length", map_length);
    k.register_builtin("empty?", map_empty);
    k.register_builtin("to_s", map_to_s);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", map_class);
    k.register_builtin("at:", map_at);
    k.register_builtin("put:value:", map_put_value);
    k.register_builtin("remove:", map_remove);
    k.register_builtin("contains:", map_contains);
    k.register_builtin("merge:", map_merge);
}

fn as_map(v: &Value) -> Result<&IndexMap<String, Value>> {
    match v {
        Value::Map(m) => Ok(m),
        other => Err(MoofError::runtime(format!("Expected Map, got {}", other.type_name()))),
    }
}

fn into_map(v: Value) -> Result<IndexMap<String, Value>> {
    match v {
        Value::Map(m) => Ok(m),
        other => Err(MoofError::runtime(format!("Expected Map, got {}", other.type_name()))),
    }
}

fn map_keys(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("keys", &args, 0)?;
    let m = as_map(&recv)?;
    let keys: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
    Ok(Value::List(keys))
}

fn map_values(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("values", &args, 0)?;
    let m = as_map(&recv)?;
    let vals: Vec<Value> = m.values().cloned().collect();
    Ok(Value::List(vals))
}

fn map_length(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("length", &args, 0)?;
    let m = as_map(&recv)?;
    Ok(Value::Integer(m.len() as i64))
}

fn map_empty(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("empty?", &args, 0)?;
    let m = as_map(&recv)?;
    Ok(Value::Bool(m.is_empty()))
}

fn map_to_s(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_s", &args, 0)?;
    Ok(Value::Str(format!("{}", recv)))
}

fn map_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("Map".to_string()))
}

fn map_at(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("at:", &args, 1)?;
    let key = args[0].as_str()?;
    let m = as_map(&recv)?;
    Ok(m.get(key).cloned().unwrap_or(Value::Nil))
}

fn map_put_value(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("put:value:", &args, 2)?;
    let key = args[0].as_str()?.to_string();
    let val = args[1].clone();
    let mut m = into_map(recv)?;
    m.insert(key, val);
    Ok(Value::Map(m))
}

fn map_remove(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("remove:", &args, 1)?;
    let key = args[0].as_str()?;
    let mut m = into_map(recv)?;
    m.shift_remove(key);
    Ok(Value::Map(m))
}

fn map_contains(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("contains:", &args, 1)?;
    let key = args[0].as_str()?;
    let m = as_map(&recv)?;
    Ok(Value::Bool(m.contains_key(key)))
}

fn map_merge(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("merge:", &args, 1)?;
    let mut m = into_map(recv)?;
    let other = into_map(args.into_iter().next().unwrap())?;
    m.extend(other);
    Ok(Value::Map(m))
}

// ══════════════════════════════════════════════════════════════════════
// Function
// ══════════════════════════════════════════════════════════════════════

fn register_function(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("Function").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("call:", fn_call);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", fn_class);
    k.register_builtin("arity", fn_arity);
}

fn fn_call(interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("call:", &args, 1)?;
    // The argument should be a list of args, or we pass args directly.
    let call_args = match args.into_iter().next().unwrap() {
        Value::List(list) => list,
        single => vec![single],
    };
    interp.call_value(&recv, call_args)
}

fn fn_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("Function".to_string()))
}

fn fn_arity(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("arity", &args, 0)?;
    match &recv {
        Value::Function(f) => Ok(Value::Integer(f.params.len() as i64)),
        Value::Builtin(_, _) => Ok(Value::Integer(-1)),
        _ => Err(MoofError::runtime("arity requires a Function")),
    }
}

// ══════════════════════════════════════════════════════════════════════
// Bool
// ══════════════════════════════════════════════════════════════════════

fn register_bool(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("Bool").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("not", bool_not);
    k.register_builtin("to_s", bool_to_s);
    k.register_builtin("nil?", common_nil_false);
    k.register_builtin("class", bool_class);
    k.register_builtin("and:", bool_and);
    k.register_builtin("or:", bool_or);
}

fn bool_not(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("not", &args, 0)?;
    Ok(Value::Bool(!recv.as_bool()?))
}

fn bool_to_s(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_s", &args, 0)?;
    Ok(Value::Str(if recv.as_bool()? { "true" } else { "false" }.to_string()))
}

fn bool_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("Bool".to_string()))
}

fn bool_and(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("and:", &args, 1)?;
    if recv.as_bool()? {
        Ok(args.into_iter().next().unwrap())
    } else {
        Ok(Value::Bool(false))
    }
}

fn bool_or(_interp: &mut Interpreter, recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("or:", &args, 1)?;
    if recv.as_bool()? {
        Ok(recv)
    } else {
        Ok(args.into_iter().next().unwrap())
    }
}

// ══════════════════════════════════════════════════════════════════════
// Nil
// ══════════════════════════════════════════════════════════════════════

fn register_nil(interp: &mut Interpreter) {
    let klass_rc = interp.class_registry.get("Nil").unwrap().clone();
    let k = &mut *klass_rc.borrow_mut();

    k.register_builtin("nil?", nil_is_nil);
    k.register_builtin("to_s", nil_to_s);
    k.register_builtin("class", nil_class);
}

fn nil_is_nil(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("nil?", &args, 0)?;
    Ok(Value::Bool(true))
}

fn nil_to_s(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("to_s", &args, 0)?;
    Ok(Value::Str("nil".to_string()))
}

fn nil_class(_interp: &mut Interpreter, _recv: Value, args: Vec<Value>) -> Result<Value> {
    check_args("class", &args, 0)?;
    Ok(Value::Str("Nil".to_string()))
}
