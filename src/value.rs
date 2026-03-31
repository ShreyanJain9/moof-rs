use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::cons::ConsCell;
use crate::moofint::MoofInt;
use crate::symbol::SymId;

// ── MoofRange (lazy) ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MoofRange {
    pub start: MoofInt,
    pub end: MoofInt,   // exclusive
    pub step: MoofInt,
}

impl MoofRange {
    pub fn len(&self) -> i64 {
        let s = self.start.to_i64().unwrap_or(0);
        let e = self.end.to_i64().unwrap_or(0);
        let st = self.step.to_i64().unwrap_or(1);
        if st == 0 { return 0; }
        let diff = if st > 0 { e - s } else { s - e };
        if diff <= 0 { 0 } else { (diff + st.abs() - 1) / st.abs() }
    }
}

// ── Core Value enum ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum Value {
    Integer(MoofInt),
    Float(f64),
    Bool(bool),
    Nil,
    Symbol(SymId),
    Str(Rc<str>),
    Cons(Rc<ConsCell>),
    Table(Rc<RefCell<MoofTable>>),
    Object(Rc<RefCell<MoofObject>>),
    Closure(Rc<MoofClosure>),
    Range(Rc<MoofRange>),
}

// ── MoofTable ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MoofTable {
    pub array: Vec<Value>,
    pub hash: indexmap::IndexMap<String, Value>,
}

impl MoofTable {
    pub fn new() -> Self {
        MoofTable {
            array: Vec::new(),
            hash: indexmap::IndexMap::new(),
        }
    }

    pub fn get(&self, key: &Value) -> Option<&Value> {
        match key {
            Value::Integer(n) => {
                let idx = n.to_i64()? as usize;
                self.array.get(idx)
            }
            Value::Str(s) => self.hash.get(s.as_ref()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: Value, val: Value) {
        match key {
            Value::Integer(n) => {
                if let Some(idx) = n.to_i64() {
                    let idx = idx as usize;
                    if idx < self.array.len() {
                        self.array[idx] = val;
                    } else {
                        // Extend array up to index, filling with Nil
                        while self.array.len() < idx {
                            self.array.push(Value::Nil);
                        }
                        self.array.push(val);
                    }
                }
            }
            Value::Str(s) => {
                self.hash.insert(s.to_string(), val);
            }
            _ => {} // Silently ignore unsupported key types for now
        }
    }

    pub fn len(&self) -> usize {
        self.array.len() + self.hash.len()
    }

    pub fn is_empty(&self) -> bool {
        self.array.is_empty() && self.hash.is_empty()
    }

    pub fn keys(&self) -> Vec<Value> {
        let mut keys = Vec::with_capacity(self.len());
        for i in 0..self.array.len() {
            keys.push(Value::Integer(MoofInt::from_i64(i as i64)));
        }
        for k in self.hash.keys() {
            keys.push(Value::Str(Rc::from(k.as_str())));
        }
        keys
    }

    pub fn values(&self) -> Vec<&Value> {
        let mut vals: Vec<&Value> = Vec::with_capacity(self.len());
        for v in &self.array {
            vals.push(v);
        }
        for v in self.hash.values() {
            vals.push(v);
        }
        vals
    }
}

// ── MoofObject ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MoofObject {
    pub class: Rc<RefCell<MoofClass>>,
    pub fields: Vec<Value>,
}

impl MoofObject {
    pub fn get_field(&self, index: usize) -> Option<&Value> {
        self.fields.get(index)
    }

    pub fn set_field(&mut self, index: usize, val: Value) {
        if index < self.fields.len() {
            self.fields[index] = val;
        }
    }
}

// ── MoofClass ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MoofClass {
    pub name: SymId,
    pub superclass: Option<Rc<RefCell<MoofClass>>>,
    pub metaclass: Option<Rc<RefCell<MoofClass>>>,
    pub methods: std::collections::HashMap<SymId, Value>,
    pub field_names: Vec<SymId>,
    pub is_meta: bool,
}

impl MoofClass {
    pub fn new(name: SymId) -> Self {
        MoofClass {
            name,
            superclass: None,
            metaclass: None,
            methods: std::collections::HashMap::new(),
            field_names: Vec::new(),
            is_meta: false,
        }
    }

    pub fn lookup(&self, selector: SymId) -> Option<Value> {
        if let Some(method) = self.methods.get(&selector) {
            return Some(method.clone());
        }
        if let Some(ref sup) = self.superclass {
            return sup.borrow().lookup(selector);
        }
        None
    }

    /// Lookup a method, also returning the name of the class that owns it.
    /// Used for super sends to know which class the currently-executing method belongs to.
    pub fn lookup_owner(&self, selector: SymId) -> Option<(Value, SymId)> {
        if let Some(method) = self.methods.get(&selector) {
            return Some((method.clone(), self.name));
        }
        if let Some(ref sup) = self.superclass {
            return sup.borrow().lookup_owner(selector);
        }
        None
    }

