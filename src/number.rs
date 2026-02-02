#[cfg(feature = "arbitrary_precision")]
#[path = "number_arbitrary.rs"]
mod imp;

#[cfg(not(feature = "arbitrary_precision"))]
#[path = "number_fast.rs"]
mod imp;

pub use imp::Number;
