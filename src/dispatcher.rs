use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::value::{Value, MoofFunction};

/// Dispatch a message send [receiver selector args...] to the appropriate handler.
pub fn send_message(
    interp: &mut Interpreter,
    receiver: Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    // For non-Object/Class receivers, check if the built-in class has user-defined methods first
    if !matches!(receiver, Value::Object(_) | Value::Class(_)) {
        let class_name = builtin_class_name(&receiver);
        if let Some(cn) = class_name {
            if let Some(klass_rc) = interp.class_registry.get(cn) {
                let method = klass_rc.borrow().lookup(selector);
                if let Some(method) = method {
                    return invoke_method(interp, &method, receiver, args);
                }
            }
        }
    }

    // Fall back to hardcoded dispatch
    match &receiver {
        Value::Integer(_) | Value::Float(_) => dispatch_number(interp, &receiver, selector, args),
        Value::Str(_) => dispatch_string(interp, &receiver, selector, args),
        Value::List(_) => dispatch_list(interp, receiver, selector, args),
        Value::Map(_) => dispatch_map(interp, receiver, selector, args),
        Value::Function(_) | Value::Builtin(_, _) => dispatch_function(interp, receiver, selector, args),
        Value::Bool(_) => dispatch_boolean(&receiver, selector, &args),
        Value::Nil => dispatch_nil(selector),
        Value::Object(_) => dispatch_moof_object(interp, receiver, selector, args),
        Value::Class(_) => dispatch_moof_class(&receiver, selector),
        _ => match selector {
            "nil?" => Ok(Value::Bool(false)),
            _ => Err(make_message_error(&receiver, selector)),
        },
    }
}

/// Backward-compatible alias used by the existing interpreter stub.
pub fn dispatch_message(
    interp: &mut Interpreter,
    receiver: &Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    send_message(interp, receiver.clone(), selector, args)
}

fn builtin_class_name(receiver: &Value) -> Option<&'static str> {
    match receiver {
        Value::Integer(_) => Some("Integer"),
        Value::Float(_) => Some("Float"),
        Value::Str(_) => Some("String"),
        Value::List(_) => Some("List"),
        Value::Map(_) => Some("Map"),
        Value::Bool(_) => Some("Bool"),
        Value::Nil => Some("Nil"),
        Value::Function(_) | Value::Builtin(_, _) => Some("Function"),
        _ => None,
    }
}

// ── Levenshtein distance ────────────────────────────────────────

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 { return m; }
    if m == 0 { return n; }

    let mut d: Vec<usize> = (0..=n).collect();
    for j in 1..=m {
        let mut prev = d[0];
        d[0] = j;
        for i in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let temp = d[i];
            d[i] = (d[i] + 1).min(d[i - 1] + 1).min(prev + cost);
            prev = temp;
        }
    }
    d[n]
}

fn collect_known_selectors(receiver: &Value) -> Vec<&'static str> {
    match receiver {
        Value::Integer(_) | Value::Float(_) => vec![
            "abs", "to_s", "to_f", "to_i", "zero?", "positive?", "negative?",
            "nil?", "class", "sqrt", "pow:", "max:", "min:",
            "+", "-", "*", "/", "%", ">", "<", ">=", "<=",
        ],
        Value::Str(_) => vec![
            "length", "uppercase", "lowercase", "reverse", "to_s", "to_i", "to_f",
            "chars", "trim", "nil?", "class",
            "at:", "contains:", "startsWith:", "endsWith:", "replaceAll:with:",
            "split:", "concat:", "slice:length:",
        ],
        Value::List(_) => vec![
            "length", "first", "last", "rest", "reverse", "sort", "uniq", "flatten",
            "empty?", "to_s", "nil?", "class",
            "at:", "push:", "prepend:", "contains:", "join:", "indexOf:",
            "take:", "drop:", "zip:",
            "map:", "filter:", "reduce:init:", "each:", "any:", "all:", "none:", "sortBy:",
        ],
        Value::Map(_) => vec![
            "keys", "values", "length", "empty?", "to_s", "nil?", "class",
            "at:", "put:value:", "remove:", "contains:", "merge:",
        ],
        Value::Function(_) | Value::Builtin(_, _) => vec!["call:", "arity", "nil?", "class"],
        Value::Bool(_) => vec!["not", "to_s", "nil?", "class", "and:", "or:"],
        Value::Nil => vec!["nil?", "to_s", "class"],
        _ => vec!["nil?"],
    }
}

