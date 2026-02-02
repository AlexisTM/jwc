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

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectEntry {
    pub key: String,
    pub key_trivia: Vec<Trivia>,
    pub value: Node,
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

/// A node in the AST, wrapping a value with its associated comments.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub value: Value,
    pub trivia: Vec<Trivia>,
    pub comma: bool,
}

impl Node {
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self {
            value,
            trivia: Vec::new(),
            comma: false,
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

impl From<i32> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(n: i32) -> Self {
        Self::Number(Number::from(n))
    }
}

impl From<f64> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(n: f64) -> Self {
        Self::Number(Number::from(n))
    }
}

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
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn push(&mut self, node: Node) -> Result<(), String> {
        if let Self::Array(elements) = self {
            if let Some(last) = elements.last_mut() {
                last.comma = true;
            }
            elements.push(node);
            Ok(())
        } else {
            Err("Not an array".to_string())
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn insert(&mut self, key: &str, node: Node) -> Result<&mut ObjectEntry, String> {
        if let Self::Object(members) = self {
            if let Some(last) = members.last_mut() {
                last.value.comma = true;
            }
            let entry = ObjectEntry::new(key.to_string(), node);
            members.push(entry);
            Ok(members.last_mut().unwrap())
        } else {
            Err("Not an object".to_string())
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
