use std::fmt;

/// Structured error for every failure surface in `jwc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Parse error with 1-based source position.
    Parse {
        line: usize,
        col: usize,
        msg: String,
    },
    /// Wrong variant of `Value` encountered while coercing.
    Type {
        expected: &'static str,
        got: &'static str,
        path: String,
    },
    /// Required object field not found during deserialization.
    MissingField { name: String, path: String },
    /// RFC 6901 pointer failure.
    Pointer { path: String, reason: String },
    /// RFC 6902 patch failure.
    Patch { path: String, reason: String },
    /// Free-form message for uncategorized errors.
    Custom(String),
}

impl Error {
    pub fn parse(line: usize, col: usize, msg: impl Into<String>) -> Self {
        Self::Parse {
            line,
            col,
            msg: msg.into(),
        }
    }
    pub fn ty(expected: &'static str, got: &'static str) -> Self {
        Self::Type {
            expected,
            got,
            path: String::new(),
        }
    }
    pub fn ty_at(expected: &'static str, got: &'static str, path: impl Into<String>) -> Self {
        Self::Type {
            expected,
            got,
            path: path.into(),
        }
    }
    pub fn missing_field(name: impl Into<String>) -> Self {
        Self::MissingField {
            name: name.into(),
            path: String::new(),
        }
    }
    pub fn pointer(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Pointer {
            path: path.into(),
            reason: reason.into(),
        }
    }
    pub fn patch(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Patch {
            path: path.into(),
            reason: reason.into(),
        }
    }
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { line, col, msg } => write!(f, "{msg} at {line}:{col}"),
            Self::Type {
                expected,
                got,
                path,
            } => {
                if path.is_empty() {
                    write!(f, "expected {expected}, got {got}")
                } else {
                    write!(f, "expected {expected}, got {got} at {path}")
                }
            }
            Self::MissingField { name, path } => {
                if path.is_empty() {
                    write!(f, "missing field `{name}`")
                } else {
                    write!(f, "missing field `{name}` at {path}")
                }
            }
            Self::Pointer { path, reason } => write!(f, "pointer error at {path}: {reason}"),
            Self::Patch { path, reason } => write!(f, "patch error at {path}: {reason}"),
            Self::Custom(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Custom(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::Custom(s.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Custom(format!("io error: {e}"))
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::Custom(format!("utf8 error: {e}"))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
