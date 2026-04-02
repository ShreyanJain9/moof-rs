use std::io::Write;

use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::pretty;
use crate::value::{ClosureBody, Value};

/// Save the current interpreter state as a .moof image file.
/// Only saves user-defined content (not stdlib baseline).
pub fn save_image(interp: &Interpreter, path: &str) -> Result<()> {
    let mut out = String::new();

    out.push_str(";; Moof Image\n");
    out.push_str(&format!(";; Saved: {}\n", chrono_lite()));
    out.push_str("\n");

    // ── User-defined classes ──────────────────────────────────────
    let user_classes = collect_user_classes(interp);
    if !user_classes.is_empty() {
        out.push_str(";; ── Classes ──────────────────────────────────────────────────\n\n");
        for (name, class_str) in &user_classes {
            let _ = name;
            out.push_str(class_str);
            out.push_str("\n\n");
        }
    }

    // ── Methods added to existing (bootstrap) classes ─────────────
    let added_methods = collect_added_methods(interp);
    if !added_methods.is_empty() {
        out.push_str(";; ── Added Methods ────────────────────────────────────────────\n\n");
        for class_block in &added_methods {
            out.push_str(class_block);
            out.push_str("\n\n");
        }
    }

    // ── ADT types ─────────────────────────────────────────────────
    let user_types = collect_user_types(interp);
    if !user_types.is_empty() {
        out.push_str(";; ── Types ────────────────────────────────────────────────────\n\n");
        for type_str in &user_types {
            out.push_str(type_str);
            out.push('\n');
        }
        out.push('\n');
    }

    // ── Global functions and variables ────────────────────────────
    let user_globals = collect_user_globals(interp);
    if !user_globals.is_empty() {
        out.push_str(";; ── Definitions ──────────────────────────────────────────────\n\n");
        for def_str in &user_globals {
            out.push_str(def_str);
            out.push('\n');
        }
        out.push('\n');
    }

    // ── Macros ────────────────────────────────────────────────────
    let user_macros = collect_user_macros(interp);
    if !user_macros.is_empty() {
        out.push_str(";; ── Macros ───────────────────────────────────────────────────\n\n");
        for macro_str in &user_macros {
            out.push_str(macro_str);
            out.push('\n');
        }
        out.push('\n');
    }

    // ── Protocols ─────────────────────────────────────────────────
    let user_protocols = collect_user_protocols(interp);
    if !user_protocols.is_empty() {
        out.push_str(";; ── Protocols ────────────────────────────────────────────────\n\n");
        for proto_str in &user_protocols {
            out.push_str(proto_str);
            out.push('\n');
        }
        out.push('\n');
    }

    // Write to file
    let mut file = std::fs::File::create(path)
        .map_err(|e| MoofError::io(format!("save: cannot create '{}': {}", path, e)))?;
    file.write_all(out.as_bytes())
        .map_err(|e| MoofError::io(format!("save: write error: {}", e)))?;

    Ok(())
}

