# Understanding the JWC AST

JWC represents JSONC with a single trivia model. Every node stores one comment list (`trivia`).

## Owned tree types

### `Node`

```rust
pub struct Node {
    pub value: Value,
    pub trivia: Vec<Trivia>,
}
```

- `trivia`: comments attached to this node.

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

## Lazy borrowed tree types

Returned by `from_str_lazy`. Scalars stay as `&'a str` slices into the source; containers are eagerly built. Objects are sorted by key at parse time so `.get()` is O(log m).

### `LazyNode`

```rust
pub struct LazyNode<'a> {
    pub value: LazyVal<'a>,
    pub trivia: Box<[Trivia]>,
}
```

### `LazyVal`

The value part of a lazy node. Scalars borrow directly from the source string; arrays and objects are `Box<[...]>` slices.

### `LazyObjectEntry`

```rust
pub struct LazyObjectEntry<'a> {
    pub key: Cow<'a, str>,
    pub key_trivia: Box<[Trivia]>,
    pub value: LazyNode<'a>,
}
```

## Adding Comments (owned path)

Comment content is stored verbatim (no `//` or `/* */` markers). The serializer
adds the markers. Use `Trivia::line` / `Trivia::block` — or pass `&str` for a
line comment.

```rust
use jwc::{Node, Trivia, Value};

// mutator style
let mut node = Node::new(Value::from(42));
node.comment(Trivia::line(" important value"));
node.comment(Trivia::block(" check range "));

// builder style
let node = Node::new(Value::from(42))
    .with_comment(Trivia::line(" important value"))
    .with_comment(Trivia::block(" check range "));

// &str shorthand (implicit line comment)
let node = Node::new(Value::from(42)).with_comment(" note");
```

For object keys:

```rust
use jwc::{Node, ObjectEntry, Trivia, Value};

let entry = ObjectEntry::new("mode".to_string(), Node::new(Value::from("dev")))
    .with_key_comment(Trivia::line(" key comment"));
```

## Why This Matters

The single-trivia model keeps comment handling predictable: comments are stored in one place per node instead of split across leading/trailing channels. Both the owned and lazy parsers use this model.
