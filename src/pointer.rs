//! JSON Pointer (RFC 6901) lookups, on nodes (comments included) and values.

use crate::ast::{Node, Value};

/// `~1` is `/`, `~0` is `~`.
#[must_use]
pub fn decode_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

/// The inverse of [`decode_token`], for building paths from keys.
#[must_use]
pub fn encode_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// The reference tokens of a path; `None` when it is not a JSON Pointer.
fn tokens(path: &str) -> Option<impl Iterator<Item = &str>> {
    let rest = if path.is_empty() {
        None
    } else {
        Some(path.strip_prefix('/')?)
    };
    Some(rest.into_iter().flat_map(|rest| rest.split('/')))
}

/// One step: a member by key, or an element by index.
fn child<'a>(value: &'a Value, token: &str) -> Option<&'a Node> {
    let key = decode_token(token);
    match value {
        Value::Object(members) => members.iter().find(|e| e.key == key).map(|e| &e.value),
        Value::Array(elements) => elements.get(key.parse::<usize>().ok()?),
        _ => None,
    }
}

fn child_mut<'a>(value: &'a mut Value, token: &str) -> Option<&'a mut Node> {
    let key = decode_token(token);
    match value {
        Value::Object(members) => members
            .iter_mut()
            .find(|e| e.key == key)
            .map(|e| &mut e.value),
        Value::Array(elements) => elements.get_mut(key.parse::<usize>().ok()?),
        _ => None,
    }
}

impl Node {
    /// The node at `path`, with its comments. `""` is the node itself.
    #[must_use]
    pub fn pointer(&self, path: &str) -> Option<&Self> {
        let mut node = self;
        for token in tokens(path)? {
            node = child(&node.value, token)?;
        }
        Some(node)
    }

    /// Mutable [`pointer`](Self::pointer).
    pub fn pointer_mut(&mut self, path: &str) -> Option<&mut Self> {
        let mut node = self;
        for token in tokens(path)? {
            node = child_mut(&mut node.value, token)?;
        }
        Some(node)
    }
}

impl Value {
    /// The value at `path`. `""` is the value itself.
    #[must_use]
    pub fn pointer(&self, path: &str) -> Option<&Self> {
        let mut tokens = tokens(path)?;
        let Some(first) = tokens.next() else {
            return Some(self);
        };
        let mut node = child(self, first)?;
        for token in tokens {
            node = child(&node.value, token)?;
        }
        Some(&node.value)
    }

    /// Mutable [`pointer`](Self::pointer).
    pub fn pointer_mut(&mut self, path: &str) -> Option<&mut Self> {
        let mut tokens = tokens(path)?;
        let Some(first) = tokens.next() else {
            return Some(self);
        };
        let mut node = child_mut(self, first)?;
        for token in tokens {
            node = child_mut(&mut node.value, token)?;
        }
        Some(&mut node.value)
    }
}
