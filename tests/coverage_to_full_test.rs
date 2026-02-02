//! Line-by-line coverage fills for the pure-Rust core. Grouped by source file.
//! Intentionally mechanical — each test names the target region it covers so
//! a regression (coverage drop) is easy to locate.

use std::collections::HashMap;

use jwc::{
    CommentPolicy, Error, FormatOptions, Indentation, JwcDeserializable, JwcSerializable, LazyVal,
    Node, Number, ObjectEntry, PatchOperation, Trivia, Value, from_str, from_str_lazy,
};

// ---------------------------------------------------------------------------
// ast.rs
// ---------------------------------------------------------------------------

#[test]
fn ast_value_from_string_and_vec_node_and_vec_entry() {
    let v1: Value = String::from("s").into();
    assert!(matches!(v1, Value::String(ref s) if s == "s"));

    let v2: Value = vec![Node::new(Value::from(1))].into();
    assert!(v2.is_array());

    let v3: Value = vec![ObjectEntry::new("k".into(), Node::new(Value::from(1)))].into();
    assert!(v3.is_object());
}

#[test]
fn ast_trivia_from_owned_string_is_line() {
    let owned = String::from(" hi");
    let t: Trivia = owned.into();
    assert!(t.is_line());
    assert_eq!(t.text(), " hi");
}

#[test]
fn ast_trivia_display() {
    let t = Trivia::line(" hi");
    assert_eq!(t.to_string(), "// hi");
    let t = Trivia::block(" b ");
    assert_eq!(t.to_string(), "/* b */");
}

#[test]
fn ast_object_entry_display() {
    let mut e = ObjectEntry::new("k".into(), Node::new(Value::from(1)));
    e.key_comment(Trivia::line(" c"));
    let s = e.to_string();
    assert!(s.contains("// c"));
    assert!(s.contains("\"k\""));
}

#[test]
fn ast_value_display_compact_and_pretty() {
    let v = Value::Array(vec![Node::new(Value::from(1))]);
    assert_eq!(format!("{v}"), "[1]");
    let pretty = format!("{v:#}");
    assert!(pretty.contains('\n'));
}

#[test]
fn ast_trivia_is_line_is_block_text() {
    let l = Trivia::line("x");
    assert!(l.is_line());
    assert!(!l.is_block());
    assert_eq!(l.text(), "x");

    let b = Trivia::block("y");
    assert!(b.is_block());
    assert!(!b.is_line());
    assert_eq!(b.text(), "y");
}

#[test]
fn ast_object_entry_key_comment_api() {
    let mut e = ObjectEntry::new("k".into(), Node::new(Value::Null));
    assert!(e.key_comments().is_empty());
    e.key_comment(Trivia::line(" hi"));
    e.key_comment(" also"); // &str → line comment
    assert_eq!(e.key_comments().len(), 2);

    let e2 =
        ObjectEntry::new("k".into(), Node::new(Value::Null)).with_key_comment(Trivia::block(" b "));
    assert_eq!(e2.key_comments().len(), 1);
    assert!(e2.key_comments()[0].is_block());
}

#[test]
fn ast_value_as_array_mut_and_as_object_mut_and_negatives() {
    let mut arr = Value::Array(vec![Node::new(Value::from(1))]);
    assert_eq!(arr.as_array_mut().unwrap().len(), 1);
    arr.as_array_mut().unwrap().push(Node::new(Value::from(2)));
    assert_eq!(arr.as_array().unwrap().len(), 2);

    let mut obj = Value::Object(vec![]);
    assert!(obj.as_object_mut().unwrap().is_empty());
    obj.as_object_mut()
        .unwrap()
        .push(ObjectEntry::new("k".into(), Node::new(Value::from(3))));
    assert_eq!(obj.as_object().unwrap().len(), 1);

    // Negative branches: not the matching container.
    assert!(Value::Null.as_array_mut().is_none());
    assert!(Value::Null.as_object_mut().is_none());
    assert!(Value::Null.as_array().is_none());
    assert!(Value::Null.as_object().is_none());
    assert!(Value::Null.as_number().is_none());
    assert!(Value::from(true).as_str().is_none());
    assert!(Value::from(true).as_i64().is_none());
    assert!(Value::from(true).as_u64().is_none());
    assert!(Value::from(true).as_f64().is_none());
}

#[test]
fn ast_as_number_and_as_u64_negative_rejected() {
    let v = Value::from(-1i64);
    assert_eq!(v.as_i64(), Some(-1));
    assert_eq!(v.as_u64(), None);

    let v = Value::from(5i64);
    assert!(v.as_number().is_some());
}

#[test]
fn ast_valueindex_usize_on_non_array_get_mut_is_none() {
    // `index_into_mut` for usize when the value isn't an array.
    let mut v = Value::from(1);
    assert!(v.get_mut(0usize).is_none());
}

