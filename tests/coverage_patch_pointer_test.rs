use jwc::{Node, PatchOperation, Value};

#[test]
fn test_pointer_errors() {
    let mut v = Value::from(42);
    assert_eq!(v.pointer("invalid"), None);
    assert_eq!(v.pointer_mut("invalid"), None);

    let mut arr = Value::Array(vec![Node::new(Value::from(1))]);
    assert_eq!(arr.pointer("/1"), None); // out of bounds
    assert_eq!(arr.pointer("/invalid"), None); // parse error
    assert_eq!(arr.pointer_mut("/1"), None);
    assert_eq!(arr.pointer_mut("/invalid"), None);

    let mut obj = Value::Object(vec![]);
    assert_eq!(obj.pointer_mut("/missing"), None);

    let mut num = Value::from(1);
    assert_eq!(num.pointer("/any"), None);
    assert_eq!(num.pointer_mut("/any"), None);
}

#[test]
fn test_patch_add_root() {
    let mut v = Value::from(1);
    v.apply_patch(vec![PatchOperation::Add {
        path: String::new(),
        value: Value::from(2),
    }])
    .unwrap();
    assert_eq!(v, Value::from(2));
}

#[test]
fn test_patch_add_array() {
    let mut v = Value::Array(vec![]);
    // push
    v.apply_patch(vec![PatchOperation::Add {
        path: "/-".to_string(),
        value: Value::from(1),
    }])
    .unwrap();
    assert_eq!(v.pointer("/0"), Some(&Value::from(1)));

    // insert
    v.apply_patch(vec![PatchOperation::Add {
        path: "/0".to_string(),
        value: Value::from(2),
    }])
    .unwrap();
    assert_eq!(v.pointer("/0"), Some(&Value::from(2)));
    assert_eq!(v.pointer("/1"), Some(&Value::from(1)));

    // err index bounds
    assert!(
        v.apply_patch(vec![PatchOperation::Add {
            path: "/5".to_string(),
            value: Value::from(3),
        }])
        .is_err()
    );

    // err parse index
    assert!(
        v.apply_patch(vec![PatchOperation::Add {
            path: "/invalid".to_string(),
            value: Value::from(3),
        }])
        .is_err()
    );
}

#[test]
fn test_patch_parent_errors() {
    let mut v = Value::from(1);
    // Directly call patch_add to hit parent error on non-obj/arr
    assert!(
        v.apply_patch(vec![PatchOperation::Add {
            path: "/foo".to_string(),
            value: Value::from(1),
        }])
        .is_err()
    );
}

#[test]
fn test_pointer_mut_bounds() {
    let mut arr = Value::Array(vec![]);
    assert_eq!(arr.pointer_mut("/0"), None);
    assert_eq!(arr.pointer_mut("/invalid"), None);
}

#[test]
fn test_patch_remove_errors() {
    let mut obj = Value::Object(vec![]);
    assert!(
        obj.apply_patch(vec![PatchOperation::Remove {
            path: "/foo".to_string(),
        }])
        .is_err()
    );

    let mut arr = Value::Array(vec![Node::new(Value::from(1))]);
    assert!(
        arr.apply_patch(vec![PatchOperation::Remove {
            path: "/1".to_string(),
        }])
        .is_err()
    );
    assert!(
        arr.apply_patch(vec![PatchOperation::Remove {
            path: "/invalid".to_string(),
        }])
        .is_err()
    );

    let mut num = Value::from(1);
    assert!(
        num.apply_patch(vec![PatchOperation::Remove {
            path: "/test".to_string(),
        }])
        .is_err()
    );
}

#[test]
fn test_patch_remove_array() {
    let mut arr = Value::Array(vec![Node::new(Value::from(1))]);
    arr.apply_patch(vec![PatchOperation::Remove {
        path: "/0".to_string(),
    }])
    .unwrap();
    assert_eq!(arr, Value::Array(vec![]));
}

#[test]
fn test_patch_invalid_path_split() {
    let mut v = Value::from(1);
    assert!(
        v.apply_patch(vec![PatchOperation::Add {
            path: "noshash".to_string(),
            value: Value::from(2),
        }])
        .is_err()
    );
}
