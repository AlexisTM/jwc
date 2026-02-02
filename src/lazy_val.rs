//! Default-lazy slice-based value.
//!
//! [`LazyVal`] keeps strings and numbers as raw source slices (decoded on
//! demand), but arrays and objects are **eagerly built and sorted at
//! parse time**. Every `LazyVal::Object` is a `Box<[LazyObjectEntry]>`
//! already sorted by key, so [`LazyVal::get`] is a plain
//! `O(log m)` binary search — no separate `.index()` step.
//!
//! ```
//! # fn main() -> jwc::Result<()> {
//! let v = jwc::from_str_lazy(r#"{"port": 8080, "name": "svc"}"#)?;
//! // Binary search over the sorted members.
//! assert_eq!(v.get("port").and_then(|v| v.as_i64()), Some(8080));
//! # Ok(())
//! # }
//! ```
//!
//! Trade: parse pays `O(m log m)` per object to sort. Access is then
//! consistent and cheap for every lookup, with no "linear path" trap.

use crate::parser_core::{MAX_DEPTH, ParserCore, fast_parse_int};
use crate::{Error, Result, ast::Trivia};
use std::borrow::Cow;

/// A parsed lazy value with attached JSONC trivia. Mirrors the owned
/// `Node { value, trivia }` shape so comments round-trip on both paths.
#[derive(Debug, Clone)]
pub struct LazyNode<'a> {
    pub value: LazyVal<'a>,
    pub trivia: Box<[Trivia]>,
}

/// An object member: `key_trivia` holds comments that preceded the key;
/// `value.trivia` holds comments between the key and the value (mirrors
/// owned `ObjectEntry`).
#[derive(Debug, Clone)]
pub struct LazyObjectEntry<'a> {
    pub key: Cow<'a, str>,
    pub key_trivia: Box<[Trivia]>,
    pub value: LazyNode<'a>,
}

