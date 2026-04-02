use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use crate::error::Result;
use crate::interpreter::Interpreter;
use crate::value::Value;

const HTML: &str = include_str!("../web/ide.html");

/// Start the web IDE server on the given port.
pub fn serve(interp: &mut Interpreter, port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .map_err(|e| crate::error::MoofError::io(format!("Cannot bind to {addr}: {e}")))?;

    eprintln!("\x1b[32mMoof IDE running at http://{addr}\x1b[0m");
    eprintln!("\x1b[2mPress Ctrl+C to stop\x1b[0m");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).ok();

                // Read headers
                let mut content_length: usize = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).ok();
                    if header.trim().is_empty() { break; }
                    if header.to_lowercase().starts_with("content-length:") {
                        content_length = header.split(':').nth(1)
                            .unwrap_or("0").trim().parse().unwrap_or(0);
                    }
                }

                // Read body
                let mut body = vec![0u8; content_length];
                if content_length > 0 {
                    reader.read_exact(&mut body).ok();
                }
                let body_str = String::from_utf8_lossy(&body).to_string();

                // Route
                let (status, content_type, response_body) = if request_line.starts_with("GET / ") {
                    ("200 OK", "text/html", HTML.to_string())
                } else if request_line.starts_with("POST /api/") {
                    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
                    let result = handle_api(interp, path, &body_str);
                    ("200 OK", "application/json", result)
                } else {
                    ("404 Not Found", "text/plain", "Not found".to_string())
                };

                let response = format!(
                    "HTTP/1.1 {status}\r\n\
                     Content-Type: {content_type}\r\n\
                     Content-Length: {}\r\n\
                     Access-Control-Allow-Origin: *\r\n\
                     Access-Control-Allow-Headers: Content-Type\r\n\
                     Connection: close\r\n\
                     \r\n\
                     {response_body}",
                    response_body.len()
                );
                stream.write_all(response.as_bytes()).ok();
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

fn handle_api(interp: &mut Interpreter, path: &str, body: &str) -> String {
    let result = match path {
        "/api/eval" => api_eval(interp, body),
        "/api/classes" => api_classes(interp),
        "/api/class-detail" => api_class_detail(interp, body),
        "/api/method-source" => api_method_source(interp, body),
        "/api/env" => api_env(interp),
        _ => r#"{"status":"error","message":"Unknown API endpoint"}"#.to_string(),
    };
    result
}

