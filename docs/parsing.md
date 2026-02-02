# Parsing and Serialization

This guide covers how to parse JSONC into JWC's AST and serialize it back.

## Quick Start

```rust
fn main() -> Result<(), String> {
    let input = r#"
    {
      // service port
      "port": 8080,
      "enabled": true,
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

## Parsing APIs

- `jwc::from_str(&str) -> Result<Node, String>`
- `jwc::from_slice(&[u8]) -> Result<Node, String>`
- `jwc::from_reader(Read) -> Result<Node, String>`

Use the highest-level API that matches your input source.

```rust
use std::fs::File;

fn load_from_file(path: &str) -> Result<jwc::Node, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    jwc::from_reader(file)
}
```

## Serialization APIs

- `jwc::to_string(&Node)` for compact output.
- `jwc::to_string_pretty(&Node, Option<&str>)` for pretty output.
- `jwc::to_vec`, `jwc::to_vec_pretty` for bytes.
- `jwc::to_writer`, `jwc::to_writer_pretty` for streaming output.

```rust
fn write_pretty(node: &jwc::Node) -> Result<Vec<u8>, String> {
    jwc::to_vec_pretty(node, Some("    "))
}
```

## Indentation Behavior

`to_string_pretty` currently accepts:

- `Some("\t")` for tabs.
- `Some("   ")` (spaces only) for N-space indentation.
- `None` for the default (4 spaces).

If you pass a non-tab, non-space pattern, the function falls back internally.

## Errors

Parsing and serialization return `Result<_, String>`. Treat errors as user input failures.

```rust
match jwc::from_str("{ \"x\": ") {
    Ok(_) => {}
    Err(err) => eprintln!("Invalid JSONC: {err}"),
}
```

## Feature Notes

- `lazy` (enabled by default): enables `jwc::LazyValue` support.
- `arbitrary_precision`: changes number handling behavior toward arbitrary precision mode.
- `profiling`: enables hotpath instrumentation attributes.