/// A JSON value. Strings/numbers lazy (raw slice into source); arrays
/// and objects eagerly parsed — object members are stored **sorted by
/// key** so lookups are `O(log m)`. Elements are wrapped in `LazyNode`
/// so JSONC trivia (comments) can attach per value.
#[derive(Debug, Clone)]
pub enum LazyVal<'a> {
    Null,
    Bool(bool),
    /// Raw number lexeme (e.g. `-12.5e3`). Decoded on `as_i64` / `as_f64`.
    Number(&'a str),
    /// String content without the outer quotes. May contain escape
    /// sequences; decoded on demand via `as_str`.
    String(&'a str),
    /// Array elements in source order, each wrapped in a `LazyNode`
    /// carrying its JSONC trivia.
    Array(Box<[LazyNode<'a>]>),
    /// Object members in key-sorted order.
    Object(Box<[LazyObjectEntry<'a>]>),
}

impl<'a> LazyVal<'a> {
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

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    /// Decode a number lexeme to `i64`. Integer fast path; falls back to
    /// `f64::parse` for lexemes containing `.` / `e`.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        let lex = self.as_number_lex()?;
        if let Some(n) = fast_parse_int(lex.as_bytes()) {
            return Some(n);
        }
        lex.parse::<f64>().ok().map(|f| f as i64)
    }

    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.as_i64()
            .and_then(|n| if n >= 0 { Some(n as u64) } else { None })
    }

    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        let lex = self.as_number_lex()?;
        lex.parse::<f64>().ok()
    }

    /// The raw number lexeme, if this is a `Number`.
    #[must_use]
    pub fn as_number_lex(&self) -> Option<&'a str> {
        if let Self::Number(lex) = self {
            Some(*lex)
        } else {
            None
        }
    }

    /// Decode a JSON string value. Returns `Cow::Borrowed` if no escapes;
    /// `Cow::Owned` if any escape had to be decoded.
    #[must_use]
    pub fn as_str(&self) -> Option<Cow<'a, str>> {
        if let Self::String(raw) = self {
            if raw.as_bytes().contains(&b'\\') {
                decode_escaped(raw).map(Cow::Owned)
            } else {
                Some(Cow::Borrowed(*raw))
            }
        } else {
            None
        }
    }

    /// Array elements in source order. `None` if `self` isn't an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[LazyNode<'a>]> {
        if let Self::Array(items) = self {
            Some(items)
        } else {
            None
        }
    }

    /// Object members in key-sorted order. `None` if `self` isn't an
    /// object.
    #[must_use]
    pub fn as_object(&self) -> Option<&[LazyObjectEntry<'a>]> {
        if let Self::Object(members) = self {
            Some(members)
        } else {
            None
        }
    }

    /// `O(log m)` key lookup. Returns `None` if `self` isn't an object
    /// or the key is absent.
    ///
    /// The sort happens once at parse time, not per call. There is no
    /// linear fallback.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&LazyNode<'a>> {
        let members = self.as_object()?;
        members
            .binary_search_by(|e| e.key.as_ref().cmp(key))
            .ok()
            .map(|i| &members[i].value)
    }

    /// Array index. `O(1)`.
    #[must_use]
    pub fn at(&self, idx: usize) -> Option<&LazyNode<'a>> {
        self.as_array()?.get(idx)
    }

    /// Length of arrays / objects / strings (bytes). `None` for scalars.
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

    /// Iterate array elements — alias for `as_array().into_iter().flatten()`.
    pub fn iter_array(&self) -> impl Iterator<Item = &LazyNode<'a>> {
        self.as_array().into_iter().flatten()
    }

    /// Iterate `(key, value)` pairs of an object in sorted key order.
    pub fn iter_object(&self) -> impl Iterator<Item = (&str, &LazyNode<'a>)> {
        self.as_object()
            .into_iter()
            .flatten()
            .map(|e| (e.key.as_ref(), &e.value))
    }

    /// Fully materialize into an owned [`crate::Value`]. Decodes every
    /// string and number, clones every key.
    ///
    /// Note: trivia (JSONC comments) is **not** transferred to the owned
    /// tree — `Value` has no trivia channel. For round-trip fidelity, keep
    /// the `LazyNode` tree or use the owned [`crate::from_str`] path.
    #[must_use]
    pub fn to_value(&self) -> crate::Value {
        use crate::{Node, Number, ObjectEntry, Value};
        match self {
            LazyVal::Null => Value::Null,
            LazyVal::Bool(b) => Value::Bool(*b),
            LazyVal::Number(lex) => {
                if let Some(n) = fast_parse_int(lex.as_bytes()) {
                    Value::Number(Number::from(n))
                } else {
                    let f = lex.parse::<f64>().unwrap_or(f64::NAN);
                    Value::Number(Number::from_parsed_and_lexeme(f, lex))
                }
            }
            LazyVal::String(_) => Value::String(self.as_str().unwrap_or_default().into_owned()),
            LazyVal::Array(items) => {
                Value::Array(items.iter().map(|n| Node::new(n.to_value())).collect())
            }
            LazyVal::Object(members) => Value::Object(
                members
                    .iter()
                    .map(|e| {
                        ObjectEntry::new(e.key.as_ref().to_string(), Node::new(e.value.to_value()))
                    })
                    .collect(),
            ),
        }
    }
}

/// Derefs to the inner `LazyVal` so every accessor (`get`, `as_i64`, …) is
/// reachable on a `LazyNode` without a `.value` hop. The only field the
/// passthrough intentionally hides is `trivia`, which callers read directly.
impl<'a> std::ops::Deref for LazyNode<'a> {
    type Target = LazyVal<'a>;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// ---------------------------------------------------------------------------
// Public entrypoint
// ---------------------------------------------------------------------------

/// Parse JSON (with optional JSONC comments) and return a [`LazyNode`]
/// borrowing into `src`. Objects are sorted by key at parse time so all
/// subsequent `get`s are `O(log m)`.
pub fn from_str_lazy(src: &str) -> Result<LazyNode<'_>> {
    let mut p = LazyParser::new(src);
    p.parse_root()
}

// ---------------------------------------------------------------------------
// LazyParser
// ---------------------------------------------------------------------------

struct LazyParser<'a> {
    core: ParserCore<'a>,
}

