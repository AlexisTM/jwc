//! Untrusted input: depth, duplicate keys, number grammar, whitespace.

use jwc::{DuplicateKeys, ErrorKind, ParseOptions, Value};

fn nested(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

#[test]
fn nesting_is_capped_instead_of_overflowing_the_stack() {
    assert!(jwc::from_str(&nested(128)).is_ok());
    let err = jwc::from_str(&nested(129)).unwrap_err();
    assert_eq!(err.kind, ErrorKind::DepthExceeded(128));
    assert_eq!((err.line, err.column), (1, 129));
    assert!(jwc::from_str(&nested(500_000)).is_err());

    let shallow = ParseOptions {
        max_depth: 2,
        ..ParseOptions::default()
    };
    assert!(jwc::from_str_with("[[1]]", shallow).is_ok());
    assert!(jwc::from_str_with("[[[1]]]", shallow).is_err());
    assert!(jwc::from_str_with("{\"a\": {\"b\": {}}}", shallow).is_err());
}

#[test]
fn duplicate_keys_are_rejected_unless_allowed() {
    let err = jwc::from_str("{\n  \"a\": 1,\n  \"a\": 2\n}").unwrap_err();
    assert_eq!(err.kind, ErrorKind::DuplicateKey("a".into()));
    assert_eq!((err.line, err.column), (3, 3));
    assert_eq!(err.to_string(), "duplicate key \"a\" at 3:3");

    // Different objects may reuse a key.
    assert!(jwc::from_str("[{\"a\": 1}, {\"a\": 2}]").is_ok());

    let allow = ParseOptions {
        duplicate_keys: DuplicateKeys::Allow,
        ..ParseOptions::default()
    };
    let node = jwc::from_str_with("{\"a\": 1, \"a\": 2}", allow).unwrap();
    let Value::Object(members) = &node.value else {
        panic!()
    };
    assert_eq!(members.len(), 2);
    assert_eq!(node.value.pointer("/a"), Some(&Value::from(1)));
}

#[test]
fn number_grammar_is_json() {
    for bad in [
        "01", "1.", ".5", "+1", "1e", "1e+", "-", "0x10", "1_000", "Infinity", "NaN",
    ] {
        let err = jwc::from_str(bad).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::Number(_) | ErrorKind::Syntax(_)),
            "{bad}: {err}"
        );
    }
    let err = jwc::from_str("{\"a\": 1e400}").unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Number(ref m) if m.contains("out of range")));
    assert_eq!((err.line, err.column), (1, 7));
}

#[test]
fn integers_are_exact_and_lexemes_survive() {
    let node = jwc::from_str(
        "[9007199254740993, -9223372036854775808, 18446744073709551615, \
         123456789012345678901234567890, 1.0, 1e3, -0, 0.10]",
    )
    .unwrap();
    let Value::Array(items) = &node.value else {
        panic!()
    };
    let n = |i: usize| match &items[i].value {
        Value::Number(n) => n,
        other => panic!("{other:?}"),
    };
    assert_eq!(n(0).as_i64(), Some(9_007_199_254_740_993));
    assert_eq!(n(1).as_i64(), Some(i64::MIN));
    assert_eq!(n(2).as_u64(), Some(u64::MAX));
    assert_eq!(n(2).as_i64(), None);
    assert!(n(3).is_f64() && n(3).as_str() == Some("123456789012345678901234567890"));
    assert!(n(4).is_f64() && !n(4).is_i64());
    assert!((n(5).as_f64() - 1000.0).abs() < f64::EPSILON);
    assert!(n(6).is_u64(), "-0 is the integer zero");
    assert!((n(7).parse::<f64>().unwrap() - 0.1).abs() < f64::EPSILON);
    assert!(n(7).parse::<i32>().is_err());

    assert_eq!(
        jwc::to_string(&node).unwrap(),
        "[9007199254740993,-9223372036854775808,18446744073709551615,\
         123456789012345678901234567890,1.0,1e3,-0,0.10]"
    );
}

#[test]
fn constructed_floats_stay_floats() {
    let node = jwc::Node::new(Value::Array(vec![
        jwc::Node::new(Value::from(1.0)),
        jwc::Node::new(Value::from(-0.0)),
        jwc::Node::new(Value::from(1e21)),
        jwc::Node::new(Value::from(f64::INFINITY)),
        jwc::Node::new(Value::from(-3)),
        jwc::Node::new(Value::from(7u64)),
    ]));
    assert_eq!(jwc::to_string(&node).unwrap(), "[1.0,-0.0,1e21,null,-3,7]");
    assert_ne!(Value::from(1), Value::from(1.0));
}

#[test]
fn only_json_whitespace_and_a_leading_bom_are_accepted() {
    assert!(jwc::from_str("\u{feff}{\"a\": 1}").is_ok());
    assert!(jwc::from_str(" \t\r\n{}\r\n").is_ok());
    let err = jwc::from_str("{\u{a0}\"a\": 1}").unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Syntax(_)));
    assert!(jwc::from_str("\u{2028}").is_err());
}

#[test]
fn errors_carry_positions_and_messages() {
    let err = jwc::from_str("{\n  \"a\": tru\n}").unwrap_err();
    assert_eq!(err.to_string(), "Unexpected identifier at 2:8");
    let err = jwc::from_str("{\"a\": \"x\u{1}\"}").unwrap_err();
    assert!(err.to_string().starts_with("Unescaped control character"));
    let err = jwc::from_str("[1 2]").unwrap_err();
    assert_eq!(
        err.to_string(),
        "Expected ',' or ']' after array element, found '2' at 1:4"
    );
    assert!(!jwc::Error::type_mismatch("x").is_positional());
    assert!(jwc::to_string_pretty(&jwc::from_str("1").unwrap(), Some(" \t")).is_err());
}

#[test]
fn object_entry_display_is_valid_jsonc() {
    let mut entry = jwc::ObjectEntry::new("key".to_string(), jwc::Node::new(Value::from(42)));
    entry.add_key_comment(" lead ");
    entry.add_key_block_comment(" trail ");
    let text = format!("{entry}");
    assert_eq!(text, "// lead \n/* trail */\"key\":42");
    assert!(jwc::from_str(&format!("{{{text}}}")).is_ok());
}
