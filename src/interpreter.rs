use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::cons;
use crate::environment::Env;
use crate::error::{MoofError, Result};
use crate::symbol::{KnownSymbols, SymId, SymbolTable};
use crate::value::{
    ClosureBody, MoofClass, MoofClosure, MoofObject, MoofTable, Value,
};

// ═══════════════════════════════════════════════════════════════════════
// Interpreter
// ═══════════════════════════════════════════════════════════════════════

pub struct Interpreter {
    pub symbols: SymbolTable,
    pub known: KnownSymbols,
    pub global_env: Env,
    pub macro_registry: HashMap<SymId, Value>,
    pub type_registry: HashMap<SymId, Vec<SymId>>,

    // Bootstrap type classes
    pub object_class: Rc<RefCell<MoofClass>>,
    pub class_class: Rc<RefCell<MoofClass>>,
    pub integer_class: Rc<RefCell<MoofClass>>,
    pub float_class: Rc<RefCell<MoofClass>>,
    pub string_class: Rc<RefCell<MoofClass>>,
    pub symbol_class: Rc<RefCell<MoofClass>>,
    pub cons_class: Rc<RefCell<MoofClass>>,
    pub table_class: Rc<RefCell<MoofClass>>,
    pub closure_class: Rc<RefCell<MoofClass>>,
    pub true_class: Rc<RefCell<MoofClass>>,
    pub false_class: Rc<RefCell<MoofClass>>,
    pub nil_class: Rc<RefCell<MoofClass>>,
    pub numeric_class: Rc<RefCell<MoofClass>>,
    pub error_class: Rc<RefCell<MoofClass>>,

    pub source_locs: HashMap<usize, (usize, usize)>,
}

impl Interpreter {
    // ── Constructor ──────────────────────────────────────────────────

