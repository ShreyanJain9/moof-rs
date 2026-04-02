use std::cell::RefCell;
use std::rc::Rc;

use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::moofint::MoofInt;
use crate::primitives;
use crate::value::{
    ClosureBody, MoofClass, MoofClosure, MoofObject, MoofRange, NativeFn, Value,
};

// ═══════════════════════════════════════════════════════════════════════
// Install everything that MUST be in Rust
// ═══════════════════════════════════════════════════════════════════════

pub fn install(interp: &mut Interpreter) {
    install_globals(interp);
    install_class_class_methods(interp);
    install_object_introspection(interp);
    install_closure_invoke(interp);
    install_error_methods(interp);
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
        upvalues: Vec::new(),
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
        upvalues: Vec::new(),
    }));
    class.borrow_mut().add_method(id, closure);
}

// ═══════════════════════════════════════════════════════════════════════
// Class-level methods (new)
// ═══════════════════════════════════════════════════════════════════════

fn class_from_class_object(interp: &mut Interpreter, class_obj: &Value) -> Result<Rc<RefCell<MoofClass>>> {
    if let Value::Object(obj_rc) = class_obj {
        let obj = obj_rc.borrow();
        let metaclass = obj.class.borrow();
        let class_name = metaclass.name;
        drop(metaclass);
        drop(obj);

        let meta_str = interp.symbols.name(class_name).to_string();
        let real_name = meta_str.strip_suffix(" meta").unwrap_or(&meta_str);
        let real_id = interp.symbols.intern(real_name);

        if let Some(real_class) = interp.find_class_by_name(real_id, &interp.global_env.clone()) {
            return Ok(real_class);
        }
        Err(MoofError::runtime(format!("new: class '{}' not found", real_name)))
    } else {
        Err(MoofError::runtime("new: receiver is not a class"))
    }
}

