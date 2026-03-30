use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::value::{Value, Method, MoofFunction};
use std::cell::RefCell;
use std::rc::Rc;

use crate::value::MoofClass;

// ── Unified message dispatch ───────────────────────────────────

/// Dispatch a message send to the appropriate handler.
///
/// Resolution order:
///   1. Look up the selector in the receiver's class (which walks the superclass chain).
///   2. For MoofObject: fall back to field access if the selector matches a field name.
///   3. For MoofClass: handle introspection selectors (name, fields, methods, …).
///   4. If nothing matched, produce an error with a Levenshtein-based suggestion.
pub fn send_message(
    interp: &mut Interpreter,
    receiver: Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    // ── Class-as-receiver (the Value::Class variant) ───────────
    if let Value::Class(ref klass_rc) = receiver {
        // Introspection selectors on the class object itself.
        match selector {
            "name" => {
                return Ok(Value::Str(klass_rc.borrow().name.clone()));
            }
            "fields" => {
                return Ok(Value::List(
                    klass_rc.borrow().all_fields().into_iter().map(Value::Str).collect(),
                ));
            }
            "methods" => {
                return Ok(Value::List(
                    klass_rc.borrow().methods.keys().map(|k| Value::Str(k.clone())).collect(),
                ));
            }
            "nil?" => return Ok(Value::Bool(false)),
            "class" => return Ok(Value::Str("Class".to_string())),
            _ => {
                // Check the class registry for class-level methods (e.g. custom
                // class methods registered on the class itself).
                let name = klass_rc.borrow().name.clone();
                return Err(MoofError::message(
                    &format!("Class({})", name),
                    selector,
                    None,
                ));
            }
        }
    }

    // ── MoofObject: look up in the object's own class hierarchy ──
    if let Value::Object(ref obj) = receiver {
        // Check the object's class hierarchy first
        let method = obj.class.borrow().lookup(selector);
        if let Some(m) = method {
            return dispatch_method(interp, m, receiver, args);
        }
    }

    // ── Determine the class for non-Object receivers ──────────
    let class_name = class_of(&receiver);

    // Look up the class in the registry (for built-in types + user extensions).
    if let Some(klass_rc) = interp.class_registry.get(class_name).cloned() {
        let method = klass_rc.borrow().lookup(selector);
        if let Some(m) = method {
            return dispatch_method(interp, m, receiver, args);
        }
    }

    // ── MoofObject fallbacks (special selectors, field access) ──
    if let Value::Object(ref obj) = receiver {
        // Special selector: set:to: — field setter
        if selector == "set:to:" {
            if args.len() != 2 {
                return Err(MoofError::arity("2", args.len(), Some("set:to:")));
            }
            match &args[0] {
                Value::Str(field_name) => {
                    let mut new_obj = obj.clone();
                    new_obj.set_field(field_name, args[1].clone());
                    return Ok(Value::Object(new_obj));
                }
                _ => return Err(MoofError::runtime("set:to: expects a String field name")),
            }
        }

        // Common object selectors
        match selector {
            "class" => return Ok(Value::Class(obj.class.clone())),
            "className" => return Ok(Value::Str(obj.class.borrow().name.clone())),
            "nil?" => return Ok(Value::Bool(false)),
            "to_s" => return Ok(Value::Str(format!("{}", receiver))),
            "respondsTo:" => {
                if args.len() != 1 {
                    return Err(MoofError::arity("1", args.len(), Some("respondsTo:")));
                }
                match &args[0] {
                    Value::Str(sel) => {
                        let found = obj.class.borrow().lookup(sel).is_some();
                        return Ok(Value::Bool(found));
                    }
                    _ => return Err(MoofError::runtime("respondsTo: expects a String")),
                }
            }
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
                return Ok(Value::List(all_methods.into_iter().map(Value::Str).collect()));
            }
            _ => {}
        }

        // Field access as fallback.
        if let Some(val) = obj.fields.get(selector) {
            return Ok(val.clone());
        }

        // Nothing matched — produce error with suggestion.
        let obj_class_name = obj.class.borrow().name.clone();
        let suggestion = suggest_selector_for_object(obj, selector);
        return Err(MoofError::message(&obj_class_name, selector, suggestion.as_deref()));
    }

    // ── Generic fallback error ─────────────────────────────────
    let suggestion = suggest_selector(&receiver, selector);
    Err(MoofError::message(class_name, selector, suggestion.as_deref()))
}

