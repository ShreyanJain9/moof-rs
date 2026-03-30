use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::{Expr, Program, CondTest, MethodDef, Loc};
use crate::environment::Env;
use crate::value::{Value, MoofFunction, MoofClass, MoofObject, MoofProtocol, MoofMacro};
use crate::error::{MoofError, Result};
use crate::builtins;
use crate::dispatcher;
use crate::pattern_matcher;

pub struct Interpreter {
    pub global_env: Env,
    pub class_registry: HashMap<String, Rc<RefCell<MoofClass>>>,
    pub trait_registry: HashMap<String, HashMap<String, MoofFunction>>,
    pub protocol_registry: HashMap<String, MoofProtocol>,
    pub macro_registry: HashMap<String, MoofMacro>,
    pub type_registry: HashMap<String, Vec<String>>,
    pub module_registry: HashMap<String, HashMap<String, Value>>,
    // Current env for eval_expr (save/restore pattern for lexical scoping)
    pub env: Env,
}

impl Interpreter {
    pub fn new() -> Self {
        let global = Env::new();
        let mut interp = Interpreter {
            global_env: global.clone(),
            class_registry: HashMap::new(),
            trait_registry: HashMap::new(),
            protocol_registry: HashMap::new(),
            macro_registry: HashMap::new(),
            type_registry: HashMap::new(),
            module_registry: HashMap::new(),
            env: global,
        };
        interp.register_builtin_classes();
        builtins::install(&mut interp);
        interp
    }

    /// Register built-in types as MoofClass so they can be extended.
    fn register_builtin_classes(&mut self) {
        for name in &["Integer", "Float", "String", "List", "Map", "Bool", "Nil", "Function"] {
            let klass = MoofClass::new(name.to_string());
            let klass_rc = Rc::new(RefCell::new(klass));
            self.class_registry.insert(name.to_string(), klass_rc.clone());
            self.global_env.define(name, Value::Class(klass_rc), false);
        }
    }

    pub fn evaluate(&mut self, program: &Program) -> Result<Value> {
        let mut result = Value::Nil;
        for expr in &program.expressions {
            result = self.evaluate_node(expr, &self.env.clone())?;
        }
        Ok(result)
    }

    pub fn evaluate_node(&mut self, expr: &Expr, env: &Env) -> Result<Value> {
        // Save and restore env around evaluation
        let old_env = self.env.clone();
        self.env = env.clone();
        let result = self.eval_expr_inner(expr);
        self.env = old_env;
        result
    }

