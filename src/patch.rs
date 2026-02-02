use crate::ast::{Node, ObjectEntry, Value};

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
    /// Note: This implementation currently doesn't support '-' index for appending to array
    /// fully robustly in all cases, but handles standard path traversal.
    pub fn apply_patch(&mut self, patch: Vec<PatchOperation>) -> Result<(), String> {
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
                        .ok_or_else(|| format!("Path not found: {from}"))?
                        .clone();
                    self.patch_add(&path, val)?;
                }
                PatchOperation::Test { path, value } => {
                    let val = self
                        .pointer(&path)
                        .ok_or_else(|| format!("Path not found: {path}"))?;
                    // Deep equality check needed.
                    // Since Value doesn't derive PartialEq yet (need to check), we might need to rely on string repr or impl it.
                    // Assuming Value has PartialEq derived if possible, or we implement simple check.
                    // If Value doesn't derive PartialEq, we can't easily compare.
                    // Let's assume we add PartialEq to Value.
                    if val != &value {
                        return Err(format!("Test failed at path {path}"));
                    }
                }
            }
        }
        Ok(())
    }

    fn patch_add(&mut self, path: &str, value: Self) -> Result<(), String> {
        if path.is_empty() {
            *self = value;
            return Ok(());
        }

        let (parent_path, key) = split_path(path)?;
        let parent = self
            .pointer_mut(&parent_path)
            .ok_or_else(|| format!("Parent path not found: {parent_path}"))?;

        match parent {
            Self::Object(members) => {
                // Check if key exists (replace) or add
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
                    let idx = key.parse::<usize>().map_err(|_| "Invalid array index")?;
                    if idx > elements.len() {
                        return Err("Array index out of bounds".to_string());
                    }
                    elements.insert(idx, Node::new(value));
                }
            }
            _ => return Err("Parent must be Object or Array".to_string()),
        }
        Ok(())
    }

    fn patch_remove(&mut self, path: &str) -> Result<Self, String> {
        let (parent_path, key) = split_path(path)?;
        let parent = self
            .pointer_mut(&parent_path)
            .ok_or_else(|| format!("Parent path not found: {parent_path}"))?;

        match parent {
            Self::Object(members) => {
                if let Some(pos) = members.iter().position(|e| e.key == key) {
                    let entry = members.remove(pos);
                    Ok(entry.value.value)
                } else {
                    Err(format!("Key not found: {key}"))
                }
            }
            Self::Array(elements) => {
                let idx = key.parse::<usize>().map_err(|_| "Invalid array index")?;
                if idx >= elements.len() {
                    return Err("Array index out of bounds".to_string());
                }
                let node = elements.remove(idx);
                Ok(node.value)
            }
            _ => Err("Parent must be Object or Array".to_string()),
        }
    }

    fn patch_replace(&mut self, path: &str, value: Self) -> Result<(), String> {
        let target = self
            .pointer_mut(path)
            .ok_or_else(|| format!("Path not found: {path}"))?;
        *target = value;
        Ok(())
    }
}

fn split_path(path: &str) -> Result<(String, String), String> {
    if let Some(idx) = path.rfind('/') {
        let _parent = if idx == 0 { "/" } else { &path[0..idx] }; // Handle root parent "/" or "/foo"
        let key = &path[idx + 1..];
        let key_decoded = key.replace("~1", "/").replace("~0", "~");
        // Root parent logic fix:
        // If path is "/foo", rfind is 0. parent should be "".
        // If path is "/foo/bar", rfind is 4. parent is "/foo".
        let parent_start = if idx == 0 { "" } else { &path[0..idx] };
        Ok((parent_start.to_string(), key_decoded))
    } else {
        Err("Invalid path".to_string())
    }
}
