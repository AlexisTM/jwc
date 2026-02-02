use crate::ast::Value;

impl Value {
    /// Retrieves a reference to a value at the given JSON Pointer path.
    #[must_use]
    pub fn pointer(&self, path: &str) -> Option<&Self> {
        if path.is_empty() {
            return Some(self);
        }
        if !path.starts_with('/') {
            return None;
        }

        let mut current = self;
        for token in path.split('/').skip(1) {
            let key = decode_token(token);
            match current {
                Self::Object(members) => {
                    if let Some(entry) = members.iter().find(|e| e.key == key) {
                        current = &entry.value.value;
                    } else {
                        return None;
                    }
                }
                Self::Array(elements) => {
                    if let Ok(idx) = key.parse::<usize>() {
                        if let Some(node) = elements.get(idx) {
                            current = &node.value;
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Retrieves a mutable reference to a value at the given JSON Pointer path.
    pub fn pointer_mut(&mut self, path: &str) -> Option<&mut Self> {
        if path.is_empty() {
            return Some(self);
        }
        if !path.starts_with('/') {
            return None;
        }

        let mut current = self;
        for token in path.split('/').skip(1) {
            let key = decode_token(token);
            match current {
                Self::Object(members) => {
                    if let Some(entry) = members.iter_mut().find(|e| e.key == key) {
                        current = &mut entry.value.value;
                    } else {
                        return None;
                    }
                }
                Self::Array(elements) => {
                    if let Ok(idx) = key.parse::<usize>() {
                        if let Some(node) = elements.get_mut(idx) {
                            current = &mut node.value;
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }
}

fn decode_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}
