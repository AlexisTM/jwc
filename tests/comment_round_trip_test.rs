use jwc::{Node, ObjectEntry, Trivia, Value};

#[test]
fn test_node_comments() {
    let mut node = Node::new(Value::from("value"));
    node.add_line_comment(" leading ");
    node.add_block_comment(" block ");

    // Check internal state
    assert_eq!(node.trivia.len(), 2);
    if let Trivia::LineComment(c) = &node.trivia[0] {
        assert_eq!(c, " leading ");
    } else {
        panic!("Expected LineComment");
    }

    if let Trivia::BlockComment(c) = &node.trivia[1] {
        assert_eq!(c, " block ");
    } else {
        panic!("Expected BlockComment");
    }

    node.add_line_comment(" trailing ");

    let _json = jwc::to_string(&node).unwrap();
}

#[test]
fn test_object_entry_comments() {
    let mut val_node = Node::new(Value::from(42));
    val_node.add_line_comment(" value comment ");

    let mut entry = ObjectEntry::new("key".to_string(), val_node);
    entry.add_key_comment(" key comment ");
    entry.add_key_block_comment(" key trailing ");

    // Wrapper object
    let obj_val = Value::Object(vec![entry]);
    let obj_node = Node::new(obj_val);

    let json = jwc::to_string(&obj_node).unwrap();

    // Re-parse
    let _parsed = jwc::from_str(&json);
}

#[test]
fn test_round_trip_complex() {
    let input = r#"
    {
        // This is a key comment
        "key": /* value comment */ "value",
        "array": [
            // Array element comment
            1,
            2 // Trailing element comment
        ],
        /* Trailing object comment */
    }
    "#;

    let node1 = jwc::from_str(input).expect("Failed to parse initial input");

    // We use pretty print to ensure line comments are safe
    let serialized = jwc::to_string_pretty(&node1, Some("  ")).expect("Failed to serialize");

    let node2 = jwc::from_str(&serialized).expect("Failed to parse serialized output");

    assert_eq!(node1, node2, "Round trip failed: ASTs do not match");
}

#[test]
fn test_manual_construction_and_parsing() {
    // Construct AST manually
    let mut root_val = Value::Object(Vec::new());

    // Test invalid operation
    assert!(root_val.push(Node::new(Value::from(true))).is_err());

    // Reset (or just continue with empty object)

    // Insert Entry 1
    let mut val1 = Node::new("bar".into());
    val1.add_line_comment(" val1 comment");

    // Helper `insert` in strictness adds a new entry.
    let entry_ref = root_val.insert("foo", val1).unwrap();
    entry_ref.add_key_comment(" key1 comment");

    let root = Node::new(root_val);

    // Serialize
    let serialized = jwc::to_string_pretty(&root, None).unwrap();

    // Parse back
    let parsed = jwc::from_str(&serialized).unwrap();

    // Compare
    assert_eq!(root, parsed);
}

#[test]
fn test_string_escape_round_trip() {
    let input = r#"{
        "escapes": "\" \\ \/ \b \f \n \r \t \u0000",
        "rocket": "\uD83D\uDE80"
    }"#;

    let parsed = jwc::from_str(input).expect("Failed to parse escaped string payload");
    let serialized = jwc::to_string(&parsed).expect("Failed to serialize escaped string payload");
    let reparsed = jwc::from_str(&serialized).expect("Failed to parse serialized escaped payload");

    assert_eq!(parsed, reparsed, "Escaped strings did not round-trip");
    assert!(
        serialized.contains("\\u0000"),
        "Expected serializer to escape NUL as unicode escape"
    );
    assert!(
        serialized.contains("\\b") && serialized.contains("\\f"),
        "Expected serializer to preserve backspace and form-feed escapes"
    );
}

#[cfg(feature = "arbitrary_precision")]
#[test]
fn test_number_lexeme_round_trip_and_parse_on_demand() {
    let input = r#"{
        "max_int64": 9223372036854775807,
        "precision_loss": 9007199254740993,
        "floats": [1.23456789e+308, 5e-324, -0.0, 0.0000000000000000000000000000000000000001]
    }"#;

    let parsed = jwc::from_str(input).expect("Failed to parse number payload");
    let serialized = jwc::to_string(&parsed).expect("Failed to serialize number payload");

    assert!(
        serialized.contains("\"max_int64\":9223372036854775807"),
        "Expected max_int64 lexeme to be preserved"
    );
    assert!(
        serialized.contains("\"precision_loss\":9007199254740993"),
        "Expected precision_loss lexeme to be preserved"
    );
    assert!(
        serialized.contains("[1.23456789e+308,5e-324,-0.0,0.0000000000000000000000000000000000000001]"),
        "Expected float lexemes to be preserved"
    );

    if let Value::Object(members) = &parsed.value {
        let n = &members[0].value.value;
        if let Value::Number(num) = n {
            assert_eq!(num.parse::<i64>().unwrap(), 9_223_372_036_854_775_807_i64);
        } else {
            panic!("Expected number");
        }
    } else {
        panic!("Expected object");
    }
}
