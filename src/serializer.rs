use crate::ast::{Node, ObjectEntry, Trivia, Value};
use crate::error::{Error, Result};
use crate::fingerprint::Fresh;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indentation {
    Spaces(u8),
    Tabs,
    None, // Minified: no newlines except the ones line comments force
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentPolicy {
    Keep,
    Remove,
    Minify, // Line comments become block comments, so no newline is ever needed
}

#[derive(Debug, Clone, Copy)]
pub struct FormatOptions {
    pub indentation: Indentation,
    pub comment_policy: CommentPolicy,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indentation: Indentation::None,
            comment_policy: CommentPolicy::Keep,
        }
    }
}

impl Indentation {
    /// `None` means four spaces; `"\t"` tabs; a run of spaces that many spaces.
    ///
    /// # Errors
    /// `ErrorKind::Format` for anything else, including more than 255 spaces.
    pub fn parse(indent: Option<&str>) -> Result<Self> {
        let unsupported = || {
            Error::format(format!(
                "unsupported indentation {indent:?}: use a tab or a run of spaces"
            ))
        };
        match indent {
            None => Ok(Self::Spaces(4)),
            Some("\t") => Ok(Self::Tabs),
            Some(s) if !s.is_empty() && s.bytes().all(|b| b == b' ') => u8::try_from(s.len())
                .map(Self::Spaces)
                .map_err(|_| unsupported()),
            Some(_) => Err(unsupported()),
        }
    }

    /// The indentation of the first indented line, four spaces if there is none.
    #[must_use]
    pub fn detect(source: &str) -> Self {
        for line in source.lines() {
            let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            if indent == 0 || line.trim().is_empty() {
                continue;
            }
            if line.starts_with('\t') {
                return Self::Tabs;
            }
            return Self::Spaces(u8::try_from(indent).unwrap_or(u8::MAX));
        }
        Self::Spaces(4)
    }
}

impl Node {
    #[must_use]
    pub fn to_formatted_string(&self, options: FormatOptions) -> String {
        let mut buffer = String::new();
        let mut formatter = Formatter {
            options,
            buffer: &mut buffer,
            depth: 0,
            preserve: None,
        };
        formatter.format_node(self);
        formatter.format_document_tail(&self.trailing);
        buffer
    }

    /// Like [`to_formatted_string`](Self::to_formatted_string), but every node
    /// or object member that still matches what was parsed from `source` is
    /// copied from it verbatim, whitespace and alignment included. Only the
    /// containers that were edited are laid out again. Comments follow the
    /// policy in `options` for the re-rendered parts.
    #[must_use]
    pub fn to_preserved_string(&self, source: &str, options: FormatOptions) -> String {
        let mut buffer = String::new();
        let mut formatter = Formatter {
            options,
            buffer: &mut buffer,
            depth: 0,
            preserve: Some(Preserve {
                source,
                fresh: Fresh::of_tree(self),
            }),
        };
        formatter.format_node(self);
        formatter.format_document_tail(&self.trailing);
        // End the file the way the source did: same number of newlines,
        // trailing spaces dropped.
        let tail = &source[source.trim_end_matches([' ', '\t', '\r', '\n']).len()..];
        let end = buffer.trim_end_matches(['\r', '\n']).len();
        buffer.truncate(end);
        buffer.extend(std::iter::repeat_n('\n', tail.matches('\n').count()));
        buffer
    }
}

struct Preserve<'s> {
    source: &'s str,
    fresh: Fresh,
}

struct Formatter<'a, 's> {
    options: FormatOptions,
    buffer: &'a mut String,
    depth: usize,
    preserve: Option<Preserve<'s>>,
}

impl Formatter<'_, '_> {
    /// The empty line the source had before this element or member.
    fn keep_blank_line(&mut self, source: Option<&crate::ast::Source>) {
        if self.pretty() && source.is_some_and(|s| s.blank_line_before) {
            self.buffer.push('\n');
        }
    }

