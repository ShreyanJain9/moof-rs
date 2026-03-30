use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use crate::value::Value;
use crate::error::{MoofError, Result};

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Debug)]
struct EnvInner {
    bindings: HashMap<String, Binding>,
    parent: Option<Env>,
}

/// Lexical environment with scope chain.
#[derive(Debug, Clone)]
pub struct Env(Rc<RefCell<EnvInner>>);

impl Env {
    pub fn new() -> Self {
        Env(Rc::new(RefCell::new(EnvInner {
            bindings: HashMap::new(),
            parent: None,
        })))
    }

    pub fn child(&self) -> Self {
        Env(Rc::new(RefCell::new(EnvInner {
            bindings: HashMap::new(),
            parent: Some(self.clone()),
        })))
    }

    pub fn define(&self, name: &str, value: Value, mutable: bool) {
        self.0.borrow_mut().bindings.insert(
            name.to_string(),
            Binding { value, mutable },
        );
    }

    pub fn get(&self, name: &str) -> Result<Value> {
        let inner = self.0.borrow();
        if let Some(binding) = inner.bindings.get(name) {
            Ok(binding.value.clone())
        } else if let Some(ref parent) = inner.parent {
            parent.get(name)
        } else {
            Err(MoofError::name(name, None, None))
        }
    }

    pub fn set(&self, name: &str, value: Value) -> Result<Value> {
        let mut inner = self.0.borrow_mut();
        if let Some(binding) = inner.bindings.get_mut(name) {
            if !binding.mutable {
                return Err(MoofError::immutable(name));
            }
            binding.value = value.clone();
            Ok(value)
        } else if let Some(ref parent) = inner.parent {
            parent.set(name, value)
        } else {
            Err(MoofError::name(name, None, None))
        }
    }

    /// Returns all bindings in this scope (not parents). For REPL introspection.
    pub fn bindings(&self) -> HashMap<String, Value> {
        self.0.borrow().bindings.iter()
            .map(|(k, b)| (k.clone(), b.value.clone()))
            .collect()
    }
}