    pub fn add_method(&mut self, selector: SymId, closure: Value) {
        self.methods.insert(selector, closure);
    }

    pub fn field_index(&self, name: SymId) -> Option<usize> {
        self.all_field_names().iter().position(|&n| n == name)
    }

    pub fn all_field_names(&self) -> Vec<SymId> {
        let mut fields = if let Some(ref sup) = self.superclass {
            sup.borrow().all_field_names()
        } else {
            Vec::new()
        };
        fields.extend_from_slice(&self.field_names);
        fields
    }
}

// ── MoofClosure ──────────────────────────────────────────────────────

pub struct MoofClosure {
    pub name: Option<SymId>,
    pub params: Vec<SymId>,
    pub rest_param: Option<SymId>,
    pub body: ClosureBody,
    pub env: crate::environment::Env,
    /// Captured upvalues for bytecode closures (empty for Expr/Native closures)
    pub upvalues: Vec<std::rc::Rc<std::cell::RefCell<Value>>>,
}

pub enum ClosureBody {
    Expr(Value),
    Native(NativeFn),
    Bytecode(std::rc::Rc<crate::bytecode::CompiledFunction>),
}

pub type NativeFn = fn(&mut crate::interpreter::Interpreter, Vec<Value>) -> crate::error::Result<Value>;

impl fmt::Debug for MoofClosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MoofClosure")
            .field("name", &self.name)
            .field("params", &self.params)
            .field("rest_param", &self.rest_param)
            .field("body", &self.body)
            .finish()
    }
}

impl fmt::Debug for ClosureBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClosureBody::Expr(_) => write!(f, "Expr(...)"),
            ClosureBody::Native(_) => write!(f, "Native"),
            ClosureBody::Bytecode(cf) => write!(f, "Bytecode({:?})", cf),
        }
    }
}

impl Clone for ClosureBody {
    fn clone(&self) -> Self {
        match self {
            ClosureBody::Expr(v) => ClosureBody::Expr(v.clone()),
            ClosureBody::Native(f) => ClosureBody::Native(*f),
            ClosureBody::Bytecode(cf) => ClosureBody::Bytecode(cf.clone()),
        }
    }
}

impl Clone for MoofClosure {
    fn clone(&self) -> Self {
        MoofClosure {
            name: self.name,
            params: self.params.clone(),
            rest_param: self.rest_param,
            body: self.body.clone(),
            env: self.env.clone(),
            upvalues: self.upvalues.clone(),
        }
    }
}

// ── Value constructors and accessors ─────────────────────────────────

impl Value {
    /// Create a Cons cell from car and cdr.
    pub fn cons(car: Value, cdr: Value) -> Value {
        Value::Cons(Rc::new(ConsCell { car, cdr }))
    }

    /// Extract the car of a cons cell.
    pub fn car(&self) -> crate::error::Result<&Value> {
        match self {
            Value::Cons(cell) => Ok(&cell.car),
            other => Err(crate::error::MoofError::runtime(format!(
                "car: expected Cons, got {}",
                other.type_name()
            ))),
        }
    }

