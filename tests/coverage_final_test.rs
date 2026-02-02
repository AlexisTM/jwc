use jwc::{CommentPolicy, FormatOptions, Indentation, Node, ObjectEntry, Value};

#[test]
fn test_pointer_mut_success_array() {
    let mut arr = Value::Array(vec![Node::new(Value::from(1))]);
    if let Some(v) = arr.pointer_mut("/0") {
        *v = Value::from(2);
    }
    assert_eq!(arr.pointer("/0"), Some(&Value::from(2)));
}

#[test]
fn test_serializer_newline_logic() {
    let mut n2 = Node::new(Value::from(1));
    n2.comment(jwc::Trivia::line(" test"));
    let s = n2.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Keep,
    });
    assert!(s.contains('\n'));
}

#[test]
fn test_serializer_comma_between_object_entries() {
    let obj = Value::Object(vec![
        ObjectEntry::new("a".to_string(), Node::new(Value::from(1))),
        ObjectEntry::new("b".to_string(), Node::new(Value::from(2))),
    ]);
    let n = Node::new(obj);
    let s = jwc::to_string(&n).unwrap();
    assert!(s.contains(','));
}

#[test]
fn test_parser_more_errors_specific() {
    // 104: EOF after \
    assert!(jwc::from_str("\"\\").is_err());

    // 120, 121: EOF while parsing number (though usually caught earlier)
    // Actually parse_number is called when it sees a digit.

    // 244: consume_trivia_slow non-whitespace > 127
    assert!(jwc::from_str("\u{00A1}1").is_err()); // Inverted exclamation mark is not whitespace

    // 257: Unexpected EOF in consume_object_colon
    assert!(jwc::from_str("{\"a\"").is_err());

    // 267, 268: Comment between key and colon
    assert!(jwc::from_str("{\"a\" // comment \n : 1}").is_ok());

    // 273, 274: Unexpected char '/' in consume_object_colon
    assert!(jwc::from_str("{\"a\" / : 1}").is_err());

    // 285-290: Non-ascii-whitespace between key and colon
    assert!(jwc::from_str("{\"a\" \u{00A0} : 1}").is_ok()); // Non-breaking space is whitespace
    assert!(jwc::from_str("{\"a\" \u{00A1} : 1}").is_err()); // Non-whitespace > 127

    // 305, 306: Expected ':' but found EOF? (Caught by 257)

    // 346, 347, 362, 363, 377, 378: Unexpected identifiers
    assert!(jwc::from_str("tree").is_err());
    assert!(jwc::from_str("full").is_err());
    assert!(jwc::from_str("none").is_err());

    // 418: Unexpected char after array element
    assert!(jwc::from_str("[1 2]").is_err());

    // 434: Unexpected char after object member
    assert!(jwc::from_str("{\"a\": 1 \"b\": 2}").is_err());
}

#[test]
fn test_keywords_explicit() {
    assert_eq!(jwc::from_str("true").unwrap().value, Value::Bool(true));
    assert_eq!(jwc::from_str("false").unwrap().value, Value::Bool(false));
    assert_eq!(jwc::from_str("null").unwrap().value, Value::Null);
}

#[test]
fn test_manual_object_no_trailing_comma() {
    let obj = Value::Object(vec![
        ObjectEntry::new("a".to_string(), Node::new(Value::from(1))),
        ObjectEntry::new("b".to_string(), Node::new(Value::from(2))),
    ]);
    let n = Node::new(obj);
    let s = jwc::to_string(&n).unwrap();
    assert_eq!(s, "{\"a\":1,\"b\":2}");
}

#[test]
fn test_parser_uncovered_trivia() {
    // 277, 278: Unexpected EOF after / in consume_object_colon
    assert!(jwc::from_str("{\"a\" /").is_err());
}
