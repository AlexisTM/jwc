# JWC: JSON With Comments

[![Crates.io](https://img.shields.io/crates/v/jwc.svg)](https://crates.io/crates/jwc)
[![Documentation](https://docs.rs/jwc/badge.svg)](https://docs.rs/jwc)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`jwc` parses and serializes JSONC (JSON with comments) while keeping useful source details such as comments, node structure, and trailing comma state.

## What You Get

- Parse JSONC into a rich AST (`Node`, `Value`, `Trivia`).
- Preserve and re-emit comments.
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
fn main() -> Result<(), String> {
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

    let out = jwc::to_string_pretty(&node, Some("  "))?;
    println!("{out}");
    Ok(())
}
```

## Feature Flags

`jwc` supports optional Cargo features:

| Feature | Default | Description |
| :-- | :-- | :-- |
| `lazy` | Yes | Enables `LazyValue` support (`jwc::LazyValue`). |
| `profiling` | No | Enables hotpath profiling integration. |
| `arbitrary_precision` | No | Enables arbitrary precision number mode. |

### Enable features

```toml
[dependencies]
jwc = { version = "0.1.0", features = ["profiling", "arbitrary_precision"] }
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
