use crate::ast::{Node, ObjectEntry, Value};
use crate::{Error, Number, Result};
use std::collections::HashMap;

pub trait JwcSerializable {
    fn to_jwc(&self) -> Value;
}

pub trait JwcDeserializable {
    fn from_jwc(value: Value) -> Result<Self>
    where
        Self: Sized;
}

impl JwcSerializable for bool {
    fn to_jwc(&self) -> Value {
        Value::Bool(*self)
    }
}

impl JwcDeserializable for bool {
    fn from_jwc(value: Value) -> Result<Self> {
        match value {
            Value::Bool(b) => Ok(b),
            other => Err(Error::ty("bool", crate::_value_kind(&other))),
        }
    }
}

// Number macros
macro_rules! impl_number_traits {
    ($($t:ty),*) => {
        $(
            impl JwcSerializable for $t {
                fn to_jwc(&self) -> Value {
                    Value::Number(Number::from(*self))
                }
            }

            impl JwcDeserializable for $t {
                fn from_jwc(value: Value) -> Result<Self> {
                    match value {
                        Value::Number(n) => n.parse::<$t>().map_err(Error::Custom),
                        other => Err(Error::ty(stringify!($t), crate::_value_kind(&other))),
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
    fn from_jwc(value: Value) -> Result<Self> {
        match value {
            Value::Null => Ok(None),
            other => Ok(Some(T::from_jwc(other)?)),
        }
    }
}

// Unit
impl JwcSerializable for () {
    fn to_jwc(&self) -> Value {
        Value::Null
    }
}

impl JwcDeserializable for () {
    fn from_jwc(value: Value) -> Result<Self> {
        match value {
            Value::Null => Ok(()),
            other => Err(Error::ty("null", crate::_value_kind(&other))),
        }
    }
}

impl JwcSerializable for String {
    fn to_jwc(&self) -> Value {
        Value::String(self.clone())
    }
}

impl JwcDeserializable for String {
    fn from_jwc(value: Value) -> Result<Self> {
        match value {
            Value::String(s) => Ok(s),
            other => Err(Error::ty("string", crate::_value_kind(&other))),
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
    fn from_jwc(value: Value) -> Result<Self> {
        match value {
            Value::Array(elements) => {
                let mut vec = Self::new();
                for node in elements {
                    vec.push(T::from_jwc(node.value)?);
                }
                Ok(vec)
            }
            other => Err(Error::ty("array", crate::_value_kind(&other))),
        }
    }
}

impl<T: JwcSerializable> JwcSerializable for HashMap<String, T> {
    fn to_jwc(&self) -> Value {
        let mut members = Vec::new();
        for (k, v) in self {
            members.push(ObjectEntry::new(k.clone(), Node::new(v.to_jwc())));
        }
        Value::Object(members)
    }
}

impl<T: JwcDeserializable> JwcDeserializable for HashMap<String, T> {
    fn from_jwc(value: Value) -> Result<Self> {
        match value {
            Value::Object(members) => {
                let mut map = Self::new();
                for entry in members {
                    map.insert(entry.key, T::from_jwc(entry.value.value)?);
                }
                Ok(map)
            }
            other => Err(Error::ty("object", crate::_value_kind(&other))),
        }
    }
}
