use jwc::{JwcDeserializable, JwcSerializable, Node, Trivia, Value};
use std::collections::HashMap;

#[test]
fn test_ast_trivia_display() {
    let lc = Trivia::LineComment(" test".to_string());
    assert_eq!(lc.to_string(), "// test");
    let bc = Trivia::BlockComment(" test ".to_string());
    assert_eq!(bc.to_string(), "/* test */");
}

#[test]
fn test_ast_from_string() {
    let v: Value = String::from("hello").into();
    assert_eq!(v, Value::String("hello".to_string()));
}

#[test]
fn test_ast_node_with_comment_builder() {
    let n = Node::new(Value::from(true))
        .with_comment(Trivia::line(" test1"))
        .with_comment(Trivia::line(" test2"));
    assert_eq!(n.trivia.len(), 2);
}

#[test]
fn test_ast_trivia_predicates_and_text() {
    let lc = Trivia::line(" test");
    assert!(lc.is_line());
    assert!(!lc.is_block());
    assert_eq!(lc.text(), " test");

    let bc = Trivia::block(" test ");
    assert!(bc.is_block());
    assert!(!bc.is_line());
    assert_eq!(bc.text(), " test ");
}

#[test]
fn test_ast_value_insert_error() {
    let mut v = Value::from(true);
    let res = v.insert("key", Node::new(Value::from(false)));
    assert!(res.is_err());
}

#[test]
fn test_lib_to_string_pretty_fallback() {
    let n = Node::new(Value::from(true));
    // Provide a non-whitespace indent to hit the fallback
    let s = jwc::to_string_pretty(&n, Some("xyz")).unwrap();
    assert_eq!(s, "true");
}

#[test]
fn test_traits_errors() {
    assert!(bool::from_jwc(Value::Null).is_err());
    assert!(i32::from_jwc(Value::Null).is_err());
    assert!(<()>::from_jwc(Value::Bool(true)).is_err());
    assert!(String::from_jwc(Value::Null).is_err());
    assert!(Vec::<i32>::from_jwc(Value::Null).is_err());
    assert!(HashMap::<String, i32>::from_jwc(Value::Null).is_err());
}

#[test]
fn test_traits_str_to_jwc() {
    let s = "hello";
    assert_eq!(s.to_jwc(), Value::String("hello".to_string()));
}