    pub fn new() -> Self {
        let mut symbols = SymbolTable::new();
        let known = KnownSymbols::new(&mut symbols);

        // --- Bootstrap class hierarchy ---

        let object_class = Rc::new(RefCell::new(MoofClass::new(
            symbols.intern("Object"),
        )));

        let class_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Class"),
            superclass: Some(object_class.clone()),
            metaclass: None, // set to self below
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));
        // Class's metaclass is itself
        class_class.borrow_mut().metaclass = Some(class_class.clone());

        let numeric_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Numeric"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let integer_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Integer"),
            superclass: Some(numeric_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let float_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Float"),
            superclass: Some(numeric_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let string_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("String"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let symbol_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Symbol"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let cons_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Cons"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let table_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Table"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let closure_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Closure"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        // Bool is abstract; TrueClass and FalseClass are concrete
        let bool_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Bool"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let true_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("TrueClass"),
            superclass: Some(bool_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let false_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("FalseClass"),
            superclass: Some(bool_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let nil_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("NilClass"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let error_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Error"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }));

        let mut interp = Interpreter {
            symbols,
            known,
            global_env: Env::new(),
            macro_registry: HashMap::new(),
            type_registry: HashMap::new(),
            object_class,
            class_class,
            integer_class,
            float_class,
            string_class,
            symbol_class,
            cons_class,
            table_class,
            closure_class,
            true_class,
            false_class,
            nil_class,
            numeric_class,
            error_class,
            source_locs: HashMap::new(),
        };

        // Install built-in functions
        crate::builtins::install(&mut interp);

        interp
    }

    // ── Top-level evaluation ────────────────────────────────────────

    pub fn evaluate_program(&mut self, exprs: &[Value]) -> Result<Value> {
        let env = self.global_env.clone();
        let mut result = Value::Nil;
        for expr in exprs {
            result = self.eval(expr, &env)?;
        }
        Ok(result)
    }

    pub fn load_source(&mut self, source: &str, _filename: &str) -> Result<Value> {
        let tokens = crate::lexer::Lexer::new(source).tokenize()?;
        let exprs = crate::parser::Parser::new(tokens, &mut self.symbols).parse_program()?;
        self.evaluate_program(&exprs)
    }

    // ── REPL introspection helpers ──────────────────────────────────

    pub fn class_of_name(&self, name: &str) -> Option<Rc<RefCell<MoofClass>>> {
        let all = self.all_type_classes();
        for class_rc in all {
            let c = class_rc.borrow();
            if self.symbols.try_name(c.name) == Some(name) {
                return Some(class_rc.clone());
            }
        }
        None
    }

    pub fn all_classes(&self) -> Vec<(String, Rc<RefCell<MoofClass>>)> {
        let mut result = Vec::new();
        for class_rc in self.all_type_classes() {
            let name = {
                let c = class_rc.borrow();
                self.symbols.try_name(c.name).unwrap_or("?").to_string()
            };
            if !name.contains("meta") {
                result.push((name, class_rc.clone()));
            }
        }
        result
    }

    fn all_type_classes(&self) -> Vec<Rc<RefCell<MoofClass>>> {
        vec![
            self.object_class.clone(),
            self.class_class.clone(),
            self.numeric_class.clone(),
            self.integer_class.clone(),
            self.float_class.clone(),
            self.string_class.clone(),
            self.symbol_class.clone(),
            self.cons_class.clone(),
            self.table_class.clone(),
            self.closure_class.clone(),
            self.true_class.clone(),
            self.false_class.clone(),
            self.nil_class.clone(),
            self.error_class.clone(),
        ]
    }

    // ── Core eval ───────────────────────────────────────────────────

    pub fn eval(&mut self, expr: &Value, env: &Env) -> Result<Value> {
        match expr {
            // Self-evaluating
            Value::Integer(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Nil
            | Value::Str(_)
            | Value::Table(_)
            | Value::Object(_)
            | Value::Closure(_) => Ok(expr.clone()),

            // Symbol lookup
            Value::Symbol(id) => env.get(*id).map_err(|_| {
                let name = self.symbols.name(*id);
                MoofError::name(format!("Undefined variable: {name}"))
            }),

            // Compound form — check for special forms, macros, function calls
            Value::Cons(cell) => {
                let car = &cell.car;
                let cdr = &cell.cdr;

                // Is the head a known special-form symbol?
                if let Value::Symbol(id) = car {
                    let id = *id;
                    let k = &self.known;

                    if id == k.if_ {
                        return self.eval_if(cdr, env);
                    }
                    if id == k.define {
                        return self.eval_define(cdr, env);
                    }
                    if id == k.lambda || id == k.fn_ {
                        return self.eval_lambda(cdr, env);
                    }
                    if id == k.let_ {
                        return self.eval_let(cdr, env);
                    }
                    if id == k.do_ {
                        return self.eval_do(cdr, env);
                    }
                    if id == k.set_bang {
                        return self.eval_set_bang(cdr, env);
                    }
                    if id == k.quote {
                        return self.eval_quote(cdr);
                    }
                    if id == k.quasiquote {
                        return self.eval_quasiquote(cdr, env);
                    }
                    if id == k.and {
                        return self.eval_and(cdr, env);
                    }
                    if id == k.or {
                        return self.eval_or(cdr, env);
                    }
                    if id == k.cond {
                        return self.eval_cond(cdr, env);
                    }
                    if id == k.match_ {
                        return self.eval_match(cdr, env);
                    }
                    if id == k.class {
                        return self.eval_class(cdr, env);
                    }
                    if id == k.try_ {
                        return self.eval_try(cdr, env);
                    }
                    if id == k.defmacro {
                        return self.eval_defmacro(cdr, env);
                    }
                    if id == k.require {
                        return self.eval_require(cdr, env);
                    }
                    if id == k.send {
                        return self.eval_send(cdr, env);
                    }
                    if id == k.table {
                        return self.eval_table(cdr, env);
                    }
                    if id == k.table_array {
                        return self.eval_table_array(cdr, env);
                    }
                    if id == k.str_interp {
                        return self.eval_str_interp(cdr, env);
                    }

                    if id == k.type_ {
                        return self.eval_type_def(cdr, env);
                    }
                    if id == k.protocol {
                        return Ok(Value::Nil);
                    }
                    if id == k.trait_ {
                        return Ok(Value::Nil);
                    }
                    if id == k.module {
                        return Ok(Value::Nil);
                    }
                    if id == k.use_ {
                        return Ok(Value::Nil);
                    }

                    // Macro check
                    if let Some(macro_val) = self.macro_registry.get(&id).cloned() {
                        return self.expand_macro(&macro_val, cdr, env);
                    }
                }

                // General function call: eval head and args
                let callee = self.eval(car, env)?;
                let args = self.eval_args(cdr, env)?;
                self.invoke(callee, args)
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Special forms
    // ═══════════════════════════════════════════════════════════════════

    // ── if ───────────────────────────────────────────────────────────

    fn eval_if(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let cond_expr = nth_car(args, 0)?;
        let then_expr = nth_car(args, 1)?;
        let cond_val = self.eval(cond_expr, env)?;
        if cond_val.is_truthy() {
            self.eval(then_expr, env)
        } else {
            match nth_car(args, 2) {
                Ok(else_expr) => self.eval(else_expr, env),
                Err(_) => Ok(Value::Nil),
            }
        }
    }

    // ── define ──────────────────────────────────────────────────────

    fn eval_define(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let first = nth_car(args, 0)?;

        match first {
            // (define (name params...) body...) — function shorthand
            Value::Cons(cell) => {
                let name_id = cell.car.as_symbol().map_err(|_| {
                    MoofError::runtime("define: expected symbol as function name")
                })?;
                let params_list = &cell.cdr;
                let body_list = nth_cdr(args, 0)?;

                // Parse params
                let (params, rest_param) = self.parse_params(params_list)?;

                // Wrap body in (do ...) if multiple expressions
                let body = self.wrap_body(body_list);

                let closure = Rc::new(MoofClosure {
                    name: Some(name_id),
                    params,
                    rest_param,
                    body: ClosureBody::Expr(body),
                    env: env.clone(),
                });
                let val = Value::Closure(closure);
                env.define(name_id, val.clone(), false);
                Ok(val)
            }
            // (define name value)
            Value::Symbol(name_id) => {
                let val_expr = nth_car(args, 1)?;
                let mut val = self.eval(val_expr, env)?;
                // Attach name to anonymous closures
                if let Value::Closure(ref c) = val {
                    if c.name.is_none() {
                        let mut named = (**c).clone();
                        named.name = Some(*name_id);
                        val = Value::Closure(Rc::new(named));
                    }
                }
                env.define(*name_id, val.clone(), false);
                Ok(val)
            }
            _ => Err(MoofError::runtime(
                "define: expected symbol or (name params...)",
            )),
        }
    }

    // ── lambda / fn ─────────────────────────────────────────────────

    fn eval_lambda(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let params_expr = nth_car(args, 0)?;
        let body_list = nth_cdr(args, 0)?;

        let (params, rest_param) = self.parse_params(params_expr)?;
        let body = self.wrap_body(body_list);

        Ok(Value::Closure(Rc::new(MoofClosure {
            name: None,
            params,
            rest_param,
            body: ClosureBody::Expr(body),
            env: env.clone(),
        })))
    }

    // ── let ─────────────────────────────────────────────────────────

    fn eval_let(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let bindings_expr = nth_car(args, 0)?;
        let body_list = nth_cdr(args, 0)?;

        let let_env = env.child();

        // Walk bindings list: ((name val) (name val) ...)
        let mut cursor = bindings_expr;
        while let Value::Cons(cell) = cursor {
            let pair = &cell.car;
            let name_id = nth_car(pair, 0)?.as_symbol().map_err(|_| {
                MoofError::runtime("let: binding name must be a symbol")
            })?;
            let val_expr = nth_car(pair, 1)?;
            let val = self.eval(val_expr, env)?;
            let_env.define(name_id, val, false);
            cursor = &cell.cdr;
        }

        // Eval body in let_env
        self.eval_body(body_list, &let_env)
    }

    // ── do ──────────────────────────────────────────────────────────

    fn eval_do(&mut self, args: &Value, env: &Env) -> Result<Value> {
        self.eval_body(args, env)
    }

    // ── set! ────────────────────────────────────────────────────────

    fn eval_set_bang(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("set!: expected symbol")
        })?;
        let val_expr = nth_car(args, 1)?;
        let val = self.eval(val_expr, env)?;
        env.set(name_id, val)
    }

    // ── quote ───────────────────────────────────────────────────────

    fn eval_quote(&mut self, args: &Value) -> Result<Value> {
        Ok(nth_car(args, 0)?.clone())
    }

    // ── quasiquote ──────────────────────────────────────────────────

    fn eval_quasiquote(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let template = nth_car(args, 0)?;
        self.qq_expand(template, env)
    }

    fn qq_expand(&mut self, template: &Value, env: &Env) -> Result<Value> {
        match template {
            Value::Cons(cell) => {
                // Check for (unquote expr)
                if let Value::Symbol(id) = &cell.car {
                    if *id == self.known.unquote {
                        let inner = nth_car(&cell.cdr, 0)?;
                        return self.eval(inner, env);
                    }
                }
                // Walk the list, handling unquote-splice
                self.qq_expand_list(template, env)
            }
            _ => Ok(template.clone()),
        }
    }

    fn qq_expand_list(&mut self, list: &Value, env: &Env) -> Result<Value> {
        let mut result_items: Vec<Value> = Vec::new();
        let mut cursor = list;

        while let Value::Cons(cell) = cursor {
            // Check if car is (unquote-splice expr)
            if let Value::Cons(inner_cell) = &cell.car {
                if let Value::Symbol(id) = &inner_cell.car {
                    if *id == self.known.unquote_splice {
                        let splice_expr = nth_car(&inner_cell.cdr, 0)?;
                        let spliced = self.eval(splice_expr, env)?;
                        // Splice: iterate the result list and append each element
                        for item in spliced.iter_list() {
                            result_items.push(item.clone());
                        }
                        cursor = &cell.cdr;
                        continue;
                    }
                }
            }
            // Normal element: recurse
            let expanded = self.qq_expand(&cell.car, env)?;
            result_items.push(expanded);
            cursor = &cell.cdr;
        }

        // Handle improper list tail
        if !cursor.is_nil() {
            let tail = self.qq_expand(cursor, env)?;
            // Build improper list
            let mut result = tail;
            for item in result_items.into_iter().rev() {
                result = Value::cons(item, result);
            }
            return Ok(result);
        }

        Ok(Value::from_slice(&result_items))
    }

    // ── and ─────────────────────────────────────────────────────────

    fn eval_and(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let a = nth_car(args, 0)?;
        let a_val = self.eval(a, env)?;
        if !a_val.is_truthy() {
            return Ok(Value::Bool(false));
        }
        match nth_car(args, 1) {
            Ok(b) => self.eval(b, env),
            Err(_) => Ok(a_val),
        }
    }

    // ── or ──────────────────────────────────────────────────────────

    fn eval_or(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let a = nth_car(args, 0)?;
        let a_val = self.eval(a, env)?;
        if a_val.is_truthy() {
            return Ok(a_val);
        }
        match nth_car(args, 1) {
            Ok(b) => self.eval(b, env),
            Err(_) => Ok(a_val),
        }
    }

    // ── cond ────────────────────────────────────────────────────────

    fn eval_cond(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            let clause = &cell.car;
            let test_expr = nth_car(clause, 0)?;
            let body_expr = nth_car(clause, 1)?;

            // Check for (else body)
            if let Value::Symbol(id) = test_expr {
                if *id == self.known.else_ {
                    return self.eval(body_expr, env);
                }
            }

            let test_val = self.eval(test_expr, env)?;
            if test_val.is_truthy() {
                return self.eval(body_expr, env);
            }
            cursor = &cell.cdr;
        }
        Ok(Value::Nil)
    }

    // ── match ───────────────────────────────────────────────────────

    fn eval_match(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let scrutinee_expr = nth_car(args, 0)?;
        let scrutinee = self.eval(scrutinee_expr, env)?;

        let clauses = nth_cdr(args, 0)?;
        let mut cursor = clauses;
        while let Value::Cons(cell) = cursor {
            let clause = &cell.car;
            let pattern = nth_car(clause, 0)?;
            let body = nth_car(clause, 1)?;

            let match_env = env.child();
            if self.match_pattern(pattern, &scrutinee, &match_env) {
                return self.eval(body, &match_env);
            }
            cursor = &cell.cdr;
        }
        Ok(Value::Nil)
    }

    // ── try/catch ───────────────────────────────────────────────────

    fn eval_try(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let body = nth_car(args, 0)?;
        match self.eval(body, env) {
            Ok(val) => Ok(val),
            Err(e) => {
                // Look for (catch var body) as second arg
                let catch_form = nth_car(args, 1)?;
                if let Value::Cons(cell) = catch_form {
                    if let Value::Symbol(id) = &cell.car {
                        if *id == self.known.catch {
                            let var = nth_car(&cell.cdr, 0)?;
                            let var_id = var.as_symbol().map_err(|_| {
                                MoofError::runtime("try: catch variable must be a symbol")
                            })?;
                            let catch_body = nth_car(&cell.cdr, 1)?;
                            let catch_env = env.child();
                            catch_env.define(
                                var_id,
                                Value::Str(Rc::from(e.message.as_str())),
                                false,
                            );
                            return self.eval(catch_body, &catch_env);
                        }
                    }
                }
                Err(e)
            }
        }
    }

    // ── defmacro ────────────────────────────────────────────────────

    fn eval_defmacro(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("defmacro: expected symbol name")
        })?;
        let params_expr = nth_car(args, 1)?;
        let body_list = nth_cdr(args, 1)?;

        let (params, rest_param) = self.parse_params(params_expr)?;
        let body = self.wrap_body(body_list);

        let closure = Value::Closure(Rc::new(MoofClosure {
            name: Some(name_id),
            params,
            rest_param,
            body: ClosureBody::Expr(body),
            env: env.clone(),
        }));

        self.macro_registry.insert(name_id, closure);
        Ok(Value::Symbol(name_id))
    }

    // ── require ─────────────────────────────────────────────────────

    fn eval_require(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let path_expr = nth_car(args, 0)?;
        let path_val = self.eval(path_expr, env)?;
        let path = path_val.as_str().map_err(|_| {
            MoofError::runtime("require: expected string path")
        })?;
        let source = std::fs::read_to_string(path)
            .map_err(|e| MoofError::io(format!("require: cannot read '{}': {}", path, e)))?;
        let filename = path.to_string();
        self.load_source(&source, &filename)
    }

    // ── __send ──────────────────────────────────────────────────────

    fn eval_send(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let receiver_expr = nth_car(args, 0)?;
        let selector_expr = nth_car(args, 1)?;

        let receiver = self.eval(receiver_expr, env)?;
        // Selector can be a String literal or a Symbol — don't evaluate it as a variable
        let selector_id = match selector_expr {
            Value::Str(s) => self.symbols.intern(s),
            Value::Symbol(id) => *id,
            other => {
                let evaled = self.eval(other, env)?;
                match &evaled {
                    Value::Str(s) => self.symbols.intern(s),
                    Value::Symbol(id) => *id,
                    _ => return Err(MoofError::runtime("__send: selector must be a string or symbol")),
                }
            }
        };

        // Remaining args
        let rest = nth_cdr(args, 1)?;
        let msg_args = self.eval_args(rest, env)?;

        self.send_message(receiver, selector_id, msg_args)
    }

    // ── __table ─────────────────────────────────────────────────────

    fn eval_table(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let mut table = MoofTable::new();
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            let key_expr = &cell.car;
            let key_val = self.eval(key_expr, env)?;
            let key_str = key_val.as_str().map_err(|_| {
                MoofError::runtime("__table: key must be a string")
            })?;
            cursor = &cell.cdr;
            let val_expr = nth_car(cursor, 0)?;
            let val = self.eval(val_expr, env)?;
            table.hash.insert(key_str.to_string(), val);
            cursor = match cursor {
                Value::Cons(c) => &c.cdr,
                _ => break,
            };
        }
        Ok(Value::Table(Rc::new(RefCell::new(table))))
    }

    // ── __table-array ───────────────────────────────────────────────

    fn eval_table_array(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let mut table = MoofTable::new();
        let items = self.eval_args(args, env)?;
        table.array = items;
        Ok(Value::Table(Rc::new(RefCell::new(table))))
    }

    // ── __str-interp ────────────────────────────────────────────────

    fn eval_str_interp(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let mut result = String::new();
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            let segment = self.eval(&cell.car, env)?;
            result.push_str(&format!("{segment}"));
            cursor = &cell.cdr;
        }
        Ok(Value::Str(Rc::from(result.as_str())))
    }

    // ═══════════════════════════════════════════════════════════════════
    // Class system
    // ═══════════════════════════════════════════════════════════════════

    /// Get the class of a value.
    pub fn class_of(&self, val: &Value) -> Rc<RefCell<MoofClass>> {
        match val {
            Value::Integer(_) => self.integer_class.clone(),
            Value::Float(_) => self.float_class.clone(),
            Value::Bool(true) => self.true_class.clone(),
            Value::Bool(false) => self.false_class.clone(),
            Value::Nil => self.nil_class.clone(),
            Value::Symbol(_) => self.symbol_class.clone(),
            Value::Str(_) => self.string_class.clone(),
            Value::Cons(_) => self.cons_class.clone(),
            Value::Table(_) => self.table_class.clone(),
            Value::Object(obj) => obj.borrow().class.clone(),
            Value::Closure(_) => self.closure_class.clone(),
        }
    }

    /// Dispatch a message to a receiver through the class hierarchy.
    pub fn send_message(
        &mut self,
        receiver: Value,
        selector: SymId,
        args: Vec<Value>,
    ) -> Result<Value> {
        let class = self.class_of(&receiver);
        let method = class.borrow().lookup(selector);

        if let Some(method_val) = method {
            // Prepend receiver as `self` argument
            let mut full_args = Vec::with_capacity(args.len() + 1);
            full_args.push(receiver);
            full_args.extend(args);

            match method_val {
                Value::Closure(ref c) => call_closure(self, c, full_args),
                _ => Err(MoofError::runtime(format!(
                    "Method '{}' is not callable",
                    self.symbols.name(selector)
                ))),
            }
        } else {
            // For Objects: try field access by selector name
            if let Value::Object(ref obj_rc) = receiver {
                let obj = obj_rc.borrow();
                let class_ref = obj.class.borrow();
                if let Some(idx) = class_ref.field_index(selector) {
                    if let Some(field_val) = obj.get_field(idx) {
                        return Ok(field_val.clone());
                    }
                }
            }

            // Error with Levenshtein suggestion
            let selector_name = self.symbols.name(selector).to_string();
            let suggestion = self.suggest_selector(&receiver, &selector_name);
            let type_name = self.value_type_name(&receiver);
            Err(MoofError::message(
                &type_name,
                &selector_name,
                suggestion.as_deref(),
            ))
        }
    }

    /// Parse a class definition from cons list args.
    fn eval_class(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("class: expected symbol name")
        })?;

        // Check if reopening an existing class
        let existing = env.get(name_id).ok().and_then(|val| {
            if let Value::Object(ref obj) = val {
                // The class is stored as an Object whose class is the metaclass.
                // The metaclass points back to the real class.
                // We find the "real" MoofClass by looking at the metaclass's methods...
                // Actually, we store a reference to the real class in the env.
                Some(obj.clone())
            } else {
                None
            }
        });

        let body = nth_cdr(args, 0)?;

        let mut superclass: Option<Rc<RefCell<MoofClass>>> = None;
        let mut field_names: Vec<SymId> = Vec::new();
        let mut methods: Vec<(SymId, Value)> = Vec::new();

        // Walk body items
        let mut cursor = body;
        while let Value::Cons(cell) = cursor {
            let item = &cell.car;
            if let Value::Cons(inner) = item {
                if let Value::Symbol(kw) = &inner.car {
                    let kw = *kw;

                    // (extends SuperName)
                    if kw == self.known.extends {
                        let super_name = nth_car(&inner.cdr, 0)?;
                        let super_id = super_name.as_symbol().map_err(|_| {
                            MoofError::runtime("class extends: expected symbol")
                        })?;
                        // Look up superclass — it's an Object in env whose class
                        // is the metaclass. We need to find the actual MoofClass.
                        // For bootstrap classes, look them up by name.
                        superclass = self.find_class_by_name(super_id, env);
                    }
                    // (fields f1 f2 ...)
                    else if kw == self.known.fields {
                        let mut fcursor = &inner.cdr;
                        while let Value::Cons(fc) = fcursor {
                            let fid = fc.car.as_symbol().map_err(|_| {
                                MoofError::runtime("class fields: expected symbols")
                            })?;
                            field_names.push(fid);
                            fcursor = &fc.cdr;
                        }
                    }
                    // (method selector (params...) body...)
                    else if kw == self.known.method {
                        let sel = nth_car(&inner.cdr, 0)?;
                        let sel_id = sel.as_symbol().map_err(|_| {
                            MoofError::runtime("class method: expected symbol selector")
                        })?;
                        let params_expr = nth_car(&inner.cdr, 1)?;

                        // Parse params, prepend `self`
                        let (mut params, rest_param) = self.parse_params(params_expr)?;
                        params.insert(0, self.known.self_);

                        let method_body_list = nth_cdr(&inner.cdr, 1)?;
                        let body = self.wrap_body(method_body_list);

                        let closure = Value::Closure(Rc::new(MoofClosure {
                            name: Some(sel_id),
                            params,
                            rest_param,
                            body: ClosureBody::Expr(body),
                            env: env.clone(),
                        }));
                        methods.push((sel_id, closure));
                    }
                }
            }
            cursor = &cell.cdr;
        }

        // If reopening, add methods to existing class
        if let Some(ref obj_rc) = existing {
            let obj = obj_rc.borrow();
            let class_rc = obj.class.borrow();
            // The metaclass points to the real class... we need the real class.
            // For reopening we look up the real class through the stored reference.
            // Actually, for an Object representing a class, we store the real MoofClass
            // as the metaclass's counterpart. Let's find it:
            if let Some(ref mc) = class_rc.metaclass {
                // mc is the class_class; the Object's .class IS the metaclass we created.
                // We need a different approach. Store real class ref in the Object's fields
                // or look it up directly. For now, iterate bootstrap classes and user classes.
                let _ = mc; // suppress unused
            }
            drop(class_rc);
            drop(obj);

            // Simpler approach: find the class by name from bootstrap or existing definitions
            if let Some(real_class) = self.find_class_by_name(name_id, env) {
                let mut klass = real_class.borrow_mut();
                for (sel, closure) in &methods {
                    klass.add_method(*sel, closure.clone());
                }
                return Ok(Value::Nil);
            }
        }

        // Build the new class
        let sup = superclass.unwrap_or_else(|| self.object_class.clone());

        let new_class = Rc::new(RefCell::new(MoofClass {
            name: name_id,
            superclass: Some(sup),
            metaclass: Some(self.class_class.clone()),
            methods: HashMap::new(),
            field_names,
            is_meta: false,
        }));

        // Add methods
        {
            let mut klass = new_class.borrow_mut();
            for (sel, closure) in methods {
                klass.add_method(sel, closure);
            }
        }

        // Create metaclass
        let meta_name = self.symbols.intern(&format!(
            "{} meta",
            self.symbols.name(name_id)
        ));
        let metaclass = Rc::new(RefCell::new(MoofClass {
            name: meta_name,
            superclass: Some(self.class_class.clone()),
            metaclass: Some(self.class_class.clone()),
            methods: HashMap::new(),
            field_names: Vec::new(),
            is_meta: true,
        }));

        new_class.borrow_mut().metaclass = Some(metaclass.clone());

        // Store class as an Object in env (the Object's class is the metaclass)
        let class_obj = MoofObject {
            class: metaclass,
            fields: Vec::new(),
        };
        let class_val = Value::Object(Rc::new(RefCell::new(class_obj)));
        env.define(name_id, class_val.clone(), false);

        // Also store the real MoofClass rc somewhere we can find it.
        // We use a convention: store it under __class:<name> in the env.
        let class_key_name = format!("__class:{}", self.symbols.name(name_id));
        let class_key_id = self.symbols.intern(&class_key_name);
        // We can't store Rc<RefCell<MoofClass>> in env directly; wrap as an Object
        // whose .class IS the class itself (a self-referential trick for lookup).
        let class_holder = MoofObject {
            class: new_class.clone(),
            fields: Vec::new(),
        };
        env.define(
            class_key_id,
            Value::Object(Rc::new(RefCell::new(class_holder))),
            false,
        );

        Ok(class_val)
    }

    // ── type (ADT definitions) ──────────────────────────────────────

    fn eval_type_def(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let type_name = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("type: expected symbol name")
        })?;

        let mut variant_names = Vec::new();
        let mut cursor = nth_cdr(args, 0)?;
        while let Value::Cons(cell) = cursor {
            if let Value::Cons(vc) = &cell.car {
                let vname_id = vc.car.as_symbol().map_err(|_| {
                    MoofError::runtime("type: variant name must be a symbol")
                })?;
                let mut fnames = Vec::new();
                let mut fc = &vc.cdr;
                while let Value::Cons(f) = fc {
                    if let Ok(fid) = f.car.as_symbol() {
                        fnames.push(fid);
                    }
                    fc = &f.cdr;
                }
                variant_names.push(vname_id);

                if fnames.is_empty() {
                    // Singleton variant
                    let klass = Rc::new(RefCell::new(MoofClass {
                        name: vname_id,
                        superclass: Some(self.object_class.clone()),
                        metaclass: Some(self.class_class.clone()),
                        methods: HashMap::new(),
                        field_names: Vec::new(),
                        is_meta: false,
                    }));
                    let instance = MoofObject { class: klass.clone(), fields: Vec::new() };
                    env.define(vname_id, Value::Object(Rc::new(RefCell::new(instance))), false);
                    let key = self.symbols.intern(&format!("__class:{}", self.symbols.name(vname_id)));
                    let holder = MoofObject { class: klass, fields: Vec::new() };
                    env.define(key, Value::Object(Rc::new(RefCell::new(holder))), false);
                } else {
                    // Constructor variant
                    let klass = Rc::new(RefCell::new(MoofClass {
                        name: vname_id,
                        superclass: Some(self.object_class.clone()),
                        metaclass: Some(self.class_class.clone()),
                        methods: HashMap::new(),
                        field_names: fnames,
                        is_meta: false,
                    }));
                    let meta_name = self.symbols.intern(&format!("{} meta", self.symbols.name(vname_id)));
                    let metaclass = Rc::new(RefCell::new(MoofClass {
                        name: meta_name,
                        superclass: Some(self.class_class.clone()),
                        metaclass: Some(self.class_class.clone()),
                        methods: HashMap::new(),
                        field_names: Vec::new(),
                        is_meta: true,
                    }));
                    klass.borrow_mut().metaclass = Some(metaclass.clone());
                    let class_obj = MoofObject { class: metaclass, fields: Vec::new() };
                    env.define(vname_id, Value::Object(Rc::new(RefCell::new(class_obj))), false);
                    let key = self.symbols.intern(&format!("__class:{}", self.symbols.name(vname_id)));
                    let holder = MoofObject { class: klass, fields: Vec::new() };
                    env.define(key, Value::Object(Rc::new(RefCell::new(holder))), false);
                }
            }
            cursor = &cell.cdr;
        }
        self.type_registry.insert(type_name, variant_names);
        Ok(Value::Symbol(type_name))
    }

    /// Try to find a MoofClass Rc by symbol id. Checks bootstrap classes first,
    /// then looks for __class:<name> in the environment.
    fn find_class_by_name(
        &self,
        name_id: SymId,
        env: &Env,
    ) -> Option<Rc<RefCell<MoofClass>>> {
        // Check bootstrap classes
        let bootstrap: &[&Rc<RefCell<MoofClass>>] = &[
            &self.object_class,
            &self.class_class,
            &self.integer_class,
            &self.float_class,
            &self.string_class,
            &self.symbol_class,
            &self.cons_class,
            &self.table_class,
            &self.closure_class,
            &self.true_class,
            &self.false_class,
            &self.nil_class,
            &self.numeric_class,
            &self.error_class,
        ];
        for cls in bootstrap {
            if cls.borrow().name == name_id {
                return Some((*cls).clone());
            }
        }

        // Look up __class:<name> in env
        let class_key_name = format!("__class:{}", self.symbols.name(name_id));
        // We need to intern but we only have &self (immutable). Use the lookup table.
        if let Some(&key_id) = self.symbols.lookup_id(&class_key_name) {
            if let Ok(Value::Object(obj_rc)) = env.get(key_id) {
                return Some(obj_rc.borrow().class.clone());
            }
        }
        None
    }

    // ═══════════════════════════════════════════════════════════════════
    // Macro expansion
    // ═══════════════════════════════════════════════════════════════════

    fn expand_macro(
        &mut self,
        macro_closure: &Value,
        args: &Value,
        env: &Env,
    ) -> Result<Value> {
        let closure = match macro_closure {
            Value::Closure(c) => c,
            _ => return Err(MoofError::runtime("defmacro: expected closure")),
        };

        // Bind params to UNEVALUATED args (they're already Values — data IS AST)
        let macro_env = closure.env.child();
        let arg_vec = list_to_vec(args);

        for (i, &param_id) in closure.params.iter().enumerate() {
            let arg = arg_vec.get(i).cloned().unwrap_or(Value::Nil);
            macro_env.define(param_id, arg, false);
        }

        // Handle rest param
        if let Some(rest_id) = closure.rest_param {
            let rest_start = closure.params.len();
            let rest_items: Vec<Value> = arg_vec[rest_start..].to_vec();
            macro_env.define(rest_id, Value::from_slice(&rest_items), false);
        }

        // Eval macro body in the macro's captured env (with bound args)
        let expanded = match &closure.body {
            ClosureBody::Expr(body) => self.eval(body, &macro_env)?,
            ClosureBody::Native(f) => f(self, arg_vec)?,
        };

        // Eval the expanded result in the CALLER's env
        self.eval(&expanded, env)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Pattern matching
    // ═══════════════════════════════════════════════════════════════════

    /// Try to match a pattern against a value, binding variables in env.
    /// Returns true on match. Bindings are added to env directly.
    pub fn match_pattern(
        &mut self,
        pattern: &Value,
        value: &Value,
        env: &Env,
    ) -> bool {
        match pattern {
            // Wildcard
            Value::Symbol(id) if *id == self.known.underscore => true,

            // Variable binding
            Value::Symbol(id) => {
                // Check if it starts with an uppercase letter — treat as constructor reference
                let name = self.symbols.name(*id);
                if name.starts_with(char::is_uppercase) {
                    // Constructor pattern without fields: match class name
                    if let Value::Object(obj_rc) = value {
                        let obj = obj_rc.borrow();
                        return obj.class.borrow().name == *id;
                    }
                    return false;
                }
                env.define(*id, value.clone(), false);
                true
            }

            // Literal matching
            Value::Integer(_) | Value::Float(_) | Value::Bool(_) | Value::Nil | Value::Str(_) => {
                pattern == value
            }

            // Cons pattern
            Value::Cons(cell) => {
                // Check for constructor pattern: (ConstructorName field_patterns...)
                if let Value::Symbol(ctor_id) = &cell.car {
                    let name = self.symbols.name(*ctor_id);
                    if name.starts_with(char::is_uppercase) {
                        // Constructor pattern
                        if let Value::Object(obj_rc) = value {
                            let obj = obj_rc.borrow();
                            if obj.class.borrow().name != *ctor_id {
                                return false;
                            }
                            // Destructure fields
                            let field_patterns = list_to_vec(&cell.cdr);
                            for (i, pat) in field_patterns.iter().enumerate() {
                                match obj.get_field(i) {
                                    Some(field_val) => {
                                        if !self.match_pattern(pat, field_val, env) {
                                            return false;
                                        }
                                    }
                                    None => return false,
                                }
                            }
                            return true;
                        }
                        return false;
                    }
                }

                // List destructuring: match car against car, cdr against cdr
                if let Value::Cons(val_cell) = value {
                    self.match_pattern(&cell.car, &val_cell.car, env)
                        && self.match_pattern(&cell.cdr, &val_cell.cdr, env)
                } else {
                    false
                }
            }

            // Other patterns don't match
            _ => false,
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Invocation
    // ═══════════════════════════════════════════════════════════════════

    /// Call a closure or construct an object from a class value.
    pub fn invoke(&mut self, callee: Value, args: Vec<Value>) -> Result<Value> {
        match callee {
            Value::Closure(ref c) => call_closure(self, c, args),
            Value::Object(ref obj_rc) => {
                // If the callee is a class-as-Object, construct an instance.
                // The class-as-Object's .class is the metaclass; the real class
                // is found through find_class_by_name or the __class: convention.
                let obj = obj_rc.borrow();
                let metaclass = obj.class.borrow();
                if metaclass.is_meta {
                    // Find the real class
                    let class_name = metaclass.name; // "<Name> meta"
                    drop(metaclass);
                    drop(obj);

                    // Derive the real class name by stripping " meta"
                    let meta_str = self.symbols.name(class_name).to_string();
                    let real_name = meta_str.strip_suffix(" meta").unwrap_or(&meta_str);
                    let real_id = self.symbols.intern(real_name);

                    if let Some(real_class) = self.find_class_by_name(real_id, &self.global_env.clone()) {
                        let all_fields = real_class.borrow().all_field_names();
                        if args.len() != all_fields.len() {
                            return Err(MoofError::arity(
                                &all_fields.len().to_string(),
                                args.len(),
                                Some(real_name),
                            ));
                        }
                        let instance = MoofObject {
                            class: real_class,
                            fields: args,
                        };
                        return Ok(Value::Object(Rc::new(RefCell::new(instance))));
                    }
                    return Err(MoofError::runtime(format!(
                        "Cannot construct: class '{}' not found",
                        real_name
                    )));
                }
                drop(metaclass);
                drop(obj);
                Err(MoofError::runtime(format!(
                    "Cannot call {}: not a function",
                    callee.type_name()
                )))
            }
            _ => Err(MoofError::runtime(format!(
                "Cannot call {}: not a function",
                callee.type_name()
            ))),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Helpers
    // ═══════════════════════════════════════════════════════════════════

    /// Evaluate each element in a cons list, collecting results.
    pub fn eval_args(&mut self, args: &Value, env: &Env) -> Result<Vec<Value>> {
        let mut result = Vec::new();
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            // Skip keyword labels (symbols ending with ':') in function calls
            if let Value::Symbol(id) = &cell.car {
                let name = self.symbols.name(*id);
                if name.ends_with(':') && !name.starts_with("__") {
                    cursor = &cell.cdr;
                    continue;
                }
            }
            result.push(self.eval(&cell.car, env)?);
            cursor = &cell.cdr;
        }
        Ok(result)
    }

    /// Evaluate a sequence of body expressions (cons list), return last.
    fn eval_body(&mut self, body: &Value, env: &Env) -> Result<Value> {
        let mut result = Value::Nil;
        let mut cursor = body;
        while let Value::Cons(cell) = cursor {
            result = self.eval(&cell.car, env)?;
            cursor = &cell.cdr;
        }
        Ok(result)
    }

    /// Parse a params list into (positional_params, optional_rest_param).
    /// Supports `(a b c)` and `(a b . rest)` via & or dot notation.
    fn parse_params(&self, params: &Value) -> Result<(Vec<SymId>, Option<SymId>)> {
        let mut positional = Vec::new();
        let mut rest_param = None;
        let mut cursor = params;

        while let Value::Cons(cell) = cursor {
            let id = cell.car.as_symbol().map_err(|_| {
                MoofError::runtime("Expected symbol in parameter list")
            })?;

            // Check for rest param marker: a symbol named "&" or "."
            let name = self.symbols.name(id);
            if name == "&" || name == "." {
                // Next element is the rest param
                let rest_expr = nth_car(&cell.cdr, 0)?;
                rest_param = Some(rest_expr.as_symbol().map_err(|_| {
                    MoofError::runtime("Expected symbol after & in parameter list")
                })?);
                break;
            }

            positional.push(id);
            cursor = &cell.cdr;
        }

        // Handle improper list tail as rest param: (a b . rest)
        if rest_param.is_none() {
            if let Value::Symbol(id) = cursor {
                rest_param = Some(*id);
            }
        }

        Ok((positional, rest_param))
    }

    /// Wrap a body cons list in a (do ...) form if it has multiple expressions,
    /// or return the single expression directly.
    fn wrap_body(&self, body: &Value) -> Value {
        match body {
            Value::Nil => Value::Nil,
            Value::Cons(cell) => {
                if cell.cdr.is_nil() {
                    // Single expression
                    cell.car.clone()
                } else {
                    // Multiple expressions: wrap in (do ...)
                    Value::cons(Value::Symbol(self.known.do_), body.clone())
                }
            }
            other => other.clone(),
        }
    }

    /// Get a human-readable type name for error messages.
    fn value_type_name(&self, val: &Value) -> String {
        match val {
            Value::Object(obj) => {
                let obj = obj.borrow();
                let name_id = obj.class.borrow().name;
                self.symbols.name(name_id).to_string()
            }
            _ => val.type_name().to_string(),
        }
    }

    /// Suggest a similar selector name using Levenshtein distance.
    fn suggest_selector(&self, receiver: &Value, selector: &str) -> Option<String> {
        let class = self.class_of(receiver);
        let methods = &class.borrow().methods;
        let mut best: Option<(usize, String)> = None;

        for &method_id in methods.keys() {
            let method_name = self.symbols.name(method_id);
            let dist = levenshtein(selector, method_name);
            if dist <= 3 {
                if best.is_none() || dist < best.as_ref().unwrap().0 {
                    best = Some((dist, method_name.to_string()));
                }
            }
        }

        // Also check superclass chain
        let mut sup = class.borrow().superclass.clone();
        while let Some(super_class) = sup {
            for &method_id in super_class.borrow().methods.keys() {
                let method_name = self.symbols.name(method_id);
                let dist = levenshtein(selector, method_name);
                if dist <= 3 {
                    if best.is_none() || dist < best.as_ref().unwrap().0 {
                        best = Some((dist, method_name.to_string()));
                    }
                }
            }
            let next = super_class.borrow().superclass.clone();
            sup = next;
        }

        best.map(|(_, name)| name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Free functions
// ═══════════════════════════════════════════════════════════════════════

/// Call a closure with the given arguments.
pub fn call_closure(
    interp: &mut Interpreter,
    closure: &MoofClosure,
    args: Vec<Value>,
) -> Result<Value> {
    match &closure.body {
        ClosureBody::Native(f) => f(interp, args),
        ClosureBody::Expr(body) => {
            let n_params = closure.params.len();
            let has_rest = closure.rest_param.is_some();

            // Arity check
            if has_rest {
                if args.len() < n_params {
                    let name = closure.name.map(|id| interp.symbols.name(id).to_string());
                    return Err(MoofError::arity(
                        &format!("at least {n_params}"),
                        args.len(),
                        name.as_deref(),
                    ));
                }
            } else if args.len() != n_params {
                let name = closure.name.map(|id| interp.symbols.name(id).to_string());
                return Err(MoofError::arity(
                    &n_params.to_string(),
                    args.len(),
                    name.as_deref(),
                ));
            }

            // Create child env from closure's captured env
            let call_env = closure.env.child();

            // Bind positional params
            for (i, &param_id) in closure.params.iter().enumerate() {
                let val = args.get(i).cloned().unwrap_or(Value::Nil);
                call_env.define(param_id, val, false);
            }

            // Bind rest param
            if let Some(rest_id) = closure.rest_param {
                let rest_items: Vec<Value> = args[n_params..].to_vec();
                call_env.define(rest_id, Value::from_slice(&rest_items), false);
            }

            // If first param is `self` and the value is an Object, bind fields
            if !closure.params.is_empty() && closure.params[0] == interp.known.self_ {
                if let Some(self_val) = args.first() {
                    if let Value::Object(obj_rc) = self_val {
                        let obj = obj_rc.borrow();
                        let class = obj.class.borrow();
                        for (i, &field_id) in class.field_names.iter().enumerate() {
                            if let Some(val) = obj.get_field(i) {
                                call_env.define(field_id, val.clone(), false);
                            }
                        }
                    }
                }
            }

            interp.eval(body, &call_env)
        }
    }
}

// ── Cons list navigation helpers ────────────────────────────────────

/// Get the nth car from a cons chain. 0-indexed.
fn nth_car(list: &Value, n: usize) -> Result<&Value> {
    let mut cursor = list;
    for _ in 0..n {
        match cursor {
            Value::Cons(cell) => cursor = &cell.cdr,
            _ => {
                return Err(MoofError::runtime(format!(
                    "Expected at least {} arguments",
                    n + 1
                )));
            }
        }
    }
    match cursor {
        Value::Cons(cell) => Ok(&cell.car),
        _ => Err(MoofError::runtime(format!(
            "Expected at least {} arguments",
            n + 1
        ))),
    }
}

/// Get the cdr after nth element. nth_cdr(list, 0) returns cdr of list.
fn nth_cdr(list: &Value, n: usize) -> Result<&Value> {
    let mut cursor = list;
    for _ in 0..n {
        match cursor {
            Value::Cons(cell) => cursor = &cell.cdr,
            _ => {
                return Err(MoofError::runtime("List too short"));
            }
        }
    }
    match cursor {
        Value::Cons(cell) => Ok(&cell.cdr),
        _ => Err(MoofError::runtime("List too short")),
    }
}

/// Collect a cons list into a Vec (non-recursive, does not eval).
pub fn list_to_vec(val: &Value) -> Vec<Value> {
    cons::cons_to_vec(val)
}

/// Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}