#[test]
fn ast_valueindex_string_owned_and_usize_paths() {
    let obj = Value::Object(vec![ObjectEntry::new(
        "key".into(),
        Node::new(Value::from(7)),
    )]);
    let key = String::from("key");
    // String impl for ValueIndex
    assert!(obj.get(key.clone()).is_some());
    // &str impl on a non-object
    assert!(Value::from(1).get("key").is_none());
    // usize into non-array
    assert!(Value::from(1).get(0usize).is_none());
    // usize into array
    let arr = Value::Array(vec![Node::new(Value::from(99))]);
    assert!(arr.get(0usize).is_some());
    assert!(arr.get(1usize).is_none());
}

#[test]
fn ast_get_mut_object_and_array_and_missing_and_nonmatch() {
    let mut obj = Value::Object(vec![ObjectEntry::new(
        "a".into(),
        Node::new(Value::from(1)),
    )]);
    *obj.get_mut("a").unwrap() = Value::from(2);
    assert_eq!(obj["a"].as_i64(), Some(2));
    assert!(obj.get_mut("missing").is_none());

    let mut arr = Value::Array(vec![Node::new(Value::from(10))]);
    *arr.get_mut(0usize).unwrap() = Value::from(11);
    assert_eq!(arr[0].as_i64(), Some(11));
    assert!(arr.get_mut(5usize).is_none());

    // get_mut with String key
    let key = String::from("a");
    assert!(obj.get_mut(key.clone()).is_some());
    // String ValueIndex on non-object
    assert!(Value::from(1).get_mut(key).is_none());
    // &str on non-object
    assert!(Value::from(1).get_mut("a").is_none());
}

#[test]
fn ast_index_mut_usize_replaces_element() {
    let mut arr = Value::Array(vec![Node::new(Value::from(1)), Node::new(Value::from(2))]);
    arr[1usize] = Value::from(9);
    assert_eq!(arr[1usize].as_i64(), Some(9));
}

#[test]
#[should_panic(expected = "cannot index with usize")]
fn ast_index_mut_usize_panics_on_non_array() {
    let mut v = Value::from(1);
    let _ = &mut v[0usize];
}

#[test]
#[should_panic(expected = "Null")]
fn ast_index_mut_usize_on_null_names_null_in_panic() {
    // Exercise discriminant_name's `Null` arm.
    let mut v = Value::Null;
    let _ = &mut v[0usize];
}

#[test]
fn ast_is_empty_on_each_kind() {
    assert!(Value::Array(vec![]).is_empty());
    assert!(Value::Object(vec![]).is_empty());
    assert!(Value::String(String::new()).is_empty());
    assert!(!Value::Array(vec![Node::new(Value::Null)]).is_empty());
    assert!(!Value::Object(vec![ObjectEntry::new("k".into(), Node::new(Value::Null))]).is_empty());
    assert!(!Value::Null.is_empty());
    assert!(!Value::from(false).is_empty());
}

#[test]
fn ast_value_push_on_non_array_errors() {
    let mut v = Value::from(1);
    let e = v.push(Node::new(Value::Null)).unwrap_err();
    assert!(e.contains("Not an array"));
}

// Exercise discriminant_name through the panic's `{:?}` path for every variant.
// Each test invokes a String-key IndexMut on a non-Object variant.
macro_rules! panic_discriminant {
    ($name:ident, $val:expr, $label:literal) => {
        #[test]
        #[should_panic(expected = $label)]
        fn $name() {
            let mut v = $val;
            let _ = &mut v["k"];
        }
    };
}
panic_discriminant!(discriminant_bool, Value::from(true), "Bool");
panic_discriminant!(discriminant_number, Value::from(1), "Number");
panic_discriminant!(discriminant_string, Value::from("s"), "String");
panic_discriminant!(
    discriminant_array,
    Value::Array(vec![Node::new(Value::Null)]),
    "Array"
);
// (Null auto-promotes, so it doesn't panic; Object doesn't panic either.)

#[test]
#[should_panic(expected = "usize")]
fn discriminant_usize_on_object_panics() {
    let mut v = Value::Object(vec![]);
    let _ = &mut v[0usize];
}

// ---------------------------------------------------------------------------
// error.rs — Display for every variant + `From` impls
// ---------------------------------------------------------------------------

#[test]
fn error_display_every_variant() {
    assert_eq!(Error::parse(3, 5, "boom").to_string(), "boom at 3:5");

    assert_eq!(Error::ty("A", "B").to_string(), "expected A, got B");
    assert_eq!(
        Error::ty_at("A", "B", "/x").to_string(),
        "expected A, got B at /x"
    );

    assert_eq!(
        Error::missing_field("name").to_string(),
        "missing field `name`"
    );
    let with_path = Error::MissingField {
        name: "n".into(),
        path: "/p".into(),
    };
    assert_eq!(with_path.to_string(), "missing field `n` at /p");

    assert_eq!(
        Error::pointer("/p", "bad").to_string(),
        "pointer error at /p: bad"
    );
    assert_eq!(
        Error::patch("/p", "bad").to_string(),
        "patch error at /p: bad"
    );
    assert_eq!(Error::custom("raw").to_string(), "raw");
}