    /// The big match dispatcher for expression evaluation.
    fn eval_expr_inner(&mut self, expr: &Expr) -> Result<Value> {
        match expr {
            // -- Literals --
            Expr::Integer(n, _) => Ok(Value::Integer(*n)),
            Expr::Float(n, _) => Ok(Value::Float(*n)),
            Expr::Str(s, _) => Ok(Value::Str(s.clone())),
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::Nil(_) => Ok(Value::Nil),

            Expr::Identifier(name, loc) => {
                self.env.get(name).map_err(|_| {
                    MoofError::name(name, loc.line, loc.column)
                })
            }

            Expr::Quote(inner, _) => self.quote_value(inner),

            Expr::Quasiquote(inner, _) => self.quasiquote_eval(inner, &self.env.clone()),

            Expr::Unquote(_, _) => Err(MoofError::runtime("Unquote (,) outside of quasiquote")),
            Expr::UnquoteSplice(_, _) => Err(MoofError::runtime("Unquote-splice (,@) outside of quasiquote")),

            Expr::MapLiteral(pairs, _) => {
                let mut result = Vec::new();
                for (key, val_expr) in pairs {
                    let val = self.eval_expr(val_expr)?;
                    result.push((key.clone(), val));
                }
                Ok(Value::Map(result))
            }

            Expr::Define(name, value_expr, _) => {
                let value = self.eval_expr(value_expr)?;
                self.env.define(name, value.clone(), false);
                Ok(value)
            }

            Expr::DefineFunction(name, params, rest_param, body, _) => {
                let func = MoofFunction {
                    name: Some(name.clone()),
                    params: params.clone(),
                    rest_param: rest_param.clone(),
                    body: body.clone(),
                    closure: self.env.clone(),
                };
                let val = Value::Function(func);
                self.env.define(name, val.clone(), false);
                Ok(val)
            }

            Expr::Lambda(params, rest_param, body, _) => {
                Ok(Value::Function(MoofFunction {
                    name: None,
                    params: params.clone(),
                    rest_param: rest_param.clone(),
                    body: body.clone(),
                    closure: self.env.clone(),
                }))
            }

            Expr::If(cond, then_expr, else_expr, _) => {
                let cond_val = self.eval_expr(cond)?;
                if cond_val.is_truthy() {
                    self.eval_expr(then_expr)
                } else if let Some(else_e) = else_expr {
                    self.eval_expr(else_e)
                } else {
                    Ok(Value::Nil)
                }
            }

            Expr::Let(bindings, body, _) => {
                let outer = self.env.clone();
                let let_env = self.env.child();
                self.env = let_env;
                for (name, val_expr) in bindings {
                    // Evaluate binding value in the outer env context
                    let old = self.env.clone();
                    self.env = outer.clone();
                    let val = self.eval_expr(val_expr)?;
                    self.env = old;
                    self.env.define(name, val, false);
                }
                let result = self.eval_expr(body);
                self.env = outer;
                result
            }

            Expr::Do(exprs, _) => {
                let mut result = Value::Nil;
                for e in exprs {
                    result = self.eval_expr(e)?;
                }
                Ok(result)
            }

            Expr::SetBang(name, value_expr, _) => {
                let value = self.eval_expr(value_expr)?;
                self.env.set(name, value)
            }

            Expr::TryCatch(body, error_name, catch_body, _) => {
                match self.eval_expr(body) {
                    Ok(val) => Ok(val),
                    Err(e) => {
                        let old_env = self.env.clone();
                        self.env = self.env.child();
                        self.env.define(error_name, Value::Str(e.message.clone()), false);
                        let result = self.eval_expr(catch_body);
                        self.env = old_env;
                        result
                    }
                }
            }

            Expr::Cond(clauses, _) => {
                for (test, body) in clauses {
                    match test {
                        CondTest::Else => return self.eval_expr(body),
                        CondTest::Expr(test_expr) => {
                            let val = self.eval_expr(test_expr)?;
                            if val.is_truthy() {
                                return self.eval_expr(body);
                            }
                        }
                    }
                }
                Ok(Value::Nil)
            }

            Expr::And(left, right, _) => {
                let l = self.eval_expr(left)?;
                if !l.is_truthy() {
                    Ok(Value::Bool(false))
                } else {
                    let r = self.eval_expr(right)?;
                    if r.is_truthy() { Ok(r) } else { Ok(Value::Bool(false)) }
                }
            }

            Expr::Or(left, right, _) => {
                let l = self.eval_expr(left)?;
                if l.is_truthy() {
                    Ok(l)
                } else {
                    self.eval_expr(right)
                }
            }

            Expr::Call(func_expr, arg_exprs, loc) => {
                // Check for macro expansion first
                if let Expr::Identifier(name, _) = func_expr.as_ref() {
                    if self.macro_registry.contains_key(name) {
                        return self.expand_and_eval_macro(name, arg_exprs, &self.env.clone());
                    }
                }

                let callee = self.eval_expr(func_expr)?;
                let args = self.evaluate_call_args(arg_exprs)?;
                self.invoke_callee(callee, args, loc)
            }

            Expr::MessageSend(receiver_expr, selector, arg_exprs, _) => {
                let receiver = self.eval_expr(receiver_expr)?;
                let mut args = Vec::new();
                for a in arg_exprs {
                    args.push(self.eval_expr(a)?);
                }
                dispatcher::send_message(self, receiver, selector, args)
            }

            Expr::KeywordArg(_, value_expr, _) => {
                self.eval_expr(value_expr)
            }

            Expr::Match(scrutinee, clauses, _) => {
                let val = self.eval_expr(scrutinee)?;
                for clause in clauses {
                    let m = pattern_matcher::match_pattern(&clause.pattern, &val, self);
                    if m.success {
                        let old_env = self.env.clone();
                        self.env = self.env.child();
                        for (name, bind_val) in &m.bindings {
                            self.env.define(name, bind_val.clone(), false);
                        }
                        // Check guard
                        if let Some(ref guard) = clause.guard {
                            let guard_val = self.eval_expr(guard)?;
                            if !guard_val.is_truthy() {
                                self.env = old_env;
                                continue;
                            }
                        }
                        let result = self.eval_expr(&clause.body);
                        self.env = old_env;
                        return result;
                    }
                }
                Ok(Value::Nil)
            }

            Expr::TypeDef(name, variants, _) => {
                let mut variant_names = Vec::new();
                for variant in variants {
                    if variant.fields.is_empty() {
                        // Singleton variant
                        let klass = MoofClass::new(variant.name.clone());
                        let klass_rc = Rc::new(RefCell::new(klass));
                        self.class_registry.insert(variant.name.clone(), klass_rc.clone());
                        // Create singleton instance
                        let obj = MoofObject {
                            class: klass_rc,
                            fields: HashMap::new(),
                        };
                        self.env.define(&variant.name, Value::Object(obj), false);
                    } else {
                        // Constructor variant
                        let mut klass = MoofClass::new(variant.name.clone());
                        klass.own_fields = variant.fields.clone();
                        klass.fields = variant.fields.clone();
                        let klass_rc = Rc::new(RefCell::new(klass));
                        self.class_registry.insert(variant.name.clone(), klass_rc.clone());
                        self.env.define(&variant.name, Value::Class(klass_rc), false);
                    }
                    variant_names.push(variant.name.clone());
                }
                self.type_registry.insert(name.clone(), variant_names);
                Ok(Value::Str(name.clone()))
            }

            Expr::ProtocolDef(name, selectors, _) => {
                let proto = MoofProtocol {
                    name: name.clone(),
                    selectors: selectors.clone(),
                };
                self.protocol_registry.insert(name.clone(), proto.clone());
                let val = Value::Protocol(proto);
                self.env.define(name, val.clone(), false);
                Ok(val)
            }

            Expr::DefMacro(name, params, body, _) => {
                let mac = MoofMacro {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                };
                self.macro_registry.insert(name.clone(), mac.clone());
                self.env.define(name, Value::Macro(mac), false);
                Ok(Value::Str(name.clone()))
            }

            Expr::ClassDef { name, superclass, fields, methods, traits, loc } => {
                self.eval_class_def(name, superclass.as_deref(), fields, methods, traits, loc)
            }

            Expr::TraitDef { name, methods, loc: _ } => {
                let mut method_table = HashMap::new();
                for mdef in methods {
                    let func = MoofFunction {
                        name: Some(format!("{}#{}", name, mdef.selector)),
                        params: mdef.params.clone(),
                        rest_param: None,
                        body: mdef.body.clone(),
                        closure: self.env.clone(),
                    };
                    method_table.insert(mdef.selector.clone(), func);
                }
                self.trait_registry.insert(name.clone(), method_table);
                Ok(Value::Nil)
            }

            Expr::ModuleDef(name, exports, body, _) => {
                self.evaluate_module(name, exports, body)
            }

            Expr::UseModule(module_name, imports, alias, _) => {
                self.import_module(module_name, imports.as_deref(), alias.as_deref())
            }

            Expr::Require(path, _) => {
                self.require_file(path)
            }

            Expr::Pipeline(initial, steps, _) => {
                let mut val = self.eval_expr(initial)?;
                for step in steps {
                    val = match step {
                        Expr::Call(f, args, _) => {
                            let func = self.eval_expr(f)?;
                            let mut eval_args = vec![val];
                            for a in args {
                                eval_args.push(self.eval_expr(a)?);
                            }
                            self.invoke_callee(func, eval_args, step.loc())?
                        }
                        Expr::Identifier(name, _) => {
                            let func = self.env.get(name)?;
                            self.invoke_callee(func, vec![val], step.loc())?
                        }
                        Expr::MessageSend(_, selector, arg_exprs, _) => {
                            let mut args = Vec::new();
                            for a in arg_exprs {
                                args.push(self.eval_expr(a)?);
                            }
                            dispatcher::send_message(self, val, selector, args)?
                        }
                        other => {
                            let func = self.eval_expr(other)?;
                            self.invoke_callee(func, vec![val], other.loc())?
                        }
                    };
                }
                Ok(val)
            }

            Expr::StringInterp(segments, _) => {
                let mut result = String::new();
                for seg in segments {
                    let val = self.eval_expr(seg)?;
                    result.push_str(&format!("{}", val));
                }
                Ok(Value::Str(result))
            }

            Expr::SelectorRef(selector, partial_args, _) => {
                let sel = selector.clone();
                let mut pargs = Vec::new();
                for a in partial_args {
                    pargs.push(self.eval_expr(a)?);
                }
                if pargs.is_empty() {
                    let body = Expr::MessageSend(
                        Box::new(Expr::Identifier("__sel_receiver".to_string(), Loc::none())),
                        sel.clone(),
                        vec![],
                        Loc::none(),
                    );
                    Ok(Value::Function(MoofFunction {
                        name: Some(format!(".{}", sel)),
                        params: vec!["__sel_receiver".to_string()],
                        rest_param: None,
                        body: Box::new(body),
                        closure: self.env.clone(),
                    }))
                } else {
                    let closure = self.env.child();
                    let mut arg_exprs = Vec::new();
                    for (i, parg) in pargs.into_iter().enumerate() {
                        let pname = format!("__sel_parg_{}", i);
                        closure.define(&pname, parg, false);
                        arg_exprs.push(Expr::Identifier(pname, Loc::none()));
                    }
                    let body = Expr::MessageSend(
                        Box::new(Expr::Identifier("__sel_receiver".to_string(), Loc::none())),
                        sel.clone(),
                        arg_exprs,
                        Loc::none(),
                    );
                    Ok(Value::Function(MoofFunction {
                        name: Some(format!(".{}", sel)),
                        params: vec!["__sel_receiver".to_string()],
                        rest_param: None,
                        body: Box::new(body),
                        closure,
                    }))
                }
            }
        }
    }

