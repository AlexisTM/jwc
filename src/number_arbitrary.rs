use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Number {
    raw: String,
}

impl Number {
    #[must_use]
    pub fn from_parsed_and_lexeme(_parsed: f64, lexeme: &str) -> Self {
        Self {
            raw: lexeme.to_string(),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn parse<T>(&self) -> Result<T, String>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        self.raw.parse::<T>().map_err(|e| e.to_string())
    }

    pub fn as_f32(&self) -> Result<f32, String> {
        self.raw.parse::<f32>().map_err(|e| e.to_string())
    }

    pub fn as_f64(&self) -> Result<f64, String> {
        self.raw.parse::<f64>().map_err(|e| e.to_string())
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

macro_rules! impl_number_from_primitive {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Number {
                fn from(value: $t) -> Self {
                    Self {
                        raw: value.to_string(),
                    }
                }
            }
        )*
    };
}

impl_number_from_primitive!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);
