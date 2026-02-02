use jwc::{Node, Value};

#[test]
fn test_add_each_element_type() {
    let mut root = Value::Object(Vec::new());

    // Bool
    root.insert("bool_true", Node::new(Value::from(true)))
        .unwrap();
    root.insert("bool_false", Node::new(Value::from(false)))
        .unwrap();

    // Number (int)
    root.insert("int", Node::new(Value::from(42))).unwrap();

    // Number (float)
    root.insert("float", Node::new(Value::from(1.25))).unwrap();

    // String
    root.insert("string", Node::new(Value::from("hello")))
        .unwrap();

    // Array
    let mut arr = Value::Array(Vec::new());
    arr.push(Node::new(Value::from(1))).unwrap();
    arr.push(Node::new(Value::from(2))).unwrap();
    root.insert("array", Node::new(arr)).unwrap();

    // Object (Nested)
    let mut obj = Value::Object(Vec::new());
    obj.insert("nested_key", Node::new(Value::from("nested_val")))
        .unwrap();
    root.insert("object", Node::new(obj)).unwrap();

    // Null
    root.insert("null", Node::new(Value::Null)).unwrap();

    let output = Node::new(root).to_string();
    println!("Elements Output:\n{output}");

    assert!(output.contains("\"bool_true\":true"));
    assert!(output.contains("\"bool_false\":false"));
    assert!(output.contains("\"int\":42"));
    assert!(output.contains("\"float\":1.25"));
    assert!(output.contains("\"string\":\"hello\""));
    assert!(output.contains("\"array\":[1,2]")); // whitespace depends on impl
    assert!(output.contains("\"object\":{\"nested_key\":\"nested_val\"}"));
    assert!(output.contains("\"null\":null"));
}