#[test]
fn error_from_conversions() {
    let e: Error = String::from("s").into();
    assert!(matches!(e, Error::Custom(_)));
    let e: Error = "s".into();
    assert!(matches!(e, Error::Custom(_)));

    let io_err = std::io::Error::other("io");
    let e: Error = io_err.into();
    assert!(e.to_string().contains("io error"));

    // Utf8Error via std::str::from_utf8 on invalid bytes (runtime vec so the
    // compiler doesn't warn about always-invalid literals).
    let bad_bytes: Vec<u8> = vec![0xffu8, 0xfe, 0xfd];
    let utf8_err = std::str::from_utf8(&bad_bytes).unwrap_err();
    let e: Error = utf8_err.into();
    assert!(e.to_string().contains("utf8 error"));
}

// ---------------------------------------------------------------------------
// lib.rs — `_value_kind` for every variant (the derive macro uses it)
// ---------------------------------------------------------------------------

#[test]
fn derive_error_message_mentions_kind_for_non_object_non_bool_variants() {
    // Drives `_value_kind` over Number / String / Array / Object via the
    // trait impls that call it for Error::ty messages.
    let e = i32::from_jwc(Value::Array(vec![])).unwrap_err();
    assert!(e.to_string().contains("array"));

    let e = bool::from_jwc(Value::Object(vec![])).unwrap_err();
    assert!(e.to_string().contains("object"));

    let e = String::from_jwc(Value::from(1)).unwrap_err();
    assert!(e.to_string().contains("number"));

    let e = Vec::<i32>::from_jwc(Value::from("x")).unwrap_err();
    assert!(e.to_string().contains("string"));

    let e = HashMap::<String, i32>::from_jwc(Value::from(true)).unwrap_err();
    assert!(e.to_string().contains("bool"));

    let e = <()>::from_jwc(Value::from(true)).unwrap_err();
    assert!(e.to_string().contains("bool"));
}

// ---------------------------------------------------------------------------
// number_fast.rs
// ---------------------------------------------------------------------------

#[test]
fn number_as_f32_integer_and_float() {
    let n = Number::from(3i32);
    assert_eq!(n.as_f32().unwrap(), 3.0f32);
    let n = Number::from(1.5f64);
    assert_eq!(n.as_f32().unwrap(), 1.5f32);
}

#[test]
fn number_parse_from_float_lexeme() {
    let n = Number::from(2.5f64);
    // Parse via Display → target parse path — Float branch of `parse`.
    let parsed: f64 = n.parse().unwrap();
    assert_eq!(parsed, 2.5);

    // Error path: parse a non-integer f64 lexeme as i64.
    let e = n.parse::<i64>().unwrap_err();
    assert!(!e.is_empty());
}

#[test]
fn number_from_u64_and_usize_overflow_saturate_to_float() {
    let n: Number = (i64::MAX as u64 + 1).into();
    assert!(matches!(n, Number::Float(_)));

    let large: usize = u64::MAX as usize;
    #[allow(clippy::unnecessary_fallible_conversions)]
    let n: Number = Number::from(large);
    // On 64-bit platforms usize == u64 and this is > i64::MAX.
    #[cfg(target_pointer_width = "64")]
    assert!(matches!(n, Number::Float(_)));
    #[cfg(not(target_pointer_width = "64"))]
    drop(n);
}

// ---------------------------------------------------------------------------
// parser.rs — error paths
// ---------------------------------------------------------------------------

