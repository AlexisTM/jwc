# JWC: JSON With Comments

[![Crates.io](https://img.shields.io/crates/v/jwc.svg)](https://crates.io/crates/jwc)
[![Documentation](https://docs.rs/jwc/badge.svg)](https://docs.rs/jwc)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`jwc` parses and serializes JSONC (JSON with comments) while keeping useful source details such as comments, node structure, and trailing comma state.

## What You Get

- Parse JSONC into a rich AST (`Node`, `Value`, `Trivia`).
- Preserve comments where they were: before a key or value, after a value on the
  same line, before a closing bracket, after the document.
- Write back with the original layout: `to_string_preserving` copies every
  untouched node verbatim (alignment, blank lines) and only re-lays out what you
  edited.
- Strict where it matters: JSON number grammar, exact 64-bit integers with the
  source lexeme kept, duplicate keys rejected by default, nesting capped at 128
  (both configurable through `ParseOptions`), no `unsafe`.
- Errors carry a kind and a line:column (`jwc::Error`).
- Query and update with JSON Pointer (RFC 6901).
- Apply JSON Patch operations (RFC 6902).
- Optional lazy values for deferred parsing.

## Quick Start

### 1. Add dependency

```toml
[dependencies]
jwc = "0.1.0"
```

### 2. Parse, modify, and serialize

```rust
fn main() -> jwc::Result<()> {
    let input = r#"
    {
      // Server config
      "port": 8080,
      "active": true
    }
    "#;

    let mut node = jwc::from_str(input)?;

    if let Some(port) = node.value.pointer_mut("/port") {
        *port = 9090.into();
    }

    // Same text as `input` except the port: comments, alignment and blank
    // lines of everything else are copied from the source.
    let out = jwc::to_string_preserving(&node, input, None)?;
    println!("{out}");

    // Or lay the whole document out again.
    println!("{}", jwc::to_string_pretty(&node, Some("  "))?);
    Ok(())
}
```

### 3. Parse options

```rust
let options = jwc::ParseOptions {
    max_depth: 32,
    duplicate_keys: jwc::DuplicateKeys::Allow, // default: Reject
};
let node = jwc::from_str_with(input, options)?;
```

## Feature Flags

`jwc` supports optional Cargo features:

| Feature | Default | Description |
| :-- | :-- | :-- |
| `lazy` | Yes | Enables `LazyValue` support (`jwc::LazyValue`). |
| `profiling` | No | Enables hotpath profiling integration. |

### Enable features

```toml
[dependencies]
jwc = { version = "0.1.0", features = ["profiling"] }
```

### Disable default features

```toml
[dependencies]
jwc = { version = "0.1.0", default-features = false }
```

## Guides

- Parsing and serialization: [`docs/parsing.md`](docs/parsing.md)
- AST model and trivia: [`docs/ast.md`](docs/ast.md)
- Pointer and patch operations: [`docs/manipulation.md`](docs/manipulation.md)
- Python frontend (`jwc_py`): [`docs/python.md`](docs/python.md)

## License

MIT. See `LICENSE`.
