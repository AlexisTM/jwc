# Understanding the JWC AST

JWC keeps comments by position, so a document serializes back with each comment
where it came from. Whitespace is not kept: the serializer lays the document out
again from the indentation you ask for.

## Core Types

### `Node`

```rust
pub struct Node {
    pub value: Value,
    pub trivia: Vec<Trivia>,    // before the value
    pub trailing: Vec<Trivia>,  // after the value and its comma, same line
    pub dangling: Vec<Trivia>,  // before this container's closing bracket
    pub comma: bool,            // was the value followed by a comma
}
```

- `trivia`: comments before the value. For an object member these are the
  comments between the `:` and the value; comments before the key are in
  `ObjectEntry::key_trivia`.
- `trailing`: comments on the same line after the value and its comma
  (`1, // one`). On the root node this slot holds every comment after the
  document, one per line.
- `dangling`: comments after the last element of an array or object, before the
  closing bracket. Always empty on scalars.
- `comma`: whether the node was followed by a comma in source.

### `Source`

Set by the parser on every node and object member:

```rust
pub struct Source {
    pub span: Range<usize>,      // where the value (or "key": value) sat in the text
    pub fingerprint: u64,        // what was there
    pub blank_line_before: bool, // an empty line separated it from the previous one
}
```

`to_string_preserving` copies the span verbatim while the fingerprint still
matches the node's content, so untouched parts keep their alignment. Pretty
output keeps the blank lines between members in every mode. `Source` is
ignored by `PartialEq`.

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
    pub key_trivia: Vec<Trivia>,  // before the key
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

## Where a comment lands

```jsonc
// header                      -> root.trivia
{
    // before key              -> entry.key_trivia
    "a": /* before value */ 1, // same line   -> value.trivia / value.trailing
    "b": [
        1,
        // before next element -> elements[1].trivia
        2,
        // before ]            -> array node .dangling
    ],
    // before }                -> object node .dangling
}
// after the document          -> root.trailing
```

## Adding Comments

```rust
use jwc::{Node, Value};

let mut node = Node::new(Value::from(42));
node.add_line_comment(" before the value");
node.add_block_comment(" inline before the value ");
node.add_trailing_comment(" after the value, same line");

let mut list = Node::new(Value::Array(vec![]));
list.add_dangling_comment(" nothing here yet");
```

For object keys:

```rust
use jwc::{Node, ObjectEntry, Value};

let mut entry = ObjectEntry::new("mode".to_string(), Node::new(Value::from("dev")));
entry.add_key_comment(" key comment");
```

## Serialization rules

- Leading line comments end their line and the next line is re-indented; leading
  block comments stay inline.
- Trailing comments follow the comma on the same line.
- Dangling comments sit on their own lines just before the closing bracket, even
  in an otherwise empty container.
- `CommentPolicy::Minify` rewrites line comments as block comments, so minified
  output needs no newlines. `CommentPolicy::Remove` drops every slot.
