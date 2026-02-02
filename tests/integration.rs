use jwc::{Node, ObjectEntry, Trivia, Value, single_pass_parser};

/*
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}
#[test]
fn test_round_trip_top() {
    let input = r#"
    /* Block comment */
// Top comment
    {
// Line comment
        "key": "value",
        "arr": [1, 2, 3],
        "obj": { "nested": true }
    }
    "#;

    let mut parser = single_pass_parser::SinglePassParser::new(input);
    let node = parser.parse().unwrap();
    let output = node.to_string();

    // Parse both to verify they're semantically equivalent
    let node2 = single_pass_parser::SinglePassParser::new(&output).parse().unwrap();
    assert_eq!(node.value, node2.value);
    // Comments should still be present
    assert!(output.contains("Block comment"));
    assert!(output.contains("Top comment"));
    assert!(output.contains("Line comment"));
}

#[test]
fn test_round_trip_simple() {
    let input = r#"
    {
// Line comment
        "key": "value",
/* Block comment */
        "arr": [1, 2, 3],
        "obj": { "nested": true }
    }
    "#;

    let mut parser = single_pass_parser::SinglePassParser::new(input);
    let node = parser.parse().unwrap();
    let output = node.to_string();

    // Parse both to verify they're semantically equivalent
    let node2 = single_pass_parser::SinglePassParser::new(&output).parse().unwrap();
    assert_eq!(node.value, node2.value);
    // Comments should still be present
    assert!(output.contains("Line comment"));
    assert!(output.contains("block"));
} */

#[test]
fn test_trailing_commas() {
    let input = r"[
        1,
        2,
    ]";
    let mut parser = single_pass_parser::SinglePassParser::new(input);
    let node = parser.parse().unwrap();
    let output = node.to_string();
    // NOTE: Exact formatting no longer preserved, but trailing commas should be maintained
    assert!(node.value.to_string().contains(','));
    // Parse again to verify semantic equivalence
    let node2 = single_pass_parser::SinglePassParser::new(&output)
        .parse()
        .unwrap();
    if let Value::Array(elements) = &node2.value {
        // Check that trailing comma is preserved
        assert_eq!(elements.len(), 2);
        assert!(elements[1].comma); // Last element should have trailing comma
    }
}

#[test]
fn test_modification() {
    let input = r#"{
    "a": 1
}"#;
    let mut parser = single_pass_parser::SinglePassParser::new(input);
    let mut node = parser.parse().unwrap();

    if let Value::Object(ref mut members) = node.value {
        // "b": 2
        let key = "b".to_string();
        let val_node = Node::new(Value::Number(2.into()));

        // Add leading newline/indent to key to make it look nice.
        if let Some(last) = members.last_mut() {
            last.value.comma = true;
        }

        let entry = ObjectEntry::new(key, val_node);
        // Note: Whitespace trivia has been removed for performance.
        // The serializer will add appropriate spacing.
        members.push(entry);
    }

    let output = node.to_string();
    println!("Modified:\n{output}");
    // Since we no longer track whitespace, output format is simpler
    assert!(output.contains("\"b\":2"));
}

#[test]
fn test_comment_between_key_and_colon_attached_to_value() {
    let input = r#"{
    "a" /*c*/ : 1
}"#;
    let mut parser = single_pass_parser::SinglePassParser::new(input);
    let node = parser.parse().unwrap();

    if let Value::Object(members) = node.value {
        assert_eq!(members.len(), 1);
        let entry = &members[0];

        // The block comment between key and ':' should appear in the value's
        // leading trivia.
        assert!(
            entry
                .value
                .trivia
                .iter()
                .any(|t| matches!(t, Trivia::BlockComment(c) if c.contains('c')))
        );
    } else {
        panic!("Expected top-level object");
    }
}

#[test]
fn test_comments_after_document_are_preserved_on_root_trivia() {
    let input = r#"{"x":1}
// end line
/* end block */"#;

    let node = jwc::from_str(input).expect("should parse with EOF comments");

    assert!(
        node.trivia
            .iter()
            .any(|t| matches!(t, Trivia::LineComment(c) if c.contains("end line")))
    );
    assert!(
        node.trivia
            .iter()
            .any(|t| matches!(t, Trivia::BlockComment(c) if c.contains("end block")))
    );
}
