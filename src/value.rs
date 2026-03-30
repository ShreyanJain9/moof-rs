use std::collections::HashMap;
use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::ast::Expr;
use crate::environment::Env;

/// Runtime values in Moof.
#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Nil,
    Symbol(String),
    List(Vec<Value>),
    Map(IndexMap<String, Value>),   // ordered map (preserves insertion order, O(1) lookup)
    Function(MoofFunction),
    Builtin(String, BuiltinFn),    // name, function pointer
    Class(Rc<RefCell<MoofClass>>),
    Object(MoofObject),
    Protocol(MoofProtocol),
    Macro(MoofMacro),
}

/// Internal evaluation result — separates TCO control flow from user-visible values.
pub enum Eval {
    Val(Value),
    TailCall { func: Value, args: Vec<Value> },
}

pub type BuiltinFn = fn(&mut crate::interpreter::Interpreter, Vec<Value>) -> crate::error::Result<Value>;

/// Built-in method: takes (interpreter, receiver, args).
pub type BuiltinMethodFn = fn(&mut crate::interpreter::Interpreter, Value, Vec<Value>) -> crate::error::Result<Value>;

/// A method is either a user-defined function or a Rust built-in.
#[derive(Clone)]
pub enum Method {
    UserDefined(MoofFunction),
    Builtin(String, BuiltinMethodFn), // name, function
}

impl fmt::Debug for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Method::UserDefined(func) => write!(f, "Method::UserDefined({:?})", func.name),
            Method::Builtin(name, _) => write!(f, "Method::Builtin({})", name),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MoofFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub rest_param: Option<String>,
    pub body: Box<Expr>,
    pub closure: Env,
}

#[derive(Debug, Clone)]
pub struct MoofClass {
    pub name: String,
    pub superclass: Option<Rc<RefCell<MoofClass>>>,
    pub fields: Vec<String>,
    pub own_fields: Vec<String>,
    pub methods: HashMap<String, Method>,
}

#[derive(Debug, Clone)]
pub struct MoofObject {
    pub class: Rc<RefCell<MoofClass>>,
    pub fields: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct MoofProtocol {
    pub name: String,
    pub selectors: Vec<String>,
    pub default_methods: HashMap<String, MoofFunction>,
}

#[derive(Debug, Clone)]
pub struct MoofMacro {
    pub name: String,
    pub params: Vec<String>,
    pub body: Box<Expr>,
}

impl MoofClass {
    pub fn new(name: String) -> Self {
        MoofClass {
            name,
            superclass: None,
            fields: vec![],
            own_fields: vec![],
            methods: HashMap::new(),
        }
    }

    pub fn all_fields(&self) -> Vec<String> {
        let mut fields = if let Some(ref sup) = self.superclass {
            sup.borrow().all_fields()
        } else {
            vec![]
        };
        fields.extend(self.own_fields.clone());
        fields
    }

    pub fn lookup(&self, selector: &str) -> Option<Method> {
        if let Some(m) = self.methods.get(selector) {
            return Some(m.clone());
        }
        if let Some(ref sup) = self.superclass {
            return sup.borrow().lookup(selector);
        }
        None
    }

    pub fn register_builtin(&mut self, selector: &str, f: BuiltinMethodFn) {
        self.methods.insert(selector.to_string(), Method::Builtin(selector.to_string(), f));
    }

    pub fn method_names(&self) -> Vec<String> {
        self.methods.keys().cloned().collect()
    }

