use crate::cons::cons_to_vec;
use crate::symbol::{KnownSymbols, SymId, SymbolTable};
use crate::value::{ClosureBody, Value};

/// Pretty-print a Moof value as readable source code.
/// Reverses parser desugaring: __send → [], lambda → {}, __table → {k:v}, etc.
pub fn pretty_print(val: &Value, symbols: &SymbolTable, known: &KnownSymbols) -> String {
    pp(val, symbols, known, 0)
}

fn pp(val: &Value, sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    match val {
        Value::Integer(n) => format!("{n}"),
        Value::Float(n) => {
            if n.fract() == 0.0 && !n.is_nan() && !n.is_infinite() {
                format!("{n:.1}")
            } else {
                format!("{n}")
            }
        }
        Value::Bool(b) => format!("{b}"),
        Value::Nil => "nil".to_string(),
        Value::Symbol(id) => sym.name(*id).to_string(),
        Value::Str(s) => format!("{:?}", &**s),
        Value::Cons(_) => pp_list(val, sym, k, indent),
        Value::Table(tbl) => {
            let tbl = tbl.borrow();
            pp_table_value(&tbl, sym, k, indent)
        }
        Value::Closure(c) => pp_closure(c, sym, k, indent),
        Value::Range(r) => {
            let step = r.step.to_i64().unwrap_or(1);
            if step != 1 {
                format!("(range {} {} {})", r.start, r.end, r.step)
            } else {
                format!("(range {} {})", r.start, r.end)
            }
        }
        Value::Object(_) => format!("{val}"),
    }
}