fn api_eval(interp: &mut Interpreter, source: &str) -> String {
    match interp.load_source(source, "<web>") {
        Ok(val) => {
            let display = interp.display_value(&val);
            let type_name = repl_type_name_str(interp, &val);
            format!(r#"{{"status":"ok","display":"{}","type":"{}"}}"#,
                json_escape(&display), json_escape(&type_name))
        }
        Err(e) => {
            format!(r#"{{"status":"error","message":"{}"}}"#, json_escape(&e.message))
        }
    }
}

fn api_classes(interp: &mut Interpreter) -> String {
    let mut classes = Vec::new();
    for val in interp.class_objects.values() {
        if let Ok(cls) = interp.real_class_from_class_object(val) {
            let name = interp.symbols.name(cls.borrow().name).to_string();
            if name.contains("meta") || name.starts_with('<') { continue; }
            let sup = cls.borrow().superclass.as_ref()
                .map(|s| interp.symbols.name(s.borrow().name).to_string())
                .unwrap_or_default();
            let method_count = cls.borrow().methods.len();
            let field_count = cls.borrow().field_names.len();
            classes.push(format!(
                r#"{{"name":"{}","superclass":"{}","methods":{},"fields":{}}}"#,
                json_escape(&name), json_escape(&sup), method_count, field_count
            ));
        }
    }
    classes.sort();
    classes.dedup();
    format!(r#"{{"status":"ok","classes":[{}]}}"#, classes.join(","))
}

fn api_class_detail(interp: &mut Interpreter, class_name: &str) -> String {
    let class_name = class_name.trim().trim_matches('"');
    let name_id = interp.symbols.intern(class_name);
    let cls = match interp.find_class_by_name(name_id, &interp.global_env.clone()) {
        Some(c) => c,
        None => return format!(r#"{{"status":"error","message":"Class not found: {}"}}"#, class_name),
    };

    let cls_ref = cls.borrow();
    let sup = cls_ref.superclass.as_ref()
        .map(|s| interp.symbols.name(s.borrow().name).to_string())
        .unwrap_or_default();

    let fields: Vec<String> = cls_ref.all_field_names().iter()
        .map(|&id| format!(r#""{}""#, json_escape(interp.symbols.name(id))))
        .collect();

    let mut methods = Vec::new();
    for (&sel_id, method_val) in &cls_ref.methods {
        let sel_name = interp.symbols.name(sel_id).to_string();
        let (params, is_native) = if let Value::Closure(c) = method_val {
            let p: Vec<String> = c.params.iter()
                .map(|&id| interp.symbols.name(id).to_string())
                .filter(|n| n != "self")
                .collect();
            let native = matches!(c.body, crate::value::ClosureBody::Native(_));
            (p, native)
        } else {
            (vec![], true)
        };
        let params_json: Vec<String> = params.iter().map(|p| format!(r#""{}""#, json_escape(p))).collect();
        methods.push(format!(
            r#"{{"name":"{}","params":[{}],"native":{}}}"#,
            json_escape(&sel_name), params_json.join(","), is_native
        ));
    }
    methods.sort();

    // Hierarchy
    let mut hierarchy = Vec::new();
    let mut current = Some(cls.clone());
    while let Some(c) = current {
        hierarchy.push(format!(r#""{}""#, json_escape(interp.symbols.name(c.borrow().name))));
        let next = c.borrow().superclass.clone();
        current = next;
    }

    format!(
        r#"{{"status":"ok","name":"{}","superclass":"{}","fields":[{}],"methods":[{}],"hierarchy":[{}]}}"#,
        json_escape(class_name), json_escape(&sup),
        fields.join(","), methods.join(","), hierarchy.join(",")
    )
}

fn api_method_source(interp: &mut Interpreter, body: &str) -> String {
    // body format: "ClassName.methodName"
    let body = body.trim().trim_matches('"');
    let parts: Vec<&str> = body.splitn(2, '.').collect();
    if parts.len() != 2 {
        return r#"{"status":"error","message":"Expected ClassName.methodName"}"#.to_string();
    }
    let class_name = parts[0];
    let method_name = parts[1];

    let name_id = interp.symbols.intern(class_name);
    let cls = match interp.find_class_by_name(name_id, &interp.global_env.clone()) {
        Some(c) => c,
        None => return format!(r#"{{"status":"error","message":"Class not found"}}"#),
    };

    let sel_id = interp.symbols.intern(method_name);
    let cls_ref = cls.borrow();
    if let Some(method) = cls_ref.lookup(sel_id) {
        if let Value::Closure(ref c) = method {
            match &c.body {
                crate::value::ClosureBody::Expr(body) => {
                    let source = crate::pretty::pp_method(
                        method_name, &c.params, c.rest_param, body,
                        &interp.symbols, &interp.known, 0,
                    );
                    format!(r#"{{"status":"ok","source":"{}"}}"#, json_escape(&source))
                }
                _ => format!(r#"{{"status":"ok","source":"<native>"}}"#),
            }
        } else {
            format!(r#"{{"status":"error","message":"Not a method"}}"#)
        }
    } else {
        format!(r#"{{"status":"error","message":"Method not found"}}"#)
    }
}

fn api_env(interp: &mut Interpreter) -> String {
    let globals: Vec<String> = interp.global_env.bindings().iter()
        .filter_map(|(id, _)| {
            let name = interp.symbols.name(*id).to_string();
            if name.starts_with("__") || name.starts_with('<') { None }
            else { Some(format!(r#""{}""#, json_escape(&name))) }
        })
        .collect();
    let macros: Vec<String> = interp.macro_registry.keys()
        .map(|&id| format!(r#""{}""#, json_escape(interp.symbols.name(id))))
        .collect();
    format!(
        r#"{{"status":"ok","globals":[{}],"macros":[{}],"classCount":{}}}"#,
        globals.join(","), macros.join(","), interp.class_objects.len()
    )
}

fn repl_type_name_str(interp: &mut Interpreter, val: &Value) -> String {
    let class_sel = interp.symbols.intern("class");
    match interp.send_message(val.clone(), class_sel, vec![]) {
        Ok(class_obj) => {
            let name_sel = interp.symbols.intern("name");
            match interp.send_message(class_obj, name_sel, vec![]) {
                Ok(Value::Str(s)) => s.to_string(),
                _ => val.type_name().to_string(),
            }
        }
        Err(_) => val.type_name().to_string(),
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
}

use std::io::Read;
