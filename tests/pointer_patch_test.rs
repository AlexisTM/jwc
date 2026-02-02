use jwc::{Node, PatchOperation, Value, parser};

fn parse(input: &str) -> Value {
    let mut parser = parser::Parser::new(input);
    let node: Node = parser.parse().unwrap();
    node.value
}

#[test]
fn test_json_pointer_rf6901() {
    let input = r#"
    {
      "foo": ["bar", "baz"],
      "": 0,
      "a/b": 1,
      "c%d": 2,
      "e^f": 3,
      "g|h": 4,
      "i\\j": 5,
      "k\"l": 6,
      " ": 7,
      "m~n": 8
   }
   "#;
    let val = parse(input);

    assert_eq!(val.pointer("").unwrap(), &val);
    assert_eq!(val.pointer("/foo").unwrap(), &parse("[\"bar\", \"baz\"]"));
    assert_eq!(
        val.pointer("/foo/0").unwrap(),
        &Value::String("bar".to_string())
    );
    assert_eq!(val.pointer("/").unwrap(), &Value::Number(0.into()));
    assert_eq!(val.pointer("/a~1b").unwrap(), &Value::Number(1.into()));
    assert_eq!(val.pointer("/c%d").unwrap(), &Value::Number(2.into()));
    assert_eq!(val.pointer("/e^f").unwrap(), &Value::Number(3.into()));
    assert_eq!(val.pointer("/g|h").unwrap(), &Value::Number(4.into()));
    assert_eq!(val.pointer("/i\\j").unwrap(), &Value::Number(5.into()));
    assert_eq!(val.pointer("/k\"l").unwrap(), &Value::Number(6.into()));
    assert_eq!(val.pointer("/ ").unwrap(), &Value::Number(7.into()));
    assert_eq!(val.pointer("/m~0n").unwrap(), &Value::Number(8.into()));
}

#[test]
fn test_patch_add() {
    let mut doc = parse(r#"{ "foo": "bar"}"#);
    let patch = vec![PatchOperation::Add {
        path: "/baz".to_string(),
        value: Value::String("qux".to_string()),
    }];
    doc.apply_patch(patch).unwrap();
    assert_eq!(
        doc.pointer("/baz").unwrap(),
        &Value::String("qux".to_string())
    );
}

#[test]
fn test_patch_remove() {
    let mut doc = parse(r#"{ "foo": "bar", "baz": "qux"}"#);
    let patch = vec![PatchOperation::Remove {
        path: "/baz".to_string(),
    }];
    doc.apply_patch(patch).unwrap();
    assert!(doc.pointer("/baz").is_none());
}

#[test]
fn test_patch_replace() {
    let mut doc = parse(r#"{ "foo": "bar"}"#);
    let patch = vec![PatchOperation::Replace {
        path: "/foo".to_string(),
        value: Value::String("baz".to_string()),
    }];
    doc.apply_patch(patch).unwrap();
    assert_eq!(
        doc.pointer("/foo").unwrap(),
        &Value::String("baz".to_string())
    );
}

#[test]
fn test_patch_move() {
    let mut doc = parse(r#"{ "foo": { "bar": "baz", "waldo": "fred" }, "qux": "corge" }"#);
    let patch = vec![PatchOperation::Move {
        from: "/foo/waldo".to_string(),
        path: "/qux".to_string(),
    }];
    doc.apply_patch(patch).unwrap();
    assert_eq!(
        doc.pointer("/qux").unwrap(),
        &Value::String("fred".to_string())
    );
    assert!(doc.pointer("/foo/waldo").is_none());
}

#[test]
fn test_patch_copy() {
    let mut doc = parse(r#"{ "foo": "bar" }"#);
    let patch = vec![PatchOperation::Copy {
        from: "/foo".to_string(),
        path: "/baz".to_string(),
    }];
    doc.apply_patch(patch).unwrap();
    assert_eq!(
        doc.pointer("/foo").unwrap(),
        &Value::String("bar".to_string())
    );
    assert_eq!(
        doc.pointer("/baz").unwrap(),
        &Value::String("bar".to_string())
    );
}

#[test]
fn test_patch_test() {
    let mut doc = parse(r#"{ "baz": "qux", "foo": [ "a", 2, "c" ] }"#);
    let patch = vec![
        PatchOperation::Test {
            path: "/baz".to_string(),
            value: Value::String("qux".to_string()),
        },
        PatchOperation::Test {
            path: "/foo/1".to_string(),
            value: Value::Number(2.into()),
        },
    ];
    assert!(doc.apply_patch(patch).is_ok());

    let fail_patch = vec![PatchOperation::Test {
        path: "/baz".to_string(),
        value: Value::String("bar".to_string()),
    }];
    assert!(doc.apply_patch(fail_patch).is_err());
}
