use crate::ast::{Node, ObjectEntry, Value};
use crate::{Error, Result};

#[derive(Debug, PartialEq)]
pub enum PatchOperation {
    Add { path: String, value: Value },
    Remove { path: String },
    Replace { path: String, value: Value },
    Move { from: String, path: String },
    Copy { from: String, path: String },
    Test { path: String, value: Value },
}

impl Value {
    /// Applies a list of JSON Patch operations (RFC 6902).
    pub fn apply_patch(&mut self, patch: Vec<PatchOperation>) -> Result<()> {
        for op in patch {
            match op {
                PatchOperation::Add { path, value } => self.patch_add(&path, value)?,
                PatchOperation::Remove { path } => {
                    self.patch_remove(&path)?;
                }
                PatchOperation::Replace { path, value } => self.patch_replace(&path, value)?,
                PatchOperation::Move { from, path } => {
                    let val = self.patch_remove(&from)?;
                    self.patch_add(&path, val)?;
                }
                PatchOperation::Copy { from, path } => {
                    let val = self
                        .pointer(&from)
                        .ok_or_else(|| Error::pointer(&from, "not found"))?
                        .clone();
                    self.patch_add(&path, val)?;
                }
                PatchOperation::Test { path, value } => {
                    let val = self
                        .pointer(&path)
                        .ok_or_else(|| Error::pointer(&path, "not found"))?;
                    if val != &value {
                        return Err(Error::patch(path, "test failed: value mismatch"));
                    }
                }
            }
        }
        Ok(())
    }

    fn patch_add(&mut self, path: &str, value: Self) -> Result<()> {
        if path.is_empty() {
            *self = value;
            return Ok(());
        }

        let (parent_path, key) = split_path(path)?;
        let parent = self
            .pointer_mut(&parent_path)
            .ok_or_else(|| Error::pointer(&parent_path, "parent not found"))?;

        match parent {
            Self::Object(members) => {
                if let Some(pos) = members.iter().position(|e| e.key == key) {
                    members[pos].value.value = value;
                } else {
                    members.push(ObjectEntry::new(key, Node::new(value)));
                }
            }
            Self::Array(elements) => {
                if key == "-" {
                    elements.push(Node::new(value));
                } else {
                    let idx = key
                        .parse::<usize>()
                        .map_err(|_| Error::patch(path, format!("invalid array index {key:?}")))?;
                    if idx > elements.len() {
                        return Err(Error::patch(path, "array index out of bounds"));
                    }
                    elements.insert(idx, Node::new(value));
                }
            }
            _ => return Err(Error::patch(path, "parent must be object or array")),
        }
        Ok(())
    }

    fn patch_remove(&mut self, path: &str) -> Result<Self> {
        let (parent_path, key) = split_path(path)?;
        let parent = self
            .pointer_mut(&parent_path)
            .ok_or_else(|| Error::pointer(&parent_path, "parent not found"))?;

        match parent {
            Self::Object(members) => {
                if let Some(pos) = members.iter().position(|e| e.key == key) {
                    let entry = members.remove(pos);
                    Ok(entry.value.value)
                } else {
                    Err(Error::patch(path, format!("key {key:?} not found")))
                }
            }
            Self::Array(elements) => {
                let idx = key
                    .parse::<usize>()
                    .map_err(|_| Error::patch(path, format!("invalid array index {key:?}")))?;
                if idx >= elements.len() {
                    return Err(Error::patch(path, "array index out of bounds"));
                }
                let node = elements.remove(idx);
                Ok(node.value)
            }
            _ => Err(Error::patch(path, "parent must be object or array")),
        }
    }

    fn patch_replace(&mut self, path: &str, value: Self) -> Result<()> {
        let target = self
            .pointer_mut(path)
            .ok_or_else(|| Error::pointer(path, "not found"))?;
        *target = value;
        Ok(())
    }
}

fn split_path(path: &str) -> Result<(String, String)> {
    if let Some(idx) = path.rfind('/') {
        let parent_start = if idx == 0 { "" } else { &path[0..idx] };
        let key = &path[idx + 1..];
        let key_decoded = key.replace("~1", "/").replace("~0", "~");
        Ok((parent_start.to_string(), key_decoded))
    } else {
        Err(Error::pointer(path, "path must start with '/'"))
    }
}