    /// Copy the node's original text when nothing inside it changed.
    fn emit_verbatim_node(&mut self, node: &Node) -> bool {
        let Some(p) = &self.preserve else {
            return false;
        };
        let Some(source) = &node.source else {
            return false;
        };
        if p.fresh.nodes.get(&std::ptr::from_ref(node)) != Some(&source.fingerprint) {
            return false;
        }
        match p.source.get(source.span.clone()) {
            Some(text) => {
                self.buffer.push_str(text);
                true
            }
            None => false,
        }
    }

    /// Copy `"key": value` from the source when the member did not change.
    fn emit_verbatim_entry(&mut self, entry: &ObjectEntry) -> bool {
        let Some(p) = &self.preserve else {
            return false;
        };
        let Some(source) = &entry.source else {
            return false;
        };
        if p.fresh.entries.get(&std::ptr::from_ref(entry)) != Some(&source.fingerprint) {
            return false;
        }
        match p.source.get(source.span.clone()) {
            Some(text) => {
                self.buffer.push_str(text);
                true
            }
            None => false,
        }
    }
    fn pretty(&self) -> bool {
        self.options.indentation != Indentation::None
    }

    fn write_indent(&mut self) {
        match self.options.indentation {
            Indentation::Spaces(n) => {
                for _ in 0..self.depth * (n as usize) {
                    self.buffer.push(' ');
                }
            }
            Indentation::Tabs => {
                for _ in 0..self.depth {
                    self.buffer.push('\t');
                }
            }
            Indentation::None => {}
        }
    }

    fn write_newline(&mut self) {
        if self.pretty() {
            self.buffer.push('\n');
        }
    }

    /// The comment as source text under the current policy; `true` when it is
    /// a line comment and therefore must be followed by a newline.
    fn comment_text(&self, trivia: &Trivia) -> Option<(bool, String)> {
        match (self.options.comment_policy, trivia) {
            (CommentPolicy::Remove, _) => None,
            (CommentPolicy::Minify, Trivia::LineComment(c) | Trivia::BlockComment(c)) => {
                Some((false, format!("/*{c}*/")))
            }
            (CommentPolicy::Keep, Trivia::LineComment(c)) => Some((true, format!("//{c}"))),
            (CommentPolicy::Keep, Trivia::BlockComment(c)) => Some((false, format!("/*{c}*/"))),
        }
    }

    /// Comments before a key or value: line comments each on their own line at
    /// the current indentation, block comments inline.
    fn format_leading(&mut self, trivia: &[Trivia]) {
        for t in trivia {
            let Some((line, text)) = self.comment_text(t) else {
                continue;
            };
            self.buffer.push_str(&text);
            if line {
                self.buffer.push('\n');
                self.write_indent();
            } else if self.pretty() {
                self.buffer.push(' ');
            }
        }
    }

    /// Comments after a value and its comma, on that same line. The caller
    /// writes the newline that ends the line, except in minified output where
    /// a line comment has to end itself.
    fn format_trailing(&mut self, trivia: &[Trivia]) {
        for (i, t) in trivia.iter().enumerate() {
            let Some((line, text)) = self.comment_text(t) else {
                continue;
            };
            if self.pretty() {
                self.buffer.push(' ');
            }
            self.buffer.push_str(&text);
            if line && (i + 1 < trivia.len() || !self.pretty()) {
                self.buffer.push('\n');
                self.write_indent();
            }
        }
    }

    /// Comments after the last element, before the closing bracket.
    fn format_dangling(&mut self, trivia: &[Trivia]) {
        for t in trivia {
            let Some((line, text)) = self.comment_text(t) else {
                continue;
            };
            self.write_indent();
            self.buffer.push_str(&text);
            if line || self.pretty() {
                self.buffer.push('\n');
            }
        }
    }

