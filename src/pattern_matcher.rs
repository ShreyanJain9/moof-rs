use crate::ast::{Expr, Pattern};
use crate::interpreter::Interpreter;
use crate::value::Value;

pub struct MatchResult {
    pub success: bool,
    pub bindings: Vec<(String, Value)>,
}

impl MatchResult {
    fn success(bindings: Vec<(String, Value)>) -> Self {
        MatchResult { success: true, bindings }
    }

    fn failure() -> Self {
        MatchResult { success: false, bindings: vec![] }
    }
}

pub fn match_pattern(pattern: &Pattern, value: &Value, interp: &Interpreter) -> MatchResult {
    match pattern {
        Pattern::Wildcard => MatchResult::success(vec![]),

        Pattern::Bind(name) => {
            if name == "_" {
                MatchResult::success(vec![])
            } else {
                MatchResult::success(vec![(name.clone(), value.clone())])
            }
        }

        Pattern::Literal(expr) => {
            let matches = match expr.as_ref() {
                Expr::Integer(n, _) => matches!(value, Value::Integer(v) if *v == *n),
                Expr::Float(n, _) => matches!(value, Value::Float(v) if *v == *n),
                Expr::Str(s, _) => matches!(value, Value::Str(v) if v == s),
                Expr::Bool(b, _) => matches!(value, Value::Bool(v) if *v == *b),
                Expr::Nil(_) => matches!(value, Value::Nil),
                _ => false,
            };
            if matches {
                MatchResult::success(vec![])
            } else {
                MatchResult::failure()
            }
        }

        Pattern::List(elements, rest) => match_list(elements, rest.as_deref(), value, interp),
        Pattern::Map(pairs) => match_map(pairs, value, interp),
        Pattern::Constructor(class_name, bindings) => {
            match_constructor(class_name, bindings, value, interp)
        }
    }
}

fn match_list(
    elements: &[Pattern],
    rest: Option<&Pattern>,
    value: &Value,
    interp: &Interpreter,
) -> MatchResult {
    let items = match value {
        Value::List(items) => items,
        _ => return MatchResult::failure(),
    };

    if rest.is_some() {
        if items.len() < elements.len() {
            return MatchResult::failure();
        }
    } else if items.len() != elements.len() {
        return MatchResult::failure();
    }

    let mut bindings = Vec::new();

    for (i, pat) in elements.iter().enumerate() {
        let result = match_pattern(pat, &items[i], interp);
        if !result.success {
            return MatchResult::failure();
        }
        bindings.extend(result.bindings);
    }

    if let Some(rest_pat) = rest {
        let rest_value = Value::List(items[elements.len()..].to_vec());
        let result = match_pattern(rest_pat, &rest_value, interp);
        if !result.success {
            return MatchResult::failure();
        }
        bindings.extend(result.bindings);
    }

    MatchResult::success(bindings)
}

fn match_map(
    pairs: &[(String, Pattern)],
    value: &Value,
    interp: &Interpreter,
) -> MatchResult {
    let map_pairs = match value {
        Value::Map(pairs) => pairs,
        _ => return MatchResult::failure(),
    };

    let mut bindings = Vec::new();

    for (key, sub_pattern) in pairs {
        let found = map_pairs.iter().find(|(k, _)| k == key);
        match found {
            Some((_, val)) => {
                let result = match_pattern(sub_pattern, val, interp);
                if !result.success {
                    return MatchResult::failure();
                }
                bindings.extend(result.bindings);
            }
            None => return MatchResult::failure(),
        }
    }

    MatchResult::success(bindings)
}

fn match_constructor(
    class_name: &str,
    pat_bindings: &[Pattern],
    value: &Value,
    interp: &Interpreter,
) -> MatchResult {
    let obj = match value {
        Value::Object(obj) => obj,
        _ => return MatchResult::failure(),
    };

    let obj_class_name = obj.class.borrow().name.clone();
    if obj_class_name != class_name {
        return MatchResult::failure();
    }

    let field_names = obj.class.borrow().all_fields();
    let mut bindings = Vec::new();

    for (i, bind_pattern) in pat_bindings.iter().enumerate() {
        let field_name = match field_names.get(i) {
            Some(name) => name,
            None => return MatchResult::failure(),
        };
        let field_value = obj.fields.get(field_name).cloned().unwrap_or(Value::Nil);
        let result = match_pattern(bind_pattern, &field_value, interp);
        if !result.success {
            return MatchResult::failure();
        }
        bindings.extend(result.bindings);
    }

    MatchResult::success(bindings)
}