/// Collect user-defined classes (not in bootstrap).
fn collect_user_classes(interp: &Interpreter) -> Vec<(String, String)> {
    let bootstrap_names: std::collections::HashSet<_> = interp.all_type_classes().iter()
        .map(|c| c.borrow().name)
        .collect();

    let mut result = Vec::new();

    for (&name_id, _) in &interp.class_objects {
        if bootstrap_names.contains(&name_id) {
            continue;
        }
        // Skip internal names
        let name = interp.symbols.name(name_id);
        if name.starts_with("__") || name.starts_with('<') || name.contains(" meta") {
            continue;
        }

        if let Some(class_rc) = interp.find_class_by_name(name_id, &interp.global_env.clone()) {
            let class = class_rc.borrow();
            let mut class_str = format!("(class {}", name);

            // Superclass (if not Object)
            if let Some(ref sup) = class.superclass {
                let sup_name = interp.symbols.name(sup.borrow().name);
                if sup_name != "Object" {
                    class_str.push_str(&format!("\n  (extends {})", sup_name));
                }
            }

            // Fields
            if !class.field_names.is_empty() {
                let fields: Vec<&str> = class.field_names.iter()
                    .map(|&id| interp.symbols.name(id))
                    .collect();
                class_str.push_str(&format!("\n  (fields {})", fields.join(" ")));
            }

            // Methods
            for (&sel_id, method_val) in &class.methods {
                if let Value::Closure(c) = method_val {
                    if let ClosureBody::Expr(ref body) = c.body {
                        let sel_name = interp.symbols.name(sel_id);
                        let method_str = pretty::pp_method(
                            sel_name,
                            &c.params,
                            c.rest_param,
                            body,
                            &interp.symbols,
                            &interp.known,
                            2,
                        );
                        class_str.push_str(&format!("\n{method_str}"));
                    }
                }
            }

            class_str.push(')');
            result.push((name.to_string(), class_str));
        }
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Collect methods added to bootstrap classes after baseline.
fn collect_added_methods(interp: &Interpreter) -> Vec<String> {
    let mut result = Vec::new();

    for class_rc in interp.all_type_classes() {
        let class = class_rc.borrow();
        let class_name_id = class.name;
        let class_name = interp.symbols.name(class_name_id);

        // Skip metaclasses
        if class_name.contains("meta") {
            continue;
        }

        let mut added = Vec::new();
        for (&sel_id, method_val) in &class.methods {
            if interp.baseline_methods.contains(&(class_name_id, sel_id)) {
                continue;
            }
            if let Value::Closure(c) = method_val {
                if let ClosureBody::Expr(ref body) = c.body {
                    let sel_name = interp.symbols.name(sel_id);
                    let method_str = pretty::pp_method(
                        sel_name,
                        &c.params,
                        c.rest_param,
                        body,
                        &interp.symbols,
                        &interp.known,
                        2,
                    );
                    added.push(method_str);
                }
            }
        }

        if !added.is_empty() {
            let mut block = format!("(class {class_name}");
            for m in &added {
                block.push_str(&format!("\n{m}"));
            }
            block.push(')');
            result.push(block);
        }
    }

    result
}

/// Collect user-defined ADT types.
fn collect_user_types(interp: &Interpreter) -> Vec<String> {
    let mut result = Vec::new();
    for (&name_id, variants) in &interp.type_registry {
        if interp.baseline_types.contains(&name_id) {
            continue;
        }
        let name = interp.symbols.name(name_id);
        let variant_strs: Vec<String> = variants.iter().map(|&vid| {
            let vname = interp.symbols.name(vid);
            // Check if variant has fields by looking up its class
            if let Some(class_rc) = interp.find_class_by_name(vid, &interp.global_env.clone()) {
                let class = class_rc.borrow();
                if class.field_names.is_empty() {
                    vname.to_string()
                } else {
                    let fields: Vec<&str> = class.field_names.iter()
                        .map(|&fid| interp.symbols.name(fid))
                        .collect();
                    format!("({} {})", vname, fields.join(" "))
                }
            } else {
                vname.to_string()
            }
        }).collect();
        result.push(format!("(type {} {})", name, variant_strs.join(" ")));
    }
    result
}

/// Collect user-defined global functions and variables.
fn collect_user_globals(interp: &Interpreter) -> Vec<String> {
    let mut result = Vec::new();

    for (id, val) in interp.global_env.bindings() {
        if interp.baseline_globals.contains(&id) {
            continue;
        }
        let name = interp.symbols.name(id);

        // Skip internal names, class objects, __class: entries
        if name.starts_with("__") || name.starts_with('<') {
            continue;
        }
        if interp.class_objects.contains_key(&id) {
            continue;
        }
        // Skip _ (REPL last result)
        if name == "_" {
            continue;
        }

        match val {
            Value::Closure(ref c) => {
                let pp_str = pretty::pp_closure(c, &interp.symbols, &interp.known, 0);
                if pp_str != "<native>" && pp_str != "<bytecode>" {
                    result.push(pp_str);
                }
            }
            _ => {
                // Serialize the value
                let val_str = serialize_value(&val, interp);
                result.push(format!("(define {} {})", name, val_str));
            }
        }
    }

    result.sort();
    result
}

/// Collect user-defined macros.
fn collect_user_macros(interp: &Interpreter) -> Vec<String> {
    let mut result = Vec::new();
    for (&name_id, macro_val) in &interp.macro_registry {
        if interp.baseline_macros.contains(&name_id) {
            continue;
        }
        let name = interp.symbols.name(name_id);
        if let Value::Closure(c) = macro_val {
            if let ClosureBody::Expr(ref body) = c.body {
                let params: Vec<&str> = c.params.iter()
                    .map(|&id| interp.symbols.name(id))
                    .collect();
                let body_str = pretty::pretty_print(body, &interp.symbols, &interp.known);
                result.push(format!("(defmacro {} ({}) {})", name, params.join(" "), body_str));
            }
        }
    }
    result
}

/// Collect user-defined protocols.
fn collect_user_protocols(interp: &Interpreter) -> Vec<String> {
    let mut result = Vec::new();
    for (&name_id, selectors) in &interp.protocol_registry {
        if interp.baseline_protocols.contains(&name_id) {
            continue;
        }
        let name = interp.symbols.name(name_id);
        let sels: Vec<&str> = selectors.iter()
            .map(|&id| interp.symbols.name(id))
            .collect();
        result.push(format!("(protocol {} {})", name, sels.join(" ")));
    }
    result
}

/// Serialize a value as Moof source code.
fn serialize_value(val: &Value, interp: &Interpreter) -> String {
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
        Value::Symbol(id) => format!("'{}", interp.symbols.name(*id)),
        Value::Str(s) => format!("{:?}", &**s),
        Value::Cons(_) => {
            let items: Vec<String> = val.iter_list()
                .map(|v| serialize_value(v, interp))
                .collect();
            format!("(list {})", items.join(" "))
        }
        Value::Table(tbl) => {
            let tbl = tbl.borrow();
            let mut parts = Vec::new();
            for v in &tbl.array {
                parts.push(serialize_value(v, interp));
            }
            for (k, v) in &tbl.hash {
                parts.push(format!("{}: {}", k, serialize_value(v, interp)));
            }
            format!("{{{}}}", parts.join(", "))
        }
        Value::Range(r) => {
            let step = r.step.to_i64().unwrap_or(1);
            if step == 1 {
                format!("(range {} {})", r.start, r.end)
            } else {
                format!("(range {} {} {})", r.start, r.end, r.step)
            }
        }
        Value::Closure(c) => {
            pretty::pp_closure(c, &interp.symbols, &interp.known, 0)
        }
        Value::Object(_) => {
            // Can't easily serialize arbitrary objects
            ";; <unsaved object>".to_string()
        }
    }
}

fn chrono_lite() -> String {
    // Simple timestamp without chrono dependency
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("{secs}")
}
