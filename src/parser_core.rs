//! Shared cursor + tokenizer state used by both `Parser` (owned JSONC) and
//! `LazyParser` (borrowed strict JSON + comments). Grammar walks live in
//! the per-parser modules so each stays monomorphic and fully inlinable;
//! everything that is actually shared — whitespace, trivia collection,
//! string / number scanning, structural separators, position math —
//! sits here and is called by both walks.

use crate::ast::Trivia;

/// Max JSONC nesting depth. Matches `serde_json`. Past this, parsers return
/// `Error::Parse { msg: "maximum nesting depth exceeded", .. }`.
pub const MAX_DEPTH: usize = 128;

/// Pure-integer lexeme → `i64`, or `None` on overflow / non-digit. Both
/// parsers use this as a fast path before falling back to `f64::parse`
/// for lexemes containing `.` / `e` / `E`.
///
/// Accumulates with the sign applied per digit (subtract when negative)
/// so `i64::MIN` is representable — negating after the fact would overflow.
#[inline]
pub(crate) fn fast_parse_int(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() {
        return None;
    }
    let (neg, digits) = match bytes[0] {
        b'-' => (true, &bytes[1..]),
        _ => (false, bytes),
    };
    if digits.is_empty() {
        return None;
    }
    let mut n: i64 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return None;
        }
        let d = (c - b'0') as i64;
        n = n.checked_mul(10)?;
        n = if neg {
            n.checked_sub(d)?
        } else {
            n.checked_add(d)?
        };
    }
    Some(n)
}

pub(crate) struct ParserCore<'a> {
    pub(crate) input: &'a str,
    pub(crate) bytes: &'a [u8],
    pub(crate) pos: usize,
    pub(crate) pending_trivia: Vec<Trivia>,
}

