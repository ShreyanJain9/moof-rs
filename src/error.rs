use std::fmt;

#[derive(Debug, Clone)]
pub struct MoofError {
    pub kind: ErrorKind,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    Syntax,
    Runtime,
    Name,
    Message,
    Arity,
    ImmutableBinding,
    Type,
}

impl MoofError {
    pub fn syntax(msg: impl Into<String>, line: usize, col: usize) -> Self {
        MoofError { kind: ErrorKind::Syntax, message: msg.into(), line: Some(line), column: Some(col) }
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        MoofError { kind: ErrorKind::Runtime, message: msg.into(), line: None, column: None }
    }

    pub fn name(name: &str, line: Option<usize>, col: Option<usize>) -> Self {
        MoofError { kind: ErrorKind::Name, message: format!("Undefined variable: {name}"), line, column: col }
    }

    pub fn message(receiver: &str, selector: &str, suggestion: Option<&str>) -> Self {
        let mut msg = format!("{receiver} does not respond to '{selector}'");
        if let Some(s) = suggestion {
            msg.push_str(&format!("\n  Did you mean: {s}?"));
        }
        MoofError { kind: ErrorKind::Message, message: msg, line: None, column: None }
    }

    pub fn arity(expected: &str, got: usize, name: Option<&str>) -> Self {
        let fn_name = name.map(|n| format!(" for '{n}'")).unwrap_or_default();
        MoofError {
            kind: ErrorKind::Arity,
            message: format!("Wrong number of arguments{fn_name}: expected {expected}, got {got}"),
            line: None, column: None,
        }
    }

    pub fn immutable(name: &str) -> Self {
        MoofError {
            kind: ErrorKind::ImmutableBinding,
            message: format!("Cannot mutate immutable binding: {name}"),
            line: None, column: None,
        }
    }
}

impl MoofError {
    /// Attach source location if not already present.
    pub fn with_loc(mut self, line: Option<usize>, col: Option<usize>) -> Self {
        if self.line.is_none() { self.line = line; }
        if self.column.is_none() { self.column = col; }
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

pub type Result<T> = std::result::Result<T, MoofError>;