fn suggest_selector(receiver: &Value, failed: &str) -> Option<String> {
    let known = collect_known_selectors(receiver);
    if known.is_empty() { return None; }

    let mut best: Option<&str> = None;
    let mut best_dist = usize::MAX;

    for sel in &known {
        let dist = levenshtein(failed, sel);
        if dist < best_dist {
            best_dist = dist;
            best = Some(sel);
        }
    }

    let max_dist = if failed.len() <= 2 {
        1
    } else {
        3usize.min(((failed.len() as f64) / 2.0).ceil() as usize)
    };

    if best_dist <= max_dist {
        best.map(|s| s.to_string())
    } else {
        None
    }
}

fn make_message_error(receiver: &Value, selector: &str) -> MoofError {
    let desc = receiver.type_name().to_string();
    MoofError::message(&desc, selector, None)
}

fn make_message_error_with_suggest(receiver: &Value, selector: &str) -> MoofError {
    let desc = receiver.type_name().to_string();
    let suggestion = suggest_selector(receiver, selector);
    MoofError::message(&desc, selector, suggestion.as_deref())
}

fn check_args(selector: &str, args: &[Value], expected: usize) -> Result<()> {
    if args.len() != expected {
        Err(MoofError::arity(&expected.to_string(), args.len(), Some(selector)))
    } else {
        Ok(())
    }
}

// ── Number dispatch ─────────────────────────────────────────────

fn dispatch_number(
    _interp: &Interpreter,
    receiver: &Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    match selector {
        "abs" => match receiver {
            Value::Integer(n) => Ok(Value::Integer(n.abs())),
            Value::Float(n) => Ok(Value::Float(n.abs())),
            _ => unreachable!(),
        },
        "to_s" => Ok(Value::Str(format!("{}", receiver))),
        "to_f" => match receiver {
            Value::Integer(n) => Ok(Value::Float(*n as f64)),
            Value::Float(n) => Ok(Value::Float(*n)),
            _ => unreachable!(),
        },
        "to_i" => match receiver {
            Value::Integer(n) => Ok(Value::Integer(*n)),
            Value::Float(n) => Ok(Value::Integer(*n as i64)),
            _ => unreachable!(),
        },
        "zero?" => match receiver {
            Value::Integer(n) => Ok(Value::Bool(*n == 0)),
            Value::Float(n) => Ok(Value::Bool(*n == 0.0)),
            _ => unreachable!(),
        },
        "positive?" => match receiver {
            Value::Integer(n) => Ok(Value::Bool(*n > 0)),
            Value::Float(n) => Ok(Value::Bool(*n > 0.0)),
            _ => unreachable!(),
        },
        "negative?" => match receiver {
            Value::Integer(n) => Ok(Value::Bool(*n < 0)),
            Value::Float(n) => Ok(Value::Bool(*n < 0.0)),
            _ => unreachable!(),
        },
        "nil?" => Ok(Value::Bool(false)),
        "class" => match receiver {
            Value::Integer(_) => Ok(Value::Str("Integer".to_string())),
            Value::Float(_) => Ok(Value::Str("Float".to_string())),
            _ => unreachable!(),
        },
        "sqrt" => {
            let f = match receiver {
                Value::Integer(n) => *n as f64,
                Value::Float(n) => *n,
                _ => unreachable!(),
            };
            let result = f.sqrt();
            if matches!(receiver, Value::Integer(_)) && result == result.floor() {
                Ok(Value::Integer(result as i64))
            } else {
                Ok(Value::Float(result))
            }
        }
        "pow:" => {
            check_args(selector, &args, 1)?;
            match (receiver, &args[0]) {
                (Value::Integer(a), Value::Integer(b)) => {
                    if *b >= 0 {
                        Ok(Value::Integer(a.pow(*b as u32)))
                    } else {
                        Ok(Value::Float((*a as f64).powf(*b as f64)))
                    }
                }
                (Value::Integer(a), Value::Float(b)) => Ok(Value::Float((*a as f64).powf(*b))),
                (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a.powf(*b as f64))),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(*b))),
                _ => Err(MoofError::runtime("pow: requires numeric arguments")),
            }
        }
        "max:" => {
            check_args(selector, &args, 1)?;
            let (af, bf, _) = promote(receiver, &args[0]);
            if af >= bf { Ok(receiver.clone()) } else { Ok(args[0].clone()) }
        }
        "min:" => {
            check_args(selector, &args, 1)?;
            let (af, bf, _) = promote(receiver, &args[0]);
            if af <= bf { Ok(receiver.clone()) } else { Ok(args[0].clone()) }
        }
        "+" | "-" | "*" | "/" | "%" => {
            check_args(selector, &args, 1)?;
            num_binop(receiver, &args[0], selector)
        }
        ">" | "<" | ">=" | "<=" => {
            check_args(selector, &args, 1)?;
            num_cmp(receiver, &args[0], selector)
        }
        _ => Err(make_message_error_with_suggest(receiver, selector)),
    }
}

