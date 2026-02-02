use crate::number::Number;
use std::fmt;

/// Represents a value in the JSONC document.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Node>),
    Object(Vec<ObjectEntry>),
    #[cfg(feature = "lazy")]
    Lazy(Box<crate::lazy::LazyValue>),
}

/// Where a node or member sat in the parsed text, with a fingerprint of what
/// was there. Layout-preserving output copies the span verbatim while the
/// fingerprint still matches. Ignored by `PartialEq`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub span: std::ops::Range<usize>,
    pub fingerprint: u64,
    /// An empty line separated this element or member from what came before.
    /// Pretty output keeps it.
    pub blank_line_before: bool,
}

#[derive(Clone, Debug)]
pub struct ObjectEntry {
    pub key: String,
    pub key_trivia: Vec<Trivia>,
    pub value: Node,
    pub source: Option<Source>,
}

impl PartialEq for ObjectEntry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.key_trivia == other.key_trivia && self.value == other.value
    }
}

/// Represents different types of trivia (comments only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trivia {
    LineComment(String),
    BlockComment(String),
}

impl fmt::Display for Trivia {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineComment(c) => write!(f, "//{c}"),
            Self::BlockComment(c) => write!(f, "/*{c}*/"),
        }
    }
}

/// A node in the AST: a value plus the comments around it, kept by position.
#[derive(Clone, Debug)]
pub struct Node {
    pub value: Value,
    /// Comments before the value. For an object member, the comments between
    /// the `:` and the value; comments before the key live in
    /// [`ObjectEntry::key_trivia`].
    pub trivia: Vec<Trivia>,
    /// Comments after the value and its comma, on the same line. On the root
    /// node: every comment after the document.
    pub trailing: Vec<Trivia>,
    /// Comments after the last element of an array or object, before the
    /// closing bracket. Always empty on scalars.
    pub dangling: Vec<Trivia>,
    pub comma: bool,
    /// Set by the parser; see [`Source`].
    pub source: Option<Source>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.trivia == other.trivia
            && self.trailing == other.trailing
            && self.dangling == other.dangling
            && self.comma == other.comma
    }
}

impl Node {
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self {
            value,
            trivia: Vec::new(),
            trailing: Vec::new(),
            dangling: Vec::new(),
            comma: false,
            source: None,
        }
    }
}

impl ObjectEntry {
    #[must_use]
    pub const fn new(key: String, value: Node) -> Self {
        Self {
            key,
            key_trivia: Vec::new(),
            value,
            source: None,
        }
    }
}

// Helpers for easier construction
impl From<bool> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<Number> for Value {
    fn from(n: Number) -> Self {
        Self::Number(n)
    }
}

macro_rules! value_from_number {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(n: $t) -> Self {
                Self::Number(Number::from(n))
            }
        }
    )*};
}

value_from_number!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);

impl From<String> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl Node {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    #[must_use]
    pub fn new_with_comments(value: Value, comments: Vec<&str>) -> Self {
        let mut node = Self::new(value);
        for c in comments {
            node.add_line_comment(c);
        }
        node
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn add_line_comment(&mut self, comment: &str) {
        let c = comment.trim_start_matches("//").to_string();
        self.trivia.push(Trivia::LineComment(c));
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn add_block_comment(&mut self, comment: &str) {
        let c = comment
            .trim_start_matches("/*")
            .trim_end_matches("*/")
            .to_string();
        self.trivia.push(Trivia::BlockComment(c));
    }

    /// A line comment on the same line, after the value and its comma.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn add_trailing_comment(&mut self, comment: &str) {
        let c = comment.trim_start_matches("//").to_string();
        self.trailing.push(Trivia::LineComment(c));
    }

    /// A line comment on its own line before this container's closing bracket.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn add_dangling_comment(&mut self, comment: &str) {
        let c = comment.trim_start_matches("//").to_string();
        self.dangling.push(Trivia::LineComment(c));
    }
}

impl Trivia {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    #[must_use]
    pub fn as_line_comment(&self) -> Option<String> {
        if let Self::LineComment(c) = self {
            Some(c.clone())
        } else {
            None
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    #[must_use]
    pub fn as_block_comment(&self) -> Option<String> {
        if let Self::BlockComment(c) = self {
            Some(c.clone())
        } else {
            None
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn make_line_comment(&mut self) {
        if let Self::BlockComment(c) = self {
            *self = Self::LineComment(c.clone());
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn make_block_comment(&mut self) {
        if let Self::LineComment(c) = self {
            *self = Self::BlockComment(c.clone());
        }
    }
}

impl Value {
    /// Append to an array, giving the previous last element its comma.
    ///
    /// # Errors
    /// `ErrorKind::Type` when `self` is not an array.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn push(&mut self, node: Node) -> crate::Result<()> {
        if let Self::Array(elements) = self {
            if let Some(last) = elements.last_mut() {
                last.comma = true;
            }
            elements.push(node);
            Ok(())
        } else {
            Err(crate::Error::type_mismatch("Not an array"))
        }
    }

    /// Append a member to an object, giving the previous last member its comma.
    ///
    /// # Errors
    /// `ErrorKind::Type` when `self` is not an object.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn insert(&mut self, key: &str, node: Node) -> crate::Result<&mut ObjectEntry> {
        if let Self::Object(members) = self {
            if let Some(last) = members.last_mut() {
                last.value.comma = true;
            }
            let index = members.len();
            members.push(ObjectEntry::new(key.to_string(), node));
            Ok(&mut members[index])
        } else {
            Err(crate::Error::type_mismatch("Not an object"))
        }
    }
}

#[cfg(feature = "lazy")]
impl From<crate::lazy::LazyValue> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(v: crate::lazy::LazyValue) -> Self {
        Self::Lazy(Box::new(v))
    }
}
impl ObjectEntry {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn add_key_comment(&mut self, comment: &str) {
        let c = comment.trim_start_matches("//").to_string();
        self.key_trivia.push(Trivia::LineComment(c));
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn add_key_block_comment(&mut self, comment: &str) {
        let c = comment
            .trim_start_matches("/*")
            .trim_end_matches("*/")
            .to_string();
        self.key_trivia.push(Trivia::BlockComment(c));
    }
}
