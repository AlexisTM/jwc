//! Serializer behavior across the `CommentPolicy` × `Indentation` matrix.

use jwc::{CommentPolicy, FormatOptions, Indentation, Node, Trivia, Value, from_str};

fn doc_with_comments() -> Node {
    // Build from parse to get realistic trivia placement.
    from_str(
        r#"{
            // lead
            "a": /* inline */ 1,
            "b": 2 // trail
        }"#,
    )
    .unwrap()
}

#[test]
fn keep_policy_preserves_all_comments() {
    let node = doc_with_comments();
    let out = node.to_formatted_string(FormatOptions {
        indentation: Indentation::Spaces(2),
        comment_policy: CommentPolicy::Keep,
    });
    assert!(out.contains("// lead"));
    assert!(out.contains("/* inline */"));
    assert!(out.contains("// trail"));
}

#[test]
fn remove_policy_strips_all_comments() {
    let node = doc_with_comments();
    let out = node.to_formatted_string(FormatOptions {
        indentation: Indentation::Spaces(2),
        comment_policy: CommentPolicy::Remove,
    });
    assert!(!out.contains("lead"));
    assert!(!out.contains("inline"));
    assert!(!out.contains("trail"));
    assert!(!out.contains("//"));
    assert!(!out.contains("/*"));
}

#[test]
fn compact_output_no_whitespace_between_tokens() {
    let node = Node::new(jwc::jwc!({ "a": 1, "b": [2, 3] }));
    let out = jwc::to_string(&node).unwrap();
    assert_eq!(out, r#"{"a":1,"b":[2,3]}"#);
}

#[test]
fn pretty_two_space() {
    let node = Node::new(jwc::jwc!({ "a": 1 }));
    let out = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    assert!(out.contains("  \"a\": 1"));
}

#[test]
fn pretty_tab_indent() {
    let node = Node::new(jwc::jwc!({ "a": 1 }));
    let out = jwc::to_string_pretty(&node, Some("\t")).unwrap();
    assert!(out.contains("\t\"a\": 1"));
}

#[test]
fn pretty_custom_arbitrary_indent() {
    let node = Node::new(jwc::jwc!({ "a": 1 }));
    let out = jwc::to_string_pretty(&node, Some("--> ")).unwrap();
    assert!(out.contains("--> \"a\": 1"));
}

#[test]
fn pretty_none_equals_compact() {
    let node = Node::new(jwc::jwc!({ "a": 1 }));
    let compact = jwc::to_string(&node).unwrap();
    let pretty_empty = jwc::to_string_pretty(&node, Some("")).unwrap();
    assert_eq!(compact, pretty_empty);
}

#[test]
fn escape_special_string_chars() {
    let node = Node::new(Value::from("\" \\ / \n \r \t"));
    let out = jwc::to_string(&node).unwrap();
    // The solidus `/` is emitted unescaped by JSON convention.
    assert!(out.contains(r#"\" "#) || out.contains("\\\""));
    assert!(out.contains("\\\\"));
    assert!(out.contains("\\n"));
    assert!(out.contains("\\r"));
    assert!(out.contains("\\t"));
}

#[test]
fn null_byte_escaped_as_u0000() {
    let node = Node::new(Value::from("a\0b"));
    let out = jwc::to_string(&node).unwrap();
    assert!(out.contains("\\u0000"));
}

#[test]
fn trivia_rendered_with_markers() {
    let node = Node::new(Value::from(1))
        .with_comment(Trivia::line(" lead"))
        .with_comment(Trivia::block(" blk "));
    let out = node.to_formatted_string(FormatOptions {
        indentation: Indentation::Spaces(2),
        comment_policy: CommentPolicy::Keep,
    });
    assert!(out.contains("// lead"));
    assert!(out.contains("/* blk */"));
}

#[test]
fn empty_containers_minimal_output() {
    assert_eq!(jwc::to_string(&Node::new(jwc::jwc!([]))).unwrap(), "[]");
    assert_eq!(jwc::to_string(&Node::new(jwc::jwc!({}))).unwrap(), "{}");
}

#[test]
fn display_alternate_is_pretty() {
    let node = Node::new(jwc::jwc!({ "a": 1 }));
    let compact = format!("{node}");
    let pretty = format!("{node:#}");
    assert_eq!(compact, r#"{"a":1}"#);
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("    \"a\": 1")); // 4-space default
}