fn num_binop(a: &Value, b: &Value, op: &str) -> Result<Value> {
    let (af, bf, is_float) = promote(a, b);
    let result = match op {
        "+" => af + bf,
        "-" => af - bf,
        "*" => af * bf,
        "/" => {
            if bf == 0.0 { return Err(MoofError::runtime("Division by zero")); }
            af / bf
        }
        "%" => {
            if bf == 0.0 { return Err(MoofError::runtime("Modulo by zero")); }
            af % bf
        }
        _ => unreachable!(),
    };
    if is_float {
        Ok(Value::Float(result))
    } else {
        Ok(Value::Integer(result as i64))
    }
}

fn num_cmp(a: &Value, b: &Value, op: &str) -> Result<Value> {
    let (af, bf, _) = promote(a, b);
    let result = match op {
        ">" => af > bf,
        "<" => af < bf,
        ">=" => af >= bf,
        "<=" => af <= bf,
        _ => unreachable!(),
    };
    Ok(Value::Bool(result))
}

fn promote(a: &Value, b: &Value) -> (f64, f64, bool) {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => (*x as f64, *y as f64, false),
        (Value::Integer(x), Value::Float(y)) => (*x as f64, *y, true),
        (Value::Float(x), Value::Integer(y)) => (*x, *y as f64, true),
        (Value::Float(x), Value::Float(y)) => (*x, *y, true),
        _ => (0.0, 0.0, false),
    }
}

// ── String dispatch ─────────────────────────────────────────────

