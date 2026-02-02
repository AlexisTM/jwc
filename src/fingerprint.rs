//! Content fingerprints used by layout-preserving output: a node or object
//! member that still hashes to what it did when parsed is copied from the
//! source verbatim.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::ast::{Node, ObjectEntry, Trivia, Value};

/// Fresh fingerprints for every node and member of a tree, by address.
#[derive(Default)]
pub struct Fresh {
    pub nodes: HashMap<*const Node, u64>,
    pub entries: HashMap<*const ObjectEntry, u64>,
}

impl Fresh {
    /// Recompute every node and member of the tree, bottom-up.
    pub fn of_tree(root: &Node) -> Self {
        let mut fresh = Self::default();
        all(root, &mut fresh);
        fresh
    }
}

/// Fingerprint of a parsed node, reusing the children's stored values.
pub fn stored_or_new_node(n: &Node) -> u64 {
    node(n, &mut None)
}

/// Fingerprint of a parsed member, reusing the value's stored fingerprint.
pub fn stored_or_new_entry(e: &ObjectEntry) -> u64 {
    entry(e, &mut None)
}

fn trivia(h: &mut DefaultHasher, list: &[Trivia]) {
    list.len().hash(h);
    for t in list {
        match t {
            Trivia::LineComment(c) => (0u8, c).hash(h),
            Trivia::BlockComment(c) => (1u8, c).hash(h),
        }
    }
}

/// Everything inside a node's own span: its value and dangling comments, and
/// for containers every child with the trivia and commas between them.
fn node(n: &Node, fresh: &mut Option<&mut Fresh>) -> u64 {
    let mut h = DefaultHasher::new();
    match &n.value {
        Value::Null => 0u8.hash(&mut h),
        Value::Bool(b) => (1u8, b).hash(&mut h),
        Value::Number(x) => (2u8, x.to_string()).hash(&mut h),
        Value::String(s) => (3u8, s).hash(&mut h),
        Value::Array(items) => {
            (4u8, items.len()).hash(&mut h);
            for item in items {
                trivia(&mut h, &item.trivia);
                child(item, fresh).hash(&mut h);
                item.comma.hash(&mut h);
                trivia(&mut h, &item.trailing);
            }
        }
        Value::Object(members) => {
            (5u8, members.len()).hash(&mut h);
            for member in members {
                trivia(&mut h, &member.key_trivia);
                member.key.hash(&mut h);
                trivia(&mut h, &member.value.trivia);
                child(&member.value, fresh).hash(&mut h);
                member.value.comma.hash(&mut h);
                trivia(&mut h, &member.value.trailing);
            }
        }
        #[cfg(feature = "lazy")]
        Value::Lazy(lazy) => (6u8, format!("{lazy:?}")).hash(&mut h),
    }
    trivia(&mut h, &n.dangling);
    let fp = h.finish();
    if let Some(fresh) = fresh {
        fresh.nodes.insert(std::ptr::from_ref(n), fp);
    }
    fp
}

fn child(n: &Node, fresh: &mut Option<&mut Fresh>) -> u64 {
    match (&*fresh, &n.source) {
        (None, Some(source)) => source.fingerprint,
        _ => node(n, fresh),
    }
}

/// Everything inside a member's span: key, comments before the value, value.
fn entry(e: &ObjectEntry, fresh: &mut Option<&mut Fresh>) -> u64 {
    let mut h = DefaultHasher::new();
    e.key.hash(&mut h);
    trivia(&mut h, &e.value.trivia);
    child(&e.value, fresh).hash(&mut h);
    let fp = h.finish();
    if let Some(fresh) = fresh {
        fresh.entries.insert(std::ptr::from_ref(e), fp);
    }
    fp
}

/// Fill `fresh` for a whole tree, members included.
pub fn all(root: &Node, fresh: &mut Fresh) {
    fn walk(n: &Node, fresh: &mut Fresh) {
        match &n.value {
            Value::Array(items) => items.iter().for_each(|i| walk(i, fresh)),
            Value::Object(members) => {
                for m in members {
                    walk(&m.value, fresh);
                    entry(m, &mut Some(fresh));
                }
            }
            _ => {}
        }
        node(n, &mut Some(fresh));
    }
    walk(root, fresh);
}
