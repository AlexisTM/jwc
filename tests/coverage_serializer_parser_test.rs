use jwc::{CommentPolicy, FormatOptions, Indentation, Node, ObjectEntry, Value};

#[test]
fn test_serializer_indent_none() {
    let obj = Value::Object(vec![ObjectEntry::new(
        "a".to_string(),
        Node::new(Value::from(1)),
    )]);
    let n = Node::new(obj);
    let s = n.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Keep,
    });
    assert_eq!(s, "{\"a\":1}");
}

#[test]
fn test_serializer_object_entry_display() {
    let mut entry = ObjectEntry::new("key".to_string(), Node::new(Value::from(42)));
    entry.key_comment(jwc::Trivia::line(" lead "));
    entry.key_comment(jwc::Trivia::block(" trail "));

    let display_str = format!("{entry}");
    assert_eq!(display_str, "// lead /* trail */\"key\":42"); // Based on manual Display impl

    // Indentation::None Edge cases
    let empty_obj = Node::new(Value::Object(vec![]));
    assert_eq!(
        empty_obj.to_formatted_string(FormatOptions {
            indentation: Indentation::None,
            comment_policy: CommentPolicy::Keep
        }),
        "{}"
    );

    let empty_arr = Node::new(Value::Array(vec![]));
    assert_eq!(
        empty_arr.to_formatted_string(FormatOptions {
            indentation: Indentation::None,
            comment_policy: CommentPolicy::Keep
        }),
        "[]"
    );

    let mut obj_with_comment = Node::new(Value::Object(vec![ObjectEntry::new(
        "a".to_string(),
        Node::new(Value::from(1)),
    )]));
    obj_with_comment.comment(jwc::Trivia::line(" test "));
    assert_eq!(
        obj_with_comment.to_formatted_string(FormatOptions {
            indentation: Indentation::None,
            comment_policy: CommentPolicy::Keep
        }),
        "// test \n{\"a\":1}"
    );
}

#[test]
fn test_parser_errors() {
    // String errors
    assert!(jwc::from_str("\"unterminated").is_err());
    assert!(jwc::from_str("\"\\\"").is_err()); // EOF after \
    let _ = jwc::from_str("\"\\x\""); // Invalid escape sequence acts as identity in this parser

    // Number errors
    assert!(jwc::from_str("-").is_err());
    assert!(jwc::from_str("1.2.3").is_err()); // Will parse 1., then err or fail.

    // Comment errors
    assert!(jwc::from_str("/* unterminated").is_err());
    assert!(jwc::from_str("/").is_err());
    assert!(jwc::from_str("/a").is_err());

    // Object errors
    assert!(jwc::from_str("{").is_err()); // EOF in object
    assert!(jwc::from_str("{\"a\"}").is_err()); // missing :
    assert!(jwc::from_str("{a: 1}").is_err()); // unquoted key
    assert!(jwc::from_str("{\"a\": 1 ]").is_err()); // expected , or }
    assert!(jwc::from_str("{\"a\" , 1}").is_err()); // bad colon
    assert!(jwc::from_str("{\"a\" /*/} 1}").is_err()); // bad comment before colon

    // Array errors
    assert!(jwc::from_str("[").is_err()); // EOF in array
    assert!(jwc::from_str("[1 }").is_err()); // expected , or ]

    // General unexpected chars
    assert!(jwc::from_str("}").is_err());
    assert!(jwc::from_str("truX").is_err());
    assert!(jwc::from_str("falsX").is_err());
    assert!(jwc::from_str("nulX").is_err());
    assert!(jwc::from_str("t").is_err());
    assert!(jwc::from_str("f").is_err());
    assert!(jwc::from_str("n").is_err());

    assert!(jwc::from_str("\n\n\u{2028}").is_err()); // some whitespace or unhandled > 127

    // Structured parse error preserves line/col.
    let err = jwc::from_str("\n\n\"unterm").unwrap_err();
    match err {
        jwc::Error::Parse { line, .. } => assert_eq!(line, 3),
        other => panic!("expected Error::Parse, got {other:?}"),
    }

    // Valid string escapes to cover match arms
    let valid_str = "\"\\b\\f\\n\\r\\t\\/\\\\\"";
    assert!(jwc::from_str(valid_str).is_ok());

    // Number slow path / error at EOF
    assert!(jwc::from_str("1e").is_err());

    // Invalid characters inside strings or structures
    assert!(jwc::from_str("{\"a\": \x01}").is_err());
    assert!(jwc::from_str("[\x01]").is_err());
    assert!(jwc::from_str("\x01").is_err());
}