fn dispatch_string(
    _interp: &Interpreter,
    receiver: &Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    let s = match receiver {
        Value::Str(s) => s,
        _ => unreachable!(),
    };
    match selector {
        "length" => Ok(Value::Integer(s.len() as i64)),
        "uppercase" => Ok(Value::Str(s.to_uppercase())),
        "lowercase" => Ok(Value::Str(s.to_lowercase())),
        "reverse" => Ok(Value::Str(s.chars().rev().collect())),
        "to_s" => Ok(receiver.clone()),
        "to_i" => Ok(Value::Integer(s.parse::<i64>().unwrap_or(0))),
        "to_f" => Ok(Value::Float(s.parse::<f64>().unwrap_or(0.0))),
        "nil?" => Ok(Value::Bool(false)),
        "class" => Ok(Value::Str("String".to_string())),
        "chars" => {
            let chars: Vec<Value> = s.chars().map(|c| Value::Str(c.to_string())).collect();
            Ok(Value::List(chars))
        }
        "trim" => Ok(Value::Str(s.trim().to_string())),
        "at:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Integer(i) => {
                    let idx = *i as usize;
                    match s.chars().nth(idx) {
                        Some(c) => Ok(Value::Str(c.to_string())),
                        None => Ok(Value::Nil),
                    }
                }
                _ => Err(MoofError::runtime("at: expects an Integer index")),
            }
        }
        "contains:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(sub) => Ok(Value::Bool(s.contains(sub.as_str()))),
                _ => Err(MoofError::runtime("contains: expects a String")),
            }
        }
        "startsWith:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(sub) => Ok(Value::Bool(s.starts_with(sub.as_str()))),
                _ => Err(MoofError::runtime("startsWith: expects a String")),
            }
        }
        "endsWith:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(sub) => Ok(Value::Bool(s.ends_with(sub.as_str()))),
                _ => Err(MoofError::runtime("endsWith: expects a String")),
            }
        }
        "replaceAll:with:" => {
            check_args(selector, &args, 2)?;
            match (&args[0], &args[1]) {
                (Value::Str(from), Value::Str(to)) => {
                    Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
                }
                _ => Err(MoofError::runtime("replaceAll:with: expects two Strings")),
            }
        }
        "split:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(delim) => {
                    let parts: Vec<Value> =
                        s.split(delim.as_str()).map(|p| Value::Str(p.to_string())).collect();
                    Ok(Value::List(parts))
                }
                _ => Err(MoofError::runtime("split: expects a String delimiter")),
            }
        }
        "concat:" => {
            check_args(selector, &args, 1)?;
            Ok(Value::Str(format!("{}{}", s, args[0])))
        }
        "slice:length:" => {
            check_args(selector, &args, 2)?;
            match (&args[0], &args[1]) {
                (Value::Integer(start), Value::Integer(len)) => {
                    let start = *start as usize;
                    let len = *len as usize;
                    let result: String = s.chars().skip(start).take(len).collect();
                    Ok(Value::Str(result))
                }
                _ => Err(MoofError::runtime("slice:length: expects two Integers")),
            }
        }
        _ => Err(make_message_error_with_suggest(receiver, selector)),
    }
}

// ── List dispatch ───────────────────────────────────────────────

