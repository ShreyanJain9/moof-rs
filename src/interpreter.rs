use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::{Expr, Program, MethodDef, Loc};
use crate::environment::Env;
use crate::value::{Value, Eval, Method, MoofFunction, MoofClass, MoofObject, MoofProtocol, MoofMacro};
use crate::error::{MoofError, Result};
use crate::builtins;
use crate::methods;
use crate::pattern_matcher;

pub struct Interpreter {
    pub global_env: Env,
    pub class_registry: HashMap<String, Rc<RefCell<MoofClass>>>,
    pub protocol_registry: HashMap<String, MoofProtocol>,
    pub macro_registry: HashMap<String, MoofMacro>,
    pub type_registry: HashMap<String, Vec<String>>,
    pub module_registry: HashMap<String, HashMap<String, Value>>,
}

impl Interpreter {
    pub fn new() -> Self {
        let global = Env::new();
        let mut interp = Interpreter {
            global_env: global,
            class_registry: HashMap::new(),
            protocol_registry: HashMap::new(),
            macro_registry: HashMap::new(),
            type_registry: HashMap::new(),
            module_registry: HashMap::new(),
        };
        interp.register_builtin_classes();
        builtins::install(&mut interp);
        crate::builtin_methods::install(&mut interp);
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
        let env = self.global_env.clone();
        let mut result = Value::Nil;
        for expr in &program.expressions {
            result = self.eval_expr(expr, &env)?;
        }
        Ok(result)
    }