fn pp_list(val: &Value, sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    let items = cons_to_vec(val);
    if items.is_empty() {
        return "(list)".to_string();
    }

    // Check for special forms by inspecting the head
    if let Value::Symbol(head_id) = &items[0] {
        let head = *head_id;

        // __send → [receiver selector args...]
        if head == k.send {
            return pp_send(&items[1..], sym, k, indent);
        }
        // __super-send → [super selector args...]
        if head == k.super_send {
            return pp_super_send(&items[1..], sym, k, indent);
        }
        // lambda → { |params| body }
        if head == k.lambda || head == k.fn_ {
            return pp_lambda(&items, sym, k, indent);
        }
        // __table → {k: v, ...}
        if head == k.table {
            return pp_table(&items[1..], sym, k, indent);
        }
        // __table-array → {v1, v2, ...}
        if head == k.table_array {
            return pp_table_array(&items[1..], sym, k, indent);
        }
        // __str-interp → $"..."
        if head == k.str_interp {
            return pp_str_interp(&items[1..], sym, k, indent);
        }
        // quote → 'x
        if head == k.quote && items.len() == 2 {
            return format!("'{}", pp(&items[1], sym, k, indent));
        }
        // quasiquote → `x
        if head == k.quasiquote && items.len() == 2 {
            return format!("`{}", pp(&items[1], sym, k, indent));
        }
        // unquote → ,x
        if head == k.unquote && items.len() == 2 {
            return format!(",{}", pp(&items[1], sym, k, indent));
        }
        // unquote-splice → ,@x
        if head == k.unquote_splice && items.len() == 2 {
            return format!(",@{}", pp(&items[1], sym, k, indent));
        }
    }

    // (define name (lambda (params) body)) → (define (name params) body)
    if let Value::Symbol(id) = &items[0] {
        if *id == k.define && items.len() == 3 {
            if let Value::Symbol(name_id) = &items[1] {
                if let Value::Cons(_) = &items[2] {
                    let inner = cons_to_vec(&items[2]);
                    if inner.len() >= 3 {
                        if let Value::Symbol(inner_head) = &inner[0] {
                            if *inner_head == k.lambda || *inner_head == k.fn_ {
                                let name = sym.name(*name_id);
                                let params_val = &inner[1];
                                let params_vec = cons_to_vec(params_val);
                                let param_names: Vec<String> = params_vec.iter()
                                    .filter_map(|p| {
                                        if let Value::Symbol(pid) = p {
                                            let n = sym.name(*pid);
                                            if n == "." || n == "&" { None }
                                            else { Some(n.to_string()) }
                                        } else { None }
                                    })
                                    .collect();
                                let params_str = param_names.join(" ");
                                let body_parts = &inner[2..];
                                let body_strs: Vec<String> = body_parts.iter()
                                    .map(|b| pp(b, sym, k, indent + 2))
                                    .collect();
                                if body_strs.len() == 1 && body_strs[0].len() + name.len() + params_str.len() < 60 {
                                    return format!("(define ({name} {params_str}) {})", body_strs[0]);
                                } else {
                                    let pad = " ".repeat(indent + 2);
                                    let body_lines = body_strs.iter()
                                        .map(|b| format!("{pad}{b}"))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    return format!("(define ({name} {params_str})\n{body_lines})");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Regular s-expression: (head args...)
    pp_sexpr(&items, sym, k, indent)
}

/// Pretty-print a message send: [receiver selector args...]
fn pp_send(args: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    if args.is_empty() {
        return "[]".to_string();
    }
    let receiver = pp(&args[0], sym, k, indent);

    if args.len() < 2 {
        return format!("[{receiver}]");
    }

    // Get selector name
    let selector = match &args[1] {
        Value::Str(s) => s.to_string(),
        Value::Symbol(id) => sym.name(*id).to_string(),
        other => pp(other, sym, k, indent),
    };

    let msg_args = &args[2..];

    if selector.contains(':') {
        // Keyword message: split selector on colons, interleave with args
        let parts: Vec<&str> = selector.split(':').filter(|s| !s.is_empty()).collect();
        let mut out = format!("[{receiver}");
        for (i, part) in parts.iter().enumerate() {
            out.push_str(&format!(" {part}:"));
            if i < msg_args.len() {
                out.push_str(&format!(" {}", pp(&msg_args[i], sym, k, indent)));
            }
        }
        out.push(']');
        out
    } else if msg_args.is_empty() {
        // Unary message
        format!("[{receiver} {selector}]")
    } else {
        // Binary or simple message with args
        let args_str: Vec<String> = msg_args.iter().map(|a| pp(a, sym, k, indent)).collect();
        format!("[{receiver} {} {}]", selector, args_str.join(" "))
    }
}

/// Pretty-print a super send: [super selector args...]
fn pp_super_send(args: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    if args.is_empty() {
        return "[super]".to_string();
    }
    let selector = match &args[0] {
        Value::Str(s) => s.to_string(),
        Value::Symbol(id) => sym.name(*id).to_string(),
        other => pp(other, sym, k, indent),
    };
    let msg_args = &args[1..];

    if selector.contains(':') {
        let parts: Vec<&str> = selector.split(':').filter(|s| !s.is_empty()).collect();
        let mut out = "[super".to_string();
        for (i, part) in parts.iter().enumerate() {
            out.push_str(&format!(" {part}:"));
            if i < msg_args.len() {
                out.push_str(&format!(" {}", pp(&msg_args[i], sym, k, indent)));
            }
        }
        out.push(']');
        out
    } else if msg_args.is_empty() {
        format!("[super {selector}]")
    } else {
        let args_str: Vec<String> = msg_args.iter().map(|a| pp(a, sym, k, indent)).collect();
        format!("[super {} {}]", selector, args_str.join(" "))
    }
}

/// Pretty-print a lambda. Short lambdas become blocks: { |params| body }
fn pp_lambda(items: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    if items.len() < 3 {
        return pp_sexpr(items, sym, k, indent);
    }

    let head_id = if let Value::Symbol(id) = &items[0] { *id } else { return pp_sexpr(items, sym, k, indent) };
    let params_val = &items[1];
    let body = &items[2..];

    // Parse params
    let params_vec = cons_to_vec(params_val);
    let mut param_names: Vec<String> = Vec::new();
    let mut rest_param: Option<String> = None;
    let mut i = 0;
    while i < params_vec.len() {
        if let Value::Symbol(id) = &params_vec[i] {
            let name = sym.name(*id);
            if (name == "." || name == "&") && i + 1 < params_vec.len() {
                if let Value::Symbol(rest_id) = &params_vec[i + 1] {
                    rest_param = Some(sym.name(*rest_id).to_string());
                }
                break;
            }
            param_names.push(name.to_string());
        }
        i += 1;
    }

    // Decide: block syntax { |params| body } or (fn (params) body)
    let body_strs: Vec<String> = body.iter().map(|b| pp(b, sym, k, indent + 2)).collect();
    let body_str = body_strs.join(" ");
    let total_len = body_str.len() + param_names.join(" ").len();

    // Use block syntax for short anonymous lambdas
    if head_id == k.lambda && rest_param.is_none() && total_len < 60 {
        let params_str = if param_names.is_empty() {
            "||".to_string()
        } else {
            format!("|{}|", param_names.join(" "))
        };
        return format!("{{ {params_str} {body_str} }}");
    }

    // Use (fn (params) body) for named or long lambdas
    let head_name = sym.name(head_id);
    let mut params_str = param_names.join(" ");
    if let Some(rest) = &rest_param {
        if params_str.is_empty() {
            params_str = format!(". {rest}");
        } else {
            params_str = format!("{params_str} . {rest}");
        }
    }

    if body_strs.len() == 1 && total_len < 60 {
        format!("({head_name} ({params_str}) {})", body_strs[0])
    } else {
        let pad = " ".repeat(indent + 2);
        let body_lines = body.iter()
            .map(|b| format!("{pad}{}", pp(b, sym, k, indent + 2)))
            .collect::<Vec<_>>()
            .join("\n");
        format!("({head_name} ({params_str})\n{body_lines})")
    }
}

/// Pretty-print a hash table literal: {k1: v1, k2: v2}
fn pp_table(items: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    if items.is_empty() {
        return "{}".to_string();
    }
    let mut pairs = Vec::new();
    let mut i = 0;
    while i + 1 < items.len() {
        let key = match &items[i] {
            Value::Str(s) => s.to_string(),
            other => pp(other, sym, k, indent),
        };
        let val = pp(&items[i + 1], sym, k, indent);
        pairs.push(format!("{key}: {val}"));
        i += 2;
    }
    let inner = pairs.join(", ");
    if inner.len() < 60 {
        format!("{{{inner}}}")
    } else {
        let pad = " ".repeat(indent + 2);
        let lines = pairs.iter()
            .map(|p| format!("{pad}{p}"))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("{{\n{lines}}}")
    }
}

/// Pretty-print an array table literal: {v1, v2, ...}
fn pp_table_array(items: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    if items.is_empty() {
        return "{}".to_string();
    }
    let vals: Vec<String> = items.iter().map(|v| pp(v, sym, k, indent)).collect();
    let inner = vals.join(", ");
    format!("{{{inner}}}")
}

/// Pretty-print string interpolation: $"...\(expr)..."
fn pp_str_interp(parts: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    let mut out = String::from("$\"");
    for part in parts {
        match part {
            Value::Str(s) => {
                // Literal string part — escape special chars but not quotes (they're inside $"")
                for ch in s.chars() {
                    match ch {
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        _ => out.push(ch),
                    }
                }
            }
            _ => {
                // Interpolated expression
                out.push_str("\\(");
                out.push_str(&pp(part, sym, k, indent));
                out.push(')');
            }
        }
    }
    out.push('"');
    out
}

/// Pretty-print a regular s-expression: (head args...)
fn pp_sexpr(items: &[Value], sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    if items.is_empty() {
        return "(list)".to_string();
    }

    let parts: Vec<String> = items.iter().map(|v| pp(v, sym, k, indent)).collect();
    let oneline = format!("({})", parts.join(" "));

    if oneline.len() <= 72 {
        return oneline;
    }

    // Multi-line: first element on same line, rest indented
    let pad = " ".repeat(indent + 2);
    let head = &parts[0];
    let rest = parts[1..].iter()
        .map(|p| format!("{pad}{p}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("({head}\n{rest})")
}

/// Pretty-print a MoofTable value (not the __table AST form)
fn pp_table_value(tbl: &crate::value::MoofTable, sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    let mut parts = Vec::new();
    for v in &tbl.array {
        parts.push(pp(v, sym, k, indent));
    }
    for (key, val) in &tbl.hash {
        parts.push(format!("{}: {}", key, pp(val, sym, k, indent)));
    }
    if parts.is_empty() {
        "{}".to_string()
    } else {
        format!("{{{}}}", parts.join(", "))
    }
}

/// Pretty-print a closure value (for image serialization)
pub fn pp_closure(c: &crate::value::MoofClosure, sym: &SymbolTable, k: &KnownSymbols, indent: usize) -> String {
    match &c.body {
        ClosureBody::Expr(body) => {
            let mut param_names: Vec<String> = c.params.iter()
                .map(|&id| sym.name(id).to_string())
                .collect();
            let rest = c.rest_param.map(|id| sym.name(id).to_string());

            let mut params_str = param_names.join(" ");
            if let Some(rest_name) = &rest {
                if params_str.is_empty() {
                    params_str = format!(". {rest_name}");
                } else {
                    params_str = format!("{params_str} . {rest_name}");
                }
            }

            let body_str = pp(body, sym, k, indent + 2);

            if let Some(name_id) = c.name {
                let name = sym.name(name_id);
                // Named function
                if body_str.len() + params_str.len() + name.len() < 60 {
                    format!("(define ({name} {params_str}) {body_str})")
                } else {
                    let pad = " ".repeat(indent + 2);
                    format!("(define ({name} {params_str})\n{pad}{body_str})")
                }
            } else {
                // Anonymous lambda
                if body_str.len() + params_str.len() < 50 {
                    format!("{{ |{params_str}| {body_str} }}")
                } else {
                    format!("(fn ({params_str}) {body_str})")
                }
            }
        }
        ClosureBody::Native(_) => "<native>".to_string(),
        ClosureBody::Bytecode(_) => "<bytecode>".to_string(),
    }
}

/// Pretty-print a method body for image serialization.
/// Takes the selector name, param SymIds (WITHOUT self), rest_param, and body Value.
pub fn pp_method(
    selector: &str,
    params: &[SymId],
    rest_param: Option<SymId>,
    body: &Value,
    sym: &SymbolTable,
    k: &KnownSymbols,
    indent: usize,
) -> String {
    // Skip "self" if it's the first param (methods always have self prepended)
    let param_names: Vec<String> = params.iter()
        .filter(|&&id| sym.name(id) != "self")
        .map(|&id| sym.name(id).to_string())
        .collect();

    let mut params_str = param_names.join(" ");
    if let Some(rest_id) = rest_param {
        let rest_name = sym.name(rest_id);
        if params_str.is_empty() {
            params_str = format!(". {rest_name}");
        } else {
            params_str = format!("{params_str} . {rest_name}");
        }
    }

    let body_str = pp(body, sym, k, indent + 4);
    let pad = " ".repeat(indent);

    // Handle multi-keyword selectors for display
    if body_str.len() + selector.len() + params_str.len() < 50 {
        format!("{pad}(method {selector} ({params_str}) {body_str})")
    } else {
        let inner_pad = " ".repeat(indent + 4);
        format!("{pad}(method {selector} ({params_str})\n{inner_pad}{body_str})")
    }
}