fn dispatch_list(
    interp: &mut Interpreter,
    receiver: Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    let items = match &receiver {
        Value::List(items) => items.clone(),
        _ => unreachable!(),
    };
    match selector {
        "length" | "count" => Ok(Value::Integer(items.len() as i64)),
        "first" => Ok(items.first().cloned().unwrap_or(Value::Nil)),
        "last" => Ok(items.last().cloned().unwrap_or(Value::Nil)),
        "reverse" => {
            let mut rev = items;
            rev.reverse();
            Ok(Value::List(rev))
        }
        "empty?" => Ok(Value::Bool(items.is_empty())),
        "to_s" => Ok(Value::Str(format!("{}", receiver))),
        "nil?" => Ok(Value::Bool(false)),
        "class" => Ok(Value::Str("List".to_string())),
        "rest" => {
            if items.len() <= 1 {
                Ok(Value::List(vec![]))
            } else {
                Ok(Value::List(items[1..].to_vec()))
            }
        }
        "sort" => {
            let mut sorted = items;
            sorted.sort_by(value_cmp);
            Ok(Value::List(sorted))
        }
        "uniq" => {
            let mut seen = Vec::new();
            for item in &items {
                if !seen.contains(item) {
                    seen.push(item.clone());
                }
            }
            Ok(Value::List(seen))
        }
        "flatten" => {
            let mut result = Vec::new();
            flatten_into(&items, &mut result);
            Ok(Value::List(result))
        }
        "at:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Integer(i) => {
                    let idx = *i as usize;
                    Ok(items.get(idx).cloned().unwrap_or(Value::Nil))
                }
                _ => Err(MoofError::runtime("at: expects an Integer index")),
            }
        }
        "push:" => {
            check_args(selector, &args, 1)?;
            let mut new_list = items;
            new_list.push(args.into_iter().next().unwrap());
            Ok(Value::List(new_list))
        }
        "prepend:" => {
            check_args(selector, &args, 1)?;
            let mut new_list = vec![args.into_iter().next().unwrap()];
            new_list.extend(items);
            Ok(Value::List(new_list))
        }
        "contains:" => {
            check_args(selector, &args, 1)?;
            Ok(Value::Bool(items.contains(&args[0])))
        }
        "join:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(sep) => {
                    let joined: String = items
                        .iter()
                        .map(|v| format!("{}", v))
                        .collect::<Vec<_>>()
                        .join(sep);
                    Ok(Value::Str(joined))
                }
                _ => Err(MoofError::runtime("join: expects a String separator")),
            }
        }
        "indexOf:" => {
            check_args(selector, &args, 1)?;
            let idx = items.iter().position(|v| v == &args[0]);
            Ok(Value::Integer(idx.map(|i| i as i64).unwrap_or(-1)))
        }
        "take:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Integer(n) => {
                    let n = *n as usize;
                    Ok(Value::List(items.into_iter().take(n).collect()))
                }
                _ => Err(MoofError::runtime("take: expects an Integer")),
            }
        }
        "drop:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Integer(n) => {
                    let n = *n as usize;
                    Ok(Value::List(items.into_iter().skip(n).collect()))
                }
                _ => Err(MoofError::runtime("drop: expects an Integer")),
            }
        }
        "zip:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::List(other) => {
                    let zipped: Vec<Value> = items
                        .iter()
                        .zip(other.iter())
                        .map(|(a, b)| Value::List(vec![a.clone(), b.clone()]))
                        .collect();
                    Ok(Value::List(zipped))
                }
                _ => Err(MoofError::runtime("zip: expects a List")),
            }
        }
        "map:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            let mut result = Vec::with_capacity(items.len());
            for item in items {
                result.push(call_value_func(interp, &func, vec![item])?);
            }
            Ok(Value::List(result))
        }
        "filter:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            let mut result = Vec::new();
            for item in items {
                let v = call_value_func(interp, &func, vec![item.clone()])?;
                if v.is_truthy() {
                    result.push(item);
                }
            }
            Ok(Value::List(result))
        }
        "reduce:init:" => {
            check_args(selector, &args, 2)?;
            let func = &args[0];
            let mut acc = args[1].clone();
            for item in items {
                acc = call_value_func(interp, func, vec![acc, item])?;
            }
            Ok(acc)
        }
        "each:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            for item in items {
                call_value_func(interp, &func, vec![item])?;
            }
            Ok(Value::Nil)
        }
        "any:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            for item in items {
                let v = call_value_func(interp, &func, vec![item])?;
                if v.is_truthy() {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        "all:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            for item in items {
                let v = call_value_func(interp, &func, vec![item])?;
                if !v.is_truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        "none:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            for item in items {
                let v = call_value_func(interp, &func, vec![item])?;
                if v.is_truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        "sortBy:" => {
            check_args(selector, &args, 1)?;
            let func = args.into_iter().next().unwrap();
            let mut keyed: Vec<(Value, Value)> = Vec::with_capacity(items.len());
            for item in items {
                let key = call_value_func(interp, &func, vec![item.clone()])?;
                keyed.push((key, item));
            }
            keyed.sort_by(|(a, _), (b, _)| value_cmp(a, b));
            Ok(Value::List(keyed.into_iter().map(|(_, v)| v).collect()))
        }
        _ => Err(make_message_error_with_suggest(&receiver, selector)),
    }
}

fn flatten_into(items: &[Value], result: &mut Vec<Value>) {
    for item in items {
        match item {
            Value::List(inner) => flatten_into(inner, result),
            other => result.push(other.clone()),
        }
    }
}