/// Dispatch a resolved Method to either a Rust built-in or a user-defined function.
fn dispatch_method(
    interp: &mut Interpreter,
    method: Method,
    receiver: Value,
    args: Vec<Value>,
) -> Result<Value> {
    match method {
        Method::Builtin(_, f) => f(interp, receiver, args),
        Method::UserDefined(func) => invoke_method(interp, &func, receiver, args),
    }
}

// ── class_of — map Value → class name string ──────────────────

/// Return the class name string for any value.
pub fn class_of(value: &Value) -> &'static str {
    match value {
        Value::Integer(_) => "Integer",
        Value::Float(_) => "Float",
        Value::Str(_) => "String",
        Value::List(_) => "List",
        Value::Map(_) => "Map",
        Value::Bool(_) => "Bool",
        Value::Nil => "Nil",
        Value::Function(_) | Value::Builtin(_, _) => "Function",
        // Object and Class are handled specially by send_message before we get here,
        // but provide sensible fallbacks.
        Value::Object(_) => "Object",
        Value::Class(_) => "Class",
        Value::Symbol(_) => "Symbol",
        Value::Protocol(_) => "Protocol",
        Value::Macro(_) => "Macro",
    }
}

// ── class_of_rc — return the Rc<RefCell<MoofClass>> for a value ─

/// Return the class object (Rc<RefCell<MoofClass>>) for a value, if one exists
/// in the interpreter's class registry.
pub fn class_of_rc(interp: &Interpreter, value: &Value) -> Option<Rc<RefCell<MoofClass>>> {
    match value {
        Value::Object(obj) => Some(obj.class.clone()),
        Value::Class(k) => Some(k.clone()),
        other => {
            let name = class_of(other);
            interp.class_registry.get(name).cloned()
        }
    }
}

// ── invoke_method — call a user-defined method with self binding ─

/// Evaluate a user-defined method body with `self` bound to the receiver.
pub fn invoke_method(
    interp: &mut Interpreter,
    method: &MoofFunction,
    self_val: Value,
    args: Vec<Value>,
) -> Result<Value> {
    // Check arity.
    let expected = method.params.len();
    if method.rest_param.is_some() {
        if args.len() < expected {
            return Err(MoofError::arity(
                &format!("{}+", expected),
                args.len(),
                method.name.as_deref(),
            ));
        }
    } else if args.len() != expected {
        return Err(MoofError::arity(
            &expected.to_string(),
            args.len(),
            method.name.as_deref(),
        ));
    }

    // Create a child environment from the method's closure and bind self + params.
    let call_env = method.closure.child();
    call_env.define("self", self_val.clone(), false);

    // Bind object fields as local variables so they're accessible without `self.`
    if let Value::Object(ref obj) = self_val {
        for (field_name, field_val) in &obj.fields {
            call_env.define(field_name, field_val.clone(), false);
        }
    }

    for (i, param) in method.params.iter().enumerate() {
        call_env.define(param, args.get(i).cloned().unwrap_or(Value::Nil), false);
    }

    if let Some(ref rest_name) = method.rest_param {
        let rest = if args.len() > expected {
            args[expected..].to_vec()
        } else {
            vec![]
        };
        call_env.define(rest_name, Value::List(rest), false);
    }

    interp.eval_expr(&method.body, &call_env)
}

// ── Levenshtein distance & selector suggestion ─────────────────

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