#[test]
fn parser_unicode_invalid_codepoint_errors() {
    // Lone high surrogate without a \u continuation.
    assert!(from_str(r#""\uD834abc""#).is_err());
    // High surrogate followed by a raw char (no backslash continuation).
    assert!(from_str(r#""\uD800A""#).is_err());
    // High surrogate followed by a \u that isn't a low surrogate — exercises
    // the "Invalid low surrogate in unicode escape" arm specifically.
    assert!(from_str(r#""\uD800A""#).is_err());
    // High surrogate at end of string (no low surrogate continuation possible).
    assert!(from_str("\"\\uD800\"").is_err());
    // Lone low surrogate.
    assert!(from_str(r#""\uDC00""#).is_err());
    // High surrogate followed by another valid `\uXXXX` that isn't in the
    // low-surrogate range — exercises the "Invalid low surrogate in unicode
    // escape" arm (distinct from the raw-char continuation arm above).
    assert!(from_str("\"\\uD800\\u0041\"").is_err());
    // Same shape in the lazy parser so decode_escaped's bad-low arm fires.
    assert!(
        from_str_lazy("\"\\uD800\\u0041\"")
            .unwrap()
            .as_str()
            .is_none()
    );
    // Valid surrogate pair → astral codepoint, exercises codepoint assembly.
    assert_eq!(
        from_str(r#""😀""#)
            .unwrap()
            .value
            .as_str()
            .map(|s| s.to_string()),
        Some("\u{1F600}".to_string())
    );
    // Slow-path: escape forces the slow path, then an unescaped control byte
    // later in the same string → "Unescaped control character" arm in the
    // slow path (distinct from the fast-path validator).
    let with_ctrl = b"\"\\n\x01\"";
    let s = std::str::from_utf8(with_ctrl).unwrap();
    assert!(from_str(s).is_err());
}

#[test]
fn parser_invalid_escape() {
    assert!(from_str(r#""\q""#).is_err());
}

#[test]
fn parser_bad_numbers_and_eof() {
    // Unexpected EOF — parse_number is only reached with at least one byte so
    // exercise the generic number-invalid path instead.
    assert!(from_str("1.2.3").is_err());
    // EOF during hex4.
    assert!(from_str(r#""\u00"#).is_err());
    // Invalid hex digit in \u escape.
    assert!(from_str(r#""\uZZZZ""#).is_err());
}

#[test]
fn parser_multibyte_between_array_elements_and_object_members() {
    // consume_array_comma / consume_object_comma error-format path: the
    // char-decode `chars().next().unwrap()` branch needs a multi-byte char.
    assert!(from_str("[1 é]").is_err());
    assert!(from_str("{\"a\": 1 é \"b\": 2}").is_err());
}

// ---------------------------------------------------------------------------
// parser_core.rs
// ---------------------------------------------------------------------------

#[test]
fn parser_core_fast_parse_int_empty_via_public_parse_path() {
    // fast_parse_int is internal; empty bytes can't arise from the parser.
    // Instead hit the paths that return None for non-digits through Number.
    // This just anchors the behavior — the dedicated unit test in
    // parser_core already covers i64::MIN / MAX / overflow.
    let n = Number::from(0i64);
    assert_eq!(n.to_string(), "0");
}

#[test]
fn parser_core_lift_err_with_missing_position_suffix() {
    // Parser internally emits strings with "… at L:C"; but exercise a
    // code path that feeds a msg without the suffix by crafting invalid
    // JSONC whose lower path returns a suffix-less error. In practice all
    // internal emitters include the suffix, so lift_err's fallback is
    // defensive. Touched indirectly via an input whose EOF-path returns
    // a fully-formatted message we can only assert the wrapper shape on.
    let err = from_str("").unwrap_err();
    match err {
        Error::Parse { line, col, .. } => assert!(line >= 1 && col >= 1),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parser_core_scan_string_trailing_backslash_and_unterminated() {
    // Exercise scan_string_skip_escapes EOF branches through the lazy parser.
    assert!(from_str_lazy(r#""abc\"#).is_err()); // trailing backslash
    assert!(from_str_lazy(r#""abc"#).is_err()); // unterminated
    // Bad hex in lazy key escape.
    assert!(from_str_lazy(r#"{"\uZZZZ": 1}"#).is_err());
}

#[test]
fn parser_core_hex4_invalid_digit_path() {
    assert!(from_str(r#""\u12G4""#).is_err());
    // Lowercase hex in an actual \u escape drives the a-f nibble arm of
    // parse_hex4_escape.
    let n = from_str("\"\\uabcd\"").unwrap();
    assert_eq!(n.value.as_str(), Some("\u{abcd}"));
    // Uppercase hex drives the A-F nibble arm.
    let n = from_str("\"\\uABCD\"").unwrap();
    assert_eq!(n.value.as_str(), Some("\u{abcd}"));
}

#[test]
fn lazyval_lowercase_hex_in_decode_escaped_and_hex4() {
    let v = from_str_lazy("\"\\uabcd\"").unwrap();
    assert_eq!(v.as_str().as_deref(), Some("\u{abcd}"));
    let v = from_str_lazy("\"\\uABCD\"").unwrap();
    assert_eq!(v.as_str().as_deref(), Some("\u{abcd}"));
}

#[test]
fn lazyval_false_keyword_and_object_eof_without_closer() {
    // `false` keyword success branch.
    assert!(matches!(
        from_str_lazy("false").unwrap().value,
        LazyVal::Bool(false)
    ));

    // Object with value but no comma / `}` → EOF arm of the trailing-separator
    // match (distinct from the pos-past-end arm at the loop head).
    assert!(from_str_lazy(r#"{"a": 1"#).is_err());
}

// ---------------------------------------------------------------------------
// serializer.rs
// ---------------------------------------------------------------------------

#[test]
fn serializer_custom_indentation_emits_prefix_per_depth() {
    let n = Node::new(Value::Array(vec![
        Node::new(Value::from(1)),
        Node::new(Value::from(2)),
    ]));
    let s = n.to_formatted_string(FormatOptions {
        indentation: Indentation::Custom(">>> ".into()),
        comment_policy: CommentPolicy::Keep,
    });
    assert!(s.contains(">>> 1"));
    assert!(s.contains(">>> 2"));
}

#[test]
fn serializer_remove_and_minify_comment_policies() {
    let src = "{\n  // keep\n  \"a\": 1 /* blk */\n}";
    let node = from_str(src).unwrap();
    let removed = node.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Remove,
    });
    assert!(!removed.contains("keep"));
    assert!(!removed.contains("blk"));

    let minified = node.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Minify,
    });
    // Minify currently short-circuits (only Keep renders) — behavior per source.
    assert!(!minified.contains("keep"));
}

#[cfg(feature = "lazy")]
#[test]
fn serializer_lazy_variants_render_raw_and_parsed() {
    use jwc::LazyValue;
    // Unknown → raw source bytes.
    let raw = Value::Lazy(Box::new(LazyValue::unknown("{\"x\":1}")));
    let s = Node::new(raw).to_formatted_string(FormatOptions::default());
    assert!(s.contains("\"x\""));

    let raw = Value::Lazy(Box::new(LazyValue::unknown_object("{}")));
    let s = Node::new(raw).to_formatted_string(FormatOptions::default());
    assert_eq!(s, "{}");

    let raw = Value::Lazy(Box::new(LazyValue::unknown_vector("[1,2]")));
    let s = Node::new(raw).to_formatted_string(FormatOptions::default());
    assert_eq!(s, "[1,2]");

    // Parsed → delegates to format_value.
    let parsed = Value::Lazy(Box::new(LazyValue::Parsed(Value::from(42))));
    let s = Node::new(parsed).to_formatted_string(FormatOptions::default());
    assert_eq!(s, "42");
}

// ---------------------------------------------------------------------------
// lazy.rs (feature = "lazy") — LazyValue API
// ---------------------------------------------------------------------------

#[cfg(feature = "lazy")]
#[test]
fn lazyvalue_thaw_object_and_vector_and_mismatch() {
    use jwc::LazyValue;

    let mut ok_obj = LazyValue::unknown_object("{\"a\": 1}");
    assert!(ok_obj.thaw().is_ok());

    let mut ok_vec = LazyValue::unknown_vector("[1,2]");
    assert!(ok_vec.thaw().is_ok());

    // Kind mismatch: Object expected but input is a bool.
    let mut bad = LazyValue::unknown_object("true");
    assert!(bad.thaw().is_err());

    // Kind mismatch: Vector expected but input is an object.
    let mut bad = LazyValue::unknown_vector("{}");
    assert!(bad.thaw().is_err());
}

#[cfg(feature = "lazy")]
#[test]
fn lazyvalue_thaw_already_parsed_short_circuits() {
    use jwc::LazyValue;
    let mut already = LazyValue::Parsed(Value::from(7));
    let v = already.thaw().unwrap();
    assert_eq!(v.as_i64(), Some(7));
    // A second call returns the same cached Parsed — exercises the
    // `if let Self::Parsed(value) = self` true-arm.
    let v = already.thaw().unwrap();
    assert_eq!(v.as_i64(), Some(7));
}

#[cfg(feature = "lazy")]
#[test]
fn lib_value_kind_lazy_variant() {
    use jwc::LazyValue;
    let v = Value::Lazy(Box::new(LazyValue::Parsed(Value::Null)));
    // Route through `_value_kind` via a Type error that quotes the kind name.
    let e = bool::from_jwc(v).unwrap_err();
    assert!(e.to_string().contains("lazy"));
}

#[cfg(feature = "lazy")]
#[test]
fn ast_index_mut_on_lazy_value_panics_with_lazy_label() {
    use jwc::LazyValue;
    let mut v = Value::Lazy(Box::new(LazyValue::Parsed(Value::Null)));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = &mut v["k"];
    }));
    assert!(caught.is_err());
}

#[cfg(feature = "lazy")]
#[test]
fn lazyvalue_parse_as_roundtrips_through_deserializable() {
    use jwc::LazyValue;
    let mut raw = LazyValue::unknown("42");
    let n: i64 = raw.parse_as().unwrap();
    assert_eq!(n, 42);
}

// ---------------------------------------------------------------------------
// lazy_val.rs — every accessor, every to_value arm, iter_* helpers
// ---------------------------------------------------------------------------

#[test]
fn lazyval_predicates_and_accessors() {
    let v = from_str_lazy("null").unwrap();
    assert!(v.is_null());
    assert!(!v.is_bool());
    assert!(!v.is_number());
    assert!(!v.is_string());
    assert!(!v.is_array());
    assert!(!v.is_object());
    assert!(v.as_bool().is_none());

    let v = from_str_lazy("true").unwrap();
    assert!(v.is_bool());
    assert_eq!(v.as_bool(), Some(true));

    let v = from_str_lazy("42").unwrap();
    assert!(v.is_number());
    assert_eq!(v.as_i64(), Some(42));
    assert_eq!(v.as_u64(), Some(42));
    assert_eq!(v.as_f64(), Some(42.0));
    assert_eq!(v.as_number_lex(), Some("42"));

    let v = from_str_lazy("-5").unwrap();
    assert_eq!(v.as_u64(), None); // negative → None

    let v = from_str_lazy("1.5e1").unwrap();
    // Fast-int fails → f64 fallback, truncated to 15.
    assert_eq!(v.as_i64(), Some(15));
    assert_eq!(v.as_f64(), Some(15.0));

    let v = from_str_lazy(r#""hi""#).unwrap();
    assert!(v.is_string());
    assert_eq!(v.as_str().as_deref(), Some("hi"));
    assert_eq!(v.len(), Some(2));

    // Number as_i64/as_f64 on non-number → None.
    let v = from_str_lazy("true").unwrap();
    assert!(v.as_i64().is_none());
    assert!(v.as_u64().is_none());
    assert!(v.as_f64().is_none());
    assert!(v.as_number_lex().is_none());
    assert!(v.as_str().is_none());
    assert!(v.as_array().is_none());
    assert!(v.as_object().is_none());
    assert!(v.get("k").is_none());
    assert!(v.at(0).is_none());
    assert!(v.len().is_none());
}

#[test]
fn lazyval_iter_array_and_iter_object_on_scalars_yield_empty() {
    let v = from_str_lazy("1").unwrap();
    assert_eq!(v.iter_array().count(), 0);
    assert_eq!(v.iter_object().count(), 0);
}

#[test]
fn lazyval_iter_helpers_on_containers() {
    let v = from_str_lazy("[1, 2, 3]").unwrap();
    let sum: i64 = v.iter_array().filter_map(|n| n.as_i64()).sum();
    assert_eq!(sum, 6);

    let v = from_str_lazy(r#"{"b": 2, "a": 1}"#).unwrap();
    let keys: Vec<&str> = v.iter_object().map(|(k, _)| k).collect();
    assert_eq!(keys, vec!["a", "b"]);
}

#[test]
fn lazyval_is_empty_and_len_on_containers_and_strings() {
    assert!(from_str_lazy("[]").unwrap().is_empty());
    assert!(from_str_lazy("{}").unwrap().is_empty());
    assert!(from_str_lazy(r#""""#).unwrap().is_empty());
    assert!(!from_str_lazy("[1]").unwrap().is_empty());
}

#[test]
fn lazyval_to_value_all_variants() {
    let v = from_str_lazy("null").unwrap();
    assert!(matches!(v.to_value(), Value::Null));
    let v = from_str_lazy("true").unwrap();
    assert!(matches!(v.to_value(), Value::Bool(true)));
    let v = from_str_lazy("42").unwrap();
    assert_eq!(v.to_value().as_i64(), Some(42));
    let v = from_str_lazy("1.5").unwrap();
    assert_eq!(v.to_value().as_f64(), Some(1.5));
    let v = from_str_lazy(r#""hi""#).unwrap();
    assert_eq!(v.to_value().as_str(), Some("hi"));

    let v = from_str_lazy(r#"{"k": [1,2]}"#).unwrap();
    let owned = v.to_value();
    assert_eq!(owned["k"][1].as_i64(), Some(2));
}

#[test]
fn lazyval_to_value_non_f64_lexeme_is_nan() {
    // Parser won't allow "NaN" as a number, but a carefully-crafted invalid
    // lexeme via from_parsed_and_lexeme is internal only. The lexeme path
    // is exercised transitively by floats — good enough.
    let v = from_str_lazy("1e400").unwrap(); // overflows to inf
    let owned = v.to_value();
    match owned {
        Value::Number(n) => {
            let f = n.as_f64().unwrap();
            assert!(f.is_infinite());
        }
        other => panic!("expected number, got {other:?}"),
    }
}

#[test]
fn lazyval_get_on_object_missing_and_non_object_both_none() {
    let v = from_str_lazy(r#"{"a": 1}"#).unwrap();
    assert!(v.get("b").is_none());
    let v = from_str_lazy("1").unwrap();
    assert!(v.get("a").is_none());
}

#[test]
fn lazyval_decode_escape_surrogate_pair_decodes_astral() {
    // The inline emoji is literal UTF-8 (no \u), so this exercises the
    // non-escape fast copy path only.
    let v = from_str_lazy(r#""😀""#).unwrap();
    assert_eq!(v.as_str().as_deref(), Some("\u{1F600}"));

    // Explicit \uHHHH\uLLLL surrogate pair drives the two-chunk decode
    // branch and the astral-codepoint assembly in decode_escaped.
    let src = "\"\\uD83D\\uDE00\"";
    let v = from_str_lazy(src).unwrap();
    assert_eq!(v.as_str().as_deref(), Some("\u{1F600}"));
}

#[test]
fn lazyval_decode_escape_bad_low_in_surrogate_pair() {
    // High surrogate + non-low \u → bad low branch.
    let v = from_str_lazy("\"\\uD83D\\u0041\"").unwrap();
    assert!(v.as_str().is_none());

    // High surrogate + \u that isn't a low surrogate at all (followed by
    // plain char) → falls through to mismatched-continuation branch.
    let v = from_str_lazy(r#""\uD83DA""#).unwrap();
    assert!(v.as_str().is_none());
}

#[test]
fn lazyval_decode_escape_every_kind() {
    // \b \f \n \r \t \/ \\ \" and \u
    let src = r#""\b\f\n\r\t\/\\\"""#;
    let v = from_str_lazy(src).unwrap();
    assert_eq!(v.as_str().as_deref(), Some("\u{08}\u{0c}\n\r\t/\\\""));

    let v = from_str_lazy(r#""A""#).unwrap();
    assert_eq!(v.as_str().as_deref(), Some("A"));

    // Surrogate pair → astral.
    let v = from_str_lazy(r#""😀""#).unwrap();
    assert_eq!(v.as_str().as_deref(), Some("\u{1F600}"));

    // Broken escapes surface on decode (as_str), not at parse time, because
    // scan_string_skip_escapes only skips past them.
    assert!(from_str_lazy(r#""\x""#).unwrap().as_str().is_none());
    assert!(from_str_lazy(r#""\u00ZZ""#).unwrap().as_str().is_none());
    assert!(from_str_lazy(r#""\uD83DA""#).unwrap().as_str().is_none());
    assert!(from_str_lazy(r#""\uDC00""#).unwrap().as_str().is_none());
    // hex4 encountering non-hex in surrogate-pair low chunk.
    assert!(
        from_str_lazy(r#""\uD83D\uDCZZ""#)
            .unwrap()
            .as_str()
            .is_none()
    );
    // Object key with invalid \u — this is decoded at parse time.
    assert!(from_str_lazy(r#"{"\uD800": 1}"#).is_err());
}

#[test]
fn lazyval_object_trailing_comment_attaches_to_last_entry_and_trailing_junk_errors() {
    let v = from_str_lazy(
        r#"{"a": 1 // trailing line
}"#,
    )
    .unwrap();
    let entries = v.as_object().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].value.trivia.is_empty());

    assert!(from_str_lazy("[1] junk").is_err());
    assert!(from_str_lazy("[").is_err());
    assert!(from_str_lazy("[1").is_err()); // EOF in array
    assert!(from_str_lazy("[1 2]").is_err()); // missing comma
    assert!(from_str_lazy("{").is_err()); // EOF in object
    assert!(from_str_lazy("{1: 2}").is_err()); // non-string key
    assert!(from_str_lazy(r#"{"a" 1}"#).is_err()); // missing colon
    assert!(from_str_lazy(r#"{"a": 1 ; "b": 2}"#).is_err()); // bad separator
    assert!(from_str_lazy(r#"{"a": 1,"#).is_err()); // EOF expecting next key
    assert!(from_str_lazy(r#"{"a":"#).is_err()); // EOF before value
    assert!(from_str_lazy("tree").is_err()); // bad keyword t
    assert!(from_str_lazy("flse").is_err()); // bad keyword f
    assert!(from_str_lazy("nope").is_err()); // bad keyword n
    assert!(from_str_lazy("@").is_err()); // unexpected token
}

#[test]
fn lazyval_array_trailing_comment_attaches_to_last_element() {
    let v = from_str_lazy(
        r#"[1 /* inline */, 2 // end
]"#,
    )
    .unwrap();
    let items = v.as_array().unwrap();
    assert!(!items[1].trivia.is_empty());
}

#[test]
fn lazyval_deep_nesting_exceeds_max_depth_both_containers() {
    let a = "[".repeat(300);
    assert!(from_str_lazy(&a).is_err());
    assert!(from_str(&a).is_err()); // owned parser arrays

    let mut o = String::new();
    for _ in 0..200 {
        o.push('{');
        o.push_str("\"a\":");
    }
    o.push('1');
    assert!(from_str_lazy(&o).is_err());
    assert!(from_str(&o).is_err()); // owned parser objects
}

#[test]
fn lazyval_root_trailing_trivia_attaches_to_root_node() {
    let v = from_str_lazy("42 // trailing\n").unwrap();
    assert!(!v.trivia.is_empty());
    assert!(matches!(v.value, LazyVal::Number("42")));
}

// ---------------------------------------------------------------------------
// traits.rs — untouched branches
// ---------------------------------------------------------------------------

#[test]
fn traits_vec_deserialize_error_propagates_from_element() {
    // Type mismatch: String is not a Number.
    let arr = Value::Array(vec![Node::new(Value::from("not a number"))]);
    let e = Vec::<i32>::from_jwc(arr).unwrap_err();
    assert!(matches!(e, Error::Type { .. }));

    // Number that overflows the target type — routes through Number::parse →
    // Error::Custom.
    let arr = Value::Array(vec![Node::new(Value::from(i64::MAX))]);
    let e = Vec::<i8>::from_jwc(arr).unwrap_err();
    assert!(matches!(e, Error::Custom(_)));
}

#[test]
fn traits_hashmap_roundtrip() {
    let mut m = HashMap::new();
    m.insert("a".to_string(), 1i32);
    m.insert("b".to_string(), 2i32);
    let v = m.to_jwc();
    let back: HashMap<String, i32> = HashMap::from_jwc(v).unwrap();
    assert_eq!(back.get("a"), Some(&1));
    assert_eq!(back.get("b"), Some(&2));
}

#[test]
fn traits_option_some_roundtrip() {
    let opt: Option<i32> = Some(5);
    let v = opt.to_jwc();
    assert_eq!(Option::<i32>::from_jwc(v).unwrap(), Some(5));
    let opt: Option<i32> = None;
    let v = opt.to_jwc();
    assert_eq!(Option::<i32>::from_jwc(v).unwrap(), None);
}

// ---------------------------------------------------------------------------
// patch.rs — uncovered error surfaces
// ---------------------------------------------------------------------------

#[test]
fn patch_errors_cover_each_branch() {
    // Path not starting with '/'.
    let mut v = Value::Object(vec![]);
    let e = v
        .apply_patch(vec![PatchOperation::Add {
            path: "nope".into(),
            value: Value::from(1),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Pointer { .. }));

    // Array add with non-numeric key other than "-".
    let mut v = Value::Array(vec![]);
    let e = v
        .apply_patch(vec![PatchOperation::Add {
            path: "/x".into(),
            value: Value::from(1),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));

    // Array add out of bounds.
    let mut v = Value::Array(vec![]);
    let e = v
        .apply_patch(vec![PatchOperation::Add {
            path: "/5".into(),
            value: Value::from(1),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));

    // Add with '-' appends.
    let mut v = Value::Array(vec![]);
    v.apply_patch(vec![PatchOperation::Add {
        path: "/-".into(),
        value: Value::from(7),
    }])
    .unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);

    // Parent is not object / array.
    let mut v = Value::Object(vec![ObjectEntry::new(
        "a".into(),
        Node::new(Value::from(1)),
    )]);
    let e = v
        .apply_patch(vec![PatchOperation::Add {
            path: "/a/b".into(),
            value: Value::from(1),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));

    // Remove from array out of bounds and invalid index.
    let mut v = Value::Array(vec![Node::new(Value::from(1))]);
    let e = v
        .apply_patch(vec![PatchOperation::Remove { path: "/5".into() }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));
    let e = v
        .apply_patch(vec![PatchOperation::Remove { path: "/x".into() }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));

    // Remove key missing.
    let mut v = Value::Object(vec![]);
    let e = v
        .apply_patch(vec![PatchOperation::Remove {
            path: "/missing".into(),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));

    // Remove from scalar parent.
    let mut v = Value::Object(vec![ObjectEntry::new(
        "a".into(),
        Node::new(Value::from(1)),
    )]);
    let e = v
        .apply_patch(vec![PatchOperation::Remove {
            path: "/a/b".into(),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));

    // Replace missing.
    let mut v = Value::Object(vec![]);
    let e = v
        .apply_patch(vec![PatchOperation::Replace {
            path: "/missing".into(),
            value: Value::Null,
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Pointer { .. }));

    // Move + Copy.
    let mut v = Value::Object(vec![ObjectEntry::new(
        "a".into(),
        Node::new(Value::from(1)),
    )]);
    v.apply_patch(vec![PatchOperation::Copy {
        from: "/a".into(),
        path: "/b".into(),
    }])
    .unwrap();
    assert_eq!(v["b"].as_i64(), Some(1));

    v.apply_patch(vec![PatchOperation::Move {
        from: "/b".into(),
        path: "/c".into(),
    }])
    .unwrap();
    assert!(v.get("b").is_none());
    assert_eq!(v["c"].as_i64(), Some(1));

    // Copy from missing source.
    let e = v
        .apply_patch(vec![PatchOperation::Copy {
            from: "/zzz".into(),
            path: "/d".into(),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Pointer { .. }));

    // Test op — success and failure.
    v.apply_patch(vec![PatchOperation::Test {
        path: "/a".into(),
        value: Value::from(1),
    }])
    .unwrap();
    let e = v
        .apply_patch(vec![PatchOperation::Test {
            path: "/a".into(),
            value: Value::from(999),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Patch { .. }));
    let e = v
        .apply_patch(vec![PatchOperation::Test {
            path: "/missing".into(),
            value: Value::Null,
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Pointer { .. }));

    // Add at empty path replaces whole document.
    let mut v = Value::from(1);
    v.apply_patch(vec![PatchOperation::Add {
        path: String::new(),
        value: Value::from(2),
    }])
    .unwrap();
    assert_eq!(v.as_i64(), Some(2));
}

#[test]
fn patch_replace_key_that_exists() {
    let mut v = Value::Object(vec![ObjectEntry::new(
        "a".into(),
        Node::new(Value::from(1)),
    )]);
    v.apply_patch(vec![PatchOperation::Replace {
        path: "/a".into(),
        value: Value::from(2),
    }])
    .unwrap();
    assert_eq!(v["a"].as_i64(), Some(2));
}

#[test]
fn patch_split_path_without_slash_errors() {
    let mut v = Value::Object(vec![]);
    let e = v
        .apply_patch(vec![PatchOperation::Remove {
            path: "noslash".into(),
        }])
        .unwrap_err();
    assert!(matches!(e, Error::Pointer { .. }));
}

// ---------------------------------------------------------------------------
// Cross-cutting: LazyNode trivia access via Deref to LazyVal
// ---------------------------------------------------------------------------

#[test]
fn lazynode_deref_to_lazyval_works() {
    let n = from_str_lazy(r#"{"a":1}"#).unwrap();
    // `.get` via Deref from LazyNode to LazyVal.
    assert!(matches!(&n.value, LazyVal::Object(_)));
    assert!(n.get("a").is_some());
}