impl<'a> LazyParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            core: ParserCore::new(input),
        }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        let (line, col) = self.core.position_from_offset(self.core.pos);
        Error::parse(line, col, msg)
    }

    fn parse_root(&mut self) -> Result<LazyNode<'a>> {
        let mut root = self.parse_value(0).map_err(|msg| self.core.lift_err(msg))?;
        // Trailing trivia (comments after the root value) attaches to the root node.
        self.core
            .consume_trivia()
            .map_err(|msg| self.core.lift_err(msg))?;
        if !self.core.pending_trivia.is_empty() {
            let mut combined: Vec<Trivia> = root.trivia.into_vec();
            combined.extend(self.core.take_pending_trivia());
            root.trivia = combined.into_boxed_slice();
        }
        if self.core.pos < self.core.bytes.len() {
            return Err(self.err("trailing content"));
        }
        Ok(root)
    }

    fn parse_value(&mut self, depth: usize) -> std::result::Result<LazyNode<'a>, String> {
        self.core.consume_trivia()?;
        if self.core.pos >= self.core.bytes.len() {
            let (l, c) = self.core.position_from_offset(self.core.pos);
            return Err(format!("unexpected EOF at {l}:{c}"));
        }
        let token_pos = self.core.pos;
        let trivia = self.core.take_pending_trivia();
        let b = unsafe { *self.core.bytes.get_unchecked(token_pos) };
        let value = match b {
            b'{' => {
                if depth >= MAX_DEPTH {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("maximum nesting depth exceeded at {l}:{c}"));
                }
                self.core.pos += 1;
                self.parse_object(depth + 1)?
            }
            b'[' => {
                if depth >= MAX_DEPTH {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("maximum nesting depth exceeded at {l}:{c}"));
                }
                self.core.pos += 1;
                self.parse_array(depth + 1)?
            }
            b'"' => LazyVal::String(self.parse_string_slice()?),
            b'-' | b'0'..=b'9' => {
                let (lex, _) = self.core.scan_number_lexeme();
                LazyVal::Number(lex)
            }
            b't' => {
                if self.core.try_keyword(b"true") {
                    LazyVal::Bool(true)
                } else {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("expected `true` at {l}:{c}"));
                }
            }
            b'f' => {
                if self.core.try_keyword(b"false") {
                    LazyVal::Bool(false)
                } else {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("expected `false` at {l}:{c}"));
                }
            }
            b'n' => {
                if self.core.try_keyword(b"null") {
                    LazyVal::Null
                } else {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("expected `null` at {l}:{c}"));
                }
            }
            other => {
                let (l, c) = self.core.position_from_offset(token_pos);
                return Err(format!("unexpected token {:?} at {l}:{c}", other as char));
            }
        };
        Ok(LazyNode {
            value,
            trivia: trivia.into_boxed_slice(),
        })
    }

    fn parse_string_slice(&mut self) -> std::result::Result<&'a str, String> {
        let start = self.core.pos + 1; // eat opening quote
        let end = self.core.scan_string_skip_escapes(start)?;
        self.core.validate_no_control_chars(start, end)?;
        let slice = &self.core.input[start..end];
        self.core.pos = end + 1; // eat closing quote
        Ok(slice)
    }

    fn parse_array(&mut self, depth: usize) -> std::result::Result<LazyVal<'a>, String> {
        let mut items: Vec<LazyNode<'a>> = Vec::with_capacity(8);
        loop {
            self.core.consume_trivia()?;
            if self.core.pos >= self.core.bytes.len() {
                let (l, c) = self.core.position_from_offset(self.core.pos);
                return Err(format!("unexpected EOF in array at {l}:{c}"));
            }
            if self.core.bytes[self.core.pos] == b']' {
                self.core.pos += 1;
                // Attach any pending trailing trivia to the last element (or
                // discard if the array was empty and there are only leading
                // comments — those belong to the enclosing context).
                if !self.core.pending_trivia.is_empty()
                    && let Some(last) = items.last_mut()
                {
                    let pending = self.core.take_pending_trivia();
                    let mut combined: Vec<Trivia> = last.trivia.to_vec();
                    combined.extend(pending);
                    last.trivia = combined.into_boxed_slice();
                }
                return Ok(LazyVal::Array(items.into_boxed_slice()));
            }
            let node = self.parse_value(depth)?;
            items.push(node);
            self.core.consume_trivia()?;
            match self.core.bytes.get(self.core.pos).copied() {
                Some(b',') => self.core.pos += 1,
                Some(b']') => continue,
                Some(other) => {
                    let (l, c) = self.core.position_from_offset(self.core.pos);
                    return Err(format!(
                        "expected ',' or ']' in array, got {:?} at {l}:{c}",
                        other as char
                    ));
                }
                None => {
                    let (l, c) = self.core.position_from_offset(self.core.pos);
                    return Err(format!("unexpected EOF in array at {l}:{c}"));
                }
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> std::result::Result<LazyVal<'a>, String> {
        let mut entries: Vec<LazyObjectEntry<'a>> = Vec::with_capacity(8);
        loop {
            self.core.consume_trivia()?;
            if self.core.pos >= self.core.bytes.len() {
                let (l, c) = self.core.position_from_offset(self.core.pos);
                return Err(format!("unexpected EOF in object at {l}:{c}"));
            }
            if self.core.bytes[self.core.pos] == b'}' {
                self.core.pos += 1;
                if !self.core.pending_trivia.is_empty()
                    && let Some(last) = entries.last_mut()
                {
                    let pending = self.core.take_pending_trivia();
                    let mut combined: Vec<Trivia> = last.value.trivia.to_vec();
                    combined.extend(pending);
                    last.value.trivia = combined.into_boxed_slice();
                }
                entries.sort_unstable_by(|a, b| a.key.as_ref().cmp(b.key.as_ref()));
                return Ok(LazyVal::Object(entries.into_boxed_slice()));
            }
            let key_trivia = self.core.take_pending_trivia();
            if self.core.bytes[self.core.pos] != b'"' {
                let (l, c) = self.core.position_from_offset(self.core.pos);
                return Err(format!("expected string key at {l}:{c}"));
            }
            let key_start = self.core.pos;
            let key_raw = self.parse_string_slice()?;
            let key: Cow<'a, str> = if key_raw.as_bytes().contains(&b'\\') {
                Cow::Owned(decode_escaped(key_raw).ok_or_else(|| {
                    let (l, c) = self.core.position_from_offset(key_start);
                    format!("bad escape in key at {l}:{c}")
                })?)
            } else {
                Cow::Borrowed(key_raw)
            };
            self.core.consume_trivia()?;
            if self.core.bytes.get(self.core.pos) != Some(&b':') {
                let (l, c) = self.core.position_from_offset(self.core.pos);
                return Err(format!("expected ':' after key at {l}:{c}"));
            }
            self.core.pos += 1;
            // Trivia between ':' and the value becomes the value node's
            // leading trivia — parse_value's take_pending_trivia picks it up.
            let value = self.parse_value(depth)?;
            entries.push(LazyObjectEntry {
                key,
                key_trivia: key_trivia.into_boxed_slice(),
                value,
            });
            self.core.consume_trivia()?;
            match self.core.bytes.get(self.core.pos).copied() {
                Some(b',') => self.core.pos += 1,
                Some(b'}') => continue,
                Some(other) => {
                    let (l, c) = self.core.position_from_offset(self.core.pos);
                    return Err(format!(
                        "expected ',' or '}}' in object, got {:?} at {l}:{c}",
                        other as char
                    ));
                }
                None => {
                    let (l, c) = self.core.position_from_offset(self.core.pos);
                    return Err(format!("unexpected EOF in object at {l}:{c}"));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn decode_escaped(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b'\\' {
            let run_end = bytes[i..]
                .iter()
                .position(|&c| c == b'\\')
                .map_or(bytes.len(), |p| i + p);
            out.push_str(&raw[i..run_end]);
            i = run_end;
            continue;
        }
        i += 1;
        if i >= bytes.len() {
            return None;
        }
        let esc = bytes[i];
        i += 1;
        match esc {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\x08'),
            b'f' => out.push('\x0c'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                if i + 4 > bytes.len() {
                    return None;
                }
                let cp = hex4(&bytes[i..i + 4])?;
                i += 4;
                if (0xD800..=0xDBFF).contains(&cp)
                    && i + 2 <= bytes.len()
                    && bytes[i] == b'\\'
                    && bytes[i + 1] == b'u'
                {
                    let low = hex4(&bytes[i + 2..i + 6])?;
                    i += 6;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return None;
                    }
                    let codepoint =
                        0x10000 + (((cp - 0xD800) as u32) << 10) + ((low - 0xDC00) as u32);
                    out.push(char::from_u32(codepoint)?);
                } else {
                    out.push(char::from_u32(cp as u32)?);
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

fn hex4(b: &[u8]) -> Option<u16> {
    let mut v = 0u16;
    for &c in b.iter().take(4) {
        let n = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        v = (v << 4) | n as u16;
    }
    Some(v)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert!(matches!(
            from_str_lazy("null").unwrap().value,
            LazyVal::Null
        ));
        assert!(matches!(
            from_str_lazy("true").unwrap().value,
            LazyVal::Bool(true)
        ));
        assert!(matches!(
            from_str_lazy("42").unwrap().value,
            LazyVal::Number("42")
        ));
        assert!(matches!(
            from_str_lazy(r#""hello""#).unwrap().value,
            LazyVal::String("hello")
        ));
    }

    #[test]
    fn get_is_log_n_and_requires_no_index_step() {
        let src = r#"{"port": 8080, "name": "svc", "tls": true}"#;
        let v = from_str_lazy(src).unwrap();
        assert_eq!(v.get("port").and_then(|v| v.as_i64()), Some(8080));
        assert_eq!(
            v.get("name").and_then(|v| v.as_str()).as_deref(),
            Some("svc")
        );
        assert_eq!(v.get("tls").and_then(|v| v.as_bool()), Some(true));
        assert!(v.get("missing").is_none());
    }

    #[test]
    fn object_members_are_sorted_by_key() {
        let v = from_str_lazy(r#"{"zz": 0, "aa": 1, "mm": 2}"#).unwrap();
        let keys: Vec<&str> = v.iter_object().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["aa", "mm", "zz"]);
    }

    #[test]
    fn escaped_string_decodes_on_demand() {
        let v = from_str_lazy(r#""a\nb""#).unwrap();
        assert_eq!(v.as_str().as_deref(), Some("a\nb"));
    }

    #[test]
    fn nested_get_uses_binary_search_at_each_level() {
        let src = r#"{"outer": {"inner": [1, 2, 3]}}"#;
        let v = from_str_lazy(src).unwrap();
        let inner = v.get("outer").unwrap().get("inner").unwrap();
        assert_eq!(inner.at(2).and_then(|x| x.as_i64()), Some(3));
    }

    #[test]
    fn array_indexing() {
        let v = from_str_lazy("[10, 20, 30]").unwrap();
        assert_eq!(v.at(0).and_then(|x| x.as_i64()), Some(10));
        assert_eq!(v.at(2).and_then(|x| x.as_i64()), Some(30));
        assert!(v.at(3).is_none());
    }

    #[test]
    fn escaped_key_is_decoded_and_sorted_correctly() {
        let v = from_str_lazy(r#"{"é": 1, "a": 2}"#).unwrap();
        assert_eq!(v.get("a").and_then(|x| x.as_i64()), Some(2));
        assert_eq!(v.get("é").and_then(|x| x.as_i64()), Some(1));
    }

    #[test]
    fn to_value_materializes() {
        let src = r#"{"a": 1, "b": [2, 3]}"#;
        let v = from_str_lazy(src).unwrap();
        let owned = v.to_value();
        assert_eq!(owned.get("a").and_then(crate::Value::as_i64), Some(1));
        assert_eq!(owned["b"][1].as_i64(), Some(3));
    }

    #[test]
    fn reject_trailing_junk() {
        assert!(from_str_lazy("42 garbage").is_err());
    }

    #[test]
    fn reject_unterminated_string() {
        assert!(from_str_lazy(r#""unterm"#).is_err());
    }
}
