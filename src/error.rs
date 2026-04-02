use std::fmt;

#[derive(Clone, Debug)]
pub struct MoofError {
    pub kind: ErrorKind,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub error_object: Option<crate::value::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ErrorKind {
    Syntax,
    Runtime,
    Name,
    Message,
    Arity,
    Type,
    IO,
    /// Internal control flow for condition/restart system.
    /// error_object carries a table with "name" and "args" fields.
    RestartInvoked,
}

pub type Result<T> = std::result::Result<T, MoofError>;

impl MoofError {
    pub fn syntax(msg: impl Into<String>, line: usize, col: usize) -> Self {
        MoofError {
            kind: ErrorKind::Syntax,
            message: msg.into(),
            line: Some(line),
            column: Some(col),
            error_object: None,
        }
    }

    pub fn syntax_simple(msg: impl Into<String>) -> Self {
        MoofError {
            kind: ErrorKind::Syntax,
            message: msg.into(),
            line: None,
            column: None,
            error_object: None,
        }
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        MoofError {
            kind: ErrorKind::Runtime,
            message: msg.into(),
            line: None,
            column: None,
            error_object: None,
        }
    }

    pub fn name(msg: impl Into<String>) -> Self {
        MoofError {
            kind: ErrorKind::Name,
            message: msg.into(),
            line: None,
            column: None,
            error_object: None,
        }
    }

    pub fn name_with_loc(msg: impl Into<String>, line: Option<usize>, col: Option<usize>) -> Self {
        MoofError {
            kind: ErrorKind::Name,
            message: msg.into(),
            line,
            column: col,
            error_object: None,
        }
    }

    pub fn message(receiver_type: &str, selector: &str, suggestion: Option<&str>) -> Self {
        let mut msg = format!("{receiver_type} does not respond to '{selector}'");
        if let Some(s) = suggestion {
            msg.push_str(&format!("\n  Did you mean: {s}?"));
        }
        MoofError {
            kind: ErrorKind::Message,
            message: msg,
            line: None,
            column: None,
            error_object: None,
        }
    }

    pub fn arity(expected: &str, got: usize, name: Option<&str>) -> Self {
        let fn_name = name.map(|n| format!(" for '{n}'")).unwrap_or_default();
        MoofError {
            kind: ErrorKind::Arity,
            message: format!("Wrong number of arguments{fn_name}: expected {expected}, got {got}"),
            line: None,
            column: None,
            error_object: None,
        }
    }

    pub fn type_error(msg: impl Into<String>) -> Self {
        MoofError {
            kind: ErrorKind::Type,
            message: msg.into(),
            line: None,
            column: None,
            error_object: None,
        }
    }

    pub fn io(msg: impl Into<String>) -> Self {
        MoofError {
            kind: ErrorKind::IO,
            message: msg.into(),
            line: None,
            column: None,
            error_object: None,
        }
    }

    /// Attach source location if not already present.
    pub fn with_loc(mut self, line: Option<usize>, col: Option<usize>) -> Self {
        if self.line.is_none() {
            self.line = line;
        }
        if self.column.is_none() {
            self.column = col;
        }
        self
    }

    /// Attach a Moof error object to this error.
    pub fn with_object(mut self, obj: crate::value::Value) -> Self {
        self.error_object = Some(obj);
        self
    }
}

impl fmt::Display for MoofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(line) = self.line {
            write!(f, " at line {line}")?;
            if let Some(col) = self.column {
                write!(f, ":{col}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for MoofError {}
