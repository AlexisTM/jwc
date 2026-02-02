use crate::Number;
use crate::ast::{Node, ObjectEntry, Value};
use crate::parser_core::{ParserCore, fast_parse_int};

pub use crate::parser_core::MAX_DEPTH;

pub struct Parser<'a> {
    core: ParserCore<'a>,
}

impl<'a> Parser<'a> {
    #[must_use]
    pub const fn new(input: &'a str) -> Self {
        Self {
            core: ParserCore::new(input),
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_string(&mut self) -> Result<String, String> {
        let start = self.core.pos + 1; // eat opening quote
        let (end, saw_backslash) = self.core.find_string_terminator(start);
        if !saw_backslash && end < self.core.bytes.len() {
            self.core.validate_no_control_chars(start, end)?;
            let s = unsafe { std::str::from_utf8_unchecked(&self.core.bytes[start..end]) };
            self.core.pos = end + 1;
            return Ok(s.to_string());
        }
        self.core.pos = end;
        self.parse_string_slow(start)
    }

    #[cold]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_string_slow(&mut self, start: usize) -> Result<String, String> {
        let mut s = String::with_capacity(32);
        s.push_str(unsafe {
            std::str::from_utf8_unchecked(&self.core.bytes[start..self.core.pos])
        });

        while self.core.pos < self.core.bytes.len() {
            let b = unsafe { *self.core.bytes.get_unchecked(self.core.pos) };
            if b == b'"' {
                self.core.pos += 1;
                return Ok(s);
            }
            if b == b'\\' {
                self.core.pos += 1;
                if self.core.pos < self.core.bytes.len() {
                    let escaped = self.core.bytes[self.core.pos];
                    self.core.pos += 1;
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
                            let first = self.core.parse_hex4_escape()?;
                            if (0xD800..=0xDBFF).contains(&first) {
                                if self.core.pos + 2 <= self.core.bytes.len()
                                    && self.core.bytes[self.core.pos] == b'\\'
                                    && self.core.bytes[self.core.pos + 1] == b'u'
                                {
                                    self.core.pos += 2;
                                    let second = self.core.parse_hex4_escape()?;
                                    if (0xDC00..=0xDFFF).contains(&second) {
                                        let codepoint = 0x10000
                                            + (((first - 0xD800) as u32) << 10)
                                            + ((second - 0xDC00) as u32);
                                        if let Some(ch) = char::from_u32(codepoint) {
                                            s.push(ch);
                                        } else {
                                            let (line, col) = self.core.position_from_offset(
                                                self.core.pos.saturating_sub(1),
                                            );
                                            return Err(format!(
                                                "Invalid unicode escape at {line}:{col}"
                                            ));
                                        }
                                    } else {
                                        let (line, col) = self
                                            .core
                                            .position_from_offset(self.core.pos.saturating_sub(1));
                                        return Err(format!(
                                            "Invalid low surrogate in unicode escape at {line}:{col}"
                                        ));
                                    }
                                } else {
                                    let (line, col) = self
                                        .core
                                        .position_from_offset(self.core.pos.saturating_sub(1));
                                    return Err(format!(
                                        "Expected low surrogate after high surrogate at {line}:{col}"
                                    ));
                                }
                            } else if (0xDC00..=0xDFFF).contains(&first) {
                                let (line, col) = self
                                    .core
                                    .position_from_offset(self.core.pos.saturating_sub(1));
                                return Err(format!(
                                    "Unexpected low surrogate in unicode escape at {line}:{col}"
                                ));
                            } else if let Some(ch) = char::from_u32(first as u32) {
                                s.push(ch);
                            } else {
                                let (line, col) = self
                                    .core
                                    .position_from_offset(self.core.pos.saturating_sub(1));
                                return Err(format!("Invalid unicode escape at {line}:{col}"));
                            }
                        }
                        _ => {
                            let (line, col) = self
                                .core
                                .position_from_offset(self.core.pos.saturating_sub(1));
                            return Err(format!("Invalid escape sequence at {line}:{col}"));
                        }
                    }
                } else {
                    let (line, col) = self.core.position_from_offset(self.core.pos);
                    return Err(format!("Unexpected EOF after \\ at {line}:{col}"));
                }
            } else {
                let ch = self.core.input[self.core.pos..].chars().next().unwrap();
                if ch <= '\u{001F}' {
                    let (line, col) = self.core.position_from_offset(self.core.pos);
                    return Err(format!(
                        "Unescaped control character in string at {line}:{col}"
                    ));
                }
                s.push(ch);
                self.core.pos += ch.len_utf8();
            }
        }
        let (line, col) = self.core.position_from_offset(self.core.input.len());
        Err(format!("Unterminated string at {line}:{col}"))
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_number(&mut self) -> Result<Number, String> {
        let content_start = self.core.pos;
        if content_start >= self.core.bytes.len() {
            let (line, col) = self.core.position_from_offset(self.core.input.len());
            return Err(format!(
                "Unexpected EOF while parsing number at {line}:{col}"
            ));
        }
        let (lex, has_frac_or_exp) = self.core.scan_number_lexeme();
        if !has_frac_or_exp && let Some(n) = fast_parse_int(lex.as_bytes()) {
            return Ok(Number::from(n));
        }
        if let Ok(parsed) = lex.parse::<f64>() {
            Ok(Number::from_parsed_and_lexeme(parsed, lex))
        } else {
            let (line, col) = self.core.position_from_offset(content_start);
            Err(format!("Invalid number at {line}:{col}"))
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_object_colon(&mut self) -> Result<(), String> {
        self.core.consume_trivia()?;
        if self.core.pos < self.core.bytes.len() && self.core.bytes[self.core.pos] == b':' {
            self.core.pos += 1;
            Ok(())
        } else {
            let (line, col) = self.core.position_from_offset(self.core.pos);
            Err(format!("Expected ':' after key at {line}:{col}"))
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_value(&mut self, depth: usize) -> Result<Node, String> {
        self.parse_value_impl(true, depth)
    }

    #[inline(always)]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_value_after_trivia(&mut self, depth: usize) -> Result<Node, String> {
        self.parse_value_impl(false, depth)
    }

    #[inline(always)]
    fn parse_value_impl(&mut self, consume_trivia: bool, depth: usize) -> Result<Node, String> {
        if consume_trivia {
            self.core.consume_trivia()?;
        }

        if self.core.pos >= self.core.bytes.len() {
            let (line, col) = self.core.position_from_offset(self.core.pos);
            return Err(format!("Unexpected EOF at {line}:{col}"));
        }

        let token_pos = self.core.pos;
        let token = self.core.bytes[self.core.pos];

        let trivia = self.core.take_pending_trivia();

        let value = match token {
            b'{' => {
                if depth >= MAX_DEPTH {
                    let (line, col) = self.core.position_from_offset(token_pos);
                    return Err(format!("maximum nesting depth exceeded at {line}:{col}"));
                }
                self.core.pos += 1;
                self.parse_object_value(depth + 1)?
            }
            b'[' => {
                if depth >= MAX_DEPTH {
                    let (line, col) = self.core.position_from_offset(token_pos);
                    return Err(format!("maximum nesting depth exceeded at {line}:{col}"));
                }
                self.core.pos += 1;
                self.parse_array_value(depth + 1)?
            }
            b'"' => {
                let s = self.parse_string()?;
                Value::String(s)
            }
            c if c.is_ascii_digit() || c == b'-' => {
                let n = self.parse_number()?;
                Value::Number(n)
            }
            b't' => {
                if self.core.pos + 4 <= self.core.bytes.len() {
                    let val: u32 = unsafe {
                        std::ptr::read_unaligned(
                            self.core.bytes.as_ptr().add(self.core.pos).cast::<u32>(),
                        )
                    };
                    // "true" is 0x65757274 in little-endian
                    if val == u32::from_le_bytes(*b"true") {
                        self.core.pos += 4;
                        Value::Bool(true)
                    } else {
                        let (line, col) = self.core.position_from_offset(token_pos);
                        return Err(format!("Unexpected identifier at {line}:{col}"));
                    }
                } else {
                    let (line, col) = self.core.position_from_offset(token_pos);
                    return Err(format!("Unexpected identifier at {line}:{col}"));
                }
            }
            b'f' => {
                if self.core.pos + 5 <= self.core.bytes.len() {
                    let val: u32 = unsafe {
                        std::ptr::read_unaligned(
                            self.core.bytes.as_ptr().add(self.core.pos).cast::<u32>(),
                        )
                    };
                    let last_char = unsafe { *self.core.bytes.get_unchecked(self.core.pos + 4) };
                    if val == u32::from_le_bytes(*b"fals") && last_char == b'e' {
                        self.core.pos += 5;
                        Value::Bool(false)
                    } else {
                        let (line, col) = self.core.position_from_offset(token_pos);
                        return Err(format!("Unexpected identifier at {line}:{col}"));
                    }
                } else {
                    let (line, col) = self.core.position_from_offset(token_pos);
                    return Err(format!("Unexpected identifier at {line}:{col}"));
                }
            }
            b'n' => {
                if self.core.pos + 4 <= self.core.bytes.len() {
                    let val: u32 = unsafe {
                        std::ptr::read_unaligned(
                            self.core.bytes.as_ptr().add(self.core.pos).cast::<u32>(),
                        )
                    };
                    if val == u32::from_le_bytes(*b"null") {
                        self.core.pos += 4;
                        Value::Null
                    } else {
                        let (line, col) = self.core.position_from_offset(token_pos);
                        return Err(format!("Unexpected identifier at {line}:{col}"));
                    }
                } else {
                    let (line, col) = self.core.position_from_offset(token_pos);
                    return Err(format!("Unexpected identifier at {line}:{col}"));
                }
            }
            _ => {
                let ch = self.core.input[self.core.pos..].chars().next().unwrap();
                let (line, col) = self.core.position_from_offset(token_pos);
                return Err(format!("Unexpected character '{ch}' at {line}:{col}"));
            }
        };

        Ok(Node { value, trivia })
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_array_comma(&mut self) -> Result<(), String> {
        if self.core.pos < self.core.bytes.len() {
            let c = self.core.bytes[self.core.pos];
            if c == b',' {
                self.core.pos += 1;
            } else if c != b']' {
                let ch = self.core.input[self.core.pos..].chars().next().unwrap();
                let (line, col) = self.core.position_from_offset(self.core.pos);
                return Err(format!(
                    "Expected ',' or ']' after array element, found '{ch}' at {line}:{col}"
                ));
            }
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_object_comma(&mut self) -> Result<(), String> {
        if self.core.pos < self.core.bytes.len() {
            let c = self.core.bytes[self.core.pos];
            if c == b',' {
                self.core.pos += 1;
            } else if c != b'}' {
                let ch = self.core.input[self.core.pos..].chars().next().unwrap();
                let (line, col) = self.core.position_from_offset(self.core.pos);
                return Err(format!(
                    "Expected ',' or '}}' after object member, found '{ch}' at {line}:{col}"
                ));
            }
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_array_value(&mut self, depth: usize) -> Result<Value, String> {
        let mut elements: Vec<Node> = Vec::with_capacity(8);

        loop {
            self.core.consume_trivia()?;
            if self.core.pos < self.core.bytes.len() {
                if self.core.bytes[self.core.pos] == b']' {
                    self.core.pos += 1;
                    if let Some(last) = elements.last_mut() {
                        last.trivia.append(&mut self.core.pending_trivia);
                    }
                    break;
                }
            } else {
                let (line, col) = self.core.position_from_offset(self.core.pos);
                return Err(format!("Unexpected EOF in array at {line}:{col}"));
            }

            let node = self.parse_value_after_trivia(depth)?;

            self.core.consume_trivia()?;
            // We keep pending trivia to become the next element's leading trivia.
            self.consume_array_comma()?;

            elements.push(node);
        }

        Ok(Value::Array(elements))
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_object_value(&mut self, depth: usize) -> Result<Value, String> {
        let mut members: Vec<ObjectEntry> = Vec::with_capacity(8);

        loop {
            self.core.consume_trivia()?;
            if self.core.pos < self.core.bytes.len() {
                if self.core.bytes[self.core.pos] == b'}' {
                    self.core.pos += 1;
                    if let Some(last) = members.last_mut() {
                        last.value.trivia.append(&mut self.core.pending_trivia);
                    }
                    break;
                }
            } else {
                let (line, col) = self.core.position_from_offset(self.core.pos);
                return Err(format!("Unexpected EOF in object at {line}:{col}"));
            }

            let key_trivia = self.core.take_pending_trivia();

            if self.core.pos < self.core.bytes.len() && self.core.bytes[self.core.pos] == b'"' {
                let key = self.parse_string()?;

                self.consume_object_colon()?;

                // pending_trivia already holds any comments from between key and
                // colon; parse_value will consume them as value-leading trivia.
                let node = self.parse_value(depth)?;

                self.core.consume_trivia()?;
                // We keep pending trivia to become the next member value's leading trivia.
                self.consume_object_comma()?;

                members.push(ObjectEntry {
                    key,
                    key_trivia,
                    value: node,
                });
            } else {
                let (line, col) = self.core.position_from_offset(self.core.bytes.len());
                return Err(format!("Expected string key at {line}:{col}"));
            }
        }

        Ok(Value::Object(members))
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn parse(&mut self) -> crate::Result<Node> {
        self.parse_inner().map_err(|msg| self.core.lift_err(msg))
    }

    fn parse_inner(&mut self) -> Result<Node, String> {
        let mut node = self.parse_value(0)?;

        // Consume trailing whitespace/comments after the root value and keep
        // those comments on the root node in the single-trivia model.
        self.core.consume_trivia()?;
        node.trivia.append(&mut self.core.pending_trivia);

        // Reject non-trivia trailing content.
        if self.core.pos < self.core.bytes.len() {
            let ch = self.core.input[self.core.pos..].chars().next().unwrap();
            let (line, col) = self.core.position_from_offset(self.core.pos);
            return Err(format!(
                "Unexpected trailing content '{ch}' at {line}:{col}"
            ));
        }

        Ok(node)
    }
}
