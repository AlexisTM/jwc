use crate::{Node, Value, from_str};

#[derive(Clone, Debug, PartialEq)]
pub enum LazyValue {
    Unknown(String),
    UnknownObject(String),
    UnknownVector(String),
    Parsed(Value),
}

impl LazyValue {
    #[must_use]
    pub fn unknown<S: Into<String>>(source: S) -> Self {
        Self::Unknown(source.into())
    }

    #[must_use]
    pub fn unknown_object<S: Into<String>>(source: S) -> Self {
        Self::UnknownObject(source.into())
    }

    #[must_use]
    pub fn unknown_vector<S: Into<String>>(source: S) -> Self {
        Self::UnknownVector(source.into())
    }

    pub fn thaw(&mut self) -> Result<&Value, String> {
        if !matches!(self, Self::Parsed(_)) {
            let parsed = match self {
                Self::Unknown(raw) => parse_value(raw, None)?,
                Self::UnknownObject(raw) => parse_value(raw, Some(Kind::Object))?,
                Self::UnknownVector(raw) => parse_value(raw, Some(Kind::Vector))?,
                Self::Parsed(_) => unreachable!(),
            };
            *self = Self::Parsed(parsed);
        }

        if let Self::Parsed(value) = self {
            Ok(value)
        } else {
            unreachable!()
        }
    }

    pub fn parse_as<T: crate::JwcDeserializable>(&mut self) -> Result<T, String> {
        let value = self.thaw()?.clone();
        T::from_jwc(value)
    }
}

enum Kind {
    Object,
    Vector,
}

fn parse_value(raw: &str, expected: Option<Kind>) -> Result<Value, String> {
    let node: Node = from_str(raw)?;
    let value = node.value;

    match expected {
        Some(Kind::Object) if !matches!(value, Value::Object(_)) => {
            Err("Expected object value while thawing lazy value".to_string())
        }
        Some(Kind::Vector) if !matches!(value, Value::Array(_)) => {
            Err("Expected array value while thawing lazy value".to_string())
        }
        _ => Ok(value),
    }
}
