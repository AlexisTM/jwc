//! A single-pass JSONC parser: JSON plus `//` and `/* */` comments and
//! trailing commas. Comments are kept by position, every node remembers its
//! source span, and the grammar is otherwise strict JSON.

use std::collections::HashSet;

use crate::Number;
use crate::ast::{Node, ObjectEntry, Source, Trivia, Value};
use crate::error::{Error, ErrorKind, Result};
use crate::fingerprint;

/// What to do when an object repeats a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuplicateKeys {
    /// Fail with `ErrorKind::DuplicateKey`. The default: a repeated key in a
    /// hand-written file is almost always a mistake, and consumers disagree
    /// on which one wins.
    #[default]
    Reject,
    /// Keep every entry in the AST; `Value::pointer` finds the first one.
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// Containers nested deeper than this are rejected instead of exhausting
    /// the stack. 128, like `serde_json`.
    pub max_depth: usize,
    pub duplicate_keys: DuplicateKeys,
}

impl ParseOptions {
    pub const DEFAULT: Self = Self {
        max_depth: 128,
        duplicate_keys: DuplicateKeys::Reject,
    };
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub struct SinglePassParser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    pending_trivia: Vec<Trivia>,
    options: ParseOptions,
    depth: usize,
    /// An empty line was crossed since the previous element or member.
    saw_blank_line: bool,
}

const fn is_json_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

impl<'a> SinglePassParser<'a> {
    #[must_use]
    pub const fn new(input: &'a str) -> Self {
        Self::with_options(input, ParseOptions::DEFAULT)
    }

