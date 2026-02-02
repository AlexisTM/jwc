use crate::ast::{Node, ObjectEntry, Trivia, Value};
use crate::Number;

pub struct SinglePassParser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    pending_trivia: Vec<Trivia>,
}

impl<'a> SinglePassParser<'a> {
    #[must_use]
    pub const fn new(input: &'a str) -> Self {
        SinglePassParser {
            input,
            bytes: input.as_bytes(),
            pos: 0,
            pending_trivia: Vec::new(),
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn take_pending_trivia(&mut self) -> Vec<Trivia> {
        if self.pending_trivia.is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut self.pending_trivia)
        }
    }

    // Helper to compute line/col from byte offset (only used for error messages)
    #[cfg_attr(feature = "profiling", hotpath::measure)]
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

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_string(&mut self) -> Result<String, String> {
        let bytes = self.bytes;
        let start = self.pos + 1; // eat opening quote
        let mut pos = start;

        // Fast path for strictly ASCII or non-escaped strings
        while pos < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(pos) };
            if b == b'"' {
                let s = unsafe { std::str::from_utf8_unchecked(&bytes[start..pos]) };
                self.pos = pos + 1; // eat closing quote
                return Ok(s.to_string());
            } else if b == b'\\' {
                break;
            }
            pos += 1;
        }

        self.pos = pos;

        self.parse_string_slow(start)
    }

    #[cold]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_string_slow(&mut self, start: usize) -> Result<String, String> {
        let mut s = String::with_capacity(32);
        s.push_str(unsafe { std::str::from_utf8_unchecked(&self.bytes[start..self.pos]) });

        while self.pos < self.bytes.len() {
            let b = unsafe { *self.bytes.get_unchecked(self.pos) };
            if b == b'"' {
                self.pos += 1;
                return Ok(s);
            }
            if b == b'\\' {
                self.pos += 1;
                if self.pos < self.bytes.len() {
                    let escaped = self.bytes[self.pos];
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
                            let first = self.parse_hex4_escape()?;
                            if (0xD800..=0xDBFF).contains(&first) {
                                if self.pos + 2 <= self.bytes.len()
                                    && self.bytes[self.pos] == b'\\'
                                    && self.bytes[self.pos + 1] == b'u'
                                {
                                    self.pos += 2;
                                    let second = self.parse_hex4_escape()?;
                                    if (0xDC00..=0xDFFF).contains(&second) {
                                        let codepoint = 0x10000
                                            + (((first - 0xD800) as u32) << 10)
                                            + ((second - 0xDC00) as u32);
                                        if let Some(ch) = char::from_u32(codepoint) {
                                            s.push(ch);
                                        } else {
                                            let (line, col) =
                                                self.position_from_offset(self.pos.saturating_sub(1));
                                            return Err(format!(
                                                "Invalid unicode escape at {line}:{col}"
                                            ));
                                        }
                                    } else {
                                        let (line, col) =
                                            self.position_from_offset(self.pos.saturating_sub(1));
                                        return Err(format!(
                                            "Invalid low surrogate in unicode escape at {line}:{col}"
                                        ));
                                    }
                                } else {
                                    let (line, col) =
                                        self.position_from_offset(self.pos.saturating_sub(1));
                                    return Err(format!(
                                        "Expected low surrogate after high surrogate at {line}:{col}"
                                    ));
                                }
                            } else if (0xDC00..=0xDFFF).contains(&first) {
                                let (line, col) =
                                    self.position_from_offset(self.pos.saturating_sub(1));
                                return Err(format!(
                                    "Unexpected low surrogate in unicode escape at {line}:{col}"
                                ));
                            } else if let Some(ch) = char::from_u32(first as u32) {
                                s.push(ch);
                            } else {
                                let (line, col) =
                                    self.position_from_offset(self.pos.saturating_sub(1));
                                return Err(format!("Invalid unicode escape at {line}:{col}"));
                            }
                        }
                        _ => {
                            let (line, col) = self.position_from_offset(self.pos.saturating_sub(1));
                            return Err(format!("Invalid escape sequence at {line}:{col}"));
                        }
                    }
                } else {
                    return Err("Unexpected EOF after \\".into());
                }
            } else {
                let ch = self.input[self.pos..].chars().next().unwrap();
                if ch <= '\u{001F}' {
                    let (line, col) = self.position_from_offset(self.pos);
                    return Err(format!(
                        "Unescaped control character in string at {line}:{col}"
                    ));
                }
                s.push(ch);
                self.pos += ch.len_utf8();
            }
        }
        let (line, col) = self.position_from_offset(self.input.len());
        Err(format!("Unterminated string at {line}:{col}"))
    }

    #[inline(always)]
    fn parse_hex4_escape(&mut self) -> Result<u16, String> {
        if self.pos + 4 > self.bytes.len() {
            let (line, col) = self.position_from_offset(self.bytes.len());
            return Err(format!("Unexpected EOF in unicode escape at {line}:{col}"));
        }

        let mut value = 0_u16;
        for _ in 0..4 {
            let b = self.bytes[self.pos];
            self.pos += 1;
            let nibble = match b {
                b'0'..=b'9' => (b - b'0') as u16,
                b'a'..=b'f' => (b - b'a' + 10) as u16,
                b'A'..=b'F' => (b - b'A' + 10) as u16,
                _ => {
                    let (line, col) = self.position_from_offset(self.pos.saturating_sub(1));
                    return Err(format!("Invalid unicode escape hex digit at {line}:{col}"));
                }
            };
            value = (value << 4) | nibble;
        }

        Ok(value)
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_number(&mut self) -> Result<Number, String> {
        let content_start = self.pos;
        if content_start >= self.bytes.len() {
            let (line, col) = self.position_from_offset(self.input.len());
            return Err(format!(
                "Unexpected EOF while parsing number at {line}:{col}"
            ));
        }

        let mut pos = content_start + 1; // consume first digit / '-'
        let bytes = self.bytes;

        while pos < bytes.len() {
            let nc = unsafe { *bytes.get_unchecked(pos) };
            if nc.is_ascii_digit()
                || nc == b'.'
                || nc == b'e'
                || nc == b'E'
                || nc == b'+'
                || nc == b'-'
            {
                pos += 1;
            } else {
                break;
            }
        }

        self.pos = pos;
        let num_str = unsafe { std::str::from_utf8_unchecked(&bytes[content_start..pos]) };
        if let Ok(parsed) = num_str.parse::<f64>() {
            Ok(Number::from_parsed_and_lexeme(parsed, num_str))
        } else {
            let (line, col) = self.position_from_offset(content_start);
            Err(format!("Invalid number at {line}:{col}"))
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_line_comment(&mut self) -> Result<String, String> {
        self.pos += 1; // eat second '/'
        let content_start = self.pos;

        let bytes = self.bytes;
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'\n' {
                break;
            }
            self.pos += 1;
        }
        Ok(unsafe { std::str::from_utf8_unchecked(&bytes[content_start..self.pos]) }.to_string())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_block_comment(&mut self) -> Result<String, String> {
        self.pos += 1; // eat '*'
        let content_start = self.pos;

        let bytes = self.bytes;
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'*' && self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'/'
            {
                let content_end = self.pos;
                self.pos += 2; // eat '*/'
                return Ok(unsafe {
                    std::str::from_utf8_unchecked(&bytes[content_start..content_end])
                }
                .to_string());
            }
            self.pos += 1;
        }

        let (line, col) = self.position_from_offset(self.input.len()); // Approx location
        Err(format!("Unterminated block comment at {line}:{col}"))
    }

    #[inline(always)]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn skip_whitespace_fast(&mut self) {
        let bytes = self.bytes;
        let mut pos = self.pos;
        while pos < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(pos) };
            if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' {
                pos += 1;
            } else {
                break;
            }
        }
        self.pos = pos;
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_trivia(&mut self) -> Result<(), String> {
        self.skip_whitespace_fast();
        if self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'/' || b > 127 {
                return self.consume_trivia_slow();
            }
        }
        Ok(())
    }

    #[cold]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_trivia_slow(&mut self) -> Result<(), String> {
        let bytes = self.bytes;
        while self.pos < bytes.len() {
            let b = bytes[self.pos];
            match b {
                b'/' => {
                    self.pos += 1;
                    if self.pos < bytes.len() {
                        let next = bytes[self.pos];
                        if next == b'/' {
                            let comment = self.parse_line_comment()?;
                            self.pending_trivia.push(Trivia::LineComment(comment));
                        } else if next == b'*' {
                            let comment = self.parse_block_comment()?;
                            self.pending_trivia.push(Trivia::BlockComment(comment));
                        } else {
                            let (line, col) = self.position_from_offset(self.pos - 1); // Approx
                            return Err(format!("Unexpected character '/' at {line}:{col}"));
                        }
                    } else {
                        let (line, col) = self.position_from_offset(bytes.len());
                        return Err(format!("Unexpected EOF after '/' at {line}:{col}"));
                    }
                }
                b if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' => {
                    self.pos += 1;
                }
                b if b > 127 => {
                    let ch = self.input[self.pos..].chars().next().unwrap();
                    if ch.is_whitespace() {
                        self.pos += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_object_colon(&mut self) -> Result<(), String> {
        loop {
            if self.pos >= self.bytes.len() {
                return Err("Unexpected EOF while looking for ':' after key".to_string());
            }

            match self.bytes[self.pos] {
                b':' => break,
                b'/' => {
                    self.pos += 1; // eat first '/'
                    if self.pos < self.bytes.len() {
                        let next = self.bytes[self.pos];
                        if next == b'/' {
                            let comment = self.parse_line_comment()?;
                            self.pending_trivia.push(Trivia::LineComment(comment));
                        } else if next == b'*' {
                            let comment = self.parse_block_comment()?;
                            self.pending_trivia.push(Trivia::BlockComment(comment));
                        } else {
                            let (line, col) = self.position_from_offset(self.pos - 1);
                            return Err(format!("Unexpected character '/' at {line}:{col}"));
                        }
                    } else {
                        let (line, col) = self.position_from_offset(self.bytes.len());
                        return Err(format!("Unexpected EOF after '/' at {line}:{col}"));
                    }
                }
                ws if ws.is_ascii_whitespace() => {
                    self.pos += 1;
                }
                ws if ws > 127 => {
                    let ch = self.input[self.pos..].chars().next().unwrap();
                    if ch.is_whitespace() {
                        self.pos += ch.len_utf8();
                    } else {
                        let (line, col) = self.position_from_offset(self.pos);
                        return Err(format!(
                            "Unexpected character '{ch}' between key and ':' at {line}:{col}"
                        ));
                    }
                }
                other => {
                    let ch = other as char;
                    let (line, col) = self.position_from_offset(self.pos);
                    return Err(format!(
                        "Unexpected character '{ch}' between key and ':' at {line}:{col}"
                    ));
                }
            }
        }

        // Consume the ':'.
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b':' {
            self.pos += 1;
        } else {
            let (line, col) = self.position_from_offset(self.bytes.len());
            return Err(format!("Expected ':' after key at {line}:{col}"));
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_value(&mut self) -> Result<Node, String> {
        self.parse_value_impl(true)
    }

    #[inline(always)]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_value_after_trivia(&mut self) -> Result<Node, String> {
        self.parse_value_impl(false)
    }

    #[inline(always)]
    fn parse_value_impl(&mut self, consume_trivia: bool) -> Result<Node, String> {
        if consume_trivia {
            self.consume_trivia()?;
        }

        if self.pos >= self.bytes.len() {
            return Err("Unexpected EOF".to_string());
        }

        let token_pos = self.pos;
        let token = self.bytes[self.pos];

        let trivia = self.take_pending_trivia();

        let value = match token {
            b'{' => {
                self.pos += 1;
                self.parse_object_value()?
            }
            b'[' => {
                self.pos += 1;
                self.parse_array_value()?
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
                if self.pos + 4 <= self.bytes.len() {
                    let val: u32 = unsafe {
                        std::ptr::read_unaligned(self.bytes.as_ptr().add(self.pos).cast::<u32>())
                    };
                    // "true" is 0x65757274 in little-endian
                    if val == u32::from_le_bytes(*b"true") {
                        self.pos += 4;
                        Value::Bool(true)
                    } else {
                        let (line, col) = self.position_from_offset(token_pos);
                        return Err(format!("Unexpected identifier at {line}:{col}"));
                    }
                } else {
                    let (line, col) = self.position_from_offset(token_pos);
                    return Err(format!("Unexpected identifier at {line}:{col}"));
                }
            }
            b'f' => {
                if self.pos + 5 <= self.bytes.len() {
                    let val: u32 = unsafe {
                        std::ptr::read_unaligned(self.bytes.as_ptr().add(self.pos).cast::<u32>())
                    };
                    let last_char = unsafe { *self.bytes.get_unchecked(self.pos + 4) };
                    if val == u32::from_le_bytes(*b"fals") && last_char == b'e' {
                        self.pos += 5;
                        Value::Bool(false)
                    } else {
                        let (line, col) = self.position_from_offset(token_pos);
                        return Err(format!("Unexpected identifier at {line}:{col}"));
                    }
                } else {
                    let (line, col) = self.position_from_offset(token_pos);
                    return Err(format!("Unexpected identifier at {line}:{col}"));
                }
            }
            b'n' => {
                if self.pos + 4 <= self.bytes.len() {
                    let val: u32 = unsafe {
                        std::ptr::read_unaligned(self.bytes.as_ptr().add(self.pos).cast::<u32>())
                    };
                    if val == u32::from_le_bytes(*b"null") {
                        self.pos += 4;
                        Value::Null
                    } else {
                        let (line, col) = self.position_from_offset(token_pos);
                        return Err(format!("Unexpected identifier at {line}:{col}"));
                    }
                } else {
                    let (line, col) = self.position_from_offset(token_pos);
                    return Err(format!("Unexpected identifier at {line}:{col}"));
                }
            }
            _ => {
                let ch = self.input[self.pos..].chars().next().unwrap();
                let (line, col) = self.position_from_offset(token_pos);
                return Err(format!("Unexpected character '{ch}' at {line}:{col}"));
            }
        };

        Ok(Node {
            value,
            trivia,
            comma: false,
        })
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_array_comma(&mut self, node: &mut Node) -> Result<(), String> {
        if self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if c == b',' {
                self.pos += 1;
                node.comma = true;
            } else if c != b']' {
                let ch = self.input[self.pos..].chars().next().unwrap();
                let (line, col) = self.position_from_offset(self.pos);
                return Err(format!(
                    "Expected ',' or ']' after array element, found '{ch}' at {line}:{col}"
                ));
            }
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn consume_object_comma(&mut self, node: &mut Node) -> Result<(), String> {
        if self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if c == b',' {
                self.pos += 1;
                node.comma = true;
            } else if c != b'}' {
                let ch = self.input[self.pos..].chars().next().unwrap();
                let (line, col) = self.position_from_offset(self.pos);
                return Err(format!(
                    "Expected ',' or '}}' after object member, found '{ch}' at {line}:{col}"
                ));
            }
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_array_value(&mut self) -> Result<Value, String> {
        let mut elements: Vec<Node> = Vec::with_capacity(8); // Pre-allocate for typical array

        loop {
            self.consume_trivia()?;
            if self.pos < self.bytes.len() {
                if self.bytes[self.pos] == b']' {
                    self.pos += 1;
                    if let Some(last) = elements.last_mut() {
                        last.trivia.append(&mut self.pending_trivia);
                    }
                    break;
                }
            } else {
                return Err("Unexpected EOF in array".to_string());
            }

            let mut node = self.parse_value_after_trivia()?;

            self.consume_trivia()?;
            // We keep pending trivia to become the next element's leading trivia.
            self.consume_array_comma(&mut node)?;

            elements.push(node);
        }

        Ok(Value::Array(elements))
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_object_value(&mut self) -> Result<Value, String> {
        let mut members: Vec<ObjectEntry> = Vec::with_capacity(8); // Pre-allocate for typical object

        loop {
            self.consume_trivia()?;
            if self.pos < self.bytes.len() {
                if self.bytes[self.pos] == b'}' {
                    self.pos += 1;
                    if let Some(last) = members.last_mut() {
                        last.value.trivia.append(&mut self.pending_trivia);
                    }
                    break;
                }
            } else {
                return Err("Unexpected EOF in object".to_string());
            }

            let key_trivia = self.take_pending_trivia();

            if self.pos < self.bytes.len() && self.bytes[self.pos] == b'"' {
                let key = self.parse_string()?;

                self.consume_object_colon()?;

                // pending_trivia already holds any comments from between key and
                // colon; parse_value will consume them as value-leading trivia.
                let mut node = self.parse_value()?;

                self.consume_trivia()?;
                // We keep pending trivia to become the next member value's leading trivia.
                self.consume_object_comma(&mut node)?;

                members.push(ObjectEntry {
                    key,
                    key_trivia,
                    value: node,
                });
            } else {
                let (line, col) = self.position_from_offset(self.bytes.len());
                return Err(format!("Expected string key at {line}:{col}"));
            }
        }

        Ok(Value::Object(members))
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn parse(&mut self) -> Result<Node, String> {
        let mut node = self.parse_value()?;

        // Consume trailing whitespace/comments after the root value and keep
        // those comments on the root node in the single-trivia model.
        self.consume_trivia()?;
        node.trivia.append(&mut self.pending_trivia);

        // Reject non-trivia trailing content.
        if self.pos < self.bytes.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            let (line, col) = self.position_from_offset(self.pos);
            return Err(format!(
                "Unexpected trailing content '{ch}' at {line}:{col}"
            ));
        }

        Ok(node)
    }
}