    /// Public eval_expr that operates on self.env. Used internally.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        self.eval_expr_inner(expr)
    }

    // ── Call helpers ─────────────────────────────────────────────

    fn evaluate_call_args(&mut self, arguments: &[Expr]) -> Result<Vec<Value>> {
        let mut args = Vec::new();
        for arg in arguments {
            match arg {
                Expr::KeywordArg(_, value_expr, _) => {
                    args.push(self.eval_expr(value_expr)?);
                }
                other => {
                    args.push(self.eval_expr(other)?);
                }
            }
        }
        Ok(args)
    }

    fn invoke_callee(&mut self, callee: Value, args: Vec<Value>, loc: &Loc) -> Result<Value> {
        match callee {
            Value::Function(ref f) => {
                call_function(self, f, args)
            }
            Value::Builtin(_, f_ptr) => f_ptr(self, args),
            Value::Class(ref klass_rc) => {
                let klass = klass_rc.borrow();
                let all_fields = klass.all_fields();
                if args.len() != all_fields.len() {
                    return Err(MoofError::arity(
                        &all_fields.len().to_string(),
                        args.len(),
                        Some(&klass.name),
                    ));
                }
                let mut fields = HashMap::new();
                for (i, field_name) in all_fields.iter().enumerate() {
                    fields.insert(field_name.clone(), args[i].clone());
                }
                drop(klass);
                Ok(Value::Object(MoofObject {
                    class: klass_rc.clone(),
                    fields,
                }))
            }
            _ => {
                let loc_str = if let Some(line) = loc.line {
                    format!(" at line {}", line)
                } else {
                    String::new()
                };
                Err(MoofError::runtime(format!(
                    "Cannot call {}{}: not a function",
                    callee.type_name(),
                    loc_str
                )))
            }
        }
    }

    pub fn call_value(&mut self, func: &Value, args: Vec<Value>) -> Result<Value> {
        match func {
            Value::Function(f) => call_function(self, f, args),
            Value::Builtin(_, f) => f(self, args),
            _ => Err(MoofError::runtime(format!(
                "Cannot call {}: not a function",
                func.type_name()
            ))),
        }
    }

    pub fn call_function(&mut self, func: &MoofFunction, args: Vec<Value>) -> Result<Value> {
        call_function(self, func, args)
    }

    // ── Tail call evaluation ────────────────────────────────────

    pub fn evaluate_tail(&mut self, expr: &Expr, env: &Env) -> Result<Value> {
        let old_env = self.env.clone();
        self.env = env.clone();
        let result = self.eval_tail_inner(expr);
        self.env = old_env;
        result
    }

    fn eval_tail_inner(&mut self, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::Call(func_expr, arg_exprs, _loc) => {
                if let Expr::Identifier(name, _) = func_expr.as_ref() {
                    if self.macro_registry.contains_key(name) {
                        return self.expand_and_eval_macro(name, arg_exprs, &self.env.clone());
                    }
                }
                let callee = self.eval_expr(func_expr)?;
                let args = self.evaluate_call_args(arg_exprs)?;
                if matches!(callee, Value::Function(_)) {
                    Ok(Value::TailCall(Box::new(callee), args))
                } else {
                    self.invoke_callee(callee, args, expr.loc())
                }
            }

            Expr::If(cond, then_expr, else_expr, _) => {
                let cond_val = self.eval_expr(cond)?;
                if cond_val.is_truthy() {
                    self.eval_tail_inner(then_expr)
                } else if let Some(else_e) = else_expr {
                    self.eval_tail_inner(else_e)
                } else {
                    Ok(Value::Nil)
                }
            }

            Expr::Do(exprs, _) => {
                if exprs.is_empty() { return Ok(Value::Nil); }
                for e in &exprs[..exprs.len() - 1] {
                    self.eval_expr(e)?;
                }
                self.eval_tail_inner(exprs.last().unwrap())
            }

            Expr::Let(bindings, body, _) => {
                let outer = self.env.clone();
                let let_env = self.env.child();
                self.env = let_env;
                for (name, val_expr) in bindings {
                    let old = self.env.clone();
                    self.env = outer.clone();
                    let val = self.eval_expr(val_expr)?;
                    self.env = old;
                    self.env.define(name, val, false);
                }
                let result = self.eval_tail_inner(body);
                self.env = outer;
                result
            }

            Expr::Match(scrutinee, clauses, _) => {
                let val = self.eval_expr(scrutinee)?;
                for clause in clauses {
                    let m = pattern_matcher::match_pattern(&clause.pattern, &val, self);
                    if m.success {
                        let old_env = self.env.clone();
                        self.env = self.env.child();
                        for (name, bind_val) in &m.bindings {
                            self.env.define(name, bind_val.clone(), false);
                        }
                        if let Some(ref guard) = clause.guard {
                            let guard_val = self.eval_expr(guard)?;
                            if !guard_val.is_truthy() {
                                self.env = old_env;
                                continue;
                            }
                        }
                        let result = self.eval_tail_inner(&clause.body);
                        self.env = old_env;
                        return result;
                    }
                }
                Ok(Value::Nil)
            }

            Expr::Cond(clauses, _) => {
                for (test, body) in clauses {
                    match test {
                        CondTest::Else => return self.eval_tail_inner(body),
                        CondTest::Expr(test_expr) => {
                            let val = self.eval_expr(test_expr)?;
                            if val.is_truthy() {
                                return self.eval_tail_inner(body);
                            }
                        }
                    }
                }
                Ok(Value::Nil)
            }

            other => self.eval_expr_inner(other),
        }
    }

    // ── Macro expansion ─────────────────────────────────────────

    fn expand_and_eval_macro(
        &mut self,
        name: &str,
        call_args: &[Expr],
        env: &Env,
    ) -> Result<Value> {
        let mac = self.macro_registry.get(name).cloned()
            .ok_or_else(|| MoofError::runtime(format!("Unknown macro: {}", name)))?;

        let macro_env = env.child();
        for (i, param) in mac.params.iter().enumerate() {
            let arg_val = if let Some(arg_expr) = call_args.get(i) {
                self.quote_value(arg_expr)?
            } else {
                Value::Nil
            };
            macro_env.define(param, arg_val, false);
        }

        let old_env = self.env.clone();
        self.env = macro_env;
        let expanded = self.eval_expr(&mac.body)?;
        self.env = old_env;

        let ast = self.data_to_ast(&expanded);
        self.evaluate_node(&ast, env)
    }

    fn data_to_ast(&self, data: &Value) -> Expr {
        match data {
            Value::Integer(n) => Expr::Integer(*n, Loc::none()),
            Value::Float(n) => Expr::Float(*n, Loc::none()),
            Value::Str(s) => Expr::Str(s.clone(), Loc::none()),
            Value::Bool(b) => Expr::Bool(*b, Loc::none()),
            Value::Nil => Expr::Nil(Loc::none()),
            Value::Symbol(name) => Expr::Identifier(name.clone(), Loc::none()),
            Value::List(items) => {
                if items.is_empty() {
                    return Expr::Nil(Loc::none());
                }
                let elements: Vec<Expr> = items.iter().map(|el| self.data_to_ast(el)).collect();

                if let Expr::Identifier(ref name, _) = elements[0] {
                    match name.as_str() {
                        "if" if elements.len() >= 3 => {
                            return Expr::If(
                                Box::new(elements[1].clone()),
                                Box::new(elements[2].clone()),
                                elements.get(3).map(|e| Box::new(e.clone())),
                                Loc::none(),
                            );
                        }
                        "do" => {
                            return Expr::Do(elements[1..].to_vec(), Loc::none());
                        }
                        "define" if elements.len() == 3 => {
                            if let Expr::Identifier(ref n, _) = elements[1] {
                                return Expr::Define(
                                    n.clone(),
                                    Box::new(elements[2].clone()),
                                    Loc::none(),
                                );
                            }
                        }
                        "set!" if elements.len() == 3 => {
                            if let Expr::Identifier(ref n, _) = elements[1] {
                                return Expr::SetBang(
                                    n.clone(),
                                    Box::new(elements[2].clone()),
                                    Loc::none(),
                                );
                            }
                        }
                        "and" if elements.len() == 3 => {
                            return Expr::And(
                                Box::new(elements[1].clone()),
                                Box::new(elements[2].clone()),
                                Loc::none(),
                            );
                        }
                        "or" if elements.len() == 3 => {
                            return Expr::Or(
                                Box::new(elements[1].clone()),
                                Box::new(elements[2].clone()),
                                Loc::none(),
                            );
                        }
                        "quote" if elements.len() == 2 => {
                            return Expr::Quote(
                                Box::new(elements[1].clone()),
                                Loc::none(),
                            );
                        }
                        _ => {}
                    }
                }

                Expr::Call(
                    Box::new(elements[0].clone()),
                    elements[1..].to_vec(),
                    Loc::none(),
                )
            }
            _ => Expr::Nil(Loc::none()),
        }
    }

    // ── Quote / Quasiquote ──────────────────────────────────────

    fn quote_value(&self, expr: &Expr) -> Result<Value> {
        Ok(match expr {
            Expr::Identifier(name, _) => Value::Symbol(name.clone()),
            Expr::Integer(n, _) => Value::Integer(*n),
            Expr::Float(n, _) => Value::Float(*n),
            Expr::Str(s, _) => Value::Str(s.clone()),
            Expr::Bool(b, _) => Value::Bool(*b),
            Expr::Nil(_) => Value::Nil,
            Expr::Call(func, args, _) => {
                let mut items = vec![self.quote_value(func)?];
                for a in args {
                    items.push(self.quote_value(a)?);
                }
                Value::List(items)
            }
            Expr::Do(exprs, _) => {
                let mut items = Vec::new();
                for e in exprs {
                    items.push(self.quote_value(e)?);
                }
                Value::List(items)
            }
            _ => Value::Nil,
        })
    }

    fn quasiquote_eval(&mut self, expr: &Expr, env: &Env) -> Result<Value> {
        match expr {
            Expr::Unquote(inner, _) => {
                self.evaluate_node(inner, env)
            }
            Expr::UnquoteSplice(_, _) => {
                Err(MoofError::runtime("Unquote-splice ,@ not valid outside of a list context"))
            }
            Expr::Call(func, args, _) => {
                let mut all_elements = vec![func.as_ref().clone()];
                all_elements.extend(args.clone());
                self.qq_list(&all_elements, env)
            }
            Expr::Identifier(name, _) => Ok(Value::Symbol(name.clone())),
            Expr::Integer(n, _) => Ok(Value::Integer(*n)),
            Expr::Float(n, _) => Ok(Value::Float(*n)),
            Expr::Str(s, _) => Ok(Value::Str(s.clone())),
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::Nil(_) => Ok(Value::Nil),
            Expr::If(cond, then_br, else_br, _) => {
                let mut elements = vec![
                    Expr::Identifier("if".to_string(), Loc::none()),
                    cond.as_ref().clone(),
                    then_br.as_ref().clone(),
                ];
                if let Some(eb) = else_br {
                    elements.push(eb.as_ref().clone());
                }
                self.qq_list(&elements, env)
            }
            Expr::Do(exprs, _) => {
                let mut elements = vec![Expr::Identifier("do".to_string(), Loc::none())];
                elements.extend(exprs.clone());
                self.qq_list(&elements, env)
            }
            Expr::Define(name, value, _) => {
                let elements = vec![
                    Expr::Identifier("define".to_string(), Loc::none()),
                    Expr::Identifier(name.clone(), Loc::none()),
                    value.as_ref().clone(),
                ];
                self.qq_list(&elements, env)
            }
            Expr::SetBang(name, value, _) => {
                let elements = vec![
                    Expr::Identifier("set!".to_string(), Loc::none()),
                    Expr::Identifier(name.clone(), Loc::none()),
                    value.as_ref().clone(),
                ];
                self.qq_list(&elements, env)
            }
            Expr::And(left, right, _) => {
                let elements = vec![
                    Expr::Identifier("and".to_string(), Loc::none()),
                    left.as_ref().clone(),
                    right.as_ref().clone(),
                ];
                self.qq_list(&elements, env)
            }
            Expr::Or(left, right, _) => {
                let elements = vec![
                    Expr::Identifier("or".to_string(), Loc::none()),
                    left.as_ref().clone(),
                    right.as_ref().clone(),
                ];
                self.qq_list(&elements, env)
            }
            Expr::Quasiquote(inner, _) => {
                self.quote_value(inner)
            }
            other => self.quote_value(other),
        }
    }

    fn qq_list(&mut self, elements: &[Expr], env: &Env) -> Result<Value> {
        let mut result = Vec::new();
        for el in elements {
            if let Expr::UnquoteSplice(inner, _) = el {
                let spliced = self.evaluate_node(inner, env)?;
                match spliced {
                    Value::List(items) => result.extend(items),
                    _ => return Err(MoofError::runtime(",@ value must be a list")),
                }
            } else {
                result.push(self.quasiquote_eval(el, env)?);
            }
        }
        Ok(Value::List(result))
    }

    // ── Class definition ────────────────────────────────────────

    fn eval_class_def(
        &mut self,
        name: &str,
        superclass_name: Option<&str>,
        fields: &[String],
        methods: &[MethodDef],
        trait_names: &[String],
        _loc: &Loc,
    ) -> Result<Value> {
        let superclass = if let Some(sc_name) = superclass_name {
            let sc = self.class_registry.get(sc_name)
                .ok_or_else(|| MoofError::runtime(format!("Unknown superclass: {}", sc_name)))?;
            Some(sc.clone())
        } else {
            None
        };

        let mut all_trait_methods: HashMap<String, MoofFunction> = HashMap::new();
        for trait_name in trait_names {
            let tmethods = self.trait_registry.get(trait_name)
                .ok_or_else(|| MoofError::runtime(format!("Unknown trait: {}", trait_name)))?;
            for (sel, func) in tmethods {
                all_trait_methods.insert(sel.clone(), func.clone());
            }
        }

        let mut new_methods: HashMap<String, MoofFunction> = HashMap::new();
        for mdef in methods {
            let func = MoofFunction {
                name: Some(format!("{}#{}", name, mdef.selector)),
                params: mdef.params.clone(),
                rest_param: None,
                body: mdef.body.clone(),
                closure: self.env.clone(),
            };
            new_methods.insert(mdef.selector.clone(), func);
        }

        // Open class: if class already exists, merge into it
        if let Some(existing_rc) = self.class_registry.get(name) {
            let mut existing = existing_rc.borrow_mut();
            let mut combined_methods: HashMap<String, MoofFunction> = all_trait_methods;
            combined_methods.extend(new_methods);
            existing.reopen(fields.to_vec(), combined_methods);
            drop(existing);
            return Ok(Value::Class(existing_rc.clone()));
        }

        // Create new class
        let mut method_table: HashMap<String, MoofFunction> = all_trait_methods;
        method_table.extend(new_methods);

        let mut klass = MoofClass::new(name.to_string());
        klass.superclass = superclass;
        klass.own_fields = fields.to_vec();
        klass.fields = fields.to_vec();
        klass.methods = method_table;

        let klass_rc = Rc::new(RefCell::new(klass));
        self.class_registry.insert(name.to_string(), klass_rc.clone());
        let val = Value::Class(klass_rc);
        self.env.define(name, val.clone(), false);
        Ok(val)
    }

    // ── Module system ───────────────────────────────────────────

    fn evaluate_module(
        &mut self,
        name: &str,
        exports: &[String],
        body: &[Expr],
    ) -> Result<Value> {
        let mod_env = self.global_env.child();

        for expr in body {
            self.evaluate_node(expr, &mod_env)?;
        }

        let mut exported = HashMap::new();
        if exports.is_empty() {
            let bindings = mod_env.bindings();
            for (k, v) in bindings {
                exported.insert(k, v);
            }
        } else {
            for exp_name in exports {
                match mod_env.get(exp_name) {
                    Ok(val) => { exported.insert(exp_name.clone(), val); }
                    Err(_) => {
                        return Err(MoofError::runtime(format!(
                            "Module '{}' exports '{}' but it is not defined",
                            name, exp_name
                        )));
                    }
                }
            }
        }

        self.module_registry.insert(name.to_string(), exported.clone());

        let map_pairs: Vec<(String, Value)> = exported.into_iter().collect();
        self.env.define(name, Value::Map(map_pairs), false);
        Ok(Value::Nil)
    }

    fn import_module(
        &mut self,
        module_name: &str,
        imports: Option<&[String]>,
        alias: Option<&str>,
    ) -> Result<Value> {
        let module = self.module_registry.get(module_name)
            .ok_or_else(|| MoofError::runtime(format!("Unknown module: {}", module_name)))?
            .clone();

        if let Some(alias_name) = alias {
            let map_pairs: Vec<(String, Value)> = module.into_iter().collect();
            self.env.define(alias_name, Value::Map(map_pairs), false);
        } else if let Some(import_names) = imports {
            for imp in import_names {
                let val = module.get(imp)
                    .ok_or_else(|| MoofError::runtime(format!(
                        "Module '{}' does not export '{}'",
                        module_name, imp
                    )))?;
                self.env.define(imp, val.clone(), false);
            }
        } else {
            for (name, val) in &module {
                self.env.define(name, val.clone(), false);
            }
        }
        Ok(Value::Nil)
    }

    fn require_file(&mut self, path: &str) -> Result<Value> {
        let mut full_path = path.to_string();
        if !full_path.ends_with(".moof") {
            full_path.push_str(".moof");
        }

        let resolved = if std::path::Path::new(&full_path).exists() {
            full_path.clone()
        } else {
            return Err(MoofError::runtime(format!("Cannot find file: {}", full_path)));
        };

        let source = std::fs::read_to_string(&resolved)
            .map_err(|e| MoofError::runtime(format!("Cannot read file {}: {}", resolved, e)))?;

        self.load_source(&source, &resolved)
    }

    /// Load source code into the interpreter (used for stdlib and require).
    pub fn load_source(&mut self, source: &str, _filename: &str) -> Result<Value> {
        let tokens = crate::lexer::Lexer::new(source).tokenize()?;
        let program = crate::parser::Parser::new(tokens).parse_program()?;
        let normalized = crate::normalizer::normalize(program);
        self.evaluate(&normalized)
    }
}

