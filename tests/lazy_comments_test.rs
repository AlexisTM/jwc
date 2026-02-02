//! Verifies the lazy parser preserves JSONC comments as trivia on the
//! appropriate `LazyNode`, matching the owned parser's behavior.

use jwc::{Trivia, from_str_lazy};

#[test]
fn leading_line_comment_on_root() {
    let src = "// hello\n42";
    let n = from_str_lazy(src).expect("parse");
    assert_eq!(n.trivia.len(), 1, "root should have one trivia entry");
    assert_eq!(n.trivia[0], Trivia::LineComment(" hello".into()));
    assert_eq!(n.value.as_i64(), Some(42));
}

#[test]
fn block_comment_between_object_key_and_value() {
    let src = r#"{"x" /* note */ : 1}"#;
    let n = from_str_lazy(src).expect("parse");
    let obj = n.value.as_object().expect("object");
    assert_eq!(obj.len(), 1);
    let entry = &obj[0];
    assert_eq!(entry.key.as_ref(), "x");
    // The trivia between the key and the colon belongs to the value node
    // (matches the owned parser's single-trivia placement).
    assert!(
        entry
            .value
            .trivia
            .iter()
            .any(|t| matches!(t, Trivia::BlockComment(s) if s == " note ")),
        "expected block comment on value node, got {:?}",
        entry.value.trivia
    );
    assert_eq!(entry.value.as_i64(), Some(1));
}

#[test]
fn line_comment_before_array_element() {
    let src = "[\n  // first\n  1,\n  2\n]";
    let n = from_str_lazy(src).expect("parse");
    let arr = n.value.as_array().expect("array");
    assert_eq!(arr.len(), 2);
    assert!(
        arr[0]
            .trivia
            .iter()
            .any(|t| matches!(t, Trivia::LineComment(s) if s == " first")),
        "expected leading line comment on first element, got {:?}",
        arr[0].trivia
    );
}

#[test]
fn line_comment_before_object_key_becomes_key_trivia() {
    // A comment appearing before the key (not between key and `:`) lands on
    // `entry.key_trivia`, not on `entry.value.trivia`. Mirrors the owned
    // parser's placement.
    let src = "{\n  // pre-key\n  \"x\": 1\n}";
    let n = from_str_lazy(src).expect("parse");
    let obj = n.value.as_object().expect("object");
    assert_eq!(obj.len(), 1);
    let entry = &obj[0];
    assert_eq!(entry.key.as_ref(), "x");
    assert!(
        entry
            .key_trivia
            .iter()
            .any(|t| matches!(t, Trivia::LineComment(s) if s == " pre-key")),
        "expected pre-key line comment on key_trivia, got {:?}",
        entry.key_trivia
    );
    assert!(
        entry.value.trivia.is_empty(),
        "value trivia should be empty when comment is before the key, got {:?}",
        entry.value.trivia
    );
    assert_eq!(entry.value.as_i64(), Some(1));
}
