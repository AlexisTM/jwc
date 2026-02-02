pub mod ast;
mod error;
#[cfg(feature = "lazy")]
pub mod lazy;
pub mod lazy_val;
mod macros;
mod number;
pub mod parser;
mod parser_core;
pub mod patch;
pub mod pointer;
pub mod serializer;
mod simd;
pub mod traits;

// Re-exports
pub use ast::{Node, ObjectEntry, Trivia, Value, ValueIndex};
pub use error::{Error, Result};
pub use lazy_val::{LazyNode, LazyObjectEntry, LazyVal, from_str_lazy};

/// Internal helper used by derive-macro-generated code. Not a stable API.
#[doc(hidden)]
pub fn _value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        #[cfg(feature = "lazy")]
        Value::Lazy(_) => "lazy",
    }
}
#[cfg(feature = "lazy")]
pub use lazy::LazyValue;
pub use number::Number;
pub use parser::{MAX_DEPTH, Parser};
pub use patch::PatchOperation;
pub use serializer::{CommentPolicy, FormatOptions, Indentation};
pub use traits::{JwcDeserializable, JwcSerializable};

use std::io::{Read, Write};

/// Parse a string of JSONC into a Node.
pub fn from_str(s: &str) -> Result<Node> {
    Parser::new(s).parse()
}

/// Parse a byte slice of JSONC into a Node.
pub fn from_slice(v: &[u8]) -> Result<Node> {
    let s = std::str::from_utf8(v)?;
    from_str(s)
}

/// Parse a reader of JSONC into a Node.
///
/// This reads the entire input into memory before parsing.
pub fn from_reader<R: Read>(mut rdr: R) -> Result<Node> {
    let mut buffer = String::new();
    rdr.read_to_string(&mut buffer)?;
    from_str(&buffer)
}

/// Serialize a Node into a minified JSON string.
pub fn to_string(node: &Node) -> Result<String> {
    Ok(node.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Keep,
    }))
}

/// Serialize a Node into a pretty-printed JSON string.
///
/// `indent` accepts any string:
/// - `None` → 4 spaces (default)
/// - `Some("")` → no indent (equivalent to `to_string`)
/// - `Some("\t")` → tabs (fast path)
/// - `Some("  ")` or `"    "` etc. → N-space indent (fast path)
/// - Anything else (mixed or custom, e.g. `"\t "` or `"--> "`) → used verbatim per depth level.
pub fn to_string_pretty(node: &Node, indent: Option<&str>) -> Result<String> {
    let indentation = match indent {
        None => Indentation::Spaces(4),
        Some("") => Indentation::None,
        Some("\t") => Indentation::Tabs,
        Some(s) if s.chars().all(|c| c == ' ') => Indentation::Spaces(s.len() as u8),
        Some(s) => Indentation::Custom(s.to_string()),
    };

    Ok(node.to_formatted_string(FormatOptions {
        indentation,
        comment_policy: CommentPolicy::Keep,
    }))
}

/// Serialize a Node into a byte vector.
pub fn to_vec(node: &Node) -> Result<Vec<u8>> {
    to_string(node).map(std::string::String::into_bytes)
}

/// Serialize a Node into a byte vector (pretty-printed).
pub fn to_vec_pretty(node: &Node, indent: Option<&str>) -> Result<Vec<u8>> {
    to_string_pretty(node, indent).map(std::string::String::into_bytes)
}

/// Serialize a Node into a writer.
pub fn to_writer<W: Write>(mut writer: W, node: &Node) -> Result<()> {
    let s = to_string(node)?;
    writer.write_all(s.as_bytes())?;
    Ok(())
}

/// Serialize a Node into a writer (pretty-printed).
pub fn to_writer_pretty<W: Write>(mut writer: W, node: &Node, indent: Option<&str>) -> Result<()> {
    let s = to_string_pretty(node, indent)?;
    writer.write_all(s.as_bytes())?;
    Ok(())
}
