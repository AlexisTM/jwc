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

/// A single comment attached to a node or object key.
///
/// Content is stored verbatim (no `//` or `/* */` markers). The serializer
/// adds the markers. Example:
///
/// ```
/// use jwc::Trivia;
/// let t = Trivia::line(" note");        // serializes as `// note\n`
/// let t = Trivia::block(" wrap me ");   // serializes as `/* wrap me */`
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trivia {
    LineComment(String),
    BlockComment(String),
}

impl Trivia {
    /// Build a line comment. Content stored verbatim; serializer adds `//` + newline.
    pub fn line(text: impl Into<String>) -> Self {
        Self::LineComment(text.into())
    }

    /// Build a block comment. Content stored verbatim; serializer adds `/*` and `*/`.
    pub fn block(text: impl Into<String>) -> Self {
        Self::BlockComment(text.into())
    }

    #[must_use]
    pub fn is_line(&self) -> bool {
        matches!(self, Self::LineComment(_))
    }

    #[must_use]
    pub fn is_block(&self) -> bool {
        matches!(self, Self::BlockComment(_))
    }

    /// Verbatim comment content (without markers).
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::LineComment(s) | Self::BlockComment(s) => s,
        }
    }
}

impl From<&str> for Trivia {
    fn from(s: &str) -> Self {
        Self::LineComment(s.to_string())
    }
}

impl From<String> for Trivia {
    fn from(s: String) -> Self {
        Self::LineComment(s)
    }
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
}

impl Node {
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self {
            value,
            trivia: Vec::new(),
        }
    }

    /// Attach a comment. Accepts `Trivia` or `&str` (latter becomes a line comment).
    ///
    /// ```
    /// use jwc::{Node, Trivia, Value};
    /// let mut n = Node::new(Value::from(1));
    /// n.comment(" a line comment");             // via &str
    /// n.comment(Trivia::block(" block "));      // explicit block
    /// ```
    pub fn comment(&mut self, t: impl Into<Trivia>) -> &mut Self {
        self.trivia.push(t.into());
        self
    }

    /// Builder-style. Attach a comment and return `self`.
    #[must_use]
    pub fn with_comment(mut self, t: impl Into<Trivia>) -> Self {
        self.comment(t);
        self
    }

    #[must_use]
    pub fn comments(&self) -> &[Trivia] {
        &self.trivia
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

    /// Attach a comment to the key slot (appears before the key in the output).
    pub fn key_comment(&mut self, t: impl Into<Trivia>) -> &mut Self {
        self.key_trivia.push(t.into());
        self
    }

    /// Builder-style. Attach a key comment and return `self`.
    #[must_use]
    pub fn with_key_comment(mut self, t: impl Into<Trivia>) -> Self {
        self.key_comment(t);
        self
    }

    #[must_use]
    pub fn key_comments(&self) -> &[Trivia] {
        &self.key_trivia
    }
}

// --- From conversions for easier construction ---

impl From<bool> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

macro_rules! impl_from_number {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for Value {
                #[cfg_attr(feature = "profiling", hotpath::measure)]
                fn from(n: $t) -> Self {
                    Self::Number(Number::from(n))
                }
            }
        )*
    };
}

impl_from_number!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);

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

impl From<Vec<Node>> for Value {
    fn from(v: Vec<Node>) -> Self {
        Self::Array(v)
    }
}

