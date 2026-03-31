use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::symbol::SymId;
use crate::value::Value;

#[derive(Clone, Debug)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Debug)]
struct EnvInner {
    bindings: HashMap<SymId, Binding>,
    parent: Option<Env>,
}

/// Lexical environment with scope chain, keyed by interned SymId for fast lookup.
#[derive(Clone, Debug)]
pub struct Env(Rc<RefCell<EnvInner>>);

impl Env {
    /// Create a root environment with no parent.
    pub fn new() -> Self {
        Env(Rc::new(RefCell::new(EnvInner {
            bindings: HashMap::new(),
            parent: None,
        })))
    }

    /// Create a child environment with `self` as the parent scope.
    pub fn child(&self) -> Self {
        Env(Rc::new(RefCell::new(EnvInner {
            bindings: HashMap::new(),
            parent: Some(self.clone()),
        })))
    }

    /// Add a binding to THIS scope (not parents).
    pub fn define(&self, name: SymId, value: Value, mutable: bool) {
        self.0.borrow_mut().bindings.insert(
            name,
            Binding { value, mutable },
        );
    }

    /// Walk the parent chain looking for `name`. Returns the value if found.
    /// Error message includes the raw SymId; the interpreter can enhance it
    /// with the actual symbol name via the SymbolTable.
    pub fn get(&self, name: SymId) -> crate::error::Result<Value> {
        let inner = self.0.borrow();
        if let Some(binding) = inner.bindings.get(&name) {
            Ok(binding.value.clone())
        } else if let Some(ref parent) = inner.parent {
            parent.get(name)
        } else {
            Err(crate::error::MoofError::name(
                format!("Undefined variable (SymId {name})")
            ))
        }
    }

    /// Find a binding in the scope chain and mutate it. Returns the value on success.
    /// Errors if the binding is not found or is immutable.
    pub fn set(&self, name: SymId, value: Value) -> crate::error::Result<Value> {
        let mut inner = self.0.borrow_mut();
        if let Some(binding) = inner.bindings.get_mut(&name) {
            if !binding.mutable {
                return Err(crate::error::MoofError::runtime(
                    format!("Cannot mutate immutable binding (SymId {name})")
                ));
            }
            binding.value = value.clone();
            Ok(value)
        } else if let Some(ref parent) = inner.parent {
            parent.set(name, value)
        } else {
            Err(crate::error::MoofError::name(
                format!("Undefined variable (SymId {name})")
            ))
        }
    }

    /// Snapshot of THIS scope's bindings (not parents). For REPL introspection.
    pub fn bindings(&self) -> Vec<(SymId, Value)> {
        self.0
            .borrow()
            .bindings
            .iter()
            .map(|(&id, b)| (id, b.value.clone()))
            .collect()
    }
}
