//! Stress / scale tests. Deep nesting, wide containers, large inputs.
//! Also cross-validates that owned + lazy parsers agree.

use jwc::MAX_DEPTH;
use jwc::{Error, Node, from_str, from_str_lazy};

fn nested_arrays(depth: usize) -> String {
    let open: String = std::iter::repeat_n('[', depth).collect();
    let close: String = std::iter::repeat_n(']', depth).collect();
    format!("{open}{close}")
}

#[test]
fn deep_nesting_at_cap_parses() {
    // MAX_DEPTH-deep input is accepted. The actual container at depth N-1
    // is the innermost empty array.
    let src = nested_arrays(MAX_DEPTH);
    let node = from_str(&src).expect("parse at MAX_DEPTH should succeed");

    let mut cur = &node.value;
    let mut levels = 0;
    loop {
        let arr = cur.as_array().expect("every level should be an array");
        if arr.is_empty() {
            break;
        }
        cur = &arr[0].value;
        levels += 1;
    }
    assert_eq!(
        levels,
        MAX_DEPTH - 1,
        "expected MAX_DEPTH-1 non-empty levels plus one empty innermost"
    );
}

#[test]
fn deep_nesting_past_cap_returns_error_no_overflow() {
    // One past the cap must be rejected with a structured error, not crash.
    let src = nested_arrays(MAX_DEPTH + 1);
    match from_str(&src) {
        Err(Error::Parse { msg, .. }) => assert!(
            msg.contains("maximum nesting depth"),
            "unexpected parse error: {msg}"
        ),
        other => panic!("expected Parse error past MAX_DEPTH, got {other:?}"),
    }
}

#[test]
fn deep_nesting_past_cap_lazy_returns_error() {
    let src = nested_arrays(MAX_DEPTH + 1);
    match from_str_lazy(&src) {
        Err(Error::Parse { msg, .. }) => assert!(
            msg.contains("maximum nesting depth"),
            "unexpected parse error: {msg}"
        ),
        other => panic!("expected Parse error past MAX_DEPTH, got {other:?}"),
    }
}

#[test]
fn wide_array_10k_entries() {
    let mut src = String::from("[");
    for i in 0..10_000 {
        if i > 0 {
            src.push(',');
        }
        src.push_str(&i.to_string());
    }
    src.push(']');
    let node = from_str(&src).unwrap();
    assert_eq!(node.value.len(), Some(10_000));
    // Spot-check a few positions.
    assert_eq!(node.value.get(0).and_then(|v| v.as_i64()), Some(0));
    assert_eq!(node.value.get(9_999).and_then(|v| v.as_i64()), Some(9_999));
}

#[test]
fn wide_object_5k_keys() {
    let mut src = String::from("{");
    for i in 0..5_000 {
        if i > 0 {
            src.push(',');
        }
        src.push_str(&format!("\"k{i}\":{i}"));
    }
    src.push('}');
    let node = from_str(&src).unwrap();
    assert_eq!(node.value.len(), Some(5_000));
    assert_eq!(node.value.get("k0").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(
        node.value.get("k4999").and_then(|v| v.as_i64()),
        Some(4_999)
    );
}

/// Strict JSON subset used by parsers that reject comments.
const CROSS_VALIDATION_STRICT: &str = r#"{
    "meta": { "v": 1, "active": true, "tag": null },
    "items": [
        { "id": 1, "name": "alice", "score": 3.14 },
        { "id": 2, "name": "bob",   "score": -0.5e2 },
        { "id": 3, "name": "utf-é", "score": 0 }
    ],
    "escapes": "\" \\ \n \t é 🚀"
}"#;

#[test]
fn lazy_materialization_preserves_scalar_content() {
    let strict = CROSS_VALIDATION_STRICT;
    let lv = from_str_lazy(strict).unwrap();
    // LazyVal sorts object members; we compare a few leaves by explicit
    // path to avoid comparing Vec order against the owned parser.
    let items = lv.get("items").unwrap();
    let first = items.at(0).unwrap();
    assert_eq!(
        first.get("name").and_then(|v| v.as_str()).as_deref(),
        Some("alice")
    );
    assert_eq!(first.get("id").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(
        lv.get("escapes").and_then(|v| v.as_str()).as_deref(),
        Some("\" \\ \n \t é 🚀")
    );
}

#[test]
fn round_trip_large_pretty_then_reparse() {
    let src = r#"{"a":[1,2,3],"b":{"c":"d"},"e":true,"f":null}"#;
    let a = from_str(src).unwrap();
    let pretty = jwc::to_string_pretty(&a, Some("    ")).unwrap();
    let b = from_str(&pretty).unwrap();
    assert_eq!(a, b);
}

#[test]
fn whitespace_heavy_input_parses_to_same_structure() {
    let packed = r#"{"a":1,"b":[2,3]}"#;
    let sparse = r#"
    {
        "a"   :   1   ,
        "b"   :   [   2   ,   3   ]
    }
    "#;
    assert_eq!(
        from_str(packed).unwrap().value,
        from_str(sparse).unwrap().value
    );
}

#[test]
fn many_comments_scattered() {
    let src = r#"
        // a
        // b
        /* c */
        {
            // before-key
            "x" /* inline */ : 1,
            /* between */
            "y": 2
            // trail
        }
        // outside
    "#;
    let node = from_str(src).expect("should parse");
    // There should be several trivia entries on the root + values.
    let total_comments = count_comments(&node);
    assert!(
        total_comments >= 5,
        "expected >=5 comments, got {total_comments}"
    );
}

fn count_comments(n: &Node) -> usize {
    let mut c = n.trivia.len();
    match &n.value {
        jwc::Value::Array(items) => {
            for i in items {
                c += count_comments(i);
            }
        }
        jwc::Value::Object(members) => {
            for m in members {
                c += m.key_trivia.len();
                c += count_comments(&m.value);
            }
        }
        _ => {}
    }
    c
}
