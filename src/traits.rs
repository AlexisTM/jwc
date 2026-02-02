use crate::ast::{Node, ObjectEntry, Value};
use crate::Number;
use std::collections::HashMap;

pub trait JwcSerializable {
    fn to_jwc(&self) -> Value;
}

pub trait JwcDeserializable {
    fn from_jwc(value: Value) -> Result<Self, String>
    where
        Self: Sized;
}

// Implementations for primitives

impl JwcSerializable for bool {
    fn to_jwc(&self) -> Value {
        Value::Bool(*self)
    }
}

impl JwcDeserializable for bool {
    fn from_jwc(value: Value) -> Result<Self, String> {
        match value {
            Value::Bool(b) => Ok(b),
            _ => Err("Expected Bool".to_string()),
        }
    }
}

// Number Macros
macro_rules! impl_number_traits {
    ($($t:ty),*) => {
        $(
            impl JwcSerializable for $t {
                fn to_jwc(&self) -> Value {
                    Value::Number(Number::from(*self))
                }
            }

            impl JwcDeserializable for $t {
                fn from_jwc(value: Value) -> Result<Self, String> {
                    match value {
                        Value::Number(n) => n.parse::<$t>(),
                        _ => Err(format!("Expected Number for {}", stringify!($t))),
                    }
                }
            }
        )*
    };
}

impl_number_traits!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);

// Option implementation
impl<T: JwcSerializable> JwcSerializable for Option<T> {
    fn to_jwc(&self) -> Value {
        match self {
            Some(v) => v.to_jwc(),
            None => Value::Null,
        }
    }
}

impl<T: JwcDeserializable> JwcDeserializable for Option<T> {
    fn from_jwc(value: Value) -> Result<Self, String> {
        match value {
            Value::Null => Ok(None),
            _ => Ok(Some(T::from_jwc(value)?)),
        }
    }
}

// Unit implementation
impl JwcSerializable for () {
    fn to_jwc(&self) -> Value {
        Value::Null
    }
}

impl JwcDeserializable for () {
    fn from_jwc(value: Value) -> Result<Self, String> {
        match value {
            Value::Null => Ok(()),
            _ => Err("Expected Null for unit".to_string()),
        }
    }
}

impl JwcSerializable for String {
    fn to_jwc(&self) -> Value {
        Value::String(self.clone())
    }
}

impl JwcDeserializable for String {
    fn from_jwc(value: Value) -> Result<Self, String> {
        match value {
            Value::String(s) => Ok(s),
            _ => Err("Expected String".to_string()),
        }
    }
}

impl JwcSerializable for &str {
    fn to_jwc(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl<T: JwcSerializable> JwcSerializable for Vec<T> {
    fn to_jwc(&self) -> Value {
        let elements = self.iter().map(|e| Node::new(e.to_jwc())).collect();
        Value::Array(elements)
    }
}

impl<T: JwcDeserializable> JwcDeserializable for Vec<T> {
    fn from_jwc(value: Value) -> Result<Self, String> {
        match value {
            Value::Array(elements) => {
                let mut vec = Self::new();
                for node in elements {
                    vec.push(T::from_jwc(node.value)?);
                }
                Ok(vec)
            }
            _ => Err("Expected Array".to_string()),
        }
    }
}

impl<T: JwcSerializable> JwcSerializable for HashMap<String, T> {
    fn to_jwc(&self) -> Value {
        let mut members = Vec::new();
        // Determine order? HashMap is unordered.
        // We just collect them.
        for (k, v) in self {
            members.push(ObjectEntry::new(k.clone(), Node::new(v.to_jwc())));
        }
        Value::Object(members)
    }
}

impl<T: JwcDeserializable> JwcDeserializable for HashMap<String, T> {
    fn from_jwc(value: Value) -> Result<Self, String> {
        match value {
            Value::Object(members) => {
                let mut map = Self::new();
                for entry in members {
                    map.insert(entry.key, T::from_jwc(entry.value.value)?);
                }
                Ok(map)
            }
            _ => Err("Expected Object".to_string()),
        }
    }
}