fn value_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Integer(x), Value::Float(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Float(x), Value::Integer(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

// ── Map dispatch ────────────────────────────────────────────────

fn dispatch_map(
    _interp: &mut Interpreter,
    receiver: Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    let pairs = match &receiver {
        Value::Map(pairs) => pairs.clone(),
        _ => unreachable!(),
    };
    match selector {
        "keys" => Ok(Value::List(pairs.iter().map(|(k, _)| Value::Str(k.clone())).collect())),
        "values" => Ok(Value::List(pairs.iter().map(|(_, v)| v.clone()).collect())),
        "length" => Ok(Value::Integer(pairs.len() as i64)),
        "empty?" => Ok(Value::Bool(pairs.is_empty())),
        "to_s" => Ok(Value::Str(format!("{}", receiver))),
        "nil?" => Ok(Value::Bool(false)),
        "class" => Ok(Value::Str("Map".to_string())),
        "at:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(key) => {
                    let found = pairs.iter().find(|(k, _)| k == key);
                    Ok(found.map(|(_, v)| v.clone()).unwrap_or(Value::Nil))
                }
                _ => Err(MoofError::runtime("at: expects a String key")),
            }
        }
        "put:value:" => {
            check_args(selector, &args, 2)?;
            match &args[0] {
                Value::Str(key) => {
                    let mut new_pairs = pairs;
                    let existing = new_pairs.iter().position(|(k, _)| k == key);
                    if let Some(idx) = existing {
                        new_pairs[idx] = (key.clone(), args[1].clone());
                    } else {
                        new_pairs.push((key.clone(), args[1].clone()));
                    }
                    Ok(Value::Map(new_pairs))
                }
                _ => Err(MoofError::runtime("put:value: expects a String key")),
            }
        }
        "remove:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(key) => {
                    let new_pairs: Vec<_> = pairs.into_iter().filter(|(k, _)| k != key).collect();
                    Ok(Value::Map(new_pairs))
                }
                _ => Err(MoofError::runtime("remove: expects a String key")),
            }
        }
        "contains:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(key) => Ok(Value::Bool(pairs.iter().any(|(k, _)| k == key))),
                _ => Err(MoofError::runtime("contains: expects a String key")),
            }
        }
        "merge:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Map(other) => {
                    let mut merged = pairs;
                    for (k, v) in other {
                        let existing = merged.iter().position(|(mk, _)| mk == k);
                        if let Some(idx) = existing {
                            merged[idx] = (k.clone(), v.clone());
                        } else {
                            merged.push((k.clone(), v.clone()));
                        }
                    }
                    Ok(Value::Map(merged))
                }
                _ => Err(MoofError::runtime("merge: expects a Map")),
            }
        }
        _ => Err(make_message_error_with_suggest(&receiver, selector)),
    }
}

// ── Function dispatch ───────────────────────────────────────────

fn dispatch_function(
    interp: &mut Interpreter,
    receiver: Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    match selector {
        "call:" => call_value_func(interp, &receiver, args),
        "nil?" => Ok(Value::Bool(false)),
        "class" => Ok(Value::Str("Function".to_string())),
        "arity" => match &receiver {
            Value::Function(f) => Ok(Value::Integer(f.arity() as i64)),
            Value::Builtin(_, _) => Ok(Value::Integer(-1)),
            _ => unreachable!(),
        },
        _ => Err(make_message_error_with_suggest(&receiver, selector)),
    }
}

// ── Bool dispatch ───────────────────────────────────────────────

fn dispatch_boolean(receiver: &Value, selector: &str, args: &[Value]) -> Result<Value> {
    let b = match receiver {
        Value::Bool(b) => *b,
        _ => unreachable!(),
    };
    match selector {
        "not" => Ok(Value::Bool(!b)),
        "to_s" => Ok(Value::Str(b.to_string())),
        "nil?" => Ok(Value::Bool(false)),
        "class" => Ok(Value::Str("Bool".to_string())),
        "and:" => {
            check_args(selector, args, 1)?;
            if b { Ok(args[0].clone()) } else { Ok(Value::Bool(false)) }
        }
        "or:" => {
            check_args(selector, args, 1)?;
            if b { Ok(receiver.clone()) } else { Ok(args[0].clone()) }
        }
        _ => Err(make_message_error(receiver, selector)),
    }
}

// ── Nil dispatch ────────────────────────────────────────────────

fn dispatch_nil(selector: &str) -> Result<Value> {
    match selector {
        "nil?" => Ok(Value::Bool(true)),
        "to_s" => Ok(Value::Str("nil".to_string())),
        "class" => Ok(Value::Str("Nil".to_string())),
        _ => Err(MoofError::message("Nil", selector, None)),
    }
}

// ── MoofObject dispatch ─────────────────────────────────────────

