/// Construct a [`Value`] from a JSON-like literal.
///
/// Mirrors `serde_json::json!` for the common cases. Keys in objects must be
/// string literals or expressions that implement `ToString`.
///
/// ```
/// use jwc::{jwc, Value};
/// let v = jwc!({
///     "port": 8080,
///     "tags": ["a", "b"],
///     "nested": { "enabled": true, "ratio": 0.5 },
///     "maybe": null,
/// });
/// assert!(v.is_object());
/// assert_eq!(v["port"].as_i64(), Some(8080));
/// assert_eq!(v["tags"][1].as_str(), Some("b"));
/// ```
///
/// [`Value`]: crate::Value
#[macro_export]
macro_rules! jwc {
    // --- leaves ---
    (null) => { $crate::Value::Null };
    (true) => { $crate::Value::Bool(true) };
    (false) => { $crate::Value::Bool(false) };

    // --- array ---
    ([]) => { $crate::Value::Array(::std::vec::Vec::new()) };
    ([ $($elem:tt),+ $(,)? ]) => {
        $crate::Value::Array(::std::vec![
            $( $crate::Node::new($crate::jwc!($elem)) ),+
        ])
    };

    // --- object ---
    ({}) => { $crate::Value::Object(::std::vec::Vec::new()) };
    ({ $( $key:tt : $val:tt ),+ $(,)? }) => {
        $crate::Value::Object(::std::vec![
            $(
                $crate::ObjectEntry::new(
                    ::std::string::ToString::to_string(&$key),
                    $crate::Node::new($crate::jwc!($val)),
                )
            ),+
        ])
    };

    // --- fallback: any expression convertible to Value ---
    ($other:expr) => { $crate::Value::from($other) };
}
