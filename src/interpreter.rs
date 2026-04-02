use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::cons;
use crate::environment::Env;
use crate::error::{MoofError, Result};
use crate::symbol::{KnownSymbols, SymId, SymbolTable};
use crate::value::{
    ClosureBody, MoofClass, MoofClosure, MoofObject, MoofTable, NativeFn, Value,
};

// ═══════════════════════════════════════════════════════════════════════
// Tail-call optimization
// ═══════════════════════════════════════════════════════════════════════

pub enum Eval {
    Val(Value),
    TailCall { func: Value, args: Vec<Value> },
}

// ═══════════════════════════════════════════════════════════════════════
// Interpreter
// ═══════════════════════════════════════════════════════════════════════

pub struct Interpreter {
    pub symbols: SymbolTable,
    pub known: KnownSymbols,
    pub global_env: Env,
    pub macro_registry: HashMap<SymId, Value>,
    pub type_registry: HashMap<SymId, Vec<SymId>>,
    pub protocol_registry: HashMap<SymId, Vec<SymId>>,
    pub trait_registry: HashMap<SymId, HashMap<SymId, Value>>,
    pub module_registry: HashMap<SymId, HashMap<SymId, Value>>,
    pub primitive_registry: HashMap<SymId, NativeFn>,

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
    pub range_class: Rc<RefCell<MoofClass>>,
    pub true_class: Rc<RefCell<MoofClass>>,
    pub false_class: Rc<RefCell<MoofClass>>,
    pub nil_class: Rc<RefCell<MoofClass>>,
    pub numeric_class: Rc<RefCell<MoofClass>>,
    pub error_class: Rc<RefCell<MoofClass>>,
    pub syntax_error_class: Rc<RefCell<MoofClass>>,
    pub runtime_error_class: Rc<RefCell<MoofClass>>,
    pub name_error_class: Rc<RefCell<MoofClass>>,
    pub type_error_class: Rc<RefCell<MoofClass>>,
    pub arity_error_class: Rc<RefCell<MoofClass>>,
    pub message_error_class: Rc<RefCell<MoofClass>>,
    pub io_error_class: Rc<RefCell<MoofClass>>,

    pub source_locs: HashMap<usize, (usize, usize)>,

    /// Set by send_message before calling a method, so setup_call_env can
    /// bind __current_class for super sends.
    pub current_method_class: Option<Rc<RefCell<MoofClass>>>,

    // ── Class object registry ──────────────────────────────────────
    /// Maps class name SymId → the Value::Object representing that class.
    /// Used by class_object_of() to return class objects instead of strings.
    pub class_objects: HashMap<SymId, Value>,

    // ── Import system ──────────────────────────────────────────────
    /// Path of the file currently being evaluated (for relative require resolution).
    pub current_file: Option<PathBuf>,
    /// Cache of already-loaded files: canonical path -> last result.
    pub loaded_modules: HashMap<PathBuf, Value>,
    /// Stack of files currently being loaded (for circular dependency detection).
    pub loading_stack: Vec<PathBuf>,
    /// Base directory for stdlib files (set once at startup).
    pub stdlib_dir: Option<PathBuf>,

    // ── Baseline snapshot (for image save) ─────────────────────────
    /// (class_name_id, method_selector_id) pairs from after stdlib load.
    pub baseline_methods: HashSet<(SymId, SymId)>,
    /// Global binding SymIds from after stdlib load.
    pub baseline_globals: HashSet<SymId>,
    /// Macro names from after stdlib load.
    pub baseline_macros: HashSet<SymId>,
    /// Type (ADT) names from after stdlib load.
    pub baseline_types: HashSet<SymId>,
    /// Protocol names from after stdlib load.
    pub baseline_protocols: HashSet<SymId>,
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

        let range_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Range"),
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

        let message_field = symbols.intern("message");

        let error_class = Rc::new(RefCell::new(MoofClass {
            name: symbols.intern("Error"),
            superclass: Some(object_class.clone()),
            metaclass: Some(class_class.clone()),
            methods: HashMap::new(),
            field_names: vec![message_field],
            is_meta: false,
        }));

        let make_error_subclass = |symbols: &mut SymbolTable, name: &str, parent: &Rc<RefCell<MoofClass>>| -> Rc<RefCell<MoofClass>> {
            Rc::new(RefCell::new(MoofClass {
                name: symbols.intern(name),
                superclass: Some(parent.clone()),
                metaclass: Some(class_class.clone()),
                methods: HashMap::new(),
                field_names: Vec::new(),
                is_meta: false,
            }))
        };

        let syntax_error_class = make_error_subclass(&mut symbols, "SyntaxError", &error_class);
        let runtime_error_class = make_error_subclass(&mut symbols, "RuntimeError", &error_class);
        let name_error_class = make_error_subclass(&mut symbols, "NameError", &runtime_error_class);
        let type_error_class = make_error_subclass(&mut symbols, "TypeError", &runtime_error_class);
        let arity_error_class = make_error_subclass(&mut symbols, "ArityError", &runtime_error_class);
        let message_error_class = make_error_subclass(&mut symbols, "MessageError", &runtime_error_class);
        let io_error_class = make_error_subclass(&mut symbols, "IOError", &error_class);

        let mut interp = Interpreter {
            symbols,
            known,
            global_env: Env::new(),
            macro_registry: HashMap::new(),
            type_registry: HashMap::new(),
            protocol_registry: HashMap::new(),
            trait_registry: HashMap::new(),
            module_registry: HashMap::new(),
            primitive_registry: HashMap::new(),
            object_class,
            class_class,
            integer_class,
            float_class,
            string_class,
            symbol_class,
            cons_class,
            table_class,
            closure_class,
            range_class,
            true_class,
            false_class,
            nil_class,
            numeric_class,
            error_class,
            syntax_error_class,
            runtime_error_class,
            name_error_class,
            type_error_class,
            arity_error_class,
            message_error_class,
            io_error_class,
            source_locs: HashMap::new(),
            current_method_class: None,
            class_objects: HashMap::new(),
            current_file: None,
            loaded_modules: HashMap::new(),
            loading_stack: Vec::new(),
            stdlib_dir: None,
            baseline_methods: HashSet::new(),
            baseline_globals: HashSet::new(),
            baseline_macros: HashSet::new(),
            baseline_types: HashSet::new(),
            baseline_protocols: HashSet::new(),
        };

        // Install built-in functions
        crate::builtins::install(&mut interp);

        // Install primitive FFI registry
        crate::primitives::install(&mut interp);

        // Register bootstrap classes in global env so (class Foo ...) can reopen them
        interp.register_bootstrap_classes();