    #[must_use]
    pub const fn with_options(input: &'a str, options: ParseOptions) -> Self {
        SinglePassParser {
            input,
            bytes: input.as_bytes(),
            pos: 0,
            pending_trivia: Vec::new(),
            options,
            depth: 0,
            saw_blank_line: false,
        }
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    #[inline]
    fn take_pending_trivia(&mut self) -> Vec<Trivia> {
        if self.pending_trivia.is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut self.pending_trivia)
        }
    }

    // Only used for error messages, so the scan is fine.
    fn position_from_offset(&self, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (i, ch) in self.input.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn error(&self, offset: usize, kind: ErrorKind) -> Error {
        let (line, col) = self.position_from_offset(offset);
        Error::at(kind, line, col)
    }

    fn syntax(&self, offset: usize, message: impl Into<String>) -> Error {
        self.error(offset, ErrorKind::Syntax(message.into()))
    }

    fn char_at(&self, offset: usize) -> char {
        self.input[offset..].chars().next().unwrap_or('\0')
    }

    fn eof(&self, what: &str) -> Error {
        self.syntax(self.bytes.len(), format!("Unexpected EOF {what}"))
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_string(&mut self) -> Result<String> {
        let start = self.pos + 1; // opening quote
        let mut pos = start;

        // Fast path: no escapes, no control characters.
        while let Some(&b) = self.bytes.get(pos) {
            match b {
                b'"' => {
                    self.pos = pos + 1;
                    return Ok(self.input[start..pos].to_string());
                }
                b'\\' => break,
                b if b < 0x20 => {
                    return Err(self.syntax(pos, "Unescaped control character in string"));
                }
                _ => pos += 1,
            }
        }

        self.pos = pos;
        self.parse_string_slow(start)
    }

    #[cold]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_string_slow(&mut self, start: usize) -> Result<String> {
        let mut s = String::with_capacity(32);
        s.push_str(&self.input[start..self.pos]);

        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(s);
                }
                b'\\' => {
                    self.pos += 1;
                    let Some(&escaped) = self.bytes.get(self.pos) else {
                        return Err(self.eof("after '\\'"));
                    };
                    self.pos += 1;
                    match escaped {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\x08'),
                        b'f' => s.push('\x0c'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let ch = self.parse_unicode_escape()?;
                            s.push(ch);
                        }
                        _ => {
                            return Err(self.syntax(self.pos - 1, "Invalid escape sequence"));
                        }
                    }
                }
                b if b < 0x20 => {
                    return Err(self.syntax(self.pos, "Unescaped control character in string"));
                }
                _ => {
                    let ch = self.char_at(self.pos);
                    s.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
        Err(self.eof("in string"))
    }

    /// After `\u`: one BMP code unit, or a surrogate pair.
    fn parse_unicode_escape(&mut self) -> Result<char> {
        let at = self.pos - 2;
        let first = self.parse_hex4_escape()?;
        if (0xDC00..=0xDFFF).contains(&first) {
            return Err(self.syntax(at, "Unexpected low surrogate in unicode escape"));
        }
        if !(0xD800..=0xDBFF).contains(&first) {
            return char::from_u32(u32::from(first))
                .ok_or_else(|| self.syntax(at, "Invalid unicode escape"));
        }
        if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
            return Err(self.syntax(at, "Expected low surrogate after high surrogate"));
        }
        self.pos += 2;
        let second = self.parse_hex4_escape()?;
        if !(0xDC00..=0xDFFF).contains(&second) {
            return Err(self.syntax(at, "Invalid low surrogate in unicode escape"));
        }
        let codepoint =
            0x10000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(second) - 0xDC00);
        char::from_u32(codepoint).ok_or_else(|| self.syntax(at, "Invalid unicode escape"))
    }

    fn parse_hex4_escape(&mut self) -> Result<u16> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let Some(&b) = self.bytes.get(self.pos) else {
                return Err(self.eof("in unicode escape"));
            };
            let nibble = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return Err(self.syntax(self.pos, "Invalid unicode escape hex digit")),
            };
            self.pos += 1;
            value = (value << 4) | u16::from(nibble);
        }
        Ok(value)
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_number(&mut self) -> Result<Number> {
        let start = self.pos;
        let mut pos = start + 1; // first digit or '-'
        while self
            .bytes
            .get(pos)
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
        {
            pos += 1;
        }
        self.pos = pos;
        Number::from_lexeme(&self.input[start..pos]).map_err(|e| self.error(start, e.kind))
    }

    /// At `//`: the text up to (not including) the newline.
    fn parse_line_comment(&mut self) -> String {
        let start = self.pos + 2;
        let end = self.bytes[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.bytes.len(), |i| start + i);
        self.pos = end;
        self.input[start..end].to_string()
    }

    /// At `/*`: the text up to the closing `*/`.
    fn parse_block_comment(&mut self) -> Result<String> {
        let at = self.pos;
        let start = self.pos + 2;
        match self.input[start..].find("*/") {
            Some(len) => {
                self.pos = start + len + 2;
                Ok(self.input[start..start + len].to_string())
            }
            None => Err(self.syntax(at, "Unterminated block comment")),
        }
    }

    /// Whitespace and comments; comments become pending trivia for the next
    /// node (or the enclosing container's dangling comments).
    /// Only whitespace (spaces, tabs, CR) since the previous newline?
    fn line_is_blank_before(&self, pos: usize) -> bool {
        self.bytes[..pos]
            .iter()
            .rev()
            .find(|&&b| !matches!(b, b' ' | b'\t' | b'\r'))
            == Some(&b'\n')
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_trivia(&mut self) -> Result<()> {
        loop {
            match self.peek() {
                Some(b'\n') => {
                    if !self.saw_blank_line && self.line_is_blank_before(self.pos) {
                        self.saw_blank_line = true;
                    }
                    self.pos += 1;
                }
                Some(b) if is_json_whitespace(b) => self.pos += 1,
                Some(b'/') => {
                    let comment = self.parse_comment()?;
                    self.pending_trivia.push(comment);
                }
                _ => return Ok(()),
            }
        }
    }

    /// At a `/`: either kind of comment, else a syntax error.
    fn parse_comment(&mut self) -> Result<Trivia> {
        match self.bytes.get(self.pos + 1) {
            Some(b'/') => Ok(Trivia::LineComment(self.parse_line_comment())),
            Some(b'*') => Ok(Trivia::BlockComment(self.parse_block_comment()?)),
            Some(_) => Err(self.syntax(self.pos, "Unexpected character '/'")),
            None => Err(self.eof("after '/'")),
        }
    }

    /// Comments on the same line as the value just parsed, before any newline.
    /// They stay with that value; anything after the newline is pending trivia.
    #[inline]
    fn consume_trailing_trivia(&mut self, node: &mut Node) -> Result<()> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r') => self.pos += 1,
                Some(b'/') if matches!(self.bytes.get(self.pos + 1), Some(b'/' | b'*')) => {
                    let comment = self.parse_comment()?;
                    node.trailing.push(comment);
                }
                _ => return Ok(()),
            }
        }
    }

    fn consume_object_colon(&mut self) -> Result<()> {
        loop {
            match self.peek() {
                None => return Err(self.eof("while looking for ':' after key")),
                Some(b':') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(b'/') => {
                    let comment = self.parse_comment()?;
                    self.pending_trivia.push(comment);
                }
                Some(b) if is_json_whitespace(b) => self.pos += 1,
                Some(_) => {
                    let ch = self.char_at(self.pos);
                    return Err(self.syntax(
                        self.pos,
                        format!("Unexpected character '{ch}' between key and ':'"),
                    ));
                }
            }
        }
    }

    fn parse_value(&mut self) -> Result<Node> {
        self.consume_trivia()?;
        self.parse_value_after_trivia()
    }

    fn enter(&mut self, at: usize) -> Result<()> {
        self.depth += 1;
        if self.depth > self.options.max_depth {
            return Err(self.error(at, ErrorKind::DepthExceeded(self.options.max_depth)));
        }
        Ok(())
    }

    fn literal(&mut self, word: &[u8], value: Value) -> Result<Value> {
        if self.bytes[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.syntax(self.pos, "Unexpected identifier"))
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_value_after_trivia(&mut self) -> Result<Node> {
        let Some(token) = self.peek() else {
            return Err(self.eof("while expecting a value"));
        };
        let token_pos = self.pos;
        let trivia = self.take_pending_trivia();

        let value = match token {
            b'{' => {
                self.enter(token_pos)?;
                self.pos += 1;
                let value = self.parse_object_value()?;
                self.depth -= 1;
                value
            }
            b'[' => {
                self.enter(token_pos)?;
                self.pos += 1;
                let value = self.parse_array_value()?;
                self.depth -= 1;
                value
            }
            b'"' => Value::String(self.parse_string()?),
            b'-' | b'0'..=b'9' => Value::Number(self.parse_number()?),
            b't' => self.literal(b"true", Value::Bool(true))?,
            b'f' => self.literal(b"false", Value::Bool(false))?,
            b'n' => self.literal(b"null", Value::Null)?,
            _ => {
                let ch = self.char_at(token_pos);
                return Err(self.syntax(token_pos, format!("Unexpected character '{ch}'")));
            }
        };

        // A container leaves the comments before its closing bracket pending.
        let dangling = self.take_pending_trivia();
        let mut node = Node {
            value,
            trivia,
            trailing: Vec::new(),
            dangling,
            comma: false,
            source: None,
        };
        node.source = Some(Source {
            span: token_pos..self.pos,
            fingerprint: fingerprint::stored_or_new_node(&node),
            blank_line_before: false,
        });
        Ok(node)
    }

    fn consume_comma(&mut self, node: &mut Node, closer: u8, what: &str) -> Result<()> {
        match self.peek() {
            Some(b',') => {
                self.pos += 1;
                node.comma = true;
                Ok(())
            }
            Some(b) if b == closer => Ok(()),
            Some(_) => {
                let ch = self.char_at(self.pos);
                Err(self.syntax(
                    self.pos,
                    format!(
                        "Expected ',' or '{}' after {what}, found '{ch}'",
                        closer as char
                    ),
                ))
            }
            None => Ok(()), // the container loop reports the EOF
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_array_value(&mut self) -> Result<Value> {
        let mut elements: Vec<Node> = Vec::with_capacity(8);
        loop {
            self.consume_trivia()?;
            match self.peek() {
                None => return Err(self.eof("in array")),
                Some(b']') => {
                    self.pos += 1;
                    // pending trivia becomes the array node's dangling comments
                    return Ok(Value::Array(elements));
                }
                Some(_) => {}
            }

            let blank_line_before = std::mem::take(&mut self.saw_blank_line);
            let mut node = self.parse_value_after_trivia()?;
            if let Some(source) = &mut node.source {
                source.blank_line_before = blank_line_before;
            }
            // Blank lines inside the value are its own business.
            self.saw_blank_line = false;
            self.consume_trailing_trivia(&mut node)?;
            self.consume_trivia()?;
            // pending trivia becomes the next element's leading trivia
            self.consume_comma(&mut node, b']', "array element")?;
            self.consume_trailing_trivia(&mut node)?;
            elements.push(node);
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_object_value(&mut self) -> Result<Value> {
        let mut members: Vec<ObjectEntry> = Vec::with_capacity(8);
        let mut seen: HashSet<String> = HashSet::new();
        loop {
            self.consume_trivia()?;
            match self.peek() {
                None => return Err(self.eof("in object")),
                Some(b'}') => {
                    self.pos += 1;
                    // pending trivia becomes the object node's dangling comments
                    return Ok(Value::Object(members));
                }
                Some(b'"') => {}
                Some(_) => {
                    let ch = self.char_at(self.pos);
                    return Err(self.syntax(self.pos, format!("Expected string key, found '{ch}'")));
                }
            }

            let key_trivia = self.take_pending_trivia();
            let blank_line_before = std::mem::take(&mut self.saw_blank_line);
            let key_start = self.pos;
            let key = self.parse_string()?;
            if self.options.duplicate_keys == DuplicateKeys::Reject && !seen.insert(key.clone()) {
                return Err(self.error(key_start, ErrorKind::DuplicateKey(key)));
            }

            self.consume_object_colon()?;
            // pending trivia (between key and ':') becomes the value's leading trivia
            let mut node = self.parse_value()?;
            let value_end = self.pos;
            // Blank lines inside the value are its own business.
            self.saw_blank_line = false;

            self.consume_trailing_trivia(&mut node)?;
            self.consume_trivia()?;
            // pending trivia becomes the next member's key trivia
            self.consume_comma(&mut node, b'}', "object member")?;
            self.consume_trailing_trivia(&mut node)?;

            let mut entry = ObjectEntry {
                key,
                key_trivia,
                value: node,
                source: None,
            };
            entry.source = Some(Source {
                span: key_start..value_end,
                fingerprint: fingerprint::stored_or_new_entry(&entry),
                blank_line_before,
            });
            members.push(entry);
        }
    }

    /// Parse the whole input as one document.
    ///
    /// # Errors
    /// Syntax, number, depth and duplicate-key errors, with line and column;
    /// anything but whitespace and comments after the value is an error too.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn parse(&mut self) -> Result<Node> {
        if self.input.starts_with('\u{feff}') {
            self.pos += '\u{feff}'.len_utf8();
        }
        let mut node = self.parse_value()?;

        // Comments after the document belong to the root node's trailing slot.
        self.consume_trivia()?;
        node.trailing.append(&mut self.pending_trivia);

        if self.pos < self.bytes.len() {
            let ch = self.char_at(self.pos);
            return Err(self.syntax(self.pos, format!("Unexpected trailing content '{ch}'")));
        }
        Ok(node)
    }
}
