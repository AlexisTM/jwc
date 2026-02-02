use crate::{Error, Node, Result, Value, from_str};

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

    /// Parse the deferred text once and keep the result.
    ///
    /// # Errors
    /// Parse errors from the raw text, or `ErrorKind::Type` when the value is
    /// not the object or array this lazy value was declared to hold.
    pub fn thaw(&mut self) -> Result<&Value> {
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

    /// [`thaw`](Self::thaw) and convert.
    ///
    /// # Errors
    /// As [`thaw`](Self::thaw), plus the type's own conversion errors.
    pub fn parse_as<T: crate::JwcDeserializable>(&mut self) -> Result<T> {
        let value = self.thaw()?.clone();
        T::from_jwc(value)
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Object,
    Vector,
}

fn parse_value(raw: &str, expected: Option<Kind>) -> Result<Value> {
    let node: Node = from_str(raw)?;
    let value = node.value;

    match expected {
        Some(Kind::Object) if !matches!(value, Value::Object(_)) => Err(Error::type_mismatch(
            "Expected object value while thawing lazy value",
        )),
        Some(Kind::Vector) if !matches!(value, Value::Array(_)) => Err(Error::type_mismatch(
            "Expected array value while thawing lazy value",
        )),
        _ => Ok(value),
    }
}
