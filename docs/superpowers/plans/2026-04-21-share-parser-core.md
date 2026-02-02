# Share Parser Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop arena parser. Rename `Parser` → `Parser`. Share tokenizer/trivia state between `Parser` and `LazyParser` via a `ParserCore<'a>` helper so both support JSONC comments, while keeping the grammar walks monomorphic for aggressive inlining.

**Architecture:** New `parser_core` module owns byte cursor + `pending_trivia: Vec<Trivia>` + all shared token helpers (whitespace, comments, string scan, number scan, keyword check, hex escape, position → line:col, error lifting, structural separators). `Parser<'a>` and `LazyParser<'a>` each embed a `ParserCore<'a>` by composition and implement their own `parse_value` / `parse_array` / `parse_object` that dispatch tokens via core but build their own output type (`Node` vs `LazyNode`). Lazy tree gains a `LazyNode { value: LazyVal, trivia: Box<[Trivia]> }` wrapper so JSONC comments attach per value like the owned tree. `MAX_DEPTH = 128` cap lives on the `parser` module.

**Tech Stack:** Rust 2024, no `serde` in core crate, criterion benches, bumpalo removed.

---

## File Map

Create:
- `src/parser_core.rs` — shared cursor + tokenizer helpers
- `src/parser.rs` — renamed from `src/parser.rs`
- `tests/lazy_comments_test.rs` — new test coverage for trivia via lazy path

Modify:
- `src/lib.rs` — drop arena, rename module, re-exports
- `src/lazy_val.rs` — adopt core, add `LazyNode` wrapper, grow trivia collection
- `src/ast.rs` — no change expected
- `Cargo.toml` — drop `bumpalo` dep + `arena` feature
- `benches/common/mod.rs`, `benches/parse_small.rs`, `benches/parse_large.rs`, `benches/parse_nested.rs`, `benches/access.rs` — drop arena, update `sum_numbers_lazy` sig
- `examples/profile_parsers.rs` — drop arena block
- `tests/stress_test.rs` — drop `CROSS_VALIDATION_JSON` / `arena_matches_owned` cfg block; `MAX_DEPTH` import path
- `tests/utf8_test.rs`, `tests/error_positions_test.rs` — adjust any `LazyVal` → `LazyNode` chains
- `CLAUDE.md`, `README.md`, `docs/parsing.md`, `docs/ast.md` — remove arena, document LazyNode+trivia
- All call sites of `Parser` across `examples/`, `tests/`

Delete:
- `src/arena.rs`
- `tests/arena_test.rs`

---

## Task 1: Drop arena

Arena isolated. Kill first, land clean, move on.

**Files:**
- Delete: `src/arena.rs`
- Delete: `tests/arena_test.rs`
- Modify: `Cargo.toml`, `src/lib.rs`
- Modify: `benches/parse_small.rs`, `benches/parse_large.rs`, `benches/parse_nested.rs`
- Modify: `examples/profile_parsers.rs`
- Modify: `tests/stress_test.rs`

- [ ] **Step 1: Delete arena source + test**

```bash
rm src/arena.rs tests/arena_test.rs
```

- [ ] **Step 2: Strip arena feature + dep from Cargo.toml**

In `Cargo.toml` remove:
```toml
bumpalo = { version = "3", features = ["collections"], optional = true }
```
and:
```toml
# Arena-allocated parse path: all container Vecs + decoded strings live in a
# user-provided `bumpalo::Bump`. Zero per-container heap allocations.
arena = ["dep:bumpalo"]
```

- [ ] **Step 3: Strip arena re-exports from src/lib.rs**

Replace lines 1-23 (top of file through re-exports):
```rust
pub mod ast;
mod error;
#[cfg(feature = "lazy")]
pub mod lazy;
pub mod lazy_val;
mod macros;
mod number;
pub mod patch;
pub mod pointer;
pub mod serializer;
mod simd;
pub mod parser;
pub mod traits;

// Re-exports
pub use ast::{Node, ObjectEntry, Trivia, Value, ValueIndex};
pub use error::{Error, Result};
pub use lazy_val::{LazyVal, from_str_lazy};
```

