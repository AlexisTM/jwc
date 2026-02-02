# jwc — JSON With Comments

[![Crates.io](https://img.shields.io/crates/v/jwc.svg)](https://crates.io/crates/jwc)
[![Documentation](https://docs.rs/jwc/badge.svg)](https://docs.rs/jwc)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Parse, edit, and serialize JSONC (JSON with `//` and `/* */` comments) in
Rust. Comments survive the round-trip. Two APIs pick the right trade-off
for you: an owned tree for editing, a borrowed tree for fast reads.

```rust
let src = r#"
{
    // runtime config
    "port": 8080,
    "tls":  true,
}
"#;

let node = jwc::from_str(src)?;
assert_eq!(node.value["port"].as_i64(), Some(8080));
```

## Install

```bash
cargo add jwc
```

Rust edition 2024. No runtime dependencies beyond `std` (SIMD and derive
are opt-in features).

## Why JSONC?

JSON is strict by design — no comments, no trailing commas — which makes
it a poor fit for config files humans edit. JSONC is the pragmatic
superset that most editors and tools (`tsconfig.json`, VSCode settings,
…) already accept. `jwc` treats comments as first-class citizens: they
round-trip on the owned path **and** on the lazy path, so you can keep
the original author's intent in place while your program reads or
modifies the values.

## Pick your API

- **Editing, writing, round-trip?** → `jwc::from_str`. Returns a
  mutable `Node` tree with comments attached. Write it back with
  `jwc::to_string_pretty` and nothing is lost.
- **Reading a few fields fast?** → `jwc::from_str_lazy`. Returns a
  `LazyNode<'a>` borrowing into your source buffer. Scalars stay as
  slices, objects are pre-sorted, `.get()` is an O(log m) binary
  search. Comments are still preserved on every node.

| function | returns | good at | costs |
|---|---|---|---|
| `jwc::from_str(s)` | `Node` | mutation, pretty printing, JSONC round-trip | full owned allocation |
| `jwc::from_str_lazy(s)` | `LazyNode<'_>` | hot-path reads, one or many field lookups | borrows source; decoding deferred |

### Editing — `from_str`

```rust
let input = r#"
{
    // server config
    "port": 8080,
    "enabled": true,
}
"#;

let mut node = jwc::from_str(input)?;

// Read / write by key.
assert_eq!(node.value["port"].as_i64(), Some(8080));
node.value["port"] = 9090.into();

// Pretty-print back with comments preserved.
println!("{}", jwc::to_string_pretty(&node, Some("  "))?);
```

### Reading — `from_str_lazy`

```rust
let node = jwc::from_str_lazy(r#"{"port": 8080, "name": "svc"}"#)?;

// Object keys are sorted at parse time; .get() is a binary search.
assert_eq!(node.get("port").and_then(|v| v.as_i64()), Some(8080));
assert_eq!(node.get("name").and_then(|v| v.as_str()).as_deref(), Some("svc"));
```

Numbers and strings are kept as `&str` slices into your source — decoding
happens only when you call `.as_i64()`, `.as_f64()`, `.as_str()`. Arrays
and objects are built eagerly so navigation is cheap. Each `LazyNode`
carries its attached comments in `.trivia`.

Need a fully owned copy? One call:

```rust
let owned: jwc::Value = node.to_value();
```

## Structured errors

`jwc` returns rich errors you can match on, not opaque strings:

```rust
match jwc::from_str("\n\n\"unterminated") {
    Err(jwc::Error::Parse { line, col, msg }) => {
        eprintln!("parse error at {line}:{col}: {msg}")
    }
    _ => {}
}
```

Variants: `Parse { line, col, msg }`, `Type { expected, got, path }`,
`MissingField { name, path }`, `Pointer { path, reason }`,
`Patch { path, reason }`, `Custom(String)`. All implement `Display` and
`std::error::Error`.

## Derive your own types

`JwcSerializable` / `JwcDeserializable` let you parse straight into your
own structs, no `serde` required:

```rust
use jwc::{JwcDeserializable, JwcSerializable};
use jwcc_derive::{JwcDeserializable, JwcSerializable};

#[derive(JwcSerializable, JwcDeserializable, Default, Debug, PartialEq)]
struct Config {
    #[jwc(rename = "log-level")]
    log_level: String,
    #[jwc(default)]
    retries: u32,
    #[jwc(skip)]
    cached: Vec<u8>,
}

let c = Config::from_jwc(jwc::from_str(r#"{"log-level":"debug"}"#)?.value)?;
assert_eq!(c.log_level, "debug");
assert_eq!(c.retries, 0); // filled by `#[jwc(default)]`
```

Supported field attributes: `rename = "..."`, `default`, `skip`,
`skip_serializing`, `skip_deserializing`.

> The proc-macro crate is published as `jwc_derive` but imported as
> **`jwcc_derive`** (double `c`) — it's renamed in `Cargo.toml` to avoid
> a collision with the main crate.

## JSON Pointer & Patch

RFC 6901 pointers and RFC 6902 patches out of the box:

```rust
use jwc::{jwc, PatchOperation, Value};

let mut v = jwc!({ "a": [10, 20, 30] });
assert_eq!(v.pointer("/a/1"), Some(&Value::from(20)));

v.apply_patch(vec![
    PatchOperation::Add { path: "/a/-".into(), value: Value::from(40) },
    PatchOperation::Replace { path: "/a/0".into(), value: Value::from(99) },
])?;
```

## Literal construction with `jwc!`

Build `Value` trees inline with a `serde_json::json!`-style macro:

```rust
use jwc::jwc;

let v = jwc!({
    "port": 8080,
    "tags": ["a", "b"],
    "nested": { "enabled": true, "ratio": 0.5 },
    "maybe": null,
});
```

Scalars, arrays, objects, trailing commas, dynamic expressions, and
arbitrary nesting all work.

## Performance

Release build, Criterion. Nested fixture = 256 small objects in an
outer array. `access_repeated_lookups` parses once then reads 20 keys.

| parser | parse_small (~350 B) | parse_nested (256 obj) | access_repeated_lookups |
|---|---|---|---|
| `jwc::from_str_lazy` | 1.6 µs | 11.7 µs | **6.0 µs** |
| `jwc::from_str` | 1.8 µs | 42 µs | 16.3 µs |
| `sonic-rs` | 0.8 µs | 9.1 µs | 10.8 µs |
| `simd-json` | 1.3 µs | 34 µs | — |
| `serde_json` | 1.8 µs | 41 µs | 17.1 µs |

The lazy path beats `sonic-rs` on repeated-lookup workloads because its
objects are sorted at parse time. `sonic-rs` still wins raw parse on
tiny inputs thanks to its `pclmulqdq` stage. Your workload may differ —
run the benches yourself:

```bash
cargo bench --bench parse_small
cargo bench --bench parse_large
cargo bench --bench parse_nested
cargo bench --bench access
```

## Feature flags

| feature | default | effect |
|---|---|---|
| `lazy` | on | opt-in per-subtree `LazyValue` (wrap raw source, `thaw()` later) — orthogonal to `from_str_lazy` |
| `simd` | on | x86_64 SSE4.2 / AVX2 scanners for whitespace, strings, newlines; scalar fallback otherwise |
| `arbitrary_precision` | off | preserves original number lexemes for lossless round-trip |
| `profiling` | off | enables `hotpath::measure` instrumentation for profile builds |

## Workspace

- `.` — the core `jwc` library
- `jwc_derive/` — proc-macro crate (imported as `jwcc_derive`)
- `jwc_py/` — Python bindings via PyO3 (see `jwc_py/README.md`)

## Further reading

- [Parsing & serialization](docs/parsing.md)
- [AST & trivia](docs/ast.md)
- [Pointer & patch](docs/manipulation.md)
- [Python frontend](docs/python.md)

## License

MIT. See [`LICENSE`](LICENSE).