fn dispatch_moof_object(
    interp: &mut Interpreter,
    receiver: Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    let obj = match &receiver {
        Value::Object(obj) => obj.clone(),
        _ => unreachable!(),
    };

    match selector {
        "class" => Ok(Value::Class(obj.class.clone())),
        "className" => Ok(Value::Str(obj.class.borrow().name.clone())),
        "methods" => {
            let mut all_methods = Vec::new();
            let klass = obj.class.borrow();
            for key in klass.methods.keys() {
                if !all_methods.contains(key) {
                    all_methods.push(key.clone());
                }
            }
            let mut sup = klass.superclass.clone();
            drop(klass);
            while let Some(s) = sup {
                let sb = s.borrow();
                for key in sb.methods.keys() {
                    if !all_methods.contains(key) {
                        all_methods.push(key.clone());
                    }
                }
                sup = sb.superclass.clone();
            }
            Ok(Value::List(all_methods.into_iter().map(Value::Str).collect()))
        }
        "respondsTo:" => {
            check_args(selector, &args, 1)?;
            match &args[0] {
                Value::Str(sel) => {
                    let found = obj.class.borrow().lookup(sel).is_some();
                    Ok(Value::Bool(found))
                }
                _ => Err(MoofError::runtime("respondsTo: expects a String")),
            }
        }
        "nil?" => Ok(Value::Bool(false)),
        "to_s" => Ok(Value::Str(format!("{}", receiver))),
        "set:to:" => {
            check_args(selector, &args, 2)?;
            match &args[0] {
                Value::Str(field_name) => {
                    let mut new_obj = obj;
                    new_obj.set_field(field_name, args[1].clone());
                    Ok(Value::Object(new_obj))
                }
                _ => Err(MoofError::runtime("set:to: expects a String field name")),
            }
        }
        _ => {
            let method = obj.class.borrow().lookup(selector);
            if let Some(method) = method {
                return invoke_method(interp, &method, receiver, args);
            }
            if let Some(val) = obj.fields.get(selector) {
                return Ok(val.clone());
            }
            let class_name = obj.class.borrow().name.clone();
            Err(MoofError::message(&class_name, selector, None))
        }
    }
}

// ── MoofClass dispatch ──────────────────────────────────────────

fn dispatch_moof_class(receiver: &Value, selector: &str) -> Result<Value> {
    let klass_rc = match receiver {
        Value::Class(k) => k,
        _ => unreachable!(),
    };
    let klass = klass_rc.borrow();
    match selector {
        "name" => Ok(Value::Str(klass.name.clone())),
        "fields" => Ok(Value::List(klass.all_fields().into_iter().map(Value::Str).collect())),
        "methods" => Ok(Value::List(klass.methods.keys().map(|k| Value::Str(k.clone())).collect())),
        "nil?" => Ok(Value::Bool(false)),
        "class" => Ok(Value::Str("Class".to_string())),
        _ => {
            let name = klass.name.clone();
            Err(MoofError::message(&format!("Class({})", name), selector, None))
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn invoke_method(
    interp: &mut Interpreter,
    method: &MoofFunction,
    self_obj: Value,
    args: Vec<Value>,
) -> Result<Value> {
    if let Some(ref _rest) = method.rest_param {
        if args.len() < method.params.len() {
            return Err(MoofError::arity(
                &format!("{}+", method.params.len()),
                args.len(),
                method.name.as_deref(),
            ));
        }
    } else if args.len() != method.params.len() {
        return Err(MoofError::arity(
            &method.params.len().to_string(),
            args.len(),
            method.name.as_deref(),
        ));
    }

    let call_env = method.closure.child();
    call_env.define("self", self_obj, false);
    for (i, param) in method.params.iter().enumerate() {
        call_env.define(param, args.get(i).cloned().unwrap_or(Value::Nil), false);
    }
    if let Some(ref rest) = method.rest_param {
        let rest_args = if args.len() > method.params.len() {
            Value::List(args[method.params.len()..].to_vec())
        } else {
            Value::List(vec![])
        };
        call_env.define(rest, rest_args, false);
    }

    interp.evaluate_node(&method.body, &call_env)
}

/// Call a Value that is expected to be a function (Function or Builtin).
pub fn call_value_func(
    interp: &mut Interpreter,
    func: &Value,
    args: Vec<Value>,
) -> Result<Value> {
    match func {
        Value::Function(f) => crate::interpreter::call_function(interp, f, args),
        Value::Builtin(_, f_ptr) => f_ptr(interp, args),
        _ => Err(MoofError::runtime(format!(
            "Expected a function, got {}",
            func.type_name()
        ))),
    }
}
