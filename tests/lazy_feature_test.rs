#![cfg(feature = "lazy")]

use jwc::{JwcDeserializable, LazyValue, Node, Value};

#[test]
fn thaw_unknown_value() {
    let mut value = LazyValue::unknown("123");
    assert_eq!(value.thaw().expect("should thaw"), &Value::Number(123.into()));
}

#[test]
fn thaw_unknown_object_and_vector() {
    let mut object = LazyValue::unknown_object("{\"x\": 1}");
    assert!(matches!(
        object.thaw().expect("object should thaw"),
        Value::Object(_)
    ));

    let mut vector = LazyValue::unknown_vector("[1, 2, 3]");
    assert!(matches!(
        vector.thaw().expect("array should thaw"),
        Value::Array(_)
    ));
}

#[test]
fn parse_as_specific_type() {
    let mut number = LazyValue::unknown("42");
    let n: i32 = number.parse_as().expect("i32 parsing should work");
    assert_eq!(n, 42);

    let mut text = LazyValue::unknown("\"hello\"");
    let s: String = text.parse_as().expect("string parsing should work");
    assert_eq!(s, "hello");

    let mut vector = LazyValue::unknown_vector("[1,2,3]");
    let list: Vec<i32> = vector.parse_as().expect("vector parse should work");
    assert_eq!(list, vec![1, 2, 3]);
}

#[test]
fn parsed_lazy_value_works_in_ast() {
    let lazy = LazyValue::unknown("true");
    let node = Node::new(Value::from(lazy));
    let rendered = node.to_string();
    assert_eq!(rendered, "true");
}

#[test]
fn typed_unknown_rejects_wrong_shape() {
    let mut object = LazyValue::unknown_object("[1,2,3]");
    let err = object.thaw().expect_err("shape mismatch should fail");
    assert!(err.contains("Expected object"));
}

#[test]
fn custom_type_parse_from_lazy() {
    #[derive(Debug, PartialEq)]
    struct Age(u8);

    impl JwcDeserializable for Age {
        fn from_jwc(value: Value) -> Result<Self, String> {
            let n = u8::from_jwc(value)?;
            Ok(Self(n))
        }
    }

    let mut age = LazyValue::unknown("7");
    assert_eq!(
        age.parse_as::<Age>().expect("custom parse should work"),
        Age(7)
    );
}
