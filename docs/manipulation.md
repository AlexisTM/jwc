# Querying and Patching

This guide covers JSON Pointer queries and JSON Patch updates.

## JSON Pointer (`pointer`, `pointer_mut`)

Use JSON Pointer paths to navigate nested values.

```rust
let node = jwc::from_str(r#"{ "app": { "ports": [8080, 8081] } }"#)?;

let second = node
    .value
    .pointer("/app/ports/1")
    .ok_or_else(|| "missing value".to_string())?;

assert_eq!(second, &jwc::Value::from(8081));
```

Mutable access:

```rust
let mut node = jwc::from_str(r#"{ "enabled": false }"#)?;

if let Some(v) = node.value.pointer_mut("/enabled") {
    *v = true.into();
}
```

## Pointer Escaping

Use RFC 6901 escaping in path tokens:

- `~1` means `/`
- `~0` means `~`

Example: key `a/b` is addressed as `/a~1b`.

## JSON Patch (`apply_patch`)

JWC supports these operations through `PatchOperation`:

- `Add`
- `Remove`
- `Replace`
- `Move`
- `Copy`
- `Test`

```rust
use jwc::{PatchOperation, Value};

let mut doc = jwc::from_str(r#"{"settings": {"theme": "light"}}"#)?.value;

let patch = vec![
    PatchOperation::Replace {
        path: "/settings/theme".to_string(),
        value: Value::from("dark"),
    },
    PatchOperation::Add {
        path: "/settings/notifications".to_string(),
        value: Value::from(true),
    },
];

doc.apply_patch(patch)?;
```

## Common Failure Cases

- Pointer path does not exist.
- Parent path for `Add`/`Remove` is invalid.
- Array index is out of bounds.
- `Test` value does not match.

Handle these as regular `Result` errors and return useful messages to callers.

## Practical Pattern

1. Parse into `Node`.
2. Apply targeted pointer/patch changes to `node.value`.
3. Serialize with `to_string_pretty`.

This keeps edits focused and avoids manual tree traversal code.
