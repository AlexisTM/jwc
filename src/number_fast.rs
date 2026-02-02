use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct Number {
    raw: f64,
}

impl Number {
    #[must_use]
    pub fn from_parsed_and_lexeme(parsed: f64, _lexeme: &str) -> Self {
        Self { raw: parsed }
    }

    pub fn parse<T>(&self) -> Result<T, String>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        self.raw.to_string().parse::<T>().map_err(|e| e.to_string())
    }

    pub fn as_f32(&self) -> Result<f32, String> {
        Ok(self.raw as f32)
    }

    pub fn as_f64(&self) -> Result<f64, String> {
        Ok(self.raw)
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
                    Self { raw: value as f64 }
                }
            }
        )*
    };
}

impl_number_from_primitive!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);
