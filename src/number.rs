use std::fmt;
use std::str::FromStr;

use crate::error::{Error, ErrorKind, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Repr {
    PosInt(u64),
    NegInt(i64),
    Float(f64),
}

/// A JSON number. Integers are exact up to 64 bits, and a number parsed from
/// source keeps its lexeme so `1.0`, `1e3` or a 30-digit integer serialize
/// exactly as written.
#[derive(Debug, Clone)]
pub struct Number {
    repr: Repr,
    lexeme: Option<Box<str>>,
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.repr == other.repr
    }
}

impl Number {
    /// Validates the JSON number grammar and classifies the value.
    ///
    /// # Errors
    /// `ErrorKind::Number` for anything that is not a JSON number, and for a
    /// float outside the `f64` range.
    pub fn from_lexeme(lexeme: &str) -> Result<Self> {
        if !is_json_number(lexeme) {
            return Err(Error::new(ErrorKind::Number(format!(
                "invalid number {lexeme:?}"
            ))));
        }
        let float = || -> Result<Repr> {
            let f: f64 = lexeme
                .parse()
                .map_err(|e| Error::new(ErrorKind::Number(format!("{lexeme:?}: {e}"))))?;
            if !f.is_finite() {
                return Err(Error::new(ErrorKind::Number(format!(
                    "number out of range: {lexeme}"
                ))));
            }
            Ok(Repr::Float(f))
        };
        let repr = if lexeme.bytes().any(|b| matches!(b, b'.' | b'e' | b'E')) {
            float()?
        } else if let Ok(u) = lexeme.parse::<u64>() {
            Repr::PosInt(u)
        } else if let Ok(i) = lexeme.parse::<i64>() {
            // "-0" is the integer zero
            u64::try_from(i).map_or(Repr::NegInt(i), Repr::PosInt)
        } else {
            // Beyond 64 bits: approximate value, exact lexeme.
            float()?
        };
        Ok(Self {
            repr,
            lexeme: Some(lexeme.into()),
        })
    }

    /// The number as written in the source, when it was parsed.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        self.lexeme.as_deref()
    }

    #[must_use]
    pub const fn is_i64(&self) -> bool {
        match self.repr {
            Repr::PosInt(u) => u <= i64::MAX as u64,
            Repr::NegInt(_) => true,
            Repr::Float(_) => false,
        }
    }

    #[must_use]
    pub const fn is_u64(&self) -> bool {
        matches!(self.repr, Repr::PosInt(_))
    }

    #[must_use]
    pub const fn is_f64(&self) -> bool {
        matches!(self.repr, Repr::Float(_))
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self.repr {
            Repr::PosInt(u) => i64::try_from(u).ok(),
            Repr::NegInt(i) => Some(i),
            Repr::Float(_) => None,
        }
    }

    #[must_use]
    pub const fn as_u64(&self) -> Option<u64> {
        match self.repr {
            Repr::PosInt(u) => Some(u),
            _ => None,
        }
    }

    /// Lossy for integers above 2^53.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_f64(&self) -> f64 {
        match self.repr {
            Repr::PosInt(u) => u as f64,
            Repr::NegInt(i) => i as f64,
            Repr::Float(f) => f,
        }
    }

    /// Lossy: `f32` has 24 bits of mantissa.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn as_f32(&self) -> f32 {
        self.as_f64() as f32
    }

    /// Parses the number as written, so `1.5` is an error for an integer type.
    ///
    /// # Errors
    /// `ErrorKind::Type` when `T` cannot represent the number as written.
    pub fn parse<T>(&self) -> Result<T>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        self.to_string().parse::<T>().map_err(|e| {
            Error::type_mismatch(format!(
                "cannot read {self} as {}: {e}",
                std::any::type_name::<T>()
            ))
        })
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(lexeme) = &self.lexeme {
            return f.write_str(lexeme);
        }
        match self.repr {
            Repr::PosInt(u) => write!(f, "{u}"),
            Repr::NegInt(i) => write!(f, "{i}"),
            // Debug keeps a decimal point or exponent, so the value reads back
            // as a float. JSON has no spelling for NaN or infinity.
            Repr::Float(x) if x.is_finite() => write!(f, "{x:?}"),
            Repr::Float(_) => f.write_str("null"),
        }
    }
}

fn is_json_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if b.first() == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == b.len()
}

macro_rules! from_unsigned {
    ($($t:ty),*) => {$(
        impl From<$t> for Number {
            fn from(value: $t) -> Self {
                Self { repr: Repr::PosInt(u64::from(value)), lexeme: None }
            }
        }
    )*};
}

macro_rules! from_signed {
    ($($t:ty),*) => {$(
        impl From<$t> for Number {
            fn from(value: $t) -> Self {
                let repr = match u64::try_from(value) {
                    Ok(u) => Repr::PosInt(u),
                    Err(_) => Repr::NegInt(i64::from(value)),
                };
                Self { repr, lexeme: None }
            }
        }
    )*};
}

from_unsigned!(u8, u16, u32, u64);
from_signed!(i8, i16, i32, i64);

impl From<usize> for Number {
    fn from(value: usize) -> Self {
        // usize is at most 64 bits wide on every supported target.
        Self::from(value as u64)
    }
}

impl From<isize> for Number {
    fn from(value: isize) -> Self {
        Self::from(value as i64)
    }
}

impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Self {
            repr: Repr::Float(value),
            lexeme: None,
        }
    }
}

impl From<f32> for Number {
    fn from(value: f32) -> Self {
        Self::from(f64::from(value))
    }
}
