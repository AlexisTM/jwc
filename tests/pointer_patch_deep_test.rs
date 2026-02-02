//! RFC 6901 pointer and RFC 6902 patch behavior — including escape semantics,
//! structured error propagation, and round-trip semantics.

use jwc::{Error, PatchOperation, Value, jwc};

// ---------- Pointer ----------

#[test]
fn pointer_root_and_deep() {
    let v = jwc!({ "a": { "b": [10, 20, 30] }, "x": null });

    assert_eq!(v.pointer("").unwrap(), &v);
    assert_eq!(v.pointer("/a/b/0"), Some(&Value::from(10)));
    assert_eq!(v.pointer("/a/b/2"), Some(&Value::from(30)));
    assert_eq!(v.pointer("/x"), Some(&Value::Null));
    assert_eq!(v.pointer("/a/b/99"), None);
    assert_eq!(v.pointer("/missing"), None);
}

#[test]
fn pointer_escapes_slash_and_tilde() {
    let v = jwc!({
        "a/b": 1,
        "c~d": 2,
    });
    // `/` → `~1`, `~` → `~0`
    assert_eq!(v.pointer("/a~1b"), Some(&Value::from(1)));
    assert_eq!(v.pointer("/c~0d"), Some(&Value::from(2)));
}

#[test]
fn pointer_requires_leading_slash() {
    let v = jwc!({ "a": 1 });
    assert_eq!(v.pointer("a"), None);
}

#[test]
fn pointer_mut_mutates_in_place() {
    let mut v = jwc!({ "a": [1, 2, 3] });
    if let Some(n) = v.pointer_mut("/a/1") {
        *n = Value::from(99);
    }
    assert_eq!(v.pointer("/a/1"), Some(&Value::from(99)));
}

// ---------- Patch: add / remove / replace ----------

#[test]
fn patch_add_to_object() {
    let mut v = jwc!({ "a": 1 });
    v.apply_patch(vec![PatchOperation::Add {
        path: "/b".into(),
        value: Value::from(2),
    }])
    .unwrap();
    assert_eq!(v["b"].as_i64(), Some(2));
}

#[test]
fn patch_add_replaces_existing_key() {
    let mut v = jwc!({ "a": 1 });
    v.apply_patch(vec![PatchOperation::Add {
        path: "/a".into(),
        value: Value::from(99),
    }])
    .unwrap();
    assert_eq!(v["a"].as_i64(), Some(99));
}

#[test]
fn patch_add_to_array_at_index_and_dash() {
    let mut v = jwc!({ "arr": [10, 20, 30] });

    // Insert at index 1
    v.apply_patch(vec![PatchOperation::Add {
        path: "/arr/1".into(),
        value: Value::from(15),
    }])
    .unwrap();
    assert_eq!(v["arr"][1].as_i64(), Some(15));
    assert_eq!(v["arr"].len(), Some(4));

    // Append via "-"
    v.apply_patch(vec![PatchOperation::Add {
        path: "/arr/-".into(),
        value: Value::from(40),
    }])
    .unwrap();
    assert_eq!(v["arr"][v["arr"].len().unwrap() - 1].as_i64(), Some(40));
}

#[test]
fn patch_remove_from_object_and_array() {
    let mut v = jwc!({ "a": 1, "b": 2, "arr": [10, 20, 30] });

    v.apply_patch(vec![PatchOperation::Remove { path: "/a".into() }])
        .unwrap();
    assert!(v.get("a").is_none());

    v.apply_patch(vec![PatchOperation::Remove {
        path: "/arr/1".into(),
    }])
    .unwrap();
    assert_eq!(v["arr"].len(), Some(2));
    assert_eq!(v["arr"][0].as_i64(), Some(10));
    assert_eq!(v["arr"][1].as_i64(), Some(30));
}

#[test]
fn patch_replace_existing() {
    let mut v = jwc!({ "a": 1 });
    v.apply_patch(vec![PatchOperation::Replace {
        path: "/a".into(),
        value: Value::from("x"),
    }])
    .unwrap();
    assert_eq!(v["a"].as_str(), Some("x"));
}

#[test]
fn patch_move_and_copy() {
    let mut v = jwc!({ "a": 1, "b": 2 });
    v.apply_patch(vec![
        PatchOperation::Move {
            from: "/a".into(),
            path: "/c".into(),
        },
        PatchOperation::Copy {
            from: "/b".into(),
            path: "/d".into(),
        },
    ])
    .unwrap();
    assert!(v.get("a").is_none());
    assert_eq!(v["c"].as_i64(), Some(1));
    assert_eq!(v["b"].as_i64(), Some(2)); // copy doesn't remove src
    assert_eq!(v["d"].as_i64(), Some(2));
}

#[test]
fn patch_test_success_and_failure() {
    let mut v = jwc!({ "a": 1 });
    v.apply_patch(vec![PatchOperation::Test {
        path: "/a".into(),
        value: Value::from(1),
    }])
    .unwrap();

    let err = v
        .apply_patch(vec![PatchOperation::Test {
            path: "/a".into(),
            value: Value::from(2),
        }])
        .unwrap_err();
    match err {
        Error::Patch { path, reason } => {
            assert_eq!(path, "/a");
            assert!(reason.contains("test failed"));
        }
        other => panic!("expected Error::Patch, got {other:?}"),
    }
}

#[test]
fn patch_missing_path_reports_structurally() {
    let mut v = jwc!({ "a": 1 });
    let err = v
        .apply_patch(vec![PatchOperation::Replace {
            path: "/nope".into(),
            value: Value::from(2),
        }])
        .unwrap_err();
    match err {
        Error::Pointer { path, .. } => assert_eq!(path, "/nope"),
        other => panic!("expected Error::Pointer, got {other:?}"),
    }
}

#[test]
fn patch_invalid_array_index() {
    let mut v = jwc!({ "arr": [1, 2] });
    let err = v
        .apply_patch(vec![PatchOperation::Add {
            path: "/arr/notanindex".into(),
            value: Value::from(9),
        }])
        .unwrap_err();
    matches!(err, Error::Patch { .. });
}

// ---------- Lazy ----------

#[cfg(feature = "lazy")]
mod lazy {
    use jwc::{Error, LazyValue, Value};

    #[test]
    fn thaw_is_idempotent() {
        let mut v = LazyValue::unknown("42");
        let once = v.thaw().unwrap().clone();
        let twice = v.thaw().unwrap().clone();
        assert_eq!(once, twice);
        assert!(matches!(v, LazyValue::Parsed(_)));
    }

    #[test]
    fn typed_shape_enforcement() {
        let mut obj = LazyValue::unknown_object("{\"x\":1}");
        assert!(obj.thaw().is_ok());

        let mut arr_bad = LazyValue::unknown_object("[1,2]");
        let err = arr_bad.thaw().unwrap_err();
        assert!(
            matches!(err, Error::Type { expected, got, .. } if expected == "object" && got == "array")
        );
    }

    #[test]
    fn parse_as_concrete_type() {
        let mut v = LazyValue::unknown_vector("[1, 2, 3]");
        let nums: Vec<i32> = v.parse_as().unwrap();
        assert_eq!(nums, vec![1, 2, 3]);
    }

    #[test]
    fn lazy_value_renders_raw_source_if_unthawed() {
        let v = Value::from(LazyValue::unknown("{ /* keep */ \"x\": 1 }"));
        let rendered = v.to_string();
        assert!(rendered.contains("/* keep */"));
    }
}