impl From<Vec<ObjectEntry>> for Value {
    fn from(v: Vec<ObjectEntry>) -> Self {
        Self::Object(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

// Any `Into<Value>` can become a `Node` via this blanket impl.
// Example: `Node::from(42)`, `Node::from("x")`, `Node::from(true)`.
impl<V: Into<Value>> From<V> for Node {
    fn from(v: V) -> Self {
        Self::new(v.into())
    }
}

impl Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn push(&mut self, node: Node) -> Result<(), String> {
        if let Self::Array(elements) = self {
            elements.push(node);
            Ok(())
        } else {
            Err("Not an array".to_string())
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn insert(&mut self, key: &str, node: Node) -> Result<&mut ObjectEntry, String> {
        if let Self::Object(members) = self {
            let entry = ObjectEntry::new(key.to_string(), node);
            members.push(entry);
            Ok(members.last_mut().unwrap())
        } else {
            Err("Not an object".to_string())
        }
    }

    // --- type predicates ---

    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    #[must_use]
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }
    #[must_use]
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }
    #[must_use]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }
    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    // --- typed accessors ---

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Number(n) = self {
            n.as_i64()
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        if let Self::Number(n) = self {
            n.as_u64()
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Number(n) = self {
            n.as_f64().ok()
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_number(&self) -> Option<&Number> {
        if let Self::Number(n) = self {
            Some(n)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Node]> {
        if let Self::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Node>> {
        if let Self::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_object(&self) -> Option<&[ObjectEntry]> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Vec<ObjectEntry>> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }

    // --- keyed / indexed lookup ---

    /// Look up an object key or array index. Returns `None` if `self` is not
    /// the matching container, or if the key/index is absent.
    #[must_use]
    pub fn get<I: ValueIndex>(&self, index: I) -> Option<&Self> {
        index.index_into(self)
    }

    pub fn get_mut<I: ValueIndex>(&mut self, index: I) -> Option<&mut Self> {
        index.index_into_mut(self)
    }

    /// Length for arrays/objects/strings; `None` for scalars.
    #[must_use]
    pub fn len(&self) -> Option<usize> {
        match self {
            Self::Array(a) => Some(a.len()),
            Self::Object(o) => Some(o.len()),
            Self::String(s) => Some(s.len()),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len().is_some_and(|n| n == 0)
    }
}

/// Types that can index a [`Value`] (strings look up object keys, integers
/// look up array positions).
pub trait ValueIndex: private::Sealed {
    fn index_into<'a>(&self, v: &'a Value) -> Option<&'a Value>;
    fn index_into_mut<'a>(&self, v: &'a mut Value) -> Option<&'a mut Value>;
}

mod private {
    pub trait Sealed {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl Sealed for &str {}
    impl Sealed for usize {}
}

impl ValueIndex for str {
    fn index_into<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        if let Value::Object(members) = v {
            members
                .iter()
                .find(|e| e.key == self)
                .map(|e| &e.value.value)
        } else {
            None
        }
    }
    fn index_into_mut<'a>(&self, v: &'a mut Value) -> Option<&'a mut Value> {
        if let Value::Object(members) = v {
            members
                .iter_mut()
                .find(|e| e.key == self)
                .map(|e| &mut e.value.value)
        } else {
            None
        }
    }
}

impl ValueIndex for &str {
    fn index_into<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        (*self).index_into(v)
    }
    fn index_into_mut<'a>(&self, v: &'a mut Value) -> Option<&'a mut Value> {
        (*self).index_into_mut(v)
    }
}

impl ValueIndex for String {
    fn index_into<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        self.as_str().index_into(v)
    }
    fn index_into_mut<'a>(&self, v: &'a mut Value) -> Option<&'a mut Value> {
        self.as_str().index_into_mut(v)
    }
}

impl ValueIndex for usize {
    fn index_into<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        if let Value::Array(elements) = v {
            elements.get(*self).map(|n| &n.value)
        } else {
            None
        }
    }
    fn index_into_mut<'a>(&self, v: &'a mut Value) -> Option<&'a mut Value> {
        if let Value::Array(elements) = v {
            elements.get_mut(*self).map(|n| &mut n.value)
        } else {
            None
        }
    }
}

// --- std::ops::Index / IndexMut ---

/// Static `Null` returned for missing keys/indices on read (serde_json style).
static NULL_SENTINEL: Value = Value::Null;

impl<I: ValueIndex> std::ops::Index<I> for Value {
    type Output = Value;
    fn index(&self, index: I) -> &Value {
        index.index_into(self).unwrap_or(&NULL_SENTINEL)
    }
}

impl std::ops::IndexMut<&str> for Value {
    fn index_mut(&mut self, key: &str) -> &mut Value {
        // Auto-promote Null → empty Object for ergonomic building.
        if matches!(self, Self::Null) {
            *self = Self::Object(Vec::new());
        }
        let members = match self {
            Self::Object(m) => m,
            other => panic!(
                "cannot index with key {:?} into a {:?}",
                key,
                discriminant_name(other)
            ),
        };
        if let Some(pos) = members.iter().position(|e| e.key == key) {
            &mut members[pos].value.value
        } else {
            members.push(ObjectEntry::new(key.to_string(), Node::new(Value::Null)));
            &mut members.last_mut().unwrap().value.value
        }
    }
}

impl std::ops::IndexMut<usize> for Value {
    fn index_mut(&mut self, idx: usize) -> &mut Value {
        let elements = match self {
            Self::Array(a) => a,
            other => panic!(
                "cannot index with usize into a {:?}",
                discriminant_name(other)
            ),
        };
        &mut elements[idx].value
    }
}

fn discriminant_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "Null",
        Value::Bool(_) => "Bool",
        Value::Number(_) => "Number",
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
        #[cfg(feature = "lazy")]
        Value::Lazy(_) => "Lazy",
    }
}

#[cfg(feature = "lazy")]
impl From<crate::lazy::LazyValue> for Value {
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn from(v: crate::lazy::LazyValue) -> Self {
        Self::Lazy(Box::new(v))
    }
}