(Task 2 renames `parser` → `parser`. Don't do that yet.)

- [ ] **Step 4: Strip arena bench entries**

Edit `benches/parse_small.rs`: delete lines 21-27 (the `#[cfg(feature = "arena")]` block).
Edit `benches/parse_large.rs`: delete lines 21-27.
Edit `benches/parse_nested.rs`: delete lines 22-36 (both arena blocks).

- [ ] **Step 5: Strip arena block in examples/profile_parsers.rs**

Delete lines 25-33 (the `#[cfg(feature = "arena")]` block inside `main`).

- [ ] **Step 6: Strip arena test + unused const in tests/stress_test.rs**

Delete lines 95-110 (the `CROSS_VALIDATION_JSON` const) and lines 141-150 (the `arena_matches_owned` test). Update the file header comment at line 2 from "Also cross-validates that owned / lazy / arena parsers agree." to "Also cross-validates that owned + lazy parsers agree.".

- [ ] **Step 7: Verify build + tests pass**

Run:
```bash
cargo check --workspace
cargo test --workspace
```
Expected: PASS (minus pre-existing `reject_unescaped_control_character` failure — unrelated).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Drop arena parser

bumpalo gone. feature flag gone. tests/benches/examples stripped.
jwc now ships two parsers: Node tree (JSONC) + LazyVal (strict JSON)."
```

---

## Task 2: Rename `Parser` → `Parser`, module → `parser`

Mechanical rename. No behavior change. Ensures downstream tasks read cleanly.

**Files:**
- Rename: `src/parser.rs` → `src/parser.rs`
- Modify: `src/lib.rs`
- Modify: all call sites in `examples/`, `tests/`

- [ ] **Step 1: Move the file**

```bash
git mv src/parser.rs src/parser.rs
```

- [ ] **Step 2: Rename type inside src/parser.rs**

In `src/parser.rs`, replace every occurrence of `Parser` with `Parser`.

```bash
sed -i 's/Parser/Parser/g' src/parser.rs
```

Then fix the self-references — `Self` is unchanged but the `impl` blocks and `pub struct Parser<'a>` come through correctly.

- [ ] **Step 3: Update module path + re-exports in src/lib.rs**

Replace:
```rust
pub mod parser;
```
with:
```rust
pub mod parser;
```

Replace:
```rust
pub use parser::Parser;
```
with:
```rust
pub use parser::{MAX_DEPTH, Parser};
```

(Exposes `MAX_DEPTH` at crate root too for ergonomic test imports.)

- [ ] **Step 4: Update external references**

Grep-replace across the repo (excluding target/):
```bash
grep -rl --exclude-dir=target 'Parser\|parser' . | \
  xargs sed -i 's/parser/parser/g; s/Parser/Parser/g'
```

Confirm the following files are rewritten cleanly (visual check, no stray mentions):
- `examples/usage_demo.rs`
- `examples/profile.rs`
- `examples/profile_instrumented.rs`
- `examples/profile_minimal.rs`
- `examples/profile_target.rs`
- `tests/integration.rs`
- `tests/formatting_test.rs`
- `tests/pointer_patch_test.rs`
- `tests/stress_test.rs` (MAX_DEPTH import)

- [ ] **Step 5: Fix stress_test.rs MAX_DEPTH import**

In `tests/stress_test.rs`, change:
```rust
use jwc::parser::MAX_DEPTH;
```
to:
```rust
use jwc::MAX_DEPTH;
```

- [ ] **Step 6: Update CLAUDE.md references**

In `CLAUDE.md`:
- Replace `src/parser.rs` with `src/parser.rs` (one occurrence at line ~51).
- Leave the "Owned parser" / "Arena parser" sections for Task 8.

- [ ] **Step 7: Verify**

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: all PASS (minus the known pre-existing `reject_unescaped_control_character`).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Rename Parser to Parser

Module parser -> parser. Mechanical rename.
No behavior change. Sets up parser-core extraction."
```

---

## Task 3: Extract `ParserCore<'a>`

Move cursor + shared helpers to new `parser_core` module. `Parser<'a>` holds a `ParserCore<'a>` by composition. Grammar walk stays in `Parser`. Still one parser, no behavior change.

**Files:**
- Create: `src/parser_core.rs`
- Modify: `src/parser.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create src/parser_core.rs**

```rust
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

    #[inline(always)]
    pub(crate) fn take_pending_trivia(&mut self) -> Vec<Trivia> {
        if self.pending_trivia.is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut self.pending_trivia)
        }
    }

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

    fn parse_line_comment(&mut self) -> Result<String, String> {
        self.pos += 1; // eat second '/'
        let start = self.pos;
        self.pos = crate::simd::find_newline(self.bytes, self.pos);
        Ok(unsafe { std::str::from_utf8_unchecked(&self.bytes[start..self.pos]) }.to_string())
    }

    fn parse_block_comment(&mut self) -> Result<String, String> {
        self.pos += 1; // eat '*'
        let start = self.pos;
        let bytes = self.bytes;
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'*' && self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'/' {
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
    /// (end_pos, saw_backslash). If saw_backslash is false, bytes[start..end]
    /// is a clean slice with no escapes. Does not advance `self.pos` past the
    /// terminator — callers decide.
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
```

- [ ] **Step 2: Wire src/parser.rs onto ParserCore**

Replace the `Parser` struct in `src/parser.rs` (now called `Parser`) so it embeds core. At the top of `src/parser.rs`:

```rust
use crate::Number;
use crate::ast::{Node, ObjectEntry, Trivia, Value};
use crate::parser_core::ParserCore;

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
```

Everywhere inside the impl block that previously referenced `self.input`, `self.bytes`, `self.pos`, `self.pending_trivia`, change to `self.core.input`, `self.core.bytes`, `self.core.pos`, `self.core.pending_trivia`. Everywhere that called `self.take_pending_trivia()`, `self.position_from_offset(..)`, `self.skip_whitespace_fast()`, `self.consume_trivia()`, `self.consume_trivia_slow()`, `self.parse_line_comment()`, `self.parse_block_comment()`, `self.parse_hex4_escape()`, `self.lift_err(..)` — route to `self.core.*`.

Delete the moved method bodies from `src/parser.rs` (they now live in `parser_core`). Keep only the methods that build `Value` / `Node` and the ones specific to owned parsing:
- `parse_string` (fast-path slice to owned `String`, slow-path decode)
- `parse_string_slow`
- `parse_number` (builds `Number` from lexeme)
- `fast_parse_int_owned`
- `consume_object_colon` (stays here OR move to core — keep here for simplicity since it's called only from owned parser's `parse_object_value`; mirror in lazy)
- `consume_array_comma`, `consume_object_comma` — also stay here for now; lazy has its own inline variants today

Rewrite `parse_string` and `parse_number` to use the core scan helpers:

```rust
#[cfg_attr(feature = "profiling", hotpath::measure)]
fn parse_string(&mut self) -> Result<String, String> {
    let start = self.core.pos + 1; // eat opening quote
    let (end, saw_backslash) = self.core.find_string_terminator(start);
    if !saw_backslash && end < self.core.bytes.len() {
        let s = unsafe { std::str::from_utf8_unchecked(&self.core.bytes[start..end]) };
        self.core.pos = end + 1;
        return Ok(s.to_string());
    }
    self.core.pos = end;
    self.parse_string_slow(start)
}
```

`parse_string_slow` keeps its escape-decoding body but swaps every `self.bytes` → `self.core.bytes`, every `self.pos` → `self.core.pos`, every `self.parse_hex4_escape` → `self.core.parse_hex4_escape`, every `self.position_from_offset` → `self.core.position_from_offset`, every `self.input` → `self.core.input`.

`parse_number` becomes:
```rust
#[cfg_attr(feature = "profiling", hotpath::measure)]
fn parse_number(&mut self) -> Result<Number, String> {
    let content_start = self.core.pos;
    if content_start >= self.core.bytes.len() {
        let (line, col) = self.core.position_from_offset(self.core.input.len());
        return Err(format!("Unexpected EOF while parsing number at {line}:{col}"));
    }
    let (lex, has_frac_or_exp) = self.core.scan_number_lexeme();
    if !has_frac_or_exp
        && let Some(n) = fast_parse_int_owned(lex.as_bytes())
    {
        return Ok(Number::from(n));
    }
    if let Ok(parsed) = lex.parse::<f64>() {
        Ok(Number::from_parsed_and_lexeme(parsed, lex))
    } else {
        let (line, col) = self.core.position_from_offset(content_start);
        Err(format!("Invalid number at {line}:{col}"))
    }
}
```

Keyword matching uses `self.core.try_keyword(b"true")` etc.

- [ ] **Step 3: Register module in src/lib.rs**

In `src/lib.rs`:
```rust
pub mod parser;
mod parser_core;
```

and:
```rust
pub use parser::{MAX_DEPTH, Parser};
```

- [ ] **Step 4: Verify**

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: all PASS (minus the known pre-existing failure).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Extract ParserCore with shared tokenizer helpers

Cursor + pending_trivia + whitespace/comment/number/string scan +
keyword match + hex escape + position math now live in parser_core.
Parser composes a ParserCore and keeps its monomorphic grammar walk.
Prep for LazyParser to share the same core."
```

---

## Task 4: Add `LazyNode` wrapper + `LazyObjectEntry` to `src/lazy_val.rs`

LazyVal currently has no home for trivia. Introduce `LazyNode { value, trivia }` mirroring owned `Node`. `from_str_lazy` returns `LazyNode<'a>`. Nested array items and object values become `LazyNode<'a>`. Object entries become `LazyObjectEntry<'a>` with `key_trivia`.

Add passthrough accessors on `LazyNode` so existing call chains (`lv.get("x").as_i64()`) keep compiling.

This is the API-break step. Test that trivia can be read back BEFORE wiring the parser. Parser-side wiring is Task 5.

**Files:**
- Modify: `src/lazy_val.rs`
- Create: `tests/lazy_comments_test.rs`

- [ ] **Step 1: Write failing test for lazy trivia read-back**

Create `tests/lazy_comments_test.rs`:
```rust
//! Verifies the lazy parser preserves JSONC comments as trivia on the
//! appropriate `LazyNode`, matching the owned parser's behavior.

use jwc::{Trivia, from_str_lazy};

#[test]
fn leading_line_comment_on_root() {
    let src = "// hello\n42";
    let n = from_str_lazy(src).expect("parse");
    assert_eq!(n.trivia.len(), 1, "root should have one trivia entry");
    assert_eq!(n.trivia[0], Trivia::LineComment(" hello".into()));
    assert_eq!(n.value.as_i64(), Some(42));
}

#[test]
fn block_comment_between_object_key_and_value() {
    let src = r#"{"x" /* note */ : 1}"#;
    let n = from_str_lazy(src).expect("parse");
    let obj = n.value.as_object().expect("object");
    assert_eq!(obj.len(), 1);
    let entry = &obj[0];
    assert_eq!(entry.key.as_ref(), "x");
    // The trivia between the key and the colon belongs to the value node
    // (matches the owned parser's single-trivia placement).
    assert!(
        entry.value.trivia.iter().any(|t| matches!(t, Trivia::BlockComment(s) if s == " note ")),
        "expected block comment on value node, got {:?}",
        entry.value.trivia
    );
    assert_eq!(entry.value.value.as_i64(), Some(1));
}

#[test]
fn line_comment_before_array_element() {
    let src = "[\n  // first\n  1,\n  2\n]";
    let n = from_str_lazy(src).expect("parse");
    let arr = n.value.as_array().expect("array");
    assert_eq!(arr.len(), 2);
    assert!(
        arr[0].trivia.iter().any(|t| matches!(t, Trivia::LineComment(s) if s == " first")),
        "expected leading line comment on first element, got {:?}",
        arr[0].trivia
    );
}
```

- [ ] **Step 2: Run test to verify it fails to compile**

```bash
cargo test --test lazy_comments_test
```
Expected: compile error — `n.trivia` unknown, `entry.key` unknown, `entry.value.trivia` unknown, etc. This drives the API change.

- [ ] **Step 3: Rewrite `src/lazy_val.rs` public types**

Replace the `LazyVal` enum and its doc block with:
```rust
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

#[derive(Debug, Clone)]
pub enum LazyVal<'a> {
    Null,
    Bool(bool),
    /// Raw number lexeme (e.g. `-12.5e3`). Decoded on `as_i64` / `as_f64`.
    Number(&'a str),
    /// String content without outer quotes. May contain escapes; decoded
    /// on demand via `as_str`.
    String(&'a str),
    /// Array elements, each wrapped in a `LazyNode` for trivia.
    Array(Box<[LazyNode<'a>]>),
    /// Object members, sorted by key so `get` is O(log m).
    Object(Box<[LazyObjectEntry<'a>]>),
}
```

- [ ] **Step 4: Update `LazyVal` accessors to return `&LazyNode`**

In the `impl<'a> LazyVal<'a>` block:
- `pub fn as_array(&self) -> Option<&[LazyNode<'a>]>` — return slice of nodes.
- `pub fn as_object(&self) -> Option<&[LazyObjectEntry<'a>]>`.
- `pub fn get(&self, key: &str) -> Option<&LazyNode<'a>>` — binary-search over `LazyObjectEntry`, return `&entry.value`.
- `pub fn at(&self, idx: usize) -> Option<&LazyNode<'a>>`.
- `pub fn iter_array(&self) -> impl Iterator<Item = &LazyNode<'a>>`.
- `pub fn iter_object(&self) -> impl Iterator<Item = (&str, &LazyNode<'a>)>` — yield `(entry.key.as_ref(), &entry.value)`.

Keep existing `is_null` / `is_bool` / `as_bool` / `as_i64` / `as_u64` / `as_f64` / `as_str` / `len` / `is_empty` / `as_number_lex` unchanged — they operate on the raw variant, not nodes.

Update `to_value` array arm:
```rust
LazyVal::Array(items) => {
    Value::Array(items.iter().map(|n| Node::new(n.value.to_value())).collect())
}
LazyVal::Object(members) => Value::Object(
    members
        .iter()
        .map(|e| ObjectEntry::new(e.key.as_ref().to_string(), Node::new(e.value.value.to_value())))
        .collect(),
),
```

- [ ] **Step 5: Add `LazyNode` passthrough accessors**

New `impl<'a> LazyNode<'a>` block — each method delegates to `self.value.*`:

```rust
impl<'a> LazyNode<'a> {
    #[must_use] pub fn is_null(&self) -> bool { self.value.is_null() }
    #[must_use] pub fn is_bool(&self) -> bool { self.value.is_bool() }
    #[must_use] pub fn is_number(&self) -> bool { self.value.is_number() }
    #[must_use] pub fn is_string(&self) -> bool { self.value.is_string() }
    #[must_use] pub fn is_array(&self) -> bool { self.value.is_array() }
    #[must_use] pub fn is_object(&self) -> bool { self.value.is_object() }
    #[must_use] pub fn as_bool(&self) -> Option<bool> { self.value.as_bool() }
    #[must_use] pub fn as_i64(&self) -> Option<i64> { self.value.as_i64() }
    #[must_use] pub fn as_u64(&self) -> Option<u64> { self.value.as_u64() }
    #[must_use] pub fn as_f64(&self) -> Option<f64> { self.value.as_f64() }
    #[must_use] pub fn as_number_lex(&self) -> Option<&'a str> { self.value.as_number_lex() }
    #[must_use] pub fn as_str(&self) -> Option<Cow<'a, str>> { self.value.as_str() }
    #[must_use] pub fn as_array(&self) -> Option<&[LazyNode<'a>]> { self.value.as_array() }
    #[must_use] pub fn as_object(&self) -> Option<&[LazyObjectEntry<'a>]> { self.value.as_object() }
    #[must_use] pub fn get(&self, key: &str) -> Option<&LazyNode<'a>> { self.value.get(key) }
    #[must_use] pub fn at(&self, idx: usize) -> Option<&LazyNode<'a>> { self.value.at(idx) }
    #[must_use] pub fn len(&self) -> Option<usize> { self.value.len() }
    #[must_use] pub fn is_empty(&self) -> bool { self.value.is_empty() }
    pub fn iter_array(&self) -> impl Iterator<Item = &LazyNode<'a>> { self.value.iter_array() }
    pub fn iter_object(&self) -> impl Iterator<Item = (&str, &LazyNode<'a>)> { self.value.iter_object() }
    #[must_use] pub fn to_value(&self) -> crate::Value { self.value.to_value() }
}
```

- [ ] **Step 6: Update `from_str_lazy` signature**

```rust
pub fn from_str_lazy(src: &str) -> Result<LazyNode<'_>> {
    let mut p = Parser::new(src);
    p.parse_root()
}
```

Keep `Parser::parse_root` (the private one in lazy_val.rs — will be fully rewritten in Task 5) returning `Result<LazyNode<'a>>` and for now emit empty `trivia: Box::new([])` on every node. Task 5 fills it in.

Update the private parser's `parse_value` / `parse_array` / `parse_object` returning `Result<LazyNode<'a>>` where appropriate (array items and object values). Concretely: wrap every returned scalar in `LazyNode { value: .., trivia: Box::new([]) }`. Still no real comment handling in this task — this step only proves the type shape compiles.

- [ ] **Step 7: Export `LazyNode` + `LazyObjectEntry` from src/lib.rs**

```rust
pub use lazy_val::{LazyNode, LazyObjectEntry, LazyVal, from_str_lazy};
```

- [ ] **Step 8: Run tests**

```bash
cargo test --workspace
```
Expected: new `lazy_comments_test` FAILS on the trivia-presence asserts (we emit empty trivia for now). `stress_test::lazy_materialization_preserves_scalar_content` and utf8 tests should still PASS because passthrough accessors keep chained calls working.

If other callers break due to `&LazyVal` → `&LazyNode` in `as_array`/`as_object`/`get`/`at`, fix by calling `.value` where they need the raw variant.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Add LazyNode wrapper + LazyObjectEntry to lazy_val

Mirrors owned Node/ObjectEntry shape. Trivia not yet populated
(empty boxes) — Task 5 wires LazyParser onto ParserCore and
collects JSONC comments. API passthroughs on LazyNode keep
existing .get(k).as_i64() style call chains working."
```

---

## Task 5: LazyParser adopts `ParserCore` and collects JSONC trivia

Rewrite the internal `Parser` in `src/lazy_val.rs` (rename to avoid clash — call it `LazyParser` now) to wrap a `ParserCore<'a>`, call shared helpers, and emit trivia on every `LazyNode`. Match owned parser's single-trivia placement rules so comments land on the same logical position in both trees.

**Files:**
- Modify: `src/lazy_val.rs`

- [ ] **Step 1: Rename internal struct + route through ParserCore**

In `src/lazy_val.rs`, replace the private parser:

```rust
use crate::parser_core::{MAX_DEPTH, ParserCore};

struct LazyParser<'a> {
    core: ParserCore<'a>,
}

impl<'a> LazyParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { core: ParserCore::new(input) }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        let (line, col) = self.core.position_from_offset(self.core.pos);
        Error::parse(line, col, msg)
    }
}
```

Delete the old inline `skip_ws`, `position_from_offset`, `err`, `parse_root`, `parse_value`, `parse_keyword`, `parse_string` (the raw-slice one), `parse_number_lexeme`, `parse_array`, `parse_object` bodies — they move/delegate to core or get rewritten below.

- [ ] **Step 2: Rewrite `parse_root` on LazyParser**

```rust
impl<'a> LazyParser<'a> {
    fn parse_root(&mut self) -> Result<LazyNode<'a>> {
        let mut root = self.parse_value(0).map_err(|msg| self.core.lift_err(msg))?;
        // Trailing trivia attaches to the root node.
        self.core.consume_trivia().map_err(|msg| self.core.lift_err(msg))?;
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
}
```

- [ ] **Step 3: Rewrite `parse_value` with depth cap + leading trivia**

```rust
impl<'a> LazyParser<'a> {
    fn parse_value(&mut self, depth: usize) -> std::result::Result<LazyNode<'a>, String> {
        self.core.consume_trivia()?;
        if self.core.pos >= self.core.bytes.len() {
            return Err("unexpected EOF".into());
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
                if self.core.try_keyword(b"true") { LazyVal::Bool(true) }
                else {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("expected `true` at {l}:{c}"));
                }
            }
            b'f' => {
                if self.core.try_keyword(b"false") { LazyVal::Bool(false) }
                else {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("expected `false` at {l}:{c}"));
                }
            }
            b'n' => {
                if self.core.try_keyword(b"null") { LazyVal::Null }
                else {
                    let (l, c) = self.core.position_from_offset(token_pos);
                    return Err(format!("expected `null` at {l}:{c}"));
                }
            }
            other => {
                let (l, c) = self.core.position_from_offset(token_pos);
                return Err(format!("unexpected token {:?} at {l}:{c}", other as char));
            }
        };
        Ok(LazyNode { value, trivia: trivia.into_boxed_slice() })
    }
}
```

- [ ] **Step 4: Rewrite `parse_string_slice` (raw slice, lazy)**

```rust
impl<'a> LazyParser<'a> {
    fn parse_string_slice(&mut self) -> std::result::Result<&'a str, String> {
        let start = self.core.pos + 1;
        let end = self.core.scan_string_skip_escapes(start)?;
        let slice = &self.core.input[start..end];
        self.core.pos = end + 1;
        Ok(slice)
    }
}
```

- [ ] **Step 5: Rewrite `parse_array` with trailing-trivia attach**

```rust
impl<'a> LazyParser<'a> {
    fn parse_array(&mut self, depth: usize) -> std::result::Result<LazyVal<'a>, String> {
        let mut items: Vec<LazyNode<'a>> = Vec::with_capacity(8);
        loop {
            self.core.consume_trivia()?;
            if self.core.pos >= self.core.bytes.len() {
                return Err("unexpected EOF in array".into());
            }
            if self.core.bytes[self.core.pos] == b']' {
                self.core.pos += 1;
                // Attach any pending trailing trivia to the last element.
                if !self.core.pending_trivia.is_empty()
                    && let Some(last) = items.last_mut()
                {
                    let pending = self.core.take_pending_trivia();
                    let mut combined: Vec<Trivia> = last.trivia.clone().into_vec();
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
                    return Err(format!(
                        "expected ',' or ']' in array, got {:?}",
                        other as char
                    ));
                }
                None => return Err("unexpected EOF in array".into()),
            }
        }
    }
}
```

- [ ] **Step 6: Rewrite `parse_object` with `key_trivia` + sort**

```rust
impl<'a> LazyParser<'a> {
    fn parse_object(&mut self, depth: usize) -> std::result::Result<LazyVal<'a>, String> {
        let mut entries: Vec<LazyObjectEntry<'a>> = Vec::with_capacity(8);
        loop {
            self.core.consume_trivia()?;
            if self.core.pos >= self.core.bytes.len() {
                return Err("unexpected EOF in object".into());
            }
            if self.core.bytes[self.core.pos] == b'}' {
                self.core.pos += 1;
                if !self.core.pending_trivia.is_empty()
                    && let Some(last) = entries.last_mut()
                {
                    let pending = self.core.take_pending_trivia();
                    let mut combined: Vec<Trivia> = last.value.trivia.clone().into_vec();
                    combined.extend(pending);
                    last.value.trivia = combined.into_boxed_slice();
                }
                entries.sort_unstable_by(|a, b| a.key.as_ref().cmp(b.key.as_ref()));
                return Ok(LazyVal::Object(entries.into_boxed_slice()));
            }
            let key_trivia = self.core.take_pending_trivia();
            if self.core.bytes[self.core.pos] != b'"' {
                return Err("expected string key".into());
            }
            let key_raw = self.parse_string_slice()?;
            let key: Cow<'a, str> = if key_raw.as_bytes().contains(&b'\\') {
                Cow::Owned(decode_escaped(key_raw).ok_or_else(|| "bad escape in key".to_string())?)
            } else {
                Cow::Borrowed(key_raw)
            };
            self.core.consume_trivia()?;
            if self.core.bytes.get(self.core.pos) != Some(&b':') {
                return Err("expected ':' after key".into());
            }
            self.core.pos += 1;
            // pending_trivia between ':' and value becomes the value's leading trivia
            // (already picked up by parse_value's take_pending_trivia call).
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
                    return Err(format!(
                        "expected ',' or '}}' in object, got {:?}",
                        other as char
                    ));
                }
                None => return Err("unexpected EOF in object".into()),
            }
        }
    }
}
```

- [ ] **Step 7: Update `from_str_lazy`**

```rust
pub fn from_str_lazy(src: &str) -> Result<LazyNode<'_>> {
    let mut p = LazyParser::new(src);
    p.parse_root()
}
```

- [ ] **Step 8: Run lazy-comments test**

```bash
cargo test --test lazy_comments_test
```
Expected: all three tests PASS.

- [ ] **Step 9: Run full workspace tests**

```bash
cargo test --workspace
```
Expected: all PASS except the pre-existing `reject_unescaped_control_character` failure. (Side note: `scan_string_skip_escapes` does now surface an error for unterminated strings in lazy, but still does not reject raw `\n` — that's a separate fix.)

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "LazyParser shares ParserCore and collects JSONC trivia

Private parser in lazy_val.rs now wraps ParserCore. Whitespace,
comments, number/string scanning, keyword match, hex escapes,
position math and depth cap all come from the shared core.
Trivia attaches to LazyNode/LazyObjectEntry with the same
single-trivia placement as the owned parser."
```

---

## Task 6: Update call sites + docs

Propagate `LazyVal` → `LazyNode` signature changes to benches/tests; refresh docs for the new shape.

**Files:**
- Modify: `benches/common/mod.rs`
- Modify: `tests/stress_test.rs`, `tests/utf8_test.rs`, `tests/error_positions_test.rs`
- Modify: `CLAUDE.md`, `README.md`, `docs/parsing.md`, `docs/ast.md`

- [ ] **Step 1: Update `sum_numbers_lazy` in benches/common/mod.rs**

Replace the function with:
```rust
pub fn sum_numbers_lazy(n: &jwc::LazyNode<'_>) -> i64 {
    if let Some(x) = n.as_i64() {
        return x;
    }
    if let Some(arr) = n.as_array() {
        return arr.iter().map(sum_numbers_lazy).sum();
    }
    if let Some(obj) = n.as_object() {
        return obj.iter().map(|e| sum_numbers_lazy(&e.value)).sum();
    }
    0
}
```

- [ ] **Step 2: Fix remaining bench call sites**

Search for any other LazyVal usages in benches:
```bash
grep -n 'LazyVal\|\.as_array()\|\.as_object()' benches/
```
`access.rs` already chains `get(...).and_then(|v| v.as_i64())` — LazyNode passthrough keeps this compiling. No change needed there.

- [ ] **Step 3: Fix test call sites**

Review these files for `LazyVal`/array/object unpacking and update where needed:
- `tests/stress_test.rs`: `lazy_materialization_preserves_scalar_content` — `lv.get("items").unwrap()` returns `&LazyNode`, `.at(0)` still works via passthrough, `.get("name").and_then(|v| v.as_str())` still compiles. Should be unchanged.
- `tests/utf8_test.rs`: any `.as_array()` / `.as_object()` destructure that names an inner value type needs `entry.value` indirection. Grep first; patch minimally.
- `tests/error_positions_test.rs`: just asserts errors — should compile unchanged.

Command:
```bash
cargo test --workspace 2>&1 | head -80
```
Patch each compile error in-place using the passthrough accessors on `LazyNode`.

- [ ] **Step 4: Update docs**

`CLAUDE.md`:
- Header: `two parser architectures` → `two parsers: owned JSONC + lazy borrowed JSONC`.
- Delete the bullet about "Arena".
- Delete the whole `### Arena parser (...)` section.
- Update the entrypoint-matrix table: remove the `from_str_arena` row; update the `from_str_lazy` row to reflect JSONC support + `LazyNode` return type.
- Delete the `--features arena` test/bench/clippy lines from Common commands.
- Update the "Lazy parser" section: strict → JSONC, return type `LazyNode<'a>`.
- Delete references to `profile_parsers.rs --features "profiling arena"`.

`README.md`:
- Remove the `from_str_arena` table row + "Arena — `from_str_arena`" section.
- Remove the `arena` feature-flag row.
- Replace "rejected" with "preserved as trivia" in the lazy row.
- Remove the arena perf number.

`docs/parsing.md`, `docs/ast.md`:
- Remove arena mentions.
- Document `LazyNode`/`LazyObjectEntry` alongside `Node`/`ObjectEntry`.

- [ ] **Step 5: Verify everything**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --features lazy
cargo bench --bench parse_small --no-run
cargo bench --bench parse_nested --no-run
cargo bench --bench access --no-run
```
Expected: formatting clean, clippy clean, tests PASS (minus the pre-existing unescaped-control failure), all bench binaries build.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Wire bench/test call sites + docs to LazyNode

sum_numbers_lazy takes &LazyNode. CLAUDE/README/docs drop arena
references and document the new LazyNode+trivia shape."
```

---

## Self-Review Checklist

- [ ] Arena deletion covers: `src/arena.rs`, `tests/arena_test.rs`, `Cargo.toml` feature + dep, `src/lib.rs` re-exports, all 3 parse benches, `profile_parsers.rs`, `stress_test.rs`. Covered.
- [ ] Rename covers: module, type, `MAX_DEPTH` import path, CLAUDE.md path mention, all examples + tests referencing `Parser`. Covered.
- [ ] `ParserCore` exposes every method both parsers need (whitespace, trivia, number scan, string scan, hex escape, keyword match, position, lift_err). String/number/container *emission* stays per-parser to preserve monomorphism. Covered.
- [ ] `LazyNode` + `LazyObjectEntry` expose `trivia` / `key_trivia` publicly; passthrough accessors keep `.get(...).as_i64()` working. Covered.
- [ ] New trivia test exercises three trivia placements: leading on root, between key and value, leading on array element. Covered.
- [ ] `MAX_DEPTH` lives in one place (`parser_core`), re-exported via `parser` and crate root. No duplication. Covered.
- [ ] No placeholders. Every code step shows the code.
- [ ] Identifiers consistent: `LazyNode`, `LazyObjectEntry`, `LazyVal`, `LazyParser`, `ParserCore`, `Parser`, `MAX_DEPTH`.