    /// Extract the cdr of a cons cell.
    pub fn cdr(&self) -> crate::error::Result<&Value> {
        match self {
            Value::Cons(cell) => Ok(&cell.cdr),
            other => Err(crate::error::MoofError::runtime(format!(
                "cdr: expected Cons, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn is_cons(&self) -> bool {
        matches!(self, Value::Cons(_))
    }

    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Bool(false) | Value::Nil)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Integer(_) => "Integer",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Nil => "Nil",
            Value::Symbol(_) => "Symbol",
            Value::Str(_) => "String",
            Value::Cons(_) => "Cons",
            Value::Table(_) => "Table",
            Value::Object(_) => "Object",
            Value::Closure(_) => "Closure",
            Value::Range(_) => "Range",
        }
    }

    /// Iterate over elements of a proper cons list.
    pub fn iter_list(&self) -> crate::cons::ListIter {
        crate::cons::ListIter::new(self)
    }

    /// Collect a proper cons list into a Vec. Returns error if not Cons or Nil.
    pub fn to_vec(&self) -> crate::error::Result<Vec<Value>> {
        match self {
            Value::Nil => Ok(Vec::new()),
            Value::Cons(_) => Ok(crate::cons::cons_to_vec(self)),
            other => Err(crate::error::MoofError::runtime(format!(
                "to_vec: expected list, got {}",
                other.type_name()
            ))),
        }
    }

    /// Build a cons list from a slice.
    pub fn from_slice(items: &[Value]) -> Value {
        crate::cons::vec_to_cons(items)
    }

    /// Like Display but strings get quotes.
    pub fn inspect(&self) -> String {
        match self {
            Value::Str(s) => format!("{:?}", &**s),
            Value::Range(_) => format!("{self}"),
            other => format!("{other}"),
        }
    }

    // ── Typed accessors ──────────────────────────────────────────────

    pub fn as_int(&self) -> crate::error::Result<&MoofInt> {
        match self {
            Value::Integer(n) => Ok(n),
            other => Err(crate::error::MoofError::runtime(format!(
                "Expected Integer, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn as_float(&self) -> crate::error::Result<f64> {
        match self {
            Value::Float(n) => Ok(*n),
            other => Err(crate::error::MoofError::runtime(format!(
                "Expected Float, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn as_str(&self) -> crate::error::Result<&str> {
        match self {
            Value::Str(s) => Ok(s),
            other => Err(crate::error::MoofError::runtime(format!(
                "Expected String, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn as_symbol(&self) -> crate::error::Result<SymId> {
        match self {
            Value::Symbol(id) => Ok(*id),
            other => Err(crate::error::MoofError::runtime(format!(
                "Expected Symbol, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn as_number_f64(&self) -> crate::error::Result<f64> {
        match self {
            Value::Integer(n) => Ok(n.to_f64()),
            Value::Float(n) => Ok(*n),
            other => Err(crate::error::MoofError::runtime(format!(
                "Expected number, got {}",
                other.type_name()
            ))),
        }
    }
}

// ── Display ──────────────────────────────────────────────────────────

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{n}"),
            Value::Float(n) => {
                if n.fract() == 0.0 && !n.is_nan() && !n.is_infinite() {
                    write!(f, "{n:.1}")
                } else {
                    write!(f, "{n}")
                }
            }
            Value::Bool(b) => write!(f, "{b}"),
            Value::Nil => write!(f, "nil"),
            Value::Symbol(id) => write!(f, "#<symbol:{id}>"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Cons(_) => crate::cons::cons_display(self, f),
            Value::Table(tbl) => {
                let tbl = tbl.borrow();
                let has_array = !tbl.array.is_empty();
                let has_hash = !tbl.hash.is_empty();
                write!(f, "{{")?;
                let mut first = true;
                if has_array {
                    for v in &tbl.array {
                        if !first { write!(f, ", ")?; }
                        first = false;
                        write!(f, "{v}")?;
                    }
                }
                if has_hash {
                    for (k, v) in &tbl.hash {
                        if !first { write!(f, ", ")?; }
                        first = false;
                        write!(f, "{k}: {v}")?;
                    }
                }
                write!(f, "}}")
            }
            Value::Object(obj) => {
                let obj = obj.borrow();
                let class = obj.class.borrow();
                write!(f, "#<{}>", class.name)
            }
            Value::Closure(c) => {
                let arity = c.params.len();
                if let Some(name) = c.name {
                    write!(f, "<{name}/{arity}>")
                } else {
                    write!(f, "<lambda/{arity}>")
                }
            }
            Value::Range(r) => {
                let step_i64 = r.step.to_i64().unwrap_or(1);
                if step_i64 != 1 {
                    write!(f, "{}..{}:{}", r.start, r.end, r.step)
                } else {
                    write!(f, "{}..{}", r.start, r.end)
                }
            }
        }
    }
}

// ── PartialEq ────────────────────────────────────────────────────────

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            // Cross-numeric comparison: promote int to f64
            (Value::Integer(a), Value::Float(b)) => a.to_f64() == *b,
            (Value::Float(a), Value::Integer(b)) => *a == b.to_f64(),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Cons(a), Value::Cons(b)) => a.car == b.car && a.cdr == b.cdr,
            (Value::Table(a), Value::Table(b)) => {
                let a = a.borrow();
                let b = b.borrow();
                a.array == b.array && a.hash == b.hash
            }
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            // Closures are not comparable
            (Value::Closure(_), Value::Closure(_)) => false,
            (Value::Range(a), Value::Range(b)) => {
                a.start == b.start && a.end == b.end && a.step == b.step
            }
            _ => false,
        }
    }
}

// ── Hash ─────────────────────────────────────────────────────────────

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Type tag to distinguish variants
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Integer(n) => n.hash(state),
            Value::Float(n) => n.to_bits().hash(state),
            Value::Bool(b) => b.hash(state),
            Value::Nil => {}
            Value::Symbol(id) => id.hash(state),
            Value::Str(s) => s.hash(state),
            Value::Cons(cell) => {
                cell.car.hash(state);
                cell.cdr.hash(state);
            }
            Value::Table(tbl) => {
                let tbl = tbl.borrow();
                tbl.array.len().hash(state);
                for v in &tbl.array {
                    v.hash(state);
                }
                tbl.hash.len().hash(state);
                for (k, v) in &tbl.hash {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::Object(obj) => {
                // Pointer identity
                (Rc::as_ptr(obj) as usize).hash(state);
            }
            Value::Closure(c) => {
                // Pointer identity
                (Rc::as_ptr(c) as usize).hash(state);
            }
            Value::Range(r) => {
                r.start.hash(state);
                r.end.hash(state);
                r.step.hash(state);
            }
        }
    }
}
