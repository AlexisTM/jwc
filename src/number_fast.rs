//! Fast-path `Number`: preserves integer precision (i64) while still
//! supporting floats. The previous implementation always stored an `f64`,
//! which lost precision near `i64::MAX`.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    /// Used by parsers that already produced an `f64` from the lexeme. The
    /// lexeme is kept only on the `arbitrary_precision` build.
    #[must_use]
    pub fn from_parsed_and_lexeme(parsed: f64, _lexeme: &str) -> Self {
        Self::Float(parsed)
    }

    /// Generic parse via stringify → target parse. Works for any `FromStr`.
    /// Hot-path callers that want `i64`/`u64`/`f64` should prefer
    /// `as_i64` / `as_u64` / `as_f64` — those skip the allocation for the
    /// `Int` variant.
    pub fn parse<T>(&self) -> Result<T, String>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        let s = match self {
            Self::Int(n) => n.to_string(),
            Self::Float(f) => f.to_string(),
        };
        s.parse::<T>().map_err(|e| e.to_string())
    }

    /// Direct `i64` read. No allocation for the `Int` variant.
    ///
    /// Float fallback preserves historical behavior: `Float(3.0)` → `Some(3)`
    /// (because `3.0f64.to_string()` is `"3"`), `Float(3.5)` → `None`.
    // TODO(semantics): decide what Float→i64 should actually mean. The
    // current round-trip is an accident of `f64::Display`. Options:
    //   1. Only accept exact integral finite floats in [i64::MIN, i64::MAX]:
    //        if f.is_finite() && f.fract() == 0.0
    //           && f >= i64::MIN as f64 && f <= i64::MAX as f64
    //        { Some(f as i64) } else { None }
    //   2. Return `None` for all `Float` (matches serde_json, since
    //      serde_json stores integer-shaped inputs as i64 up-front).
    //   3. Keep current stringify-parse behavior forever.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            Self::Float(f) => f.to_string().parse::<i64>().ok(),
        }
    }

    /// Direct `u64` read. No allocation for in-range `Int` values.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Int(n) if *n >= 0 => Some(*n as u64),
            Self::Int(_) => None,
            Self::Float(f) => f.to_string().parse::<u64>().ok(),
        }
    }

    pub fn as_f32(&self) -> Result<f32, String> {
        Ok(match self {
            Self::Int(n) => *n as f32,
            Self::Float(f) => *f as f32,
        })
    }

    pub fn as_f64(&self) -> Result<f64, String> {
        Ok(match self {
            Self::Int(n) => *n as f64,
            Self::Float(f) => *f,
        })
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(v) => write!(f, "{v}"),
        }
    }
}

// --- From<integer primitive> → Int ---

macro_rules! impl_int_from {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Number {
                fn from(value: $t) -> Self { Self::Int(value as i64) }
            }
        )*
    };
}

impl_int_from!(i8, i16, i32, i64, isize, u8, u16, u32);

// u64 / usize may exceed i64::MAX; saturate to Float in that case.
impl From<u64> for Number {
    fn from(value: u64) -> Self {
        if value <= i64::MAX as u64 {
            Self::Int(value as i64)
        } else {
            Self::Float(value as f64)
        }
    }
}

impl From<usize> for Number {
    fn from(value: usize) -> Self {
        if (value as u64) <= i64::MAX as u64 {
            Self::Int(value as i64)
        } else {
            Self::Float(value as f64)
        }
    }
}

// --- From<floating> → Float ---

impl From<f32> for Number {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}
