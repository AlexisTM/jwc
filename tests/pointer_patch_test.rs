use jwc::{Node, PatchOperation, Value, single_pass_parser};

fn parse(input: &str) -> Value {
    let mut parser = single_pass_parser::SinglePassParser::new(input);
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

#[test]
fn node_pointer_keeps_comments_and_shares_the_value_walk() {
    let mut node =
        jwc::from_str("{\n  \"a\": [\n    // first\n    1,\n    {\"b~c/d\": 2}\n  ]\n}").unwrap();
    assert_eq!(
        node.pointer("/a/0").unwrap().trivia,
        vec![jwc::Trivia::LineComment(" first".into())]
    );
    assert_eq!(
        node.pointer("/a/1/b~0c~1d").unwrap().value,
        jwc::Value::from(2)
    );
    assert_eq!(node.pointer("").map(|n| &n.value), Some(&node.value));
    assert!(node.pointer("a").is_none());
    assert!(node.pointer("/a/x").is_none());
    assert!(node.pointer("/a/0/b").is_none());
    assert_eq!(
        node.value.pointer("/a/1/b~0c~1d"),
        Some(&jwc::Value::from(2))
    );

    node.pointer_mut("/a/0")
        .unwrap()
        .add_trailing_comment(" one");
    *node.value.pointer_mut("/a/1/b~0c~1d").unwrap() = jwc::Value::from(3);
    assert_eq!(
        jwc::to_string(&node).unwrap(),
        "{\"a\":[// first\n1,// one\n{\"b~c/d\":3}]}"
    );
}
