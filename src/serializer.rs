use crate::ast::{Node, ObjectEntry, Trivia, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indentation {
    Spaces(u8),
    Tabs,
    None, // Minified/Default single line if no trivia
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentPolicy {
    Keep,
    Remove,
    Minify, // Convert Line comments to Block comments
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

impl Node {
    #[must_use]
    pub fn to_formatted_string(&self, options: FormatOptions) -> String {
        let mut buffer = String::new();
        let mut formatter = Formatter {
            options,
            buffer: &mut buffer,
            depth: 0,
        };
        formatter.format_node(self).unwrap();
        buffer
    }
}

struct Formatter<'a> {
    options: FormatOptions,
    buffer: &'a mut String,
    depth: usize,
}

impl Formatter<'_> {
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
        if self.options.indentation == Indentation::None {
            // No newlines in minified mode unless forced by comment?
            // Actually, if we minify comments to block comments, we don't strictly need newlines.
            // But if we keep line comments, we MUST have newlines.
        } else {
            self.buffer.push('\n');
        }
    }

    fn format_trivia(&mut self, trivia: &[Trivia]) {
        match self.options.comment_policy {
            CommentPolicy::Remove => (),
            CommentPolicy::Minify => (),
            CommentPolicy::Keep => {
                for t in trivia {
                    match t {
                        Trivia::LineComment(c) => {
                            self.buffer.push_str(format!("//{c}\n").as_str());
                        }
                        Trivia::BlockComment(c) => {
                            self.buffer.push_str(format!("/*{c}*/").as_str());
                        }
                    }
                }
            }
        }
    }

    fn format_node(&mut self, node: &Node) -> fmt::Result {
        self.format_trivia(&node.trivia);
        self.format_value(&node.value)?;
        if node.comma {
            // If we are formatting, checks strict JSON? JWC allows trailing commas.
            if matches!(self.options.indentation, Indentation::None) {
                // In strict minify, maybe remove? But JWC preserves.
            }
            self.buffer.push(',');
        }
        Ok(())
    }

    fn format_value(&mut self, value: &Value) -> fmt::Result {
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
                if !elements.is_empty() {
                    self.depth += 1;
                    if !matches!(self.options.indentation, Indentation::None) {
                        self.write_newline();
                    }

                    for (i, element) in elements.iter().enumerate() {
                        if !matches!(self.options.indentation, Indentation::None) {
                            self.write_indent();
                        }

                        self.format_node(element)?;

                        if i < elements.len() - 1 && !element.comma {
                            self.buffer.push(',');
                        }

                        if !matches!(self.options.indentation, Indentation::None) {
                            self.write_newline();
                        }
                    }
                    self.depth -= 1;
                    if !matches!(self.options.indentation, Indentation::None) {
                        self.write_indent();
                    }
                }
                self.buffer.push(']');
            }
            Value::Object(members) => {
                self.buffer.push('{');
                if !members.is_empty() {
                    self.depth += 1;
                    if !matches!(self.options.indentation, Indentation::None) {
                        self.write_newline();
                    }

                    for (i, entry) in members.iter().enumerate() {
                        if !matches!(self.options.indentation, Indentation::None) {
                            self.write_indent();
                        }

                        self.format_object_entry(entry)?;

                        if i < members.len() - 1 && !entry.value.comma {
                            self.buffer.push(',');
                        }

                        if !matches!(self.options.indentation, Indentation::None) {
                            self.write_newline();
                        }
                    }
                    self.depth -= 1;
                    if !matches!(self.options.indentation, Indentation::None) {
                        self.write_indent();
                    }
                }
                self.buffer.push('}');
            }
            #[cfg(feature = "lazy")]
            Value::Lazy(lazy) => match lazy.as_ref() {
                crate::lazy::LazyValue::Unknown(raw)
                | crate::lazy::LazyValue::UnknownObject(raw)
                | crate::lazy::LazyValue::UnknownVector(raw) => self.buffer.push_str(raw),
                crate::lazy::LazyValue::Parsed(value) => self.format_value(value)?,
            },
        }
        Ok(())
    }

    fn format_object_entry(&mut self, entry: &ObjectEntry) -> fmt::Result {
        self.format_trivia(&entry.key_trivia);

        self.buffer.push('"');
        self.buffer.push_str(&escape_string(&entry.key));
        self.buffer.push('"');

        self.buffer.push(':');
        if !matches!(self.options.indentation, Indentation::None) {
            self.buffer.push(' ');
        }

        self.format_node(&entry.value)?;
        Ok(())
    }
}

// Preserve Display for back-compat/default behavior (equivalent to Indentation::None, Keep comments)
impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We use the new formatter with default options which preserves everything
        let s = self.to_formatted_string(FormatOptions::default());
        write!(f, "{s}")
    }
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Wrap in a temp node
        let node = Node::new(self.clone());
        write!(f, "{node}")
    }
}
impl fmt::Display for ObjectEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This is harder because ObjectEntry isn't a Node.
        // But we can just use default manual formatting or construct a Formatter.
        // For now, let's keep the manual impl for simple Display, OR delegate to Formatter logic?
        // Formatter requires a buffer.

        // Original impl:
        for trivia in &self.key_trivia {
            write!(f, "{trivia}")?;
        }
        write!(f, "\"{}\"", escape_string(&self.key))?;
        write!(f, ":")?;
        write!(f, "{}", self.value)?;
        Ok(())
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