        interp
    }

    /// Register all bootstrap classes in the global env so that
    /// `(class Object ...)` etc. in stdlib files can reopen them.
    fn register_bootstrap_classes(&mut self) {
        let classes: Vec<Rc<RefCell<MoofClass>>> = vec![
            self.object_class.clone(),
            self.class_class.clone(),
            self.integer_class.clone(),
            self.float_class.clone(),
            self.string_class.clone(),
            self.symbol_class.clone(),
            self.cons_class.clone(),
            self.table_class.clone(),
            self.closure_class.clone(),
            self.range_class.clone(),
            self.true_class.clone(),
            self.false_class.clone(),
            self.nil_class.clone(),
            self.numeric_class.clone(),
            self.error_class.clone(),
            self.syntax_error_class.clone(),
            self.runtime_error_class.clone(),
            self.name_error_class.clone(),
            self.type_error_class.clone(),
            self.arity_error_class.clone(),
            self.message_error_class.clone(),
            self.io_error_class.clone(),
        ];

        for class_rc in classes {
            let name_id = class_rc.borrow().name;
            let metaclass = class_rc.borrow().metaclass.clone()
                .unwrap_or_else(|| self.class_class.clone());
            let class_obj = Value::Object(Rc::new(RefCell::new(MoofObject {
                class: metaclass,
                fields: Vec::new(),
            })));
            self.global_env.define(name_id, class_obj.clone(), false);
            self.class_objects.insert(name_id, class_obj);
        }
    }

    /// Snapshot the current state as the baseline (called after load_prelude).
    /// Everything present at this point is "stdlib"; anything added later is "user".
    pub fn snapshot_baseline(&mut self) {
        // Snapshot methods on all known classes
        let all_classes = self.all_type_classes();
        for class_rc in &all_classes {
            let class = class_rc.borrow();
            let class_name = class.name;
            for &sel in class.methods.keys() {
                self.baseline_methods.insert((class_name, sel));
            }
        }

        // Also snapshot user-defined classes from class_objects
        for (&name_id, _) in &self.class_objects {
            if let Some(class_rc) = self.find_class_by_name(name_id, &self.global_env.clone()) {
                let class = class_rc.borrow();
                for &sel in class.methods.keys() {
                    self.baseline_methods.insert((name_id, sel));
                }
            }
        }

        // Snapshot global bindings
        for (id, _) in self.global_env.bindings() {
            self.baseline_globals.insert(id);
        }

        // Snapshot macros
        for &id in self.macro_registry.keys() {
            self.baseline_macros.insert(id);
        }

        // Snapshot types
        for &id in self.type_registry.keys() {
            self.baseline_types.insert(id);
        }

        // Snapshot protocols
        for &id in self.protocol_registry.keys() {
            self.baseline_protocols.insert(id);
        }
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

    /// Parse source into a list of AST expressions (for use by the bytecode compiler).
    pub fn parse_source(&mut self, source: &str, _filename: &str) -> Result<Vec<Value>> {
        let tokens = crate::lexer::Lexer::new(source).tokenize()?;
        crate::parser::Parser::new(tokens, &mut self.symbols).parse_program()
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

    /// Format a value with symbol name resolution.
    pub fn display_value(&self, val: &Value) -> String {
        match val {
            Value::Symbol(id) => self.symbols.try_name(*id).unwrap_or("?").to_string(),
            Value::Cons(_) => self.display_cons(val),
            Value::Object(_) => {
                // Check if this is a class object
                if let Ok(real_class) = self.real_class_from_class_object(val) {
                    let name = self.symbols.name(real_class.borrow().name);
                    return name.to_string();
                }
                format!("{}", val)
            }
            _ => format!("{}", val),
        }
    }

    /// Format a value with symbol resolution, quoting strings.
    pub fn inspect_value(&self, val: &Value) -> String {
        match val {
            Value::Str(s) => format!("{:?}", &**s),
            Value::Symbol(id) => self.symbols.try_name(*id).unwrap_or("?").to_string(),
            Value::Cons(_) => self.display_cons(val),
            Value::Object(obj) => {
                // Check if this is a class object
                if let Ok(real_class) = self.real_class_from_class_object(val) {
                    let name = self.symbols.name(real_class.borrow().name);
                    return name.to_string();
                }
                // Regular object: show class name + fields
                let obj = obj.borrow();
                let class_name = self.symbols.try_name(obj.class.borrow().name)
                    .unwrap_or("?");
                if obj.fields.is_empty() {
                    format!("<{class_name}>")
                } else {
                    let fields: Vec<String> = obj.fields.iter()
                        .map(|f| self.display_value(f))
                        .collect();
                    format!("<{class_name} {}>", fields.join(" "))
                }
            }
            _ => format!("{}", val),
        }
    }

    fn display_cons(&self, val: &Value) -> String {
        let mut out = String::from("(");
        let mut cur = val;
        let mut first = true;
        while let Value::Cons(cell) = cur {
            if !first { out.push(' '); }
            first = false;
            out.push_str(&self.display_value(&cell.car));
            cur = &cell.cdr;
        }
        if !cur.is_nil() {
            out.push_str(" . ");
            out.push_str(&self.display_value(cur));
        }
        out.push(')');
        out
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

    pub fn all_type_classes(&self) -> Vec<Rc<RefCell<MoofClass>>> {
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
            self.range_class.clone(),
            self.true_class.clone(),
            self.false_class.clone(),
            self.nil_class.clone(),
            self.error_class.clone(),
            self.syntax_error_class.clone(),
            self.runtime_error_class.clone(),
            self.name_error_class.clone(),
            self.type_error_class.clone(),
            self.arity_error_class.clone(),
            self.message_error_class.clone(),
            self.io_error_class.clone(),
        ]
    }

    /// Create a Moof error object (instance of an Error subclass).
    pub fn make_error_object(&self, class: &Rc<RefCell<MoofClass>>, message: &str) -> Value {
        Value::Object(Rc::new(RefCell::new(MoofObject {
            class: class.clone(),
            fields: vec![Value::Str(Rc::from(message))],
        })))
    }

    /// Map an ErrorKind to the appropriate error class.
    pub fn error_class_for_kind(&self, kind: &crate::error::ErrorKind) -> Rc<RefCell<MoofClass>> {
        use crate::error::ErrorKind;
        match kind {
            ErrorKind::Syntax => self.syntax_error_class.clone(),
            ErrorKind::Runtime => self.runtime_error_class.clone(),
            ErrorKind::Name => self.name_error_class.clone(),
            ErrorKind::Type => self.type_error_class.clone(),
            ErrorKind::Arity => self.arity_error_class.clone(),
            ErrorKind::Message => self.message_error_class.clone(),
            ErrorKind::IO => self.io_error_class.clone(),
        }
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
            | Value::Closure(_)
            | Value::Range(_) => Ok(expr.clone()),

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
                    if id == k.super_send {
                        return self.eval_super_send(cdr, env);
                    }
                    if id == k.__primitive {
                        return self.eval_primitive(cdr, env);
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
                        return self.eval_protocol(cdr, env);
                    }
                    if id == k.trait_ {
                        return self.eval_trait(cdr, env);
                    }
                    if id == k.module {
                        return self.eval_module(cdr, env);
                    }
                    if id == k.use_ {
                        return self.eval_use(cdr, env);
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
    // Tail-position evaluation (returns Eval instead of Value)
    // ═══════════════════════════════════════════════════════════════════

    pub fn eval_tail(&mut self, expr: &Value, env: &Env) -> Result<Eval> {
        match expr {
            // Self-evaluating
            Value::Integer(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Nil
            | Value::Str(_)
            | Value::Table(_)
            | Value::Object(_)
            | Value::Closure(_)
            | Value::Range(_) => Ok(Eval::Val(expr.clone())),

            // Symbol lookup
            Value::Symbol(id) => {
                let val = env.get(*id).map_err(|_| {
                    let name = self.symbols.name(*id);
                    MoofError::name(format!("Undefined variable: {name}"))
                })?;
                Ok(Eval::Val(val))
            }

            // Compound form
            Value::Cons(cell) => {
                let car = &cell.car;
                let cdr = &cell.cdr;

                if let Value::Symbol(id) = car {
                    let id = *id;
                    let k = &self.known;

                    // Tail-position special forms
                    if id == k.if_ {
                        return self.eval_tail_if(cdr, env);
                    }
                    if id == k.do_ {
                        return self.eval_tail_do(cdr, env);
                    }
                    if id == k.let_ {
                        return self.eval_tail_let(cdr, env);
                    }
                    if id == k.match_ {
                        return self.eval_tail_match(cdr, env);
                    }
                    if id == k.cond {
                        return self.eval_tail_cond(cdr, env);
                    }
                    if id == k.and {
                        return self.eval_tail_and(cdr, env);
                    }
                    if id == k.or {
                        return self.eval_tail_or(cdr, env);
                    }
                    if id == k.try_ {
                        return self.eval_tail_try(cdr, env);
                    }

                    // Non-tail special forms: delegate to eval, wrap in Val
                    if id == k.define
                        || id == k.lambda
                        || id == k.fn_
                        || id == k.set_bang
                        || id == k.quote
                        || id == k.quasiquote
                        || id == k.defmacro
                        || id == k.require
                        || id == k.send
                        || id == k.super_send
                        || id == k.__primitive
                        || id == k.table
                        || id == k.table_array
                        || id == k.str_interp
                        || id == k.type_
                        || id == k.protocol
                        || id == k.trait_
                        || id == k.module
                        || id == k.use_
                        || id == k.class
                    {
                        return Ok(Eval::Val(self.eval(expr, env)?));
                    }

                    // Macro check
                    if let Some(macro_val) = self.macro_registry.get(&id).cloned() {
                        let expanded = self.expand_macro_to_ast(&macro_val, cdr)?;
                        return self.eval_tail(&expanded, env);
                    }
                }

                // General function call: eval head and args, return TailCall for closures
                let callee = self.eval(car, env)?;
                let args = self.eval_args(cdr, env)?;

                match &callee {
                    Value::Closure(c) => match &c.body {
                        ClosureBody::Native(f) => Ok(Eval::Val(f(self, args)?)),
                        ClosureBody::Expr(_) | ClosureBody::Bytecode(_) => Ok(Eval::TailCall { func: callee.clone(), args }),
                    },
                    // For non-closures (e.g. class constructors), invoke normally
                    _ => Ok(Eval::Val(self.invoke(callee, args)?)),
                }
            }
        }
    }

    // ── Tail-position special form helpers ──────────────────────────

    fn eval_tail_if(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let cond_expr = nth_car(args, 0)?;
        let then_expr = nth_car(args, 1)?;
        let cond_val = self.eval(cond_expr, env)?;
        if cond_val.is_truthy() {
            self.eval_tail(then_expr, env)
        } else {
            match nth_car(args, 2) {
                Ok(else_expr) => self.eval_tail(else_expr, env),
                Err(_) => Ok(Eval::Val(Value::Nil)),
            }
        }
    }

    fn eval_tail_do(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            if cell.cdr.is_nil() {
                // Last expression: tail position
                return self.eval_tail(&cell.car, env);
            }
            self.eval(&cell.car, env)?;
            cursor = &cell.cdr;
        }
        Ok(Eval::Val(Value::Nil))
    }

    fn eval_tail_let(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let bindings_expr = nth_car(args, 0)?;
        let body_list = nth_cdr(args, 0)?;

        let let_env = env.child();

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

        self.eval_tail_body(body_list, &let_env)
    }

    fn eval_tail_match(&mut self, args: &Value, env: &Env) -> Result<Eval> {
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
                return self.eval_tail(body, &match_env);
            }
            cursor = &cell.cdr;
        }
        Ok(Eval::Val(Value::Nil))
    }

    fn eval_tail_cond(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            let clause = &cell.car;
            let test_expr = nth_car(clause, 0)?;
            let body_expr = nth_car(clause, 1)?;

            if let Value::Symbol(id) = test_expr {
                if *id == self.known.else_ {
                    return self.eval_tail(body_expr, env);
                }
            }

            let test_val = self.eval(test_expr, env)?;
            if test_val.is_truthy() {
                return self.eval_tail(body_expr, env);
            }
            cursor = &cell.cdr;
        }
        Ok(Eval::Val(Value::Nil))
    }

    fn eval_tail_and(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let a = nth_car(args, 0)?;
        let a_val = self.eval(a, env)?;
        if !a_val.is_truthy() {
            return Ok(Eval::Val(Value::Bool(false)));
        }
        match nth_car(args, 1) {
            Ok(b) => self.eval_tail(b, env),
            Err(_) => Ok(Eval::Val(a_val)),
        }
    }

    fn eval_tail_or(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let a = nth_car(args, 0)?;
        let a_val = self.eval(a, env)?;
        if a_val.is_truthy() {
            return Ok(Eval::Val(a_val));
        }
        match nth_car(args, 1) {
            Ok(b) => self.eval_tail(b, env),
            Err(_) => Ok(Eval::Val(a_val)),
        }
    }

    fn eval_tail_try(&mut self, args: &Value, env: &Env) -> Result<Eval> {
        let body = nth_car(args, 0)?;
        match self.eval_tail(body, env) {
            Ok(eval_result) => {
                // Need to resolve TailCall before we can say "no error"
                match eval_result {
                    Eval::Val(_) => Ok(eval_result),
                    Eval::TailCall { func, args: tc_args } => {
                        // Resolve the tail call, then wrap result
                        match self.invoke(func, tc_args) {
                            Ok(v) => Ok(Eval::Val(v)),
                            Err(e) => self.handle_try_catch(e, args, env),
                        }
                    }
                }
            }
            Err(e) => self.handle_try_catch(e, args, env),
        }
    }

    fn handle_try_catch(&mut self, e: MoofError, args: &Value, env: &Env) -> Result<Eval> {
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
                    return Ok(Eval::Val(self.eval(catch_body, &catch_env)?));
                }
            }
        }
        Err(e)
    }

    /// Like eval_body but returns Eval — last expression is in tail position.
    fn eval_tail_body(&mut self, body: &Value, env: &Env) -> Result<Eval> {
        let mut cursor = body;
        while let Value::Cons(cell) = cursor {
            if cell.cdr.is_nil() {
                return self.eval_tail(&cell.car, env);
            }
            self.eval(&cell.car, env)?;
            cursor = &cell.cdr;
        }
        Ok(Eval::Val(Value::Nil))
    }

    /// Expand a macro but return the expanded AST without evaluating it.
    pub fn expand_macro_to_ast(
        &mut self,
        macro_closure: &Value,
        args: &Value,
    ) -> Result<Value> {
        let closure = match macro_closure {
            Value::Closure(c) => c,
            _ => return Err(MoofError::runtime("defmacro: expected closure")),
        };

        let macro_env = closure.env.child();
        let arg_vec = list_to_vec(args);

        for (i, &param_id) in closure.params.iter().enumerate() {
            let arg = arg_vec.get(i).cloned().unwrap_or(Value::Nil);
            macro_env.define(param_id, arg, false);
        }

        if let Some(rest_id) = closure.rest_param {
            let rest_start = closure.params.len();
            let rest_items: Vec<Value> = arg_vec[rest_start..].to_vec();
            macro_env.define(rest_id, Value::from_slice(&rest_items), false);
        }

        match &closure.body {
            ClosureBody::Expr(body) => self.eval(body, &macro_env),
            ClosureBody::Native(f) => f(self, arg_vec),
            ClosureBody::Bytecode(_) => Err(MoofError::runtime("macros cannot be bytecode-compiled")),
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

                // Parse params (with defaults)
                let (params, rest_param, defaults) = self.parse_params_with_defaults(params_list)?;

                // Evaluate default expressions now (eager, like Python)
                let eval_defaults: Vec<Option<Value>> = defaults.into_iter().map(|d| {
                    d.map(|expr| self.eval(&expr, env).unwrap_or(Value::Nil))
                }).collect();

                // Wrap body in (do ...) if multiple expressions
                let body = self.wrap_body(body_list);

                let closure = Rc::new(MoofClosure {
                    name: Some(name_id),
                    params,
                    rest_param,
                    defaults: eval_defaults,
                    body: ClosureBody::Expr(body),
                    env: env.clone(),
                    upvalues: Vec::new(),
                });
                let val = Value::Closure(closure);
                env.define(name_id, val.clone(), true);
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
                env.define(*name_id, val.clone(), true);
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

        let (params, rest_param, defaults) = self.parse_params_with_defaults(params_expr)?;

        // Evaluate default expressions eagerly
        let eval_defaults: Vec<Option<Value>> = defaults.into_iter().map(|d| {
            d.map(|expr| self.eval(&expr, env).unwrap_or(Value::Nil))
        }).collect();

        let body = self.wrap_body(body_list);

        Ok(Value::Closure(Rc::new(MoofClosure {
            name: None,
            params,
            rest_param,
            defaults: eval_defaults,
            body: ClosureBody::Expr(body),
            env: env.clone(),
                            upvalues: Vec::new(),
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

        // If we're inside a method (self is bound) and the variable is a field,
        // also update the object's actual field so mutations persist.
        if let Ok(self_val) = env.get(self.known.self_) {
            if let Value::Object(ref obj_rc) = self_val {
                let obj = obj_rc.borrow();
                let class = obj.class.borrow();
                let all_fields = class.all_field_names();
                if let Some(idx) = all_fields.iter().position(|&n| n == name_id) {
                    drop(class);
                    drop(obj);
                    obj_rc.borrow_mut().set_field(idx, val.clone());
                }
            }
        }

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
                            let error_val = if let Some(obj) = e.error_object {
                                obj
                            } else {
                                let cls = self.error_class_for_kind(&e.kind);
                                self.make_error_object(&cls, &e.message)
                            };
                            catch_env.define(var_id, error_val, false);
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
            defaults: Vec::new(),
            body: ClosureBody::Expr(body),
            env: env.clone(),
            upvalues: Vec::new(),
        }));

        self.macro_registry.insert(name_id, closure);
        Ok(Value::Symbol(name_id))
    }

    // ── __primitive ─────────────────────────────────────────────────

    fn eval_primitive(&mut self, args: &Value, env: &Env) -> Result<Value> {
        // (__primitive name arg1 arg2 ...)
        // name is a symbol (not evaluated as a variable), args are evaluated
        let name_expr = nth_car(args, 0)?;
        let name_id = match name_expr {
            Value::Symbol(id) => *id,
            _ => return Err(MoofError::runtime("__primitive: first argument must be a symbol")),
        };

        // Look up the primitive
        let prim_fn = match self.primitive_registry.get(&name_id) {
            Some(f) => *f,
            None => {
                let name = self.symbols.name(name_id).to_string();
                return Err(MoofError::runtime(format!("__primitive: unknown primitive '{name}'")));
            }
        };

        // Evaluate remaining args
        let rest = nth_cdr(args, 0)?;
        let prim_args = self.eval_args(rest, env)?;

        // Call the primitive
        prim_fn(self, prim_args)
    }

    // ── require ─────────────────────────────────────────────────────

    fn eval_require(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let path_expr = nth_car(args, 0)?;
        let path_val = self.eval(path_expr, env)?;
        let raw_path = path_val.as_str().map_err(|_| {
            MoofError::runtime("require: expected string path")
        })?.to_string();

        // Resolve the path
        let resolved = self.resolve_require_path(&raw_path)?;

        // Canonicalize for cache key
        let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());

        // Check cache
        if let Some(cached) = self.loaded_modules.get(&canonical) {
            return Ok(cached.clone());
        }

        // Circular dependency check
        if self.loading_stack.contains(&canonical) {
            return Err(MoofError::runtime(format!(
                "require: circular dependency detected: {}",
                canonical.display()
            )));
        }

        // Read the file
        let source = std::fs::read_to_string(&resolved)
            .map_err(|e| MoofError::io(format!("require: cannot read '{}': {}", resolved.display(), e)))?;
        let filename = resolved.to_string_lossy().to_string();

        // Push onto loading stack, save current_file
        self.loading_stack.push(canonical.clone());
        let prev_file = self.current_file.take();
        self.current_file = Some(resolved);

        // Evaluate
        let result = self.load_source(&source, &filename);

        // Restore state
        self.current_file = prev_file;
        self.loading_stack.pop();

        // Cache the result on success
        match result {
            Ok(val) => {
                self.loaded_modules.insert(canonical, val.clone());
                Ok(val)
            }
            Err(e) => Err(e),
        }
    }

    /// Resolve a require path to an absolute file path.
    /// Resolution order:
    /// 1. Relative to the current file's directory
    /// 2. MOOF_PATH entries (colon-separated env var)
    /// 3. stdlib_dir (next to the binary)
    fn resolve_require_path(&self, raw: &str) -> Result<PathBuf> {
        let candidates = self.require_search_paths();
        let extensions = ["", ".moof"];

        for base in &candidates {
            for ext in &extensions {
                let mut path = base.join(raw);
                if !ext.is_empty() {
                    let with_ext = format!("{}{}", path.display(), ext);
                    path = PathBuf::from(with_ext);
                }
                if path.is_file() {
                    return Ok(path);
                }
            }
        }

        Err(MoofError::io(format!(
            "require: cannot find '{}' (searched: {})",
            raw,
            candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        )))
    }

    /// Build the list of directories to search for require.
    fn require_search_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Relative to current file
        if let Some(ref current) = self.current_file {
            if let Some(parent) = current.parent() {
                paths.push(parent.to_path_buf());
            }
        }

        // Also try CWD
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(cwd);
        }

        // 2. MOOF_PATH entries
        if let Ok(moof_path) = std::env::var("MOOF_PATH") {
            for entry in moof_path.split(':') {
                let p = PathBuf::from(entry);
                if p.is_dir() {
                    paths.push(p);
                }
            }
        }

        // 3. stdlib dir
        if let Some(ref stdlib) = self.stdlib_dir {
            paths.push(stdlib.clone());
        }

        paths
    }

    /// Load the prelude from the stdlib directory.
    pub fn load_prelude(&mut self) -> Result<Value> {
        // Find stdlib dir via several strategies
        let stdlib_dir = self.find_stdlib_dir();

        match stdlib_dir {
            Some(dir) => {
                self.stdlib_dir = Some(dir.clone());
                let prelude = dir.join("prelude.moof");
                let source = std::fs::read_to_string(&prelude)
                    .map_err(|e| MoofError::io(format!("Failed to load prelude: {e}")))?;
                self.current_file = Some(prelude);
                let result = self.load_source(&source, "<prelude>");
                self.current_file = None;
                result
            }
            None => Ok(Value::Nil),
        }
    }

    fn find_stdlib_dir(&self) -> Option<PathBuf> {
        // 1. MOOF_STDLIB env var
        if let Ok(dir) = std::env::var("MOOF_STDLIB") {
            let p = PathBuf::from(&dir);
            if p.join("prelude.moof").is_file() {
                return Some(p);
            }
        }

        // 2. stdlib/ in CWD
        if let Ok(cwd) = std::env::current_dir() {
            let p = cwd.join("stdlib");
            if p.join("prelude.moof").is_file() {
                return Some(p);
            }
        }

        // 3. Relative to binary: ../stdlib, ../../stdlib
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                for ancestor in &["../stdlib", "../../stdlib", "stdlib"] {
                    let p = exe_dir.join(ancestor);
                    if p.join("prelude.moof").is_file() {
                        return Some(p);
                    }
                }
            }
        }

        None
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

    // ── __super-send ─────────────────────────────────────────────────

    fn eval_super_send(&mut self, args: &Value, env: &Env) -> Result<Value> {
        // __super-send is like __send but starts method lookup from the superclass
        // of the class where the current method is defined.
        let selector_expr = nth_car(args, 1)?;
        let selector_id = match selector_expr {
            Value::Str(s) => self.symbols.intern(s),
            Value::Symbol(id) => *id,
            other => {
                let evaled = self.eval(other, env)?;
                match &evaled {
                    Value::Str(s) => self.symbols.intern(s),
                    Value::Symbol(id) => *id,
                    _ => return Err(MoofError::runtime("super send: selector must be a string or symbol")),
                }
            }
        };

        // Get `self` from the environment (super sends use the same receiver)
        let receiver = env.get(self.known.self_).map_err(|_| {
            MoofError::runtime("super send: can only be used inside a method")
        })?;

        // Get __current_class to find which class the current method belongs to
        let current_class_val = env.get(self.known.__current_class).map_err(|_| {
            MoofError::runtime("super send: can only be used inside a method")
        })?;

        // __current_class is stored as an Object whose .class IS the defining class
        let defining_class = if let Value::Object(ref obj_rc) = current_class_val {
            obj_rc.borrow().class.clone()
        } else {
            return Err(MoofError::runtime("super send: invalid __current_class"));
        };

        // Start lookup from the superclass of the defining class
        let superclass = defining_class.borrow().superclass.clone();
        let method = superclass
            .as_ref()
            .and_then(|sup| sup.borrow().lookup_owner(selector_id));

        if let Some((method_val, owner_name)) = method {
            let rest = nth_cdr(args, 1)?;
            let msg_args = self.eval_args(rest, env)?;

            let mut full_args = Vec::with_capacity(msg_args.len() + 1);
            full_args.push(receiver);
            full_args.extend(msg_args);

            // Set current_method_class to the class that owns the super method
            let prev = self.current_method_class.take();
            self.current_method_class = self.find_class_by_name(owner_name, &self.global_env.clone());

            let result = match method_val {
                Value::Closure(ref c) => call_closure(self, c, full_args),
                _ => Err(MoofError::runtime("super: method is not callable")),
            };

            self.current_method_class = prev;
            result
        } else {
            let selector_name = self.symbols.name(selector_id).to_string();
            Err(MoofError::runtime(format!(
                "super: method '{}' not found in superclass",
                selector_name
            )))
        }
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
            Value::Range(_) => self.range_class.clone(),
        }
    }

    /// Return the class object (Value::Object) for a given value.
    /// This is what `.class` should return — a first-class class object.
    pub fn class_object_of(&self, val: &Value) -> Value {
        let class_rc = self.class_of(val);
        let name_id = class_rc.borrow().name;
        // Look up the class-as-Value in our registry
        if let Some(class_val) = self.class_objects.get(&name_id) {
            return class_val.clone();
        }
        // Fallback: for Object instances, their class might be a user-defined class
        // whose name is in the env. Try to find it.
        if let Ok(val) = self.global_env.get(name_id) {
            return val;
        }
        // Last resort: return a string (shouldn't happen with proper registration)
        Value::Str(Rc::from(self.symbols.name(name_id)))
    }

    /// Given a class-as-Value::Object, find the real MoofClass it represents.
    /// Works for both bootstrap classes (which use class_class as metaclass)
    /// and user-defined classes (which have <Name> meta metaclasses).
    pub fn real_class_from_class_object(&self, class_val: &Value) -> Result<Rc<RefCell<MoofClass>>> {
        // Check class_objects registry by pointer identity
        if let Value::Object(obj_rc) = class_val {
            for (&name_id, stored_val) in &self.class_objects {
                if let Value::Object(stored_rc) = stored_val {
                    if Rc::ptr_eq(obj_rc, stored_rc) {
                        // Found it — look up the actual MoofClass
                        if let Some(class) = self.find_class_by_name(name_id, &self.global_env.clone()) {
                            return Ok(class);
                        }
                    }
                }
            }
            // Fallback: try the old metaclass-based approach for user classes
            // not yet in the registry
            let obj = obj_rc.borrow();
            let metaclass = obj.class.borrow();
            let meta_name = self.symbols.name(metaclass.name).to_string();
            let real_name = meta_name.strip_suffix(" meta").unwrap_or(&meta_name);
            let real_id = self.symbols.lookup_id(real_name).copied();
            drop(metaclass);
            drop(obj);
            if let Some(real_id) = real_id {
                if let Some(class) = self.find_class_by_name(real_id, &self.global_env.clone()) {
                    return Ok(class);
                }
            }
        }
        Err(MoofError::runtime("Expected a class object"))
    }

    /// Dispatch a message to a receiver through the class hierarchy.
    pub fn send_message(
        &mut self,
        receiver: Value,
        selector: SymId,
        args: Vec<Value>,
    ) -> Result<Value> {
        let class = self.class_of(&receiver);
        let lookup_result = class.borrow().lookup_owner(selector);

        if let Some((method_val, owner_class_name)) = lookup_result {
            // Prepend receiver as `self` argument
            let mut full_args = Vec::with_capacity(args.len() + 1);
            full_args.push(receiver);
            full_args.extend(args);

            // Set current_method_class so setup_call_env can bind __current_class
            let defining_class = self.find_class_by_name(owner_class_name, &self.global_env.clone());
            let prev = self.current_method_class.take();
            self.current_method_class = defining_class;

            let result = match method_val {
                Value::Closure(ref c) => call_closure(self, c, full_args),
                _ => Err(MoofError::runtime(format!(
                    "Method '{}' is not callable",
                    self.symbols.name(selector)
                ))),
            };

            self.current_method_class = prev;
            result
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

            // Try doesNotUnderstand: hook (avoid infinite recursion by checking selector)
            let dnu_id = self.symbols.intern("doesNotUnderstand:");
            if selector != dnu_id {
                if let Some(dnu_method) = class.borrow().lookup(dnu_id) {
                    // Build a message table: { selector: "name", args: (arg-list) }
                    let selector_name = self.symbols.name(selector).to_string();
                    let mut msg_table = MoofTable::new();
                    msg_table.hash.insert("selector".to_string(), Value::Str(Rc::from(selector_name.as_str())));
                    msg_table.hash.insert("args".to_string(), Value::from_slice(&args));
                    let msg_val = Value::Table(Rc::new(RefCell::new(msg_table)));

                    let full_args = vec![receiver.clone(), msg_val];
                    if let Value::Closure(ref c) = dnu_method {
                        return call_closure(self, c, full_args);
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
        // Check if named or anonymous class
        let first = nth_car(args, 0)?;
        let (name_id, is_anonymous) = if let Ok(sym_id) = first.as_symbol() {
            // Named class: (class Foo ...)
            (sym_id, false)
        } else {
            // Anonymous class: (class (fields ...) (method ...) ...)
            let anon_name = format!("<anon-class-{}>", self.class_objects.len());
            let anon_id = self.symbols.intern(&anon_name);
            (anon_id, true)
        };

        // Check if reopening an existing class
        let existing = if is_anonymous { None } else { env.get(name_id).ok().and_then(|val| {
            if let Value::Object(ref obj) = val {
                Some(obj.clone())
            } else {
                None
            }
        }) };

        // For named classes, body is after the name; for anonymous, body IS the args
        let body = if is_anonymous { args } else { nth_cdr(args, 0)? };

        let mut superclass: Option<Rc<RefCell<MoofClass>>> = None;
        let mut field_names: Vec<SymId> = Vec::new();
        let mut methods: Vec<(SymId, Value)> = Vec::new();
        let mut class_methods: Vec<(SymId, Value)> = Vec::new();
        let mut trait_names: Vec<SymId> = Vec::new();

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
                    // (uses TraitName ...) — mix in trait methods
                    else if kw == self.known.uses {
                        let mut tcursor = &inner.cdr;
                        while let Value::Cons(tc) = tcursor {
                            if let Ok(trait_id) = tc.car.as_symbol() {
                                trait_names.push(trait_id);
                            }
                            tcursor = &tc.cdr;
                        }
                    }
                    // (delegates-to field-name) — auto-forward unknown messages
                    else if kw == self.known.delegates_to {
                        let target_field = nth_car(&inner.cdr, 0)?;
                        let target_id = target_field.as_symbol().map_err(|_| {
                            MoofError::runtime("delegates-to: expected field name symbol")
                        })?;
                        // Generate: (method doesNotUnderstand: (msg)
                        //   [target send: [msg at: "selector"]])
                        let dnu_sel = self.symbols.intern("doesNotUnderstand:");
                        let msg_sym = self.symbols.intern("__dnu_msg");

                        // Build: (__send __dnu_msg "at:" "selector")
                        let get_selector = Value::from_slice(&[
                            Value::Symbol(self.known.send),
                            Value::Symbol(msg_sym),
                            Value::Str(Rc::from("at:")),
                            Value::Str(Rc::from("selector")),
                        ]);
                        // Build: (__send target <get_selector>)
                        // This sends the forwarded message to the target
                        let body = Value::from_slice(&[
                            Value::Symbol(self.known.send),
                            Value::Symbol(target_id),
                            get_selector,
                        ]);

                        let closure = Value::Closure(Rc::new(MoofClosure {
                            name: Some(dnu_sel),
                            params: vec![self.known.self_, msg_sym],
                            rest_param: None,
                            defaults: Vec::new(),
                            body: ClosureBody::Expr(body),
                            env: env.clone(),
                            upvalues: Vec::new(),
                        }));
                        methods.push((dnu_sel, closure));
                    }
                    // (method selector (params...) body...)
                    // Multi-keyword selectors: (method foo:bar: (a b) body)
                    // The parser splits foo:bar: into two symbols; we merge them.
                    else if kw == self.known.method {
                        let (sel_id, rest_after_sel) = self.parse_method_selector(&inner.cdr)?;
                        let params_expr = match rest_after_sel {
                            Value::Cons(ref c) => &c.car,
                            _ => return Err(MoofError::runtime("class method: expected params")),
                        };

                        // Parse params, prepend `self`
                        let (mut params, rest_param) = self.parse_params(params_expr)?;
                        params.insert(0, self.known.self_);

                        let method_body_list = match rest_after_sel {
                            Value::Cons(ref c) => &c.cdr,
                            _ => return Err(MoofError::runtime("class method: expected body")),
                        };
                        let body = self.wrap_body(method_body_list);

                        let closure = Value::Closure(Rc::new(MoofClosure {
                            name: Some(sel_id),
                            params,
                            rest_param,
                            defaults: Vec::new(),
                            body: ClosureBody::Expr(body),
                            env: env.clone(),
                            upvalues: Vec::new(),
                        }));
                        methods.push((sel_id, closure));
                    }
                    // (classmethod selector (params...) body...)
                    else if kw == self.known.classmethod {
                        let (sel_id, rest_after_sel) = self.parse_method_selector(&inner.cdr)?;
                        let params_expr = match rest_after_sel {
                            Value::Cons(ref c) => &c.car,
                            _ => return Err(MoofError::runtime("classmethod: expected params")),
                        };

                        // Parse params, prepend `self` (self = the class object)
                        let (mut params, rest_param) = self.parse_params(params_expr)?;
                        params.insert(0, self.known.self_);

                        let method_body_list = match rest_after_sel {
                            Value::Cons(ref c) => &c.cdr,
                            _ => return Err(MoofError::runtime("classmethod: expected body")),
                        };
                        let body = self.wrap_body(method_body_list);

                        let closure = Value::Closure(Rc::new(MoofClosure {
                            name: Some(sel_id),
                            params,
                            rest_param,
                            defaults: Vec::new(),
                            body: ClosureBody::Expr(body),
                            env: env.clone(),
                            upvalues: Vec::new(),
                        }));
                        class_methods.push((sel_id, closure));
                    }
                }
            }
            cursor = &cell.cdr;
        }

        // Copy trait methods (trait methods are added first so explicit methods override)
        for trait_id in &trait_names {
            if let Some(trait_methods) = self.trait_registry.get(trait_id).cloned() {
                for (sel, closure) in trait_methods {
                    // Only add if not already defined explicitly
                    if !methods.iter().any(|(s, _)| *s == sel) {
                        methods.push((sel, closure));
                    }
                }
            }
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
                {
                    let mut klass = real_class.borrow_mut();
                    for (sel, closure) in &methods {
                        klass.add_method(*sel, closure.clone());
                    }
                }
                // Add class methods to the metaclass
                if !class_methods.is_empty() {
                    let metaclass = real_class.borrow().metaclass.clone();
                    if let Some(mc) = metaclass {
                        let mut mc_ref = mc.borrow_mut();
                        for (sel, closure) in &class_methods {
                            mc_ref.add_method(*sel, closure.clone());
                        }
                    }
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

            // Auto-generate reader methods for fields that don't have explicit methods
            let all_fields = klass.all_field_names();
            for &field_id in &all_fields {
                if klass.methods.contains_key(&field_id) {
                    continue; // explicit method already defined
                }
                // Generate: (method field_name () field_name)
                // Body is just the symbol for the field name (which resolves to the field binding)
                let reader = Value::Closure(Rc::new(MoofClosure {
                    name: Some(field_id),
                    params: vec![self.known.self_],
                    rest_param: None,
                    defaults: Vec::new(),
                    body: ClosureBody::Expr(Value::Symbol(field_id)),
                    env: self.global_env.clone(),
                    upvalues: Vec::new(),
                }));
                klass.add_method(field_id, reader);
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

        // Install class methods on the metaclass
        {
            let mut mc = metaclass.borrow_mut();
            for (sel, closure) in class_methods {
                mc.add_method(sel, closure);
            }
        }

        new_class.borrow_mut().metaclass = Some(metaclass.clone());

        // Store class as an Object in env (the Object's class is the metaclass)
        let class_obj = MoofObject {
            class: metaclass,
            fields: Vec::new(),
        };
        let class_val = Value::Object(Rc::new(RefCell::new(class_obj)));

        if !is_anonymous {
            env.define(name_id, class_val.clone(), false);
        }
        self.class_objects.insert(name_id, class_val.clone());

        // Also store the real MoofClass rc somewhere we can find it.
        // We use a convention: store it under __class:<name> in the env.
        let class_key_name = format!("__class:{}", self.symbols.name(name_id));
        let class_key_id = self.symbols.intern(&class_key_name);
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

    // ── protocol ────────────────────────────────────────────────────

    fn eval_protocol(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("protocol: expected symbol name")
        })?;

        // Collect required selectors
        let mut selectors = Vec::new();
        let mut cursor = nth_cdr(args, 0)?;
        while let Value::Cons(cell) = cursor {
            if let Ok(sel_id) = cell.car.as_symbol() {
                selectors.push(sel_id);
            }
            cursor = &cell.cdr;
        }

        // Register protocol
        self.protocol_registry.insert(name_id, selectors.clone());

        // Store a table in the environment as a marker object
        let mut table = MoofTable::new();
        table.hash.insert(
            "name".to_string(),
            Value::Str(Rc::from(self.symbols.name(name_id))),
        );
        let sel_values: Vec<Value> = selectors
            .iter()
            .map(|s| Value::Str(Rc::from(self.symbols.name(*s))))
            .collect();
        table.hash.insert(
            "selectors".to_string(),
            Value::from_slice(&sel_values),
        );
        table.hash.insert("type".to_string(), Value::Str(Rc::from("protocol")));

        let proto_val = Value::Table(Rc::new(RefCell::new(table)));
        env.define(name_id, proto_val.clone(), false);
        Ok(proto_val)
    }

    // ── trait ──────────────────────────────────────────────────────────

    fn eval_trait(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("trait: expected symbol name")
        })?;

        let mut trait_methods: HashMap<SymId, Value> = HashMap::new();

        // Walk body: each item should be (method selector (params) body...)
        let mut cursor = nth_cdr(args, 0)?;
        while let Value::Cons(cell) = cursor {
            if let Value::Cons(inner) = &cell.car {
                if let Value::Symbol(kw) = &inner.car {
                    if *kw == self.known.method {
                        let sel = nth_car(&inner.cdr, 0)?;
                        let sel_id = sel.as_symbol().map_err(|_| {
                            MoofError::runtime("trait method: expected symbol selector")
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
                            defaults: Vec::new(),
                            body: ClosureBody::Expr(body),
                            env: env.clone(),
                            upvalues: Vec::new(),
                        }));
                        trait_methods.insert(sel_id, closure);
                    }
                }
            }
            cursor = &cell.cdr;
        }

        // Register the trait
        self.trait_registry.insert(name_id, trait_methods);

        // Store a marker in the environment
        let mut table = MoofTable::new();
        table.hash.insert(
            "name".to_string(),
            Value::Str(Rc::from(self.symbols.name(name_id))),
        );
        table.hash.insert("type".to_string(), Value::Str(Rc::from("trait")));

        let trait_val = Value::Table(Rc::new(RefCell::new(table)));
        env.define(name_id, trait_val.clone(), false);
        Ok(trait_val)
    }

    // ── module ─────────────────────────────────────────────────────────

    fn eval_module(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("module: expected symbol name")
        })?;

        let body = nth_cdr(args, 0)?;

        // Look for (export sym1 sym2 ...) as the first body form
        let mut exports: Option<Vec<SymId>> = None;
        let mut body_start = body;

        if let Value::Cons(cell) = body {
            if let Value::Cons(inner) = &cell.car {
                if let Value::Symbol(kw) = &inner.car {
                    let name = self.symbols.name(*kw);
                    if name == "export" {
                        let mut export_list = Vec::new();
                        let mut ecursor = &inner.cdr;
                        while let Value::Cons(ec) = ecursor {
                            if let Ok(eid) = ec.car.as_symbol() {
                                export_list.push(eid);
                            }
                            ecursor = &ec.cdr;
                        }
                        exports = Some(export_list);
                        body_start = &cell.cdr;
                    }
                }
            }
        }

        // Create child environment and evaluate body
        let module_env = env.child();
        let mut cursor = body_start;
        while let Value::Cons(cell) = cursor {
            self.eval(&cell.car, &module_env)?;
            cursor = &cell.cdr;
        }

        // Collect exported bindings into a table
        let mut module_table = MoofTable::new();
        let mut module_bindings: HashMap<SymId, Value> = HashMap::new();

        match exports {
            Some(ref export_ids) => {
                for &eid in export_ids {
                    if let Ok(val) = module_env.get(eid) {
                        let key = self.symbols.name(eid).to_string();
                        module_table.hash.insert(key, val.clone());
                        module_bindings.insert(eid, val);
                    }
                }
            }
            None => {
                // No explicit exports — re-walk body AST to find define forms
                let mut dcursor = body_start;
                while let Value::Cons(cell) = dcursor {
                    if let Value::Cons(inner) = &cell.car {
                        if let Value::Symbol(kw) = &inner.car {
                            if *kw == self.known.define {
                                if let Ok(name_expr) = nth_car(&inner.cdr, 0) {
                                    let def_id = match name_expr {
                                        Value::Symbol(id) => Some(*id),
                                        Value::Cons(c) => c.car.as_symbol().ok(),
                                        _ => None,
                                    };
                                    if let Some(did) = def_id {
                                        if let Ok(val) = module_env.get(did) {
                                            let key = self.symbols.name(did).to_string();
                                            module_table.hash.insert(key, val.clone());
                                            module_bindings.insert(did, val);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    dcursor = &cell.cdr;
                }
            }
        }

        // Store in module registry
        self.module_registry.insert(name_id, module_bindings);

        // Define the module name in the current env as a table
        module_table.hash.insert(
            "name".to_string(),
            Value::Str(Rc::from(self.symbols.name(name_id))),
        );
        module_table.hash.insert("type".to_string(), Value::Str(Rc::from("module")));

        let module_val = Value::Table(Rc::new(RefCell::new(module_table)));
        env.define(name_id, module_val.clone(), false);
        Ok(module_val)
    }

    // ── use ────────────────────────────────────────────────────────────

    fn eval_use(&mut self, args: &Value, env: &Env) -> Result<Value> {
        let name_id = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::runtime("use: expected module name symbol")
        })?;

        let module_bindings = self.module_registry.get(&name_id).cloned().ok_or_else(|| {
            MoofError::runtime(format!(
                "use: module '{}' not found",
                self.symbols.name(name_id)
            ))
        })?;

        // Check for qualifier: (use Mod (only sym1 sym2)) or (use Mod (as Alias))
        let rest = nth_cdr(args, 0);
        if let Ok(Value::Cons(cell)) = rest {
            if let Value::Cons(inner) = &cell.car {
                if let Value::Symbol(qual_id) = &inner.car {
                    let qual_name = self.symbols.name(*qual_id).to_string();

                    if qual_name == "only" {
                        let mut icursor = &inner.cdr;
                        while let Value::Cons(ic) = icursor {
                            if let Ok(sym_id) = ic.car.as_symbol() {
                                if let Some(val) = module_bindings.get(&sym_id) {
                                    env.define(sym_id, val.clone(), false);
                                }
                            }
                            icursor = &ic.cdr;
                        }
                        return Ok(Value::Nil);
                    } else if qual_name == "as" {
                        let alias_id = nth_car(&inner.cdr, 0)?.as_symbol().map_err(|_| {
                            MoofError::runtime("use as: expected symbol alias")
                        })?;
                        if let Ok(mod_val) = env.get(name_id) {
                            env.define(alias_id, mod_val, false);
                        }
                        return Ok(Value::Nil);
                    }
                }
            }
        }

        // Default: import all bindings into current env
        for (sym_id, val) in &module_bindings {
            env.define(*sym_id, val.clone(), false);
        }
        Ok(Value::Nil)
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
    pub fn find_class_by_name(
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
            &self.range_class,
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
            ClosureBody::Bytecode(_) => return Err(MoofError::runtime("macros cannot be bytecode-compiled")),
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
                // Check for table pattern: (__table "key" pattern ...)
                if let Value::Symbol(id) = &cell.car {
                    if *id == self.known.table {
                        // Table pattern: match key-value pairs
                        if let Value::Table(tbl_rc) = value {
                            let tbl = tbl_rc.borrow();
                            let pairs = list_to_vec(&cell.cdr);
                            // Empty pattern (__table) matches any table
                            if pairs.is_empty() {
                                return true;
                            }
                            // pairs should be alternating key, pattern
                            let mut i = 0;
                            while i + 1 < pairs.len() {
                                let key = match &pairs[i] {
                                    Value::Str(s) => s.to_string(),
                                    _ => return false,
                                };
                                let pat = &pairs[i + 1];
                                match tbl.hash.get(&key) {
                                    Some(val) => {
                                        if !self.match_pattern(pat, val, env) {
                                            return false;
                                        }
                                    }
                                    None => return false,
                                }
                                i += 2;
                            }
                            return true;
                        }
                        return false;
                    }
                }

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
                // Try callable protocol: [obj call: args]
                return self.try_callable_protocol(callee, args);
            }
            _ => {
                // Try callable protocol: [val call: args]
                return self.try_callable_protocol(callee, args);
            }
        }
    }

    /// Try calling an object via the callable protocol: [obj call: args-list]
    fn try_callable_protocol(&mut self, callee: Value, args: Vec<Value>) -> Result<Value> {
        let call_sel = self.symbols.intern("call:");
        let class = self.class_of(&callee);
        if class.borrow().lookup(call_sel).is_some() {
            let args_list = Value::from_slice(&args);
            self.send_message(callee, call_sel, vec![args_list])
        } else {
            Err(MoofError::runtime(format!(
                "Cannot call {}: not a function (does not respond to call:)",
                callee.type_name()
            )))
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

    /// Parse a params list into (positional_params, optional_rest_param, defaults).
    /// Supports `(a b c)`, `(a b . rest)`, and `(a (b 10) (c 20))` for defaults.
    fn parse_params(&self, params: &Value) -> Result<(Vec<SymId>, Option<SymId>)> {
        let (params, rest, _defaults) = self.parse_params_with_defaults(params)?;
        Ok((params, rest))
    }

    fn parse_params_with_defaults(&self, params: &Value) -> Result<(Vec<SymId>, Option<SymId>, Vec<Option<Value>>)> {
        let mut positional = Vec::new();
        let mut defaults = Vec::new();
        let mut rest_param = None;
        let mut seen_default = false;
        let mut cursor = params;

        while let Value::Cons(cell) = cursor {
            match &cell.car {
                // (param-name default-value) — optional parameter
                Value::Cons(pair) => {
                    let id = pair.car.as_symbol().map_err(|_| {
                        MoofError::runtime("Expected symbol in parameter default pair")
                    })?;
                    let default_val = nth_car(&pair.cdr, 0)?;
                    positional.push(id);
                    defaults.push(Some(default_val.clone()));
                    seen_default = true;
                }
                // plain symbol — required parameter or rest marker
                Value::Symbol(id) => {
                    let name = self.symbols.name(*id);
                    if name == "&" || name == "." {
                        let rest_expr = nth_car(&cell.cdr, 0)?;
                        rest_param = Some(rest_expr.as_symbol().map_err(|_| {
                            MoofError::runtime("Expected symbol after & in parameter list")
                        })?);
                        break;
                    }
                    if seen_default {
                        return Err(MoofError::runtime(
                            "Required parameter cannot follow optional parameter"
                        ));
                    }
                    positional.push(*id);
                    defaults.push(None);
                }
                _ => return Err(MoofError::runtime("Expected symbol or (symbol default) in parameter list")),
            }
            cursor = &cell.cdr;
        }

        // Handle improper list tail as rest param: (a b . rest)
        if rest_param.is_none() {
            if let Value::Symbol(id) = cursor {
                rest_param = Some(*id);
            }
        }

        Ok((positional, rest_param, defaults))
    }

    /// Parse a method selector from a cons list, merging multi-keyword selectors.
    /// E.g. (replace_all: with: (from to) body) -> ("replace_all:with:", rest starting at (from to))
    /// Single selectors like (foo (params) body) -> ("foo", rest starting at (params))
    fn parse_method_selector(&mut self, args: &Value) -> Result<(SymId, Value)> {
        let first = nth_car(args, 0)?;
        let first_id = first.as_symbol().map_err(|_| {
            MoofError::runtime("method: expected symbol selector")
        })?;
        let first_name = self.symbols.name(first_id).to_string();

        // If the first symbol doesn't end with ':', it's a simple selector
        if !first_name.ends_with(':') {
            let rest = nth_cdr(args, 0)?;
            return Ok((first_id, rest.clone()));
        }

        // Multi-keyword: consume all consecutive colon-terminated symbols
        let mut combined = first_name;
        let mut cursor = nth_cdr(args, 0)?;

        loop {
            if let Value::Cons(cell) = cursor {
                if let Value::Symbol(id) = &cell.car {
                    let name = self.symbols.name(*id).to_string();
                    if name.ends_with(':') {
                        combined.push_str(&name);
                        cursor = &cell.cdr;
                        continue;
                    }
                }
            }
            break;
        }

        let sel_id = self.symbols.intern(&combined);
        Ok((sel_id, cursor.clone()))
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

/// Set up a call environment for a closure: arity check, param binding, field binding.
/// Returns the new environment ready for evaluation.
fn setup_call_env(
    interp: &Interpreter,
    closure: &MoofClosure,
    args: &[Value],
) -> Result<Env> {
    let n_params = closure.params.len();
    let has_rest = closure.rest_param.is_some();

    // Count required params (those without defaults)
    let n_required = if closure.defaults.is_empty() {
        n_params
    } else {
        closure.defaults.iter().take_while(|d| d.is_none()).count()
    };

    // Arity check (accounting for defaults)
    if has_rest {
        if args.len() < n_required {
            let name = closure.name.map(|id| interp.symbols.name(id).to_string());
            return Err(MoofError::arity(
                &format!("at least {n_required}"),
                args.len(),
                name.as_deref(),
            ));
        }
    } else if args.len() < n_required || args.len() > n_params {
        let name = closure.name.map(|id| interp.symbols.name(id).to_string());
        let expected = if n_required == n_params {
            n_params.to_string()
        } else {
            format!("{n_required}-{n_params}")
        };
        return Err(MoofError::arity(
            &expected,
            args.len(),
            name.as_deref(),
        ));
    }

    // Create child env from closure's captured env
    let call_env = closure.env.child();

    // Bind positional params (with defaults for missing args)
    for (i, &param_id) in closure.params.iter().enumerate() {
        let val = if let Some(v) = args.get(i) {
            v.clone()
        } else if let Some(Some(default)) = closure.defaults.get(i) {
            default.clone()
        } else {
            Value::Nil
        };
        call_env.define(param_id, val, false);
    }

    // Bind rest param
    if let Some(rest_id) = closure.rest_param {
        let rest_items: Vec<Value> = args[n_params..].to_vec();
        call_env.define(rest_id, Value::from_slice(&rest_items), false);
    }

    // If first param is `self` and the value is an Object, bind fields and __current_class
    if !closure.params.is_empty() && closure.params[0] == interp.known.self_ {
        if let Some(self_val) = args.first() {
            if let Value::Object(obj_rc) = self_val {
                let obj = obj_rc.borrow();
                let class = obj.class.borrow();
                for (i, &field_id) in class.field_names.iter().enumerate() {
                    if let Some(val) = obj.get_field(i) {
                        call_env.define(field_id, val.clone(), true);
                    }
                }
                drop(class);
                // Bind __current_class if the interpreter has one set (from send_message)
                if let Some(ref defining_class) = interp.current_method_class {
                    let holder = MoofObject {
                        class: defining_class.clone(),
                        fields: Vec::new(),
                    };
                    call_env.define(
                        interp.known.__current_class,
                        Value::Object(Rc::new(RefCell::new(holder))),
                        false,
                    );
                }
            }
        }
    }

    Ok(call_env)
}

/// Call a closure with the given arguments. Uses a trampoline for TCO.
pub fn call_closure(
    interp: &mut Interpreter,
    closure: &MoofClosure,
    args: Vec<Value>,
) -> Result<Value> {
    match &closure.body {
        ClosureBody::Native(f) => f(interp, args),
        ClosureBody::Bytecode(func) => {
            // Execute bytecode closure via a mini-VM execution
            let bp = 0;
            let mut stack: Vec<Value> = Vec::with_capacity(64);
            // Fill locals with args
            for i in 0..func.local_count as usize {
                let val = args.get(i).cloned().unwrap_or(Value::Nil);
                stack.push(val);
            }
            crate::vm::execute_bytecode(interp, func, &mut stack, bp, &closure.upvalues)
        }
        ClosureBody::Expr(body) => {
            let call_env = setup_call_env(interp, closure, &args)?;
            let mut result = interp.eval_tail(body, &call_env)?;

            // Trampoline loop
            loop {
                match result {
                    Eval::Val(v) => return Ok(v),
                    Eval::TailCall { func, args: tc_args } => {
                        match func {
                            Value::Closure(ref c) => {
                                match &c.body {
                                    ClosureBody::Native(f) => return f(interp, tc_args),
                                    ClosureBody::Bytecode(func) => {
                                        let mut stack: Vec<Value> = Vec::with_capacity(64);
                                        for i in 0..func.local_count as usize {
                                            let val = tc_args.get(i).cloned().unwrap_or(Value::Nil);
                                            stack.push(val);
                                        }
                                        return crate::vm::execute_bytecode(interp, func, &mut stack, 0, &c.upvalues);
                                    }
                                    ClosureBody::Expr(body) => {
                                        let new_env = setup_call_env(interp, c, &tc_args)?;
                                        result = interp.eval_tail(body, &new_env)?;
                                    }
                                }
                            }
                            _ => return interp.invoke(func, tc_args),
                        }
                    }
                }
            }
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
