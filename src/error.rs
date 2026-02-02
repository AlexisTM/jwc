use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Malformed input.
    Syntax(String),
    /// Nesting deeper than `ParseOptions::max_depth`.
    DepthExceeded(usize),
    /// The same key twice in one object (see `DuplicateKeys`).
    DuplicateKey(String),
    /// Not a JSON number, or a float outside the f64 range.
    Number(String),
    /// A value of the wrong type for what was asked.
    Type(String),
    /// A JSON Pointer that does not resolve.
    Pointer(String),
    /// A JSON Patch operation that cannot be applied.
    Patch(String),
    /// An unsupported formatting request, or a failed write.
    Format(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(m)
            | Self::Number(m)
            | Self::Type(m)
            | Self::Pointer(m)
            | Self::Patch(m)
            | Self::Format(m) => f.write_str(m),
            Self::DepthExceeded(max) => write!(f, "nesting deeper than {max} levels"),
            Self::DuplicateKey(key) => write!(f, "duplicate key {key:?}"),
        }
    }
}

/// What went wrong, and where in the source when that is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// 1-based; 0 when the error has no position in the source.
    pub line: usize,
    pub column: usize,
}

impl Error {
    #[must_use]
    pub const fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            line: 0,
            column: 0,
        }
    }

    #[must_use]
    pub const fn at(kind: ErrorKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }

    pub fn syntax(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Syntax(message.into()))
    }

    pub fn type_mismatch(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Type(message.into()))
    }

    pub fn pointer(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Pointer(message.into()))
    }

    pub fn patch(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Patch(message.into()))
    }

    pub fn format(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Format(message.into()))
    }

    #[must_use]
    pub const fn is_positional(&self) -> bool {
        self.line > 0
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_positional() {
            write!(f, "{} at {}:{}", self.kind, self.line, self.column)
        } else {
            write!(f, "{}", self.kind)
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
