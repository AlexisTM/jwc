#![forbid(unsafe_code)]

pub mod ast;
pub mod error;
mod fingerprint;
#[cfg(feature = "lazy")]
pub mod lazy;
mod number;
pub mod patch;
pub mod pointer;
pub mod serializer;
pub mod single_pass_parser;
pub mod traits;

// Re-exports
pub use ast::{Node, ObjectEntry, Source, Trivia, Value};
pub use error::{Error, ErrorKind, Result};
#[cfg(feature = "lazy")]
pub use lazy::LazyValue;
pub use number::Number;
pub use patch::PatchOperation;
pub use serializer::{CommentPolicy, FormatOptions, Indentation};
pub use single_pass_parser::{DuplicateKeys, ParseOptions, SinglePassParser};
pub use traits::{JwcDeserializable, JwcSerializable};

use std::io::{Read, Write};

/// Parse JSONC with the default options: 128 levels of nesting, duplicate
/// keys rejected.
///
/// # Errors
/// Syntax, number, depth and duplicate-key errors, with line and column.
pub fn from_str(s: &str) -> Result<Node> {
    SinglePassParser::new(s).parse()
}

/// Parse JSONC with explicit [`ParseOptions`].
///
/// # Errors
/// As [`from_str`], under the given limits.
pub fn from_str_with(s: &str, options: ParseOptions) -> Result<Node> {
    SinglePassParser::with_options(s, options).parse()
}

/// Parse a byte slice of JSONC into a Node.
///
/// # Errors
/// Invalid UTF-8 is a syntax error; otherwise as [`from_str`].
pub fn from_slice(v: &[u8]) -> Result<Node> {
    let s = std::str::from_utf8(v).map_err(|e| Error::syntax(e.to_string()))?;
    from_str(s)
}

/// Parse a reader of JSONC into a Node. The whole input is read first.
///
/// # Errors
/// A failed read is a syntax error; otherwise as [`from_str`].
pub fn from_reader<R: Read>(mut rdr: R) -> Result<Node> {
    let mut buffer = String::new();
    rdr.read_to_string(&mut buffer)
        .map_err(|e| Error::syntax(e.to_string()))?;
    from_str(&buffer)
}

/// Minified output, comments kept (so JSONC, not JSON, whenever there are any).
///
/// # Errors
/// None today; the `Result` keeps the signature stable.
pub fn to_string(node: &Node) -> Result<String> {
    Ok(node.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Keep,
    }))
}

/// Pretty-printed output. `indent` is `"\t"` or a run of spaces; `None` means
/// four spaces.
///
/// # Errors
/// `ErrorKind::Format` for any other `indent`.
pub fn to_string_pretty(node: &Node, indent: Option<&str>) -> Result<String> {
    Ok(node.to_formatted_string(FormatOptions {
        indentation: Indentation::parse(indent)?,
        comment_policy: CommentPolicy::Keep,
    }))
}

/// Pretty-printed output that keeps the original text of everything that did
/// not change since `source` was parsed: whitespace, alignment and blank lines
/// survive inside untouched nodes and members. `indent` as for
/// [`to_string_pretty`]; `None` here means "detect from `source`".
///
/// # Errors
/// `ErrorKind::Format` for an unsupported `indent`.
pub fn to_string_preserving(node: &Node, source: &str, indent: Option<&str>) -> Result<String> {
    let indentation = match indent {
        None => Indentation::detect(source),
        some => Indentation::parse(some)?,
    };
    Ok(node.to_preserved_string(
        source,
        FormatOptions {
            indentation,
            comment_policy: CommentPolicy::Keep,
        },
    ))
}

/// Serialize a Node into a byte vector.
///
/// # Errors
/// As [`to_string`].
pub fn to_vec(node: &Node) -> Result<Vec<u8>> {
    to_string(node).map(std::string::String::into_bytes)
}

/// Serialize a Node into a byte vector (pretty-printed).
///
/// # Errors
/// As [`to_string_pretty`].
pub fn to_vec_pretty(node: &Node, indent: Option<&str>) -> Result<Vec<u8>> {
    to_string_pretty(node, indent).map(std::string::String::into_bytes)
}

/// Serialize a Node into a writer.
///
/// # Errors
/// A failed write is `ErrorKind::Format`.
pub fn to_writer<W: Write>(mut writer: W, node: &Node) -> Result<()> {
    let s = to_string(node)?;
    writer
        .write_all(s.as_bytes())
        .map_err(|e| Error::format(e.to_string()))
}

/// Serialize a Node into a writer (pretty-printed).
///
/// # Errors
/// As [`to_string_pretty`], or `ErrorKind::Format` for a failed write.
pub fn to_writer_pretty<W: Write>(mut writer: W, node: &Node, indent: Option<&str>) -> Result<()> {
    let s = to_string_pretty(node, indent)?;
    writer
        .write_all(s.as_bytes())
        .map_err(|e| Error::format(e.to_string()))
}