    /// The big match dispatcher for expression evaluation.
    fn eval_expr_inner(&mut self, expr: &Expr, env: &Env) -> Result<Value> {
        match expr {
            // -- Literals --
            Expr::Integer(n, _) => Ok(Value::Integer(*n)),
            Expr::Float(n, _) => Ok(Value::Float(*n)),
            Expr::Str(s, _) => Ok(Value::Str(s.clone())),
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::Nil(_) => Ok(Value::Nil),

            Expr::Identifier(name, loc) => {
                env.get(name).map_err(|_| {
                    MoofError::name(name, loc.line, loc.column)
                })
            }

            Expr::Quote(inner, _) => self.quote_value(inner),

            Expr::Quasiquote(inner, _) => self.quasiquote_eval(inner, env),

            Expr::Unquote(_, _) => Err(MoofError::runtime("Unquote (,) outside of quasiquote")),
            Expr::UnquoteSplice(_, _) => Err(MoofError::runtime("Unquote-splice (,@) outside of quasiquote")),

            Expr::MapLiteral(pairs, _) => {
                let mut result = indexmap::IndexMap::new();
                for (key, val_expr) in pairs {
                    let val = self.eval_expr(val_expr, env)?;
                    result.insert(key.clone(), val);
                }
                Ok(Value::Map(result))
            }

            Expr::Define(name, value_expr, _) => {
                let mut value = self.eval_expr(value_expr, env)?;
                // Attach name to anonymous lambdas defined at top level
                if let Value::Function(ref mut f) = value {
                    if f.name.is_none() {
                        f.name = Some(name.clone());
                    }
                }
                env.define(name, value.clone(), false);
                Ok(value)
            }

            Expr::Lambda(params, rest_param, body, _) => {
                Ok(Value::Function(MoofFunction {
                    name: None,
                    params: params.clone(),
                    rest_param: rest_param.clone(),
                    body: body.clone(),
                    closure: env.clone(),
                }))
            }

            Expr::If(cond, then_expr, else_expr, _) => {
                let cond_val = self.eval_expr(cond, env)?;
                if cond_val.is_truthy() {
                    self.eval_expr(then_expr, env)
                } else if let Some(else_e) = else_expr {
                    self.eval_expr(else_e, env)
                } else {
                    Ok(Value::Nil)
                }
            }

            Expr::Let(bindings, body, _) => {
                let let_env = env.child();
                for (name, val_expr) in bindings {
                    // Evaluate binding value in the outer env context
                    let val = self.eval_expr(val_expr, env)?;
                    let_env.define(name, val, false);
                }
                self.eval_expr(body, &let_env)
            }

            Expr::Do(exprs, _) => {
                let mut result = Value::Nil;
                for e in exprs {
                    result = self.eval_expr(e, env)?;
                }
                Ok(result)
            }

            Expr::SetBang(name, value_expr, _) => {
                let value = self.eval_expr(value_expr, env)?;
                env.set(name, value)
            }

            Expr::TryCatch(body, error_name, catch_body, _) => {
                match self.eval_expr(body, env) {
                    Ok(val) => Ok(val),
                    Err(e) => {
                        let catch_env = env.child();
                        catch_env.define(error_name, Value::Str(e.message.clone()), false);
                        self.eval_expr(catch_body, &catch_env)
                    }
                }
            }

            Expr::Call(func_expr, arg_exprs, loc) => {
                // Check for macro expansion first
                if let Expr::Identifier(name, _) = func_expr.as_ref() {
                    if self.macro_registry.contains_key(name) {
                        return self.expand_and_eval_macro(name, arg_exprs, env);
                    }
                }

                let callee = self.eval_expr(func_expr, env)?;
                let args = self.evaluate_call_args(arg_exprs, env)?;
                self.invoke_callee(callee, args, loc)
            }

            Expr::MessageSend(receiver_expr, selector, arg_exprs, _) => {
                let receiver = self.eval_expr(receiver_expr, env)?;
                let mut args = Vec::new();
                for a in arg_exprs {
                    args.push(self.eval_expr(a, env)?);
                }
                methods::send_message(self, receiver, selector, args)
            }

            Expr::Match(scrutinee, clauses, _) => {
                let val = self.eval_expr(scrutinee, env)?;
                for clause in clauses {
                    let m = pattern_matcher::match_pattern(&clause.pattern, &val, self);
                    if m.success {
                        let match_env = env.child();
                        for (name, bind_val) in &m.bindings {
                            match_env.define(name, bind_val.clone(), false);
                        }
                        // Check guard
                        if let Some(ref guard) = clause.guard {
                            let guard_val = self.eval_expr(guard, &match_env)?;
                            if !guard_val.is_truthy() {
                                continue;
                            }
                        }
                        return self.eval_expr(&clause.body, &match_env);
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
                        env.define(&variant.name, Value::Object(obj), false);
                    } else {
                        // Constructor variant
                        let mut klass = MoofClass::new(variant.name.clone());
                        klass.own_fields = variant.fields.clone();
                        klass.fields = variant.fields.clone();
                        let klass_rc = Rc::new(RefCell::new(klass));
                        self.class_registry.insert(variant.name.clone(), klass_rc.clone());
                        env.define(&variant.name, Value::Class(klass_rc), false);
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
                    default_methods: HashMap::new(),
                };
                self.protocol_registry.insert(name.clone(), proto.clone());
                let val = Value::Protocol(proto);
                env.define(name, val.clone(), false);
                Ok(val)
            }

            Expr::DefMacro(name, params, body, _) => {
                let mac = MoofMacro {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                };
                self.macro_registry.insert(name.clone(), mac.clone());
                env.define(name, Value::Macro(mac), false);
                Ok(Value::Str(name.clone()))
            }

            Expr::ClassDef { name, superclass, fields, methods, traits, loc } => {
                self.eval_class_def(name, superclass.as_deref(), fields, methods, traits, loc, env)
            }

            Expr::TraitDef { name, methods, loc: _ } => {
                // Traits are protocols with default methods
                let mut default_methods = HashMap::new();
                let mut selectors = Vec::new();
                for mdef in methods {
                    let func = MoofFunction {
                        name: Some(format!("{}#{}", name, mdef.selector)),
                        params: mdef.params.clone(),
                        rest_param: None,
                        body: mdef.body.clone(),
                        closure: env.clone(),
                    };
                    selectors.push(mdef.selector.clone());
                    default_methods.insert(mdef.selector.clone(), func);
                }
                let proto = MoofProtocol {
                    name: name.clone(),
                    selectors,
                    default_methods,
                };
                self.protocol_registry.insert(name.clone(), proto.clone());
                let val = Value::Protocol(proto);
                env.define(name, val.clone(), false);
                Ok(val)
            }

            Expr::ModuleDef(name, exports, body, _) => {
                self.evaluate_module(name, exports, body)
            }

            Expr::UseModule(module_name, imports, alias, _) => {
                self.import_module(module_name, imports.as_deref(), alias.as_deref(), env)
            }

            Expr::Require(path, _) => {
                self.require_file(path)
            }

            // Pipeline, StringInterp, SelectorRef are desugared by the normalizer
            // and should never reach the interpreter.
            Expr::Pipeline(_, _, _) | Expr::StringInterp(_, _) | Expr::SelectorRef(_, _, _) => {
                Err(MoofError::runtime("Internal error: un-normalized AST node reached interpreter"))
            }
        }
    }

    /// Public eval_expr that takes an explicit env parameter.
    pub fn eval_expr(&mut self, expr: &Expr, env: &Env) -> Result<Value> {
        self.eval_expr_inner(expr, env).map_err(|e| {
            let loc = expr.loc();
            e.with_loc(loc.line, loc.column)
        })
    }

    // ── Call helpers ─────────────────────────────────────────────

    fn evaluate_call_args(&mut self, arguments: &[Expr], env: &Env) -> Result<Vec<Value>> {
        arguments.iter().map(|arg| self.eval_expr(arg, env)).collect()
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

    pub fn evaluate_tail(&mut self, expr: &Expr, env: &Env) -> Result<Eval> {
        self.eval_tail_inner(expr, env)
    }

    fn eval_tail_inner(&mut self, expr: &Expr, env: &Env) -> Result<Eval> {
        match expr {
            Expr::Call(func_expr, arg_exprs, _loc) => {
                if let Expr::Identifier(name, _) = func_expr.as_ref() {
                    if self.macro_registry.contains_key(name) {
                        return Ok(Eval::Val(self.expand_and_eval_macro(name, arg_exprs, env)?));
                    }
                }
                let callee = self.eval_expr(func_expr, env)?;
                let args = self.evaluate_call_args(arg_exprs, env)?;
                if matches!(callee, Value::Function(_)) {
                    Ok(Eval::TailCall { func: callee, args })
                } else {
                    Ok(Eval::Val(self.invoke_callee(callee, args, expr.loc())?))
                }
            }

            Expr::If(cond, then_expr, else_expr, _) => {
                let cond_val = self.eval_expr(cond, env)?;
                if cond_val.is_truthy() {
                    self.eval_tail_inner(then_expr, env)
                } else if let Some(else_e) = else_expr {
                    self.eval_tail_inner(else_e, env)
                } else {
                    Ok(Eval::Val(Value::Nil))
                }
            }

            Expr::Do(exprs, _) => {
                if exprs.is_empty() { return Ok(Eval::Val(Value::Nil)); }
                for e in &exprs[..exprs.len() - 1] {
                    self.eval_expr(e, env)?;
                }
                self.eval_tail_inner(exprs.last().unwrap(), env)
            }

            Expr::Let(bindings, body, _) => {
                let let_env = env.child();
                for (name, val_expr) in bindings {
                    let val = self.eval_expr(val_expr, env)?;
                    let_env.define(name, val, false);
                }
                self.eval_tail_inner(body, &let_env)
            }

            Expr::Match(scrutinee, clauses, _) => {
                let val = self.eval_expr(scrutinee, env)?;
                for clause in clauses {
                    let m = pattern_matcher::match_pattern(&clause.pattern, &val, self);
                    if m.success {
                        let match_env = env.child();
                        for (name, bind_val) in &m.bindings {
                            match_env.define(name, bind_val.clone(), false);
                        }
                        if let Some(ref guard) = clause.guard {
                            let guard_val = self.eval_expr(guard, &match_env)?;
                            if !guard_val.is_truthy() {
                                continue;
                            }
                        }
                        return self.eval_tail_inner(&clause.body, &match_env);
                    }
                }
                Ok(Eval::Val(Value::Nil))
            }

            other => Ok(Eval::Val(self.eval_expr_inner(other, env)?)),
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

        let expanded = self.eval_expr(&mac.body, &macro_env)?;

        let ast = self.data_to_ast(&expanded);
        self.eval_expr(&ast, env)
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
                            // (and a b) → (if a b false)
                            return Expr::If(
                                Box::new(elements[1].clone()),
                                Box::new(elements[2].clone()),
                                Some(Box::new(Expr::Bool(false, Loc::none()))),
                                Loc::none(),
                            );
                        }
                        "or" if elements.len() == 3 => {
                            // (or a b) → (let ((__or_tmp a)) (if __or_tmp __or_tmp b))
                            let tmp = "__or_macro_tmp".to_string();
                            let tmp_id = Expr::Identifier(tmp.clone(), Loc::none());
                            return Expr::Let(
                                vec![(tmp, elements[1].clone())],
                                Box::new(Expr::If(
                                    Box::new(tmp_id.clone()),
                                    Box::new(tmp_id),
                                    Some(Box::new(elements[2].clone())),
                                    Loc::none(),
                                )),
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
                self.eval_expr(inner, env)
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
                let spliced = self.eval_expr(inner, env)?;
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
        env: &Env,
    ) -> Result<Value> {
        let superclass = if let Some(sc_name) = superclass_name {
            let sc = self.class_registry.get(sc_name)
                .ok_or_else(|| MoofError::runtime(format!("Unknown superclass: {}", sc_name)))?;
            Some(sc.clone())
        } else {
            None
        };

        let mut all_trait_methods: HashMap<String, Method> = HashMap::new();
        for trait_name in trait_names {
            let proto = self.protocol_registry.get(trait_name)
                .ok_or_else(|| MoofError::runtime(format!("Unknown trait/protocol: {}", trait_name)))?;
            for (sel, func) in &proto.default_methods {
                all_trait_methods.insert(sel.clone(), Method::UserDefined(func.clone()));
            }
        }

        let mut new_methods: HashMap<String, Method> = HashMap::new();
        for mdef in methods {
            let func = MoofFunction {
                name: Some(format!("{}#{}", name, mdef.selector)),
                params: mdef.params.clone(),
                rest_param: None,
                body: mdef.body.clone(),
                closure: env.clone(),
            };
            new_methods.insert(mdef.selector.clone(), Method::UserDefined(func));
        }

        // Open class: if class already exists, merge into it
        if let Some(existing_rc) = self.class_registry.get(name) {
            let mut existing = existing_rc.borrow_mut();
            let mut combined_methods: HashMap<String, Method> = all_trait_methods;
            combined_methods.extend(new_methods);
            existing.reopen(fields.to_vec(), combined_methods);
            drop(existing);
            return Ok(Value::Class(existing_rc.clone()));
        }

        // Create new class
        let mut method_table: HashMap<String, Method> = all_trait_methods;
        method_table.extend(new_methods);

        let mut klass = MoofClass::new(name.to_string());
        klass.superclass = superclass;
        klass.own_fields = fields.to_vec();
        klass.fields = fields.to_vec();
        klass.methods = method_table;

        let klass_rc = Rc::new(RefCell::new(klass));
        self.class_registry.insert(name.to_string(), klass_rc.clone());
        let val = Value::Class(klass_rc);
        env.define(name, val.clone(), false);
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
            self.eval_expr(expr, &mod_env)?;
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

        let map: indexmap::IndexMap<String, Value> = exported.into_iter().collect();
        self.global_env.define(name, Value::Map(map), false);
        Ok(Value::Nil)
    }

    fn import_module(
        &mut self,
        module_name: &str,
        imports: Option<&[String]>,
        alias: Option<&str>,
        env: &Env,
    ) -> Result<Value> {
        let module = self.module_registry.get(module_name)
            .ok_or_else(|| MoofError::runtime(format!("Unknown module: {}", module_name)))?
            .clone();

        if let Some(alias_name) = alias {
            let map: indexmap::IndexMap<String, Value> = module.into_iter().collect();
            env.define(alias_name, Value::Map(map), false);
        } else if let Some(import_names) = imports {
            for imp in import_names {
                let val = module.get(imp)
                    .ok_or_else(|| MoofError::runtime(format!(
                        "Module '{}' does not export '{}'",
                        module_name, imp
                    )))?;
                env.define(imp, val.clone(), false);
            }
        } else {
            for (name, val) in &module {
                env.define(name, val.clone(), false);
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
    let mut eval_result = interp.evaluate_tail(&func.body, &call_env)?;

    // Trampoline loop
    loop {
        match eval_result {
            Eval::TailCall { func: callee, args: tc_args } => {
                match callee {
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
                        eval_result = interp.evaluate_tail(&f.body, &new_env)?;
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
            Eval::Val(v) => return Ok(v),
        }
    }
}