impl<'a> ParserCore<'a> {
    #[must_use]
    pub(crate) const fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
            pending_trivia: Vec::new(),
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    #[inline(always)]
    pub(crate) fn take_pending_trivia(&mut self) -> Vec<Trivia> {
        if self.pending_trivia.is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut self.pending_trivia)
        }
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub(crate) fn position_from_offset(&self, offset: usize) -> (usize, usize) {
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

    /// Convert parser-internal `String` (with trailing " at {line}:{col}") into
    /// a structured `Error::Parse`. Falls back to current cursor position if
    /// the suffix is missing.
    pub(crate) fn lift_err(&self, msg: String) -> crate::Error {
        if let Some(idx) = msg.rfind(" at ") {
            let (head, tail) = msg.split_at(idx);
            let pos = &tail[4..];
            if let Some((l, c)) = pos.split_once(':')
                && let (Ok(line), Ok(col)) = (l.parse::<usize>(), c.parse::<usize>())
            {
                return crate::Error::parse(line, col, head.to_string());
            }
        }
        let (line, col) = self.position_from_offset(self.pos);
        crate::Error::parse(line, col, msg)
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    #[inline(always)]
    pub(crate) fn skip_whitespace_fast(&mut self) {
        if self.pos < self.bytes.len() {
            let b = unsafe { *self.bytes.get_unchecked(self.pos) };
            if b != b' ' && b != b'\n' && b != b'\r' && b != b'\t' {
                return;
            }
        }
        self.pos = crate::simd::skip_ws(self.bytes, self.pos);
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub(crate) fn consume_trivia(&mut self) -> Result<(), String> {
        self.skip_whitespace_fast();
        if self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'/' || b > 127 {
                return self.consume_trivia_slow();
            }
        }
        Ok(())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    #[cold]
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
                            let (line, col) = self.position_from_offset(self.pos - 1);
                            return Err(format!("Unexpected character '/' at {line}:{col}"));
                        }
                    } else {
                        let (line, col) = self.position_from_offset(bytes.len());
                        return Err(format!("Unexpected EOF after '/' at {line}:{col}"));
                    }
                }
                b if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' => self.pos += 1,
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
    fn parse_line_comment(&mut self) -> Result<String, String> {
        self.pos += 1; // eat second '/'
        let start = self.pos;
        self.pos = crate::simd::find_newline(self.bytes, self.pos);
        Ok(unsafe { std::str::from_utf8_unchecked(&self.bytes[start..self.pos]) }.to_string())
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn parse_block_comment(&mut self) -> Result<String, String> {
        self.pos += 1; // eat '*'
        let start = self.pos;
        let bytes = self.bytes;
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'*' && self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'/'
            {
                let end = self.pos;
                self.pos += 2;
                return Ok(unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) }.to_string());
            }
            self.pos += 1;
        }
        let (line, col) = self.position_from_offset(self.input.len());
        Err(format!("Unterminated block comment at {line}:{col}"))
    }

    /// Scan digits / `.` / `e` / `E` / `+` / `-` starting at `self.pos`, which
    /// must already sit on the first digit or `-`. Returns the lexeme slice
    /// and whether it contains a fraction or exponent (callers that want
    /// integer fast-paths check this).
    pub(crate) fn scan_number_lexeme(&mut self) -> (&'a str, bool) {
        let start = self.pos;
        let mut pos = start + 1;
        let bytes = self.bytes;
        let mut has_frac_or_exp = false;
        while pos < bytes.len() {
            let nc = unsafe { *bytes.get_unchecked(pos) };
            if nc.is_ascii_digit() {
                pos += 1;
            } else if matches!(nc, b'.' | b'e' | b'E' | b'+' | b'-') {
                has_frac_or_exp = true;
                pos += 1;
            } else {
                break;
            }
        }
        self.pos = pos;
        let lex = unsafe { std::str::from_utf8_unchecked(&bytes[start..pos]) };
        (lex, has_frac_or_exp)
    }

    /// Scan from just past the opening `"` to the next terminator. Returns
    /// (end_pos, saw_backslash). If saw_backslash is false and end_pos < len,
    /// bytes[start..end_pos] is a clean slice with no escapes. Does not
    /// advance `self.pos` past the terminator — callers decide.
    #[inline]
    pub(crate) fn find_string_terminator(&self, start: usize) -> (usize, bool) {
        let end = crate::simd::find_string_end(self.bytes, start);
        if end >= self.bytes.len() {
            return (end, false);
        }
        (end, self.bytes[end] == b'\\')
    }

    /// Used by the lazy parser: walk past escapes until closing `"`. Returns
    /// the byte offset of the closing quote (does NOT consume it).
    pub(crate) fn scan_string_skip_escapes(&mut self, start: usize) -> Result<usize, String> {
        let mut i = start;
        let bytes = self.bytes;
        loop {
            i = crate::simd::find_string_end(bytes, i);
            if i >= bytes.len() {
                let (line, col) = self.position_from_offset(bytes.len());
                return Err(format!("unterminated string at {line}:{col}"));
            }
            if bytes[i] == b'"' {
                return Ok(i);
            }
            if i + 1 >= bytes.len() {
                let (line, col) = self.position_from_offset(bytes.len());
                return Err(format!("trailing backslash in string at {line}:{col}"));
            }
            if bytes[i + 1] == b'u' {
                i += 6;
            } else {
                i += 2;
            }
        }
    }

    /// Validate that `bytes[start..end]` contains no unescaped control bytes
    /// (< 0x20). Both parsers call this after their fast-path string scan
    /// finds a closing quote, to enforce RFC 8259's ban on raw control chars
    /// inside string literals.
    pub(crate) fn validate_no_control_chars(&self, start: usize, end: usize) -> Result<(), String> {
        let slice = &self.bytes[start..end];
        if let Some(idx) = slice.iter().position(|&b| b < 0x20) {
            let (line, col) = self.position_from_offset(start + idx);
            return Err(format!(
                "Unescaped control character in string at {line}:{col}"
            ));
        }
        Ok(())
    }

    pub(crate) fn parse_hex4_escape(&mut self) -> Result<u16, String> {
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

    /// Try-match fixed keyword (`true`, `false`, `null`). Returns true and
    /// advances past the keyword on success.
    #[inline(always)]
    pub(crate) fn try_keyword(&mut self, kw: &[u8]) -> bool {
        let end = self.pos + kw.len();
        if end <= self.bytes.len() && &self.bytes[self.pos..end] == kw {
            self.pos = end;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fast_parse_int;

    #[test]
    fn parses_i64_min() {
        assert_eq!(fast_parse_int(b"-9223372036854775808"), Some(i64::MIN));
    }

    #[test]
    fn parses_i64_max() {
        assert_eq!(fast_parse_int(b"9223372036854775807"), Some(i64::MAX));
    }

    #[test]
    fn overflow_past_i64_min_returns_none() {
        assert_eq!(fast_parse_int(b"-9223372036854775809"), None);
    }

    #[test]
    fn overflow_past_i64_max_returns_none() {
        assert_eq!(fast_parse_int(b"9223372036854775808"), None);
    }

    #[test]
    fn empty_and_nondigit_return_none() {
        assert_eq!(fast_parse_int(b""), None);
        assert_eq!(fast_parse_int(b"-"), None);
        assert_eq!(fast_parse_int(b"12x"), None);
    }

    #[test]
    fn lift_err_fallback_when_message_has_no_position_suffix() {
        // A message without the " at L:C" suffix falls through to using the
        // parser's current cursor position — cover the fallback arm of
        // `ParserCore::lift_err`.
        let core = super::ParserCore::new("abc");
        let err = core.lift_err("no suffix here".into());
        match err {
            crate::Error::Parse { line, col, msg } => {
                assert!(msg.contains("no suffix here"));
                assert_eq!((line, col), (1, 1));
            }
            other => panic!("unexpected: {other:?}"),
        }

        // A malformed suffix ("at X:Y" where X/Y aren't numbers) also falls
        // through to the position-based default.
        let err = core.lift_err("broken at foo:bar".into());
        match err {
            crate::Error::Parse { msg, .. } => assert!(msg.contains("broken at foo:bar")),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
