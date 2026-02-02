# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`jwc` is a Rust library that parses and serializes JSONC (JSON with comments). The crate ships two parsers sharing one tokenizer core (`ParserCore`), AST / error types, and SIMD substrate.

- **Owned** (`from_str`): builds a `Node` tree, preserves comments, supports mutation + pretty printing. Round-trip path.
- **Lazy borrowed** (`from_str_lazy`): builds a `LazyNode { value: LazyVal, trivia: Box<[Trivia]> }` tree. Scalars stay as source slices (decoded on demand); arrays/objects are eagerly built AND objects are sorted by key at parse time so `.get()` is O(log m). Comments are preserved as trivia on every node, matching the owned parser.

## Workspace layout

Cargo workspace with three members:

- `.` — the core `jwc` library (`src/`).
- `jwc_derive/` — proc-macro crate. **Imported under the rename `jwcc_derive`** (double `c`) because `Cargo.toml` does `jwcc_derive = { package = "jwc_derive", path = "jwc_derive" }`. Always `use jwcc_derive::...`, not `jwc_derive::...`.
- `jwc_py/` — PyO3 binding producing a `cdylib` also named `jwc`. Distributed via `maturin`.

## Common commands

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Benches are split — select the fixture you care about.
cargo bench --bench parse_small
cargo bench --bench parse_large
cargo bench --bench parse_nested
cargo bench --bench access

# Profiling build (hotpath::measure attributes activate).
cargo run --release --features profiling --example profile_parsers
```

## Architecture

### Entrypoint matrix

| entrypoint | returns | JSONC? | allocator | shape |
|---|---|---|---|---|
| `from_str(&str)` | `Node` | yes | std | full owned Vec-of-Node tree with `trivia` |
| `from_str_lazy(&str)` | `LazyNode<'_>` | yes | std | objects auto-sorted, scalars lazy, trivia boxed |

**`from_str_borrowed`, `from_str_strict`, `IndexedObject` no longer exist.** An earlier refactor added then removed them when profile data showed the complexity wasn't earning perf — don't resurrect without data.

### Owned parser (`src/parser.rs`)

Byte-walk parser, SIMD jump-helpers for whitespace/strings/newlines, parser-internal `Result<_, String>` lifted to `Error::Parse { line, col, msg }` at the `pub fn parse()` boundary via `lift_err`. If you add new parser errors, keep the trailing ` at {line}:{col}` suffix so `lift_err` can recover positions.

### Lazy parser (`src/lazy_val.rs`)

Recursive-descent parse that eagerly builds `Box<[LazyNode]>` for arrays and `Box<[LazyObjectEntry]>` for objects. Objects are sorted by key immediately after collection so `LazyVal::get(&str)` is a pure binary search. Scalars stay as `&'a str` slices into the source. Decoding happens lazily in `as_i64 / as_f64 / as_str`. `to_value()` walks the tree and materializes into an owned `Value`.

Comments are collected as `Trivia` on each `LazyNode` / `LazyObjectEntry`, matching the owned parser's single-trivia placement rules. The parser wraps a `ParserCore<'a>` so whitespace, comment, string, and number scanning come from the same code path the owned parser uses — the grammar walk stays monomorphic for inlining, but token-level logic is shared.

### SIMD helpers (`src/simd.rs`)

Four byte-scanning primitives dispatched AVX2 → SSE4.2 → scalar via cached function-pointer slots (`AtomicUsize`):

- `skip_ws` — first non-whitespace byte
- `find_string_end` — first `"` or `\`
- `find_newline` — first `\n`
- `find_structural` — first of `{ } [ ] , : " /`

Public wrappers short-circuit to scalar when the remaining slice is < 16 bytes. Callers additionally branch on a single-byte check *before* dispatching (e.g. parsers' `skip_ws` inline-returns if the first byte isn't whitespace) — this was the largest single win on dense JSON.

The `simd` feature gates all x86_64 intrinsics. On other architectures or with `simd` off, everything routes to scalar.

### AST (`src/ast.rs`)

- `Node { value: Value, trivia: Vec<Trivia> }` — owned tree node. No `comma` field (removed intentionally; serializer emits standard commas from position).
- `Value::{Null, Bool, Number, String, Array(Vec<Node>), Object(Vec<ObjectEntry>), Lazy?}` — the `Lazy` variant is feature-gated (`lazy`) and is the opt-in per-subtree `LazyValue` — NOT the same as `LazyVal` / `LazyNode` from `lazy_val.rs`.
- `ObjectEntry { key: String, key_trivia: Vec<Trivia>, value: Node }`.
- Single-trivia model — comments stored in one `Vec<Trivia>` per node, not split into leading/trailing channels. Preserve this invariant.

Accessors on `Value`: `is_null/bool/number/string/array/object`, `as_bool/str/i64/u64/f64/number/array/object`, `get<I: ValueIndex>(&i)` (`I = &str | String | usize`), `pointer / pointer_mut`, `apply_patch`, `push`, `insert`. `Index<I>` for reads (`&NULL` on miss, never panics), `IndexMut<&str>` auto-promotes `Null` → empty `Object` and inserts.

