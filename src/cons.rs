use std::fmt;
use std::rc::Rc;

use crate::value::Value;

#[derive(Clone, Debug)]
pub struct ConsCell {
    pub car: Value,
    pub cdr: Value,
}

/// Iterator over elements of a proper cons list.
pub struct ListIter<'a> {
    current: &'a Value,
}

impl<'a> ListIter<'a> {
    pub fn new(val: &'a Value) -> Self {
        ListIter { current: val }
    }
}

impl<'a> Iterator for ListIter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current {
            Value::Cons(cell) => {
                let car = &cell.car;
                self.current = &cell.cdr;
                Some(car)
            }
            Value::Nil => None,
            // Improper list — stop iteration
            _ => None,
        }
    }
}

/// Collect a proper cons list into a Vec.
pub fn cons_to_vec(val: &Value) -> Vec<Value> {
    let mut result = Vec::new();
    let mut current = val;
    loop {
        match current {
            Value::Cons(cell) => {
                result.push(cell.car.clone());
                current = &cell.cdr;
            }
            _ => break,
        }
    }
    result
}

/// Build a cons chain from a slice (right-to-left fold).
/// An empty slice produces Nil.
pub fn vec_to_cons(items: &[Value]) -> Value {
    let mut result = Value::Nil;
    for item in items.iter().rev() {
        result = Value::Cons(Rc::new(ConsCell {
            car: item.clone(),
            cdr: result,
        }));
    }
    result
}

/// Count elements in a proper cons list.
pub fn cons_length(val: &Value) -> usize {
    let mut count = 0;
    let mut current = val;
    loop {
        match current {
            Value::Cons(cell) => {
                count += 1;
                current = &cell.cdr;
            }
            _ => break,
        }
    }
    count
}

/// Display a cons structure.
/// Proper lists: `(1 2 3)`
/// Improper pairs: `(1 . 2)`
pub fn cons_display(val: &Value, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "(")?;

    let mut current = val;
    let mut first = true;

    loop {
        match current {
            Value::Cons(cell) => {
                if !first {
                    write!(f, " ")?;
                }
                first = false;
                write!(f, "{}", cell.car)?;
                current = &cell.cdr;
            }
            Value::Nil => break,
            // Improper list — dot notation
            other => {
                write!(f, " . {}", other)?;
                break;
            }
        }
    }

    write!(f, ")")
}
