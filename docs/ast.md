# Understanding the JWC AST

JWC represents JSONC with a single trivia model. Every node stores one comment list (`trivia`) plus comma state (`comma`).

## Core Types

### `Node`

```rust
pub struct Node {
    pub value: Value,
    pub trivia: Vec<Trivia>,
    pub comma: bool,
}
```

- `trivia`: comments attached to this node.
- `comma`: whether the node was followed by a comma in source.

### `Value`

```rust
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Node>),
    Object(Vec<ObjectEntry>),
    #[cfg(feature = "lazy")]
    Lazy(Box<LazyValue>),
}
```

### `ObjectEntry`

```rust
pub struct ObjectEntry {
    pub key: String,
    pub key_trivia: Vec<Trivia>,
    pub value: Node,
}
```

### `Trivia`

```rust
pub enum Trivia {
    LineComment(String),
    BlockComment(String),
}
```

## Adding Comments

```rust
use jwc::{Node, Value};

let mut node = Node::new(Value::from(42));
node.add_line_comment(" important value");
node.add_block_comment(" check range ");
```

For object keys:

```rust
use jwc::{Node, ObjectEntry, Value};

let mut entry = ObjectEntry::new("mode".to_string(), Node::new(Value::from("dev")));
entry.add_key_comment(" key comment");
```

## Why This Matters

The single-trivia model keeps comment handling predictable: comments are stored in one place per node instead of split across leading/trailing channels.