### Trivia API

Comment content is stored verbatim. Serializer adds markers:

```rust
Trivia::line("text")   // LineComment, renders as `//text\n`
Trivia::block("text")  // BlockComment, renders as `/*text*/`
node.comment(Trivia::line("…"));          // mutator
Node::new(v).with_comment("…")            // builder; &str implies line
entry.key_comment(Trivia::line("…"));
```

Do **not** restore prefix-stripping behavior. The old `add_line_comment` silently stripped `//` if present — ambiguous and removed.

### Numbers (`src/number.rs`)

Two compile-time implementations selected by `#[path = ...]` in `number.rs`:

- `number_fast.rs` — default. `Number` is `enum Int(i64) | Float(f64)`; preserves integer precision end-to-end (including `i64::MAX`). Parsers try a pure-integer fast path before falling back to `str::parse::<f64>()`.
- `number_arbitrary.rs` — enabled via `arbitrary_precision`. Stores the original lexeme for lossless round-trip.

Only `pub use imp::Number;` is re-exported. When touching numeric behavior, update both files.

### Errors (`src/error.rs`)

```rust
pub enum Error {
    Parse { line, col, msg },
    Type { expected, got, path },
    MissingField { name, path },
    Pointer { path, reason },
    Patch { path, reason },
    Custom(String),
}
```

`Display + std::error::Error`; `pub type Result<T> = std::result::Result<T, Error>` at the crate root.

### Macros

- `jwc!(...)` in `src/macros.rs` — literal construction, mirrors `serde_json::json!`. Covers scalars, `[...]`, `{...}`, trailing commas, dynamic expressions, nesting.

### Serde-free derive (`src/traits.rs`, `jwc_derive/src/lib.rs`)

`JwcSerializable` / `JwcDeserializable` traits. No `serde` dependency in `src/` (serde_json etc. are dev-deps, benches only). Derive supports:

- `#[jwc(rename = "other")]`
- `#[jwc(default)]`
- `#[jwc(skip)]`
- `#[jwc(skip_serializing)]`, `#[jwc(skip_deserializing)]`

Derive code calls `jwc::_value_kind` (hidden helper at crate root) for `Error::Type` messages.

### Per-subtree `LazyValue` (`src/lazy.rs`, feature `lazy`)

Orthogonal to `from_str_lazy`. This is an **opt-in wrapper** a caller can place around a raw JSON source string; `thaw()` / `parse_as::<T>()` parse on demand. Not the same thing as `LazyVal` — different types, different use cases.

## Project conventions

- **Profiling instrumentation**: hot functions carry `#[cfg_attr(feature = "profiling", hotpath::measure)]`. Mirror this on new hot-path functions. `examples/profile_parsers.rs` is the canonical way to generate timing reports (`cargo run --release --features profiling --example profile_parsers`). Use profile data to guide optimization — every guessed win in the git history was wrong per measurement.
- **Error type is `Error`**: public APIs return `jwc::Result<_>`. Add a variant before leaning on `Error::Custom` for recurring error shapes.
- **No `serde` in the core crate**: the many JSON dev-deps (`serde_json`, `simd-json`, `sonic-rs`, `json-rust`, `tinyjson`) exist only for the benches.
- **Edition 2024** across all workspace members. Inner `unsafe {}` blocks inside `unsafe fn` are required by `unsafe_op_in_unsafe_fn`; clippy's `unused_unsafe` is permitted in `src/simd.rs`.
- **No comment prefix-stripping**: `Trivia::line("//x")` serializes as `////x`. We don't guess what the caller meant.
- **Display contracts**: `{}` is compact (minified, preserves comments); `{:#}` is pretty with 4-space indent.

## Benches

Each benchmark is a separate target so you can run one without the others:

- `parse_small` — ~350 B mixed-type fixture.
- `parse_large` — ~1.5 KB long-string fixture.
- `parse_nested` — 256 small objects in an outer array (node-dense).
- `access` — three groups inside:
  - `access_one_field` — parse + read one deep field.
  - `access_full_walk` — parse + sum every number.
  - `access_repeated_lookups` — parse + 20 lookups on a 100-key object (regime where `jwc-lazy` auto-sort wins hardest).

Shared fixtures + helpers in `benches/common/mod.rs` (included via `#[path = "common/mod.rs"] mod common;` from each bench file).

Typical numbers (release, x86_64 AVX2):

- `parse_small/jwc-lazy`: ~1.6 µs; `sonic-rs`: ~0.8 µs.
- `parse_nested/jwc-lazy`: ~11.7 µs; `sonic-rs`: ~9.1 µs.
- `access_repeated_lookups/jwc-lazy-indexed`: **~6.0 µs** (beats `sonic-rs` at ~10.8 µs).

Remaining parse gap to sonic-rs is architectural (their stage-1 bitmap with `pclmulqdq` + lazy-slice values). Don't chase it locally; past attempts to shave it with pre-sizing / trimming node fields regressed.
