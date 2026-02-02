pub mod ast;
#[cfg(feature = "lazy")]
pub mod lazy;
mod number;
pub mod patch;
pub mod pointer;
pub mod serializer;
pub mod single_pass_parser;
pub mod traits;

// Re-exports
pub use ast::{Node, ObjectEntry, Trivia, Value};
#[cfg(feature = "lazy")]
pub use lazy::LazyValue;
pub use number::Number;
pub use patch::PatchOperation;
pub use serializer::{CommentPolicy, FormatOptions, Indentation};
pub use single_pass_parser::SinglePassParser;
pub use traits::{JwcDeserializable, JwcSerializable};

use std::io::{Read, Write};

/// Parse a string of JSONC into a Node.
pub fn from_str(s: &str) -> Result<Node, String> {
    SinglePassParser::new(s).parse()
}

/// Parse a byte slice of JSONC into a Node.
pub fn from_slice(v: &[u8]) -> Result<Node, String> {
    let s = std::str::from_utf8(v).map_err(|e| e.to_string())?;
    from_str(s)
}

/// Parse a reader of JSONC into a Node.
///
/// This reads the entire input into memory before parsing.
pub fn from_reader<R: Read>(mut rdr: R) -> Result<Node, String> {
    let mut buffer = String::new();
    rdr.read_to_string(&mut buffer).map_err(|e| e.to_string())?;
    from_str(&buffer)
}

/// Serialize a Node into a minified JSON string.
pub fn to_string(node: &Node) -> Result<String, String> {
    Ok(node.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Keep,
    }))
}

/// Serialize a Node into a pretty-printed JSON string.
///
/// If `indent` is provided, it uses that string for indentation (e.g. "  " or "\t").
/// If `indent` is None, it defaults to 4 spaces.
/// Note: The current serializer implementation supports Spaces(u8) or Tabs.
/// Arbitrary string indentation is not fully supported by Indentation enum yet,
/// so we map "  " to Spaces(2), "\t" to Tabs, etc.
pub fn to_string_pretty(node: &Node, indent: Option<&str>) -> Result<String, String> {
    let indentation = match indent {
        Some("\t") => Indentation::Tabs,
        Some(s) if s.chars().all(|c| c == ' ') => Indentation::Spaces(s.len() as u8),
        None => Indentation::Spaces(4), // Default
        _ => Indentation::Spaces(2),    // Fallback? Or error? Let's use 2 spaces as fallback.
    };

    Ok(node.to_formatted_string(FormatOptions {
        indentation,
        comment_policy: CommentPolicy::Keep,
    }))
}

/// Serialize a Node into a byte vector.
pub fn to_vec(node: &Node) -> Result<Vec<u8>, String> {
    to_string(node).map(std::string::String::into_bytes)
}

/// Serialize a Node into a byte vector (pretty-printed).
pub fn to_vec_pretty(node: &Node, indent: Option<&str>) -> Result<Vec<u8>, String> {
    to_string_pretty(node, indent).map(std::string::String::into_bytes)
}

/// Serialize a Node into a writer.
pub fn to_writer<W: Write>(mut writer: W, node: &Node) -> Result<(), String> {
    let s = to_string(node)?;
    writer.write_all(s.as_bytes()).map_err(|e| e.to_string())
}

/// Serialize a Node into a writer (pretty-printed).
pub fn to_writer_pretty<W: Write>(
    mut writer: W,
    node: &Node,
    indent: Option<&str>,
) -> Result<(), String> {
    let s = to_string_pretty(node, indent)?;
    writer.write_all(s.as_bytes()).map_err(|e| e.to_string())
}
