use jwc::{Value, from_reader, from_slice, from_str, to_string_pretty};
use std::io::Cursor;

#[test]
fn test_from_str() {
    let json = r#"{"key": "value"}"#;
    let node = from_str(json).unwrap();
    assert!(matches!(node.value, Value::Object(_)));
}

#[test]
fn test_from_slice() {
    let json = r#"{"key": 123}"#;
    let node = from_slice(json.as_bytes()).unwrap();
    assert!(matches!(node.value, Value::Object(_)));
}

#[test]
fn test_from_reader() {
    let json = r#"["a", "b"]"#;
    let cursor = Cursor::new(json);
    let node = from_reader(cursor).unwrap();
    assert!(matches!(node.value, Value::Array(_)));
}

#[test]
fn test_to_string_pretty() {
    let json = r#"{"a":1}"#;
    let node = from_str(json).unwrap();
    let pretty = to_string_pretty(&node, Some("  ")).unwrap();

    assert!(pretty.contains("{\n  \"a\": 1\n}"));
}

#[test]
fn test_round_trip_convenience() {
    let input = r#"
    {
        // comment
        "key": 42
    }"#;

    let node = from_str(input).unwrap();
    let output = to_string_pretty(&node, Some("    ")).unwrap(); // 4 spaces

    // Output should maintain structure and comments
    assert!(output.contains("// comment"));
    assert!(output.contains("\"key\": 42"));
}

#[test]
fn test_to_string_minified() {
    let json = r#"{
        "a": 1
    }"#;
    let node = from_str(json).unwrap();
    let minified = jwc::to_string(&node).unwrap();
    assert_eq!(minified, r#"{"a":1}"#);
}

#[test]
fn test_to_vec() {
    let json = r#"{"a":1}"#;
    let node = from_str(json).unwrap();
    let vec = jwc::to_vec(&node).unwrap();
    assert_eq!(vec, b"{\"a\":1}");
}

#[test]
fn test_to_vec_pretty() {
    let json = r#"{"a":1}"#;
    let node = from_str(json).unwrap();
    let vec = jwc::to_vec_pretty(&node, None).unwrap(); // Default 4 spaces
    let s = String::from_utf8(vec).unwrap();
    assert!(s.contains("    \"a\": 1"));
}

#[test]
fn test_to_writer() {
    let json = r#"{"a":1}"#;
    let node = from_str(json).unwrap();
    let mut buffer = Vec::new();
    jwc::to_writer(&mut buffer, &node).unwrap();
    assert_eq!(buffer, b"{\"a\":1}");
}

#[test]
fn test_to_writer_pretty() {
    let json = r#"{"a":1}"#;
    let node = from_str(json).unwrap();
    let mut buffer = Vec::new();
    jwc::to_writer_pretty(&mut buffer, &node, Some("\t")).unwrap();
    let s = String::from_utf8(buffer).unwrap();
    assert!(s.contains("\t\"a\": 1"));
}
