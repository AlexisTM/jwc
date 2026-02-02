//! End-to-end tests for the public ergonomic API: accessors, Index, From
//! impls, Display formatting, structured errors, pretty-printer options.

use jwc::{Error, Node, Trivia, Value, jwc};

// ---------- Type predicates / accessors ----------

#[test]
fn predicates_exhaustive() {
    assert!(Value::Null.is_null());
    assert!(Value::Bool(true).is_bool());
    assert!(Value::from(42).is_number());
    assert!(Value::from("hi").is_string());
    assert!(jwc!([1, 2]).is_array());
    assert!(jwc!({ "a": 1 }).is_object());

    // Cross-checks: only one predicate true at a time.
    let v = Value::from(1);
    assert!(v.is_number());
    assert!(!v.is_null() && !v.is_bool() && !v.is_string() && !v.is_array() && !v.is_object());
}

#[test]
fn typed_accessors_happy_path() {
    assert_eq!(Value::Bool(true).as_bool(), Some(true));
    assert_eq!(Value::from("hello").as_str(), Some("hello"));
    assert_eq!(Value::from(42_i32).as_i64(), Some(42));
    assert_eq!(Value::from(42_u32).as_u64(), Some(42));
    assert_eq!(Value::from(1.25_f64).as_f64(), Some(1.25));
}

#[test]
fn typed_accessors_return_none_on_mismatch() {
    let s = Value::from("not a number");
    assert_eq!(s.as_i64(), None);
    assert_eq!(s.as_bool(), None);
    assert_eq!(s.as_f64(), None);
    assert_eq!(s.as_array(), None);
    assert_eq!(s.as_object(), None);
}

#[test]
fn get_and_len() {
    let v = jwc!({
        "port": 8080,
        "tags": ["a", "b", "c"],
    });
    assert_eq!(v.len(), Some(2));
    assert_eq!(v.get("port").and_then(Value::as_i64), Some(8080));
    assert_eq!(v.get("missing"), None);
    let tags = v.get("tags").unwrap();
    assert_eq!(tags.len(), Some(3));
    assert_eq!(tags.get(1).and_then(Value::as_str), Some("b"));
    assert!(tags.get(99).is_none());
}

#[test]
fn is_empty_only_for_containers_and_string() {
    assert!(jwc!([]).is_empty());
    assert!(jwc!({}).is_empty());
    assert!(Value::from("").is_empty());
    assert!(!Value::from("x").is_empty());
    assert!(!Value::from(0).is_empty()); // scalars have no len, is_empty=false
}

// ---------- Index read/write ----------

#[test]
fn index_read_missing_returns_null_no_panic() {
    let v = jwc!({ "a": 1 });
    assert!(v["missing"].is_null());
    assert!(v["a"]["deeply"]["nested"].is_null());
}

#[test]
fn index_read_chain() {
    let v = jwc!({ "outer": { "inner": [10, 20, 30] } });
    assert_eq!(v["outer"]["inner"][2].as_i64(), Some(30));
}

#[test]
fn index_mut_inserts_missing_keys() {
    let mut v = Value::Null;
    v["name"] = Value::from("jwc");
    v["count"] = Value::from(3);
    assert_eq!(v["name"].as_str(), Some("jwc"));
    assert_eq!(v["count"].as_i64(), Some(3));
    assert!(v.is_object());
}

#[test]
fn index_mut_overwrites_existing_key() {
    let mut v = jwc!({ "n": 1 });
    v["n"] = Value::from(99);
    assert_eq!(v["n"].as_i64(), Some(99));
}

#[test]
#[should_panic]
fn index_mut_array_out_of_bounds_panics() {
    let mut v = jwc!([1, 2, 3]);
    v[99] = Value::from(0);
}

// ---------- From impls ----------

#[test]
fn from_numerics_all_widths() {
    assert_eq!(Value::from(1_i8).as_i64(), Some(1));
    assert_eq!(Value::from(1_i16).as_i64(), Some(1));
    assert_eq!(Value::from(1_i64).as_i64(), Some(1));
    assert_eq!(Value::from(1_isize).as_i64(), Some(1));
    assert_eq!(Value::from(1_u8).as_u64(), Some(1));
    assert_eq!(Value::from(1_u16).as_u64(), Some(1));
    assert_eq!(Value::from(1_u64).as_u64(), Some(1));
    assert_eq!(Value::from(1_usize).as_u64(), Some(1));
    assert_eq!(Value::from(1.5_f32).as_f64(), Some(1.5));
}

#[test]
fn from_option_maps_none_to_null() {
    let some: Value = Some(42_i32).into();
    let none: Value = Option::<i32>::None.into();
    assert_eq!(some.as_i64(), Some(42));
    assert!(none.is_null());
}

#[test]
fn node_from_any_into_value() {
    let n: Node = 42.into();
    assert_eq!(n.value.as_i64(), Some(42));
    let n: Node = "hi".into();
    assert_eq!(n.value.as_str(), Some("hi"));
}