/// Maximum edit distance we allow for a suggestion, based on query length.
fn max_suggestion_distance(query_len: usize) -> usize {
    if query_len <= 2 {
        1
    } else {
        3usize.min(((query_len as f64) / 2.0).ceil() as usize)
    }
}

/// Suggest a known selector close to `failed` for a MoofObject (uses its class
/// hierarchy's method names plus field names).
fn suggest_selector_for_object(obj: &crate::value::MoofObject, failed: &str) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();

    // Collect method names from class hierarchy.
    let klass = obj.class.borrow();
    candidates.extend(klass.methods.keys().cloned());
    let mut sup = klass.superclass.clone();
    drop(klass);
    while let Some(s) = sup {
        let sb = s.borrow();
        candidates.extend(sb.methods.keys().cloned());
        sup = sb.superclass.clone();
    }

    // Collect field names.
    candidates.extend(obj.fields.keys().cloned());

    // Built-in object selectors.
    for s in &["class", "className", "nil?", "to_s", "respondsTo:", "methods", "set:to:"] {
        candidates.push(s.to_string());
    }

    find_closest(failed, &candidates)
}

/// Suggest a known selector close to `failed` for a primitive value, using
/// the class registry to discover all registered methods.
pub fn suggest_selector(receiver: &Value, failed: &str) -> Option<String> {
    // Gather known selectors from the hardcoded list for backward compatibility,
    // plus any user-registered methods on the class.
    let known = collect_known_selectors(receiver);
    find_closest(failed, &known)
}

fn find_closest(query: &str, candidates: &[String]) -> Option<String> {
    if candidates.is_empty() { return None; }

    let mut best: Option<&str> = None;
    let mut best_dist = usize::MAX;

    for sel in candidates {
        let dist = levenshtein(query, sel);
        if dist < best_dist {
            best_dist = dist;
            best = Some(sel.as_str());
        }
    }

    let max_dist = max_suggestion_distance(query.len());
    if best_dist <= max_dist {
        best.map(|s| s.to_string())
    } else {
        None
    }
}

/// Collect known selectors for primitive types (used for error suggestions).
fn collect_known_selectors(receiver: &Value) -> Vec<String> {
    let statics: &[&str] = match receiver {
        Value::Integer(_) | Value::Float(_) => &[
            "abs", "to_s", "to_f", "to_i", "zero?", "positive?", "negative?",
            "nil?", "class", "sqrt", "pow:", "max:", "min:",
            "+", "-", "*", "/", "%", ">", "<", ">=", "<=",
        ],
        Value::Str(_) => &[
            "length", "uppercase", "lowercase", "reverse", "to_s", "to_i", "to_f",
            "chars", "trim", "nil?", "class",
            "at:", "contains:", "startsWith:", "endsWith:", "replaceAll:with:",
            "split:", "concat:", "slice:length:",
        ],
        Value::List(_) => &[
            "length", "first", "last", "rest", "reverse", "sort", "uniq", "flatten",
            "empty?", "to_s", "nil?", "class",
            "at:", "push:", "prepend:", "contains:", "join:", "indexOf:",
            "take:", "drop:", "zip:",
            "map:", "filter:", "reduce:init:", "each:", "any:", "all:", "none:", "sortBy:",
        ],
        Value::Map(_) => &[
            "keys", "values", "length", "empty?", "to_s", "nil?", "class",
            "at:", "put:value:", "remove:", "contains:", "merge:",
        ],
        Value::Function(_) | Value::Builtin(_, _) => &["call:", "arity", "nil?", "class"],
        Value::Bool(_) => &["not", "to_s", "nil?", "class", "and:", "or:"],
        Value::Nil => &["nil?", "to_s", "class"],
        _ => &["nil?"],
    };

    statics.iter().map(|s| s.to_string()).collect()
}

/// Backward-compatible alias used by the existing interpreter code.
pub fn dispatch_message(
    interp: &mut Interpreter,
    receiver: &Value,
    selector: &str,
    args: Vec<Value>,
) -> Result<Value> {
    send_message(interp, receiver.clone(), selector, args)
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