    /// Comments after the document, each on its own line.
    fn format_document_tail(&mut self, trivia: &[Trivia]) {
        for t in trivia {
            let Some((_, text)) = self.comment_text(t) else {
                continue;
            };
            self.buffer.push('\n');
            self.buffer.push_str(&text);
        }
    }

    fn format_node(&mut self, node: &Node) {
        self.format_leading(&node.trivia);
        if !self.emit_verbatim_node(node) {
            self.format_value(&node.value, &node.dangling);
        }
        if node.comma {
            self.buffer.push(',');
        }
    }

    fn format_value(&mut self, value: &Value, dangling: &[Trivia]) {
        match value {
            Value::Null => self.buffer.push_str("null"),
            Value::Bool(b) => self.buffer.push_str(&b.to_string()),
            Value::Number(n) => self.buffer.push_str(&n.to_string()),
            Value::String(s) => {
                self.buffer.push('"');
                self.buffer.push_str(&escape_string(s));
                self.buffer.push('"');
            }
            Value::Array(elements) => {
                self.buffer.push('[');
                if !elements.is_empty() || !dangling.is_empty() {
                    self.depth += 1;
                    self.write_newline();
                    for (i, element) in elements.iter().enumerate() {
                        self.keep_blank_line(element.source.as_ref());
                        self.write_indent();
                        self.format_node(element);
                        if i < elements.len() - 1 && !element.comma {
                            self.buffer.push(',');
                        }
                        self.format_trailing(&element.trailing);
                        self.write_newline();
                    }
                    self.format_dangling(dangling);
                    self.depth -= 1;
                    self.write_indent();
                }
                self.buffer.push(']');
            }
            Value::Object(members) => {
                self.buffer.push('{');
                if !members.is_empty() || !dangling.is_empty() {
                    self.depth += 1;
                    self.write_newline();
                    for (i, entry) in members.iter().enumerate() {
                        self.keep_blank_line(entry.source.as_ref());
                        self.write_indent();
                        self.format_object_entry(entry);
                        if i < members.len() - 1 && !entry.value.comma {
                            self.buffer.push(',');
                        }
                        self.format_trailing(&entry.value.trailing);
                        self.write_newline();
                    }
                    self.format_dangling(dangling);
                    self.depth -= 1;
                    self.write_indent();
                }
                self.buffer.push('}');
            }
            #[cfg(feature = "lazy")]
            Value::Lazy(lazy) => match lazy.as_ref() {
                crate::lazy::LazyValue::Unknown(raw)
                | crate::lazy::LazyValue::UnknownObject(raw)
                | crate::lazy::LazyValue::UnknownVector(raw) => self.buffer.push_str(raw),
                crate::lazy::LazyValue::Parsed(value) => self.format_value(value, dangling),
            },
        }
    }

    fn format_object_entry(&mut self, entry: &ObjectEntry) {
        self.format_leading(&entry.key_trivia);
        if self.emit_verbatim_entry(entry) {
            if entry.value.comma {
                self.buffer.push(',');
            }
            return;
        }

        self.buffer.push('"');
        self.buffer.push_str(&escape_string(&entry.key));
        self.buffer.push('"');

        self.buffer.push(':');
        if self.pretty() {
            self.buffer.push(' ');
        }

        self.format_node(&entry.value);
    }
}

// Display keeps the default behaviour: minified, comments kept.
impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.to_formatted_string(FormatOptions::default());
        write!(f, "{s}")
    }
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node = Node::new(self.clone());
        write!(f, "{node}")
    }
}
impl fmt::Display for ObjectEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = String::new();
        let mut formatter = Formatter {
            options: FormatOptions::default(),
            buffer: &mut buffer,
            depth: 0,
            preserve: None,
        };
        formatter.format_object_entry(self);
        formatter.format_trailing(&self.value.trailing);
        write!(f, "{buffer}")
    }
}

fn escape_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c <= '\u{001F}' => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04X}", c as u32);
            }
            c => escaped.push(c),
        }
    }
    escaped
}