// ---------- jwc! macro ----------

#[test]
fn macro_trailing_commas_allowed() {
    let v = jwc!([1, 2, 3,]);
    assert_eq!(v.len(), Some(3));
    let v = jwc!({ "a": 1, "b": 2, });
    assert_eq!(v.len(), Some(2));
}

#[test]
fn macro_dynamic_values() {
    let port = 9000;
    let name = "svc".to_string();
    let v = jwc!({
        "port": port,
        "name": name,
    });
    assert_eq!(v["port"].as_i64(), Some(9000));
    assert_eq!(v["name"].as_str(), Some("svc"));
}

// ---------- Display ----------

#[test]
fn display_compact_vs_pretty() {
    let v = jwc!({ "a": 1, "b": [2, 3] });
    let compact = format!("{v}");
    let pretty = format!("{v:#}");
    assert_eq!(compact, r#"{"a":1,"b":[2,3]}"#);
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("    \"a\": 1")); // 4-space indent
}

// ---------- Trivia ergonomics ----------

#[test]
fn trivia_builder_chain_reads_back() {
    let n = Node::new(Value::from(1))
        .with_comment(Trivia::line(" lead"))
        .with_comment(Trivia::block(" mid "))
        .with_comment(" tail-via-str");
    assert_eq!(n.comments().len(), 3);
    assert!(n.comments()[0].is_line());
    assert!(n.comments()[1].is_block());
    assert_eq!(n.comments()[1].text(), " mid ");
    assert!(n.comments()[2].is_line()); // &str implies line
}

#[test]
fn trivia_content_is_verbatim_no_stripping() {
    // Passing `//text` should NOT auto-strip the slashes — serializer does its own.
    let t = Trivia::line("//hi");
    assert_eq!(t.text(), "//hi");
    let rendered = format!("{t}");
    assert_eq!(rendered, "////hi"); // `//` added to literal `//hi`
}

// ---------- Structured errors ----------

#[test]
fn parse_error_is_structured_with_position() {
    let err = jwc::from_str("\n\n\"unterminated").unwrap_err();
    match err {
        Error::Parse { line, col, msg } => {
            assert_eq!(line, 3);
            assert!(col >= 1);
            assert!(!msg.contains(" at ")); // position stripped into fields
        }
        other => panic!("expected Parse, got {other:?}"),
    }
}

#[test]
fn type_error_carries_expected_and_got() {
    let v = Value::from("not a number");
    let err = <i64 as jwc::JwcDeserializable>::from_jwc(v).unwrap_err();
    match err {
        Error::Type { expected, got, .. } => {
            assert_eq!(expected, "i64");
            assert_eq!(got, "string");
        }
        other => panic!("expected Type, got {other:?}"),
    }
}

#[test]
fn error_display_is_useful() {
    let e = Error::parse(3, 7, "unexpected char");
    assert_eq!(format!("{e}"), "unexpected char at 3:7");

    let e = Error::missing_field("port");
    assert_eq!(format!("{e}"), "missing field `port`");

    let e = Error::ty("bool", "number");
    assert_eq!(format!("{e}"), "expected bool, got number");
}

// ---------- Pretty indent options ----------

#[test]
fn pretty_custom_indent() {
    let n = Node::new(jwc!({ "a": 1 }));
    let out = jwc::to_string_pretty(&n, Some(">>> ")).unwrap();
    assert!(out.contains(">>> \"a\": 1"));
}

#[test]
fn pretty_empty_indent_is_compact() {
    let n = Node::new(jwc!({ "a": 1 }));
    let out = jwc::to_string_pretty(&n, Some("")).unwrap();
    assert_eq!(out, r#"{"a":1}"#);
}

#[test]
fn pretty_tab_indent() {
    let n = Node::new(jwc!({ "a": 1 }));
    let out = jwc::to_string_pretty(&n, Some("\t")).unwrap();
    assert!(out.contains("\t\"a\": 1"));
}

// ---------- Round-trip ----------

#[test]
fn round_trip_preserves_comments_but_not_commas() {
    let src = r#"{
    // lead
    "a": 1,
    /* trail */
    "b": 2,
}"#;
    let node = jwc::from_str(src).unwrap();
    let out = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    // comments kept
    assert!(out.contains("// lead"));
    assert!(out.contains("/* trail */"));
    // trailing comma dropped (comma state removed)
    assert!(!out.trim_end().ends_with(",}"));
    assert!(!out.trim_end().ends_with(",\n}"));
}

#[test]
fn comments_survive_parse_serialize_reparse() {
    let src = "// top\n42";
    let a = jwc::from_str(src).unwrap();
    let serialized = jwc::to_string_pretty(&a, Some("  ")).unwrap();
    let b = jwc::from_str(&serialized).unwrap();
    assert_eq!(a, b);
}