// ── Public function call with trampoline (for TCO) ──────────────

pub fn call_function(interp: &mut Interpreter, func: &MoofFunction, args: Vec<Value>) -> Result<Value> {
    // Check arity
    if func.is_variadic() {
        if args.len() < func.params.len() {
            return Err(MoofError::arity(
                &format!("{}+", func.params.len()),
                args.len(),
                func.name.as_deref(),
            ));
        }
    } else if args.len() != func.arity() {
        return Err(MoofError::arity(
            &func.arity().to_string(),
            args.len(),
            func.name.as_deref(),
        ));
    }

    // Set up call environment
    let call_env = func.closure.child();
    for (i, param) in func.params.iter().enumerate() {
        let val = args.get(i).cloned().unwrap_or(Value::Nil);
        call_env.define(param, val, false);
    }
    if let Some(ref rest_name) = func.rest_param {
        let rest_args = if args.len() > func.params.len() {
            args[func.params.len()..].to_vec()
        } else {
            vec![]
        };
        call_env.define(rest_name, Value::List(rest_args), false);
    }

    // Evaluate body in tail position, then trampoline
    let mut result = interp.evaluate_tail(&func.body, &call_env)?;

    // Trampoline loop
    loop {
        match result {
            Value::TailCall(callee, tc_args) => {
                match *callee {
                    Value::Function(ref f) => {
                        if f.is_variadic() {
                            if tc_args.len() < f.params.len() {
                                return Err(MoofError::arity(
                                    &format!("{}+", f.params.len()),
                                    tc_args.len(),
                                    f.name.as_deref(),
                                ));
                            }
                        } else if tc_args.len() != f.arity() {
                            return Err(MoofError::arity(
                                &f.arity().to_string(),
                                tc_args.len(),
                                f.name.as_deref(),
                            ));
                        }
                        let new_env = f.closure.child();
                        for (i, param) in f.params.iter().enumerate() {
                            let val = tc_args.get(i).cloned().unwrap_or(Value::Nil);
                            new_env.define(param, val, false);
                        }
                        if let Some(ref rest_name) = f.rest_param {
                            let rest = if tc_args.len() > f.params.len() {
                                tc_args[f.params.len()..].to_vec()
                            } else {
                                vec![]
                            };
                            new_env.define(rest_name, Value::List(rest), false);
                        }
                        result = interp.evaluate_tail(&f.body, &new_env)?;
                    }
                    Value::Builtin(_, f_ptr) => {
                        return f_ptr(interp, tc_args);
                    }
                    other => {
                        return Err(MoofError::runtime(format!(
                            "Cannot call {}: not a function",
                            other.type_name()
                        )));
                    }
                }
            }
            other => return Ok(other),
        }
    }
}