fn install_class_class_methods(interp: &mut Interpreter) {
    let class = interp.class_class.clone();

    register_method(interp, &class, "new", |interp, args| {
        let class_obj = &args[0];
        let real_class = class_from_class_object(interp, class_obj)?;
        let all_fields = real_class.borrow().all_field_names();
        let fields = vec![Value::Nil; all_fields.len()];
        let instance = Value::Object(Rc::new(RefCell::new(MoofObject {
            class: real_class,
            fields,
        })));
        let init_args: Vec<Value> = args[1..].to_vec();
        let init_sel = interp.symbols.intern("initialize");
        let _ = interp.send_message(instance.clone(), init_sel, init_args);
        Ok(instance)
    });

    // name — return the class name as a string
    register_method(interp, &class, "name", |interp, args| {
        let real_class = interp.real_class_from_class_object(&args[0])?;
        let name = interp.symbols.name(real_class.borrow().name).to_string();
        Ok(Value::Str(Rc::from(name.as_str())))
    });

    // to_s / inspect — class prints as its name
    register_method(interp, &class, "to_s", |interp, args| {
        let real_class = interp.real_class_from_class_object(&args[0])?;
        let name = interp.symbols.name(real_class.borrow().name).to_string();
        Ok(Value::Str(Rc::from(name.as_str())))
    });
    register_method(interp, &class, "inspect", |interp, args| {
        let real_class = interp.real_class_from_class_object(&args[0])?;
        let name = interp.symbols.name(real_class.borrow().name).to_string();
        Ok(Value::Str(Rc::from(name.as_str())))
    });

    // class — the class of a class is Class
    register_method(interp, &class, "class", |interp, _args| {
        let class_sym = interp.symbols.intern("Class");
        Ok(interp.class_objects.get(&class_sym).cloned().unwrap_or(Value::Nil))
    });

    // superclass — return the superclass object (or nil)
    register_method(interp, &class, "superclass", |interp, args| {
        let real_class = interp.real_class_from_class_object(&args[0])?;
        let sup = real_class.borrow().superclass.clone();
        match sup {
            Some(super_rc) => {
                let name_id = super_rc.borrow().name;
                Ok(interp.class_objects.get(&name_id).cloned().unwrap_or(Value::Nil))
            }
            None => Ok(Value::Nil),
        }
    });

    // methods — return a list of method selector names
    register_method(interp, &class, "methods", |interp, args| {
        let real_class = interp.real_class_from_class_object(&args[0])?;
        let methods: Vec<Value> = real_class.borrow().methods.keys()
            .map(|&sel_id| Value::Str(Rc::from(interp.symbols.name(sel_id))))
            .collect();
        Ok(Value::from_slice(&methods))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Global builtins (variadic, Lisp-facing surface)
// ═══════════════════════════════════════════════════════════════════════

fn install_globals(interp: &mut Interpreter) {
    // Arithmetic (variadic)
    register(interp, "+", |_interp, args| {
        let mut result = Value::Integer(MoofInt::from(0i64));
        for arg in &args { result = primitives::numeric_add(&result, arg)?; }
        Ok(result)
    });
    register(interp, "-", |_interp, args| {
        if args.is_empty() { return Err(MoofError::arity("1+", 0, Some("-"))); }
        if args.len() == 1 {
            return match &args[0] {
                Value::Integer(n) => Ok(Value::Integer(-n)),
                Value::Float(n) => Ok(Value::Float(-n)),
                _ => Err(MoofError::type_error("- expects numbers")),
            };
        }
        let mut result = args[0].clone();
        for arg in &args[1..] { result = primitives::numeric_sub(&result, arg)?; }
        Ok(result)
    });
    register(interp, "*", |_interp, args| {
        let mut result = Value::Integer(MoofInt::from(1i64));
        for arg in &args { result = primitives::numeric_mul(&result, arg)?; }
        Ok(result)
    });
    register(interp, "/", |_interp, args| {
        if args.len() < 2 { return Err(MoofError::arity("2+", args.len(), Some("/"))); }
        let mut result = args[0].clone();
        for arg in &args[1..] { result = primitives::numeric_div(&result, arg)?; }
        Ok(result)
    });
    register(interp, "%", |_interp, args| {
        check_arity("%", 2, &args)?;
        primitives::numeric_rem(&args[0], &args[1])
    });

    // Comparison
    register(interp, ">", |_interp, args| {
        check_arity(">", 2, &args)?;
        Ok(Value::Bool(primitives::numeric_gt(&args[0], &args[1])?))
    });
    register(interp, "<", |_interp, args| {
        check_arity("<", 2, &args)?;
        Ok(Value::Bool(primitives::numeric_lt(&args[0], &args[1])?))
    });
    register(interp, ">=", |_interp, args| {
        check_arity(">=", 2, &args)?;
        Ok(Value::Bool(primitives::numeric_gte(&args[0], &args[1])?))
    });
    register(interp, "<=", |_interp, args| {
        check_arity("<=", 2, &args)?;
        Ok(Value::Bool(primitives::numeric_lte(&args[0], &args[1])?))
    });

    // Equality
    register(interp, "=", |_interp, args| {
        check_arity("=", 2, &args)?;
        Ok(Value::Bool(args[0] == args[1]))
    });
    register(interp, "eq?", |_interp, args| {
        check_arity("eq?", 2, &args)?;
        let same = match (&args[0], &args[1]) {
            (Value::Cons(a), Value::Cons(b)) => Rc::ptr_eq(a, b),
            (Value::Table(a), Value::Table(b)) => Rc::ptr_eq(a, b),
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            _ => args[0] == args[1],
        };
        Ok(Value::Bool(same))
    });

    // List / cons ops
    register(interp, "cons", |_interp, args| {
        check_arity("cons", 2, &args)?;
        Ok(Value::cons(args[0].clone(), args[1].clone()))
    });
    register(interp, "car", |_interp, args| {
        check_arity("car", 1, &args)?;
        args[0].car().cloned()
    });
    register(interp, "cdr", |_interp, args| {
        check_arity("cdr", 1, &args)?;
        args[0].cdr().cloned()
    });
    register(interp, "list", |_interp, args| {
        Ok(Value::from_slice(&args))
    });

    // I/O
    register(interp, "print", |interp, args| {
        let output: Vec<String> = args.iter().map(|a| interp.display_value(a)).collect();
        println!("{}", output.join(" "));
        Ok(Value::Nil)
    });
    register(interp, "display", |interp, args| {
        use std::io::Write;
        let output: Vec<String> = args.iter().map(|a| interp.display_value(a)).collect();
        print!("{}", output.join(" "));
        std::io::stdout().flush().ok();
        Ok(Value::Nil)
    });
    register(interp, "format", |_interp, args| {
        if args.is_empty() { return Err(MoofError::arity("1+", 0, Some("format"))); }
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
    });
    register(interp, "read-line", |_interp, args| {
        use std::io::{self, BufRead, Write};
        if args.len() == 1 {
            print!("{}", args[0]);
            io::stdout().flush().ok();
        }
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line).ok();
        if line.ends_with('\n') { line.pop(); }
        if line.ends_with('\r') { line.pop(); }
        Ok(Value::Str(Rc::from(line.as_str())))
    });

    // File I/O
    register(interp, "read-file", |_interp, args| {
        check_arity("read-file", 1, &args)?;
        let path = args[0].as_str()?;
        let content = std::fs::read_to_string(path)
            .map_err(|e| MoofError::io(format!("read-file: {e}")))?;
        Ok(Value::Str(Rc::from(content.as_str())))
    });
    register(interp, "write-file", |_interp, args| {
        check_arity("write-file", 2, &args)?;
        let path = args[0].as_str()?;
        let content = args[1].as_str()?;
        std::fs::write(path, content)
            .map_err(|e| MoofError::io(format!("write-file: {e}")))?;
        Ok(Value::Nil)
    });
    register(interp, "file-exists?", |_interp, args| {
        check_arity("file-exists?", 1, &args)?;
        let path = args[0].as_str()?;
        Ok(Value::Bool(std::path::Path::new(path).exists()))
    });
    register(interp, "read-lines", |_interp, args| {
        check_arity("read-lines", 1, &args)?;
        let path = args[0].as_str()?;
        let content = std::fs::read_to_string(path)
            .map_err(|e| MoofError::io(format!("read-lines: {e}")))?;
        let lines: Vec<Value> = content.lines().map(|l| Value::Str(Rc::from(l))).collect();
        Ok(Value::from_slice(&lines))
    });

    // Apply
    register(interp, "apply", |interp, args| {
        check_arity("apply", 2, &args)?;
        let func = &args[0];
        let func_args = args[1].to_vec()?;
        interp.invoke(func.clone(), func_args)
    });

    // Control
    register(interp, "error", |interp, args| {
        check_arity("error", 1, &args)?;
        let msg = format!("{}", args[0]);
        let cls = interp.runtime_error_class.clone();
        let obj = interp.make_error_object(&cls, &msg);
        Err(MoofError::runtime(&msg).with_object(obj))
    });
    register(interp, "exit", |_interp, args| {
        let code = if args.len() == 1 {
            match &args[0] {
                Value::Integer(n) => n.to_i64().unwrap_or(0) as i32,
                _ => 0,
            }
        } else { 0 };
        std::process::exit(code);
    });
    register(interp, "not", |_interp, args| {
        check_arity("not", 1, &args)?;
        Ok(Value::Bool(!args[0].is_truthy()))
    });

    // Image save
    register(interp, "save-image", |interp, args| {
        let path = if args.is_empty() {
            "image.moof".to_string()
        } else {
            args[0].as_str()?.to_string()
        };
        crate::image::save_image(interp, &path)?;
        Ok(Value::Str(Rc::from(path.as_str())))
    });

    // Introspection
    register(interp, "type-of", |interp, args| {
        check_arity("type-of", 1, &args)?;
        Ok(interp.class_object_of(&args[0]))
    });
    register(interp, "range", |_interp, args| {
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
    });

    // Timing
    register(interp, "time", |interp, args| {
        check_arity("time", 1, &args)?;
        let t0 = std::time::Instant::now();
        let result = interp.invoke(args[0].clone(), vec![])?;
        let elapsed = t0.elapsed();
        println!("Elapsed: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
        Ok(result)
    });

    // Internal helpers
    register(interp, "__curry_call", |interp, args| {
        check_arity("__curry_call", 3, &args)?;
        let func = &args[0];
        let mut all_args = args[1].to_vec()?;
        let rest = args[2].to_vec()?;
        all_args.extend(rest);
        interp.invoke(func.clone(), all_args)
    });
    register(interp, "__vm_try", |interp, args| {
        check_arity("__vm_try", 2, &args)?;
        let body = &args[0];
        let catch_handler = &args[1];
        match interp.invoke(body.clone(), vec![]) {
            Ok(val) => Ok(val),
            Err(e) => {
                let error_val = if let Some(obj) = e.error_object {
                    obj
                } else {
                    Value::Str(Rc::from(e.message.as_str()))
                };
                interp.invoke(catch_handler.clone(), vec![error_val])
            }
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Object introspection methods (need interpreter access)
// ═══════════════════════════════════════════════════════════════════════

fn install_object_introspection(interp: &mut Interpreter) {
    let class = interp.object_class.clone();

    register_method(interp, &class, "class", |interp, args| {
        Ok(interp.class_object_of(&args[0]))
    });

    register_method(interp, &class, "is_a:", |interp, args| {
        // Accept class objects, strings, or symbols
        let target_name_id = match &args[1] {
            Value::Object(_) => {
                // It's a class object — use real_class_from_class_object
                match interp.real_class_from_class_object(&args[1]) {
                    Ok(real_class) => real_class.borrow().name,
                    Err(_) => return Ok(Value::Bool(false)),
                }
            }
            Value::Str(s) => interp.symbols.intern(s),
            Value::Symbol(id) => *id,
            _ => return Err(MoofError::type_error("is_a: expects a class or class name")),
        };
        // Walk superclass chain comparing SymIds
        let mut current = interp.class_of(&args[0]);
        loop {
            if current.borrow().name == target_name_id {
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
            _ => return Err(MoofError::type_error("responds_to: expects a selector name")),
        };
        let selector_id = interp.symbols.intern(&selector_name);
        let cls = interp.class_of(&args[0]);
        Ok(Value::Bool(cls.borrow().lookup(selector_id).is_some()))
    });

    register_method(interp, &class, "send:", |interp, args| {
        let receiver = args[0].clone();
        let selector_name = match &args[1] {
            Value::Str(s) => s.to_string(),
            Value::Symbol(id) => interp.symbols.name(*id).to_string(),
            _ => return Err(MoofError::type_error("send: expects a selector name")),
        };
        let selector_id = interp.symbols.intern(&selector_name);
        let msg_args = if args.len() > 2 { args[2..].to_vec() } else { vec![] };
        interp.send_message(receiver, selector_id, msg_args)
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Closure invoke methods (need interp.invoke)
// ═══════════════════════════════════════════════════════════════════════

fn install_closure_invoke(interp: &mut Interpreter) {
    let class = interp.closure_class.clone();

    register_method(interp, &class, "value", |interp, args| {
        interp.invoke(args[0].clone(), vec![])
    });

    register_method(interp, &class, "value:", |interp, args| {
        interp.invoke(args[0].clone(), vec![args[1].clone()])
    });

    register_method(interp, &class, "value:value:", |interp, args| {
        interp.invoke(args[0].clone(), vec![args[1].clone(), args[2].clone()])
    });

    register_method(interp, &class, "call:", |interp, args| {
        let func = &args[0];
        let call_args = args[1].to_vec()?;
        interp.invoke(func.clone(), call_args)
    });

    // curry: — partial application
    register_method(interp, &class, "curry:", |interp, args| {
        let func = args[0].clone();
        let partial_args: Vec<Value> = args[1..].to_vec();

        let fn_sym = interp.symbols.intern("__curry_fn");
        let partial_sym = interp.symbols.intern("__curry_partial");
        let rest_sym = interp.symbols.intern("__curry_rest");
        let curry_call_sym = interp.symbols.intern("__curry_call");

        let curry_env = interp.global_env.child();
        curry_env.define(fn_sym, func, false);
        curry_env.define(partial_sym, Value::from_slice(&partial_args), false);

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
            upvalues: Vec::new(),
        })))
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Error methods (need object field access)
// ═══════════════════════════════════════════════════════════════════════

fn install_error_methods(interp: &mut Interpreter) {
    let class = interp.error_class.clone();

    register_method(interp, &class, "message", |_interp, args| {
        if let Value::Object(ref obj) = args[0] {
            let obj = obj.borrow();
            if let Some(msg) = obj.fields.get(0) {
                return Ok(msg.clone());
            }
        }
        Ok(Value::Str(Rc::from("")))
    });

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
// Helpers
// ═══════════════════════════════════════════════════════════════════════

fn check_arity(name: &str, expected: usize, args: &[Value]) -> Result<()> {
    if args.len() != expected {
        Err(MoofError::arity(&expected.to_string(), args.len(), Some(name)))
    } else {
        Ok(())
    }
}