    pub fn reopen(&mut self, new_fields: Vec<String>, new_methods: HashMap<String, Method>) {
        for f in new_fields {
            if !self.own_fields.contains(&f) {
                self.own_fields.push(f.clone());
                self.fields.push(f);
            }
        }
        self.methods.extend(new_methods);
    }
}

impl MoofObject {
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn set_field(&mut self, name: &str, value: Value) {
        self.fields.insert(name.to_string(), value);
    }
}

impl MoofFunction {
    pub fn arity(&self) -> usize { self.params.len() }
    pub fn is_variadic(&self) -> bool { self.rest_param.is_some() }
}

// ── Value Display ─────────────────────────────────────────────────

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Nil => write!(f, "nil"),
            Value::Symbol(s) => write!(f, "'{s}"),
            Value::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Value::Map(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Value::Function(func) => {
                if let Some(ref name) = func.name {
                    write!(f, "<fn {name}/{}>", func.arity())
                } else {
                    write!(f, "<lambda/{}>", func.arity())
                }
            }
            Value::Builtin(name, _) => write!(f, "<builtin {name}>"),
            Value::Class(class) => write!(f, "<class {}>", class.borrow().name),
            Value::Object(obj) => {
                let class = obj.class.borrow();
                if obj.fields.is_empty() {
                    write!(f, "{}", class.name)
                } else {
                    write!(f, "({}", class.name)?;
                    for field in &class.all_fields() {
                        if let Some(val) = obj.fields.get(field) {
                            write!(f, " {field}: {val}")?;
                        }
                    }
                    write!(f, ")")
                }
            }
            Value::Protocol(p) => write!(f, "<protocol {} [{}]>", p.name, p.selectors.join(" ")),
            Value::Macro(m) => write!(f, "<macro {}>", m.name),
        }
    }
}

// ── Value Inspection (with quotes around strings) ─────────────────

impl Value {
    pub fn inspect(&self) -> String {
        match self {
            Value::Str(s) => format!("{s:?}"),
            other => format!("{other}"),
        }
    }

    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Bool(false) | Value::Nil)
    }

    // ── Typed accessors ────────────────────────────────────────────

    pub fn as_int(&self) -> crate::error::Result<i64> {
        match self {
            Value::Integer(n) => Ok(*n),
            other => Err(crate::error::MoofError::runtime(format!("Expected Integer, got {}", other.type_name()))),
        }
    }

    pub fn as_float(&self) -> crate::error::Result<f64> {
        match self {
            Value::Float(n) => Ok(*n),
            other => Err(crate::error::MoofError::runtime(format!("Expected Float, got {}", other.type_name()))),
        }
    }

    pub fn as_number(&self) -> crate::error::Result<f64> {
        match self {
            Value::Integer(n) => Ok(*n as f64),
            Value::Float(n) => Ok(*n),
            other => Err(crate::error::MoofError::runtime(format!("Expected number, got {}", other.type_name()))),
        }
    }

    pub fn as_str(&self) -> crate::error::Result<&str> {
        match self {
            Value::Str(s) => Ok(s),
            other => Err(crate::error::MoofError::runtime(format!("Expected String, got {}", other.type_name()))),
        }
    }

    pub fn as_list(&self) -> crate::error::Result<&[Value]> {
        match self {
            Value::List(v) => Ok(v),
            other => Err(crate::error::MoofError::runtime(format!("Expected List, got {}", other.type_name()))),
        }
    }

    pub fn into_list(self) -> crate::error::Result<Vec<Value>> {
        match self {
            Value::List(v) => Ok(v),
            other => Err(crate::error::MoofError::runtime(format!("Expected List, got {}", other.type_name()))),
        }
    }

    pub fn as_bool(&self) -> crate::error::Result<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(crate::error::MoofError::runtime(format!("Expected Bool, got {}", other.type_name()))),
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            Value::Integer(_) => "Integer".to_string(),
            Value::Float(_) => "Float".to_string(),
            Value::Str(_) => "String".to_string(),
            Value::Bool(_) => "Bool".to_string(),
            Value::Nil => "Nil".to_string(),
            Value::Symbol(_) => "Symbol".to_string(),
            Value::List(_) => "List".to_string(),
            Value::Map(_) => "Map".to_string(),
            Value::Function(_) => "Function".to_string(),
            Value::Builtin(_, _) => "Function".to_string(),
            Value::Class(_) => "Class".to_string(),
            Value::Object(o) => o.class.borrow().name.clone(),
            Value::Protocol(_) => "Protocol".to_string(),
            Value::Macro(_) => "Macro".to_string(),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Integer(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Integer(b)) => *a == (*b as f64),
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => {
                std::ptr::eq(&*a.class as *const _, &*b.class as *const _)
                    && a.fields == b.fields
            }
            _ => false,
        }
    }
}
