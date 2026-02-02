use jwc::{JwcDeserializable, JwcSerializable, Node, Value};
use std::collections::HashMap;

#[test]
fn test_traits_roundtrip() {
    let mut map = HashMap::new();
    map.insert("a".to_string(), 1);
    map.insert("b".to_string(), 2);

    let value = map.to_jwc();
    // Verify it's an object
    if let Value::Object(entries) = &value {
        assert_eq!(entries.len(), 2);
    } else {
        panic!("Expected Object");
    }

    // Deserialize back
    let map2: HashMap<String, i32> = HashMap::from_jwc(value).unwrap();
    assert_eq!(map, map2);
}

#[test]
fn test_vec_trait() {
    let vec = vec![1, 2, 3];
    let value = vec.to_jwc();
    let vec2: Vec<i32> = Vec::from_jwc(value).unwrap();
    assert_eq!(vec, vec2);
}

#[test]
fn test_add_comments_elements_helpers() {
    // Start with empty object
    let mut root = Value::Object(Vec::new());

    // Add element
    let node = Node::new(Value::from(true));
    root.insert("debug", node).unwrap();

    // Add element with comments
    let mut node2 = Node::new(Value::from(100));
    node2.comment(jwc::Trivia::line(" timeout in ms"));
    let entry = root.insert("timeout", node2).unwrap();
    entry.key_comment(jwc::Trivia::line(" The timeout field"));

    let output = Node::new(root).to_string();
    println!("Helper Output:\n{output}");

    assert!(output.contains("\"debug\":true"));
    assert!(output.contains("// timeout in ms"));
    assert!(output.contains("// The timeout field"));
    assert!(output.contains("\"timeout\":"));
    assert!(output.contains("100"));
}
