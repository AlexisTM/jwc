use jwc::{Value, jwc};

#[test]
fn test_scalars() {
    assert_eq!(jwc!(null), Value::Null);
    assert_eq!(jwc!(true), Value::Bool(true));
    assert_eq!(jwc!(false), Value::Bool(false));
    assert_eq!(jwc!(42), Value::from(42));
    assert_eq!(jwc!(1.25_f64), Value::from(1.25_f64));
    assert_eq!(jwc!("hello"), Value::from("hello"));
}

#[test]
fn test_array() {
    let v = jwc!([1, 2, 3]);
    assert!(v.is_array());
    assert_eq!(v.len(), Some(3));
    assert_eq!(v[0].as_i64(), Some(1));
    assert_eq!(v[2].as_i64(), Some(3));

    let empty = jwc!([]);
    assert!(empty.is_array());
    assert_eq!(empty.len(), Some(0));
}

#[test]
fn test_object() {
    let v = jwc!({
        "port": 8080,
        "enabled": true,
        "name": "jwc",
    });
    assert!(v.is_object());
    assert_eq!(v["port"].as_i64(), Some(8080));
    assert_eq!(v["enabled"].as_bool(), Some(true));
    assert_eq!(v["name"].as_str(), Some("jwc"));
    assert!(v["missing"].is_null());

    let empty = jwc!({});
    assert!(empty.is_object());
    assert_eq!(empty.len(), Some(0));
}

#[test]
fn test_nested() {
    let v = jwc!({
        "outer": {
            "inner": [1, 2, 3],
            "flag": false,
        },
        "maybe": null,
    });
    assert_eq!(v["outer"]["inner"][1].as_i64(), Some(2));
    assert_eq!(v["outer"]["flag"].as_bool(), Some(false));
    assert!(v["maybe"].is_null());
}

#[test]
fn test_index_mut_auto_insert() {
    let mut v = Value::Null;
    v["name"] = Value::from("jwc");
    v["port"] = Value::from(8080);
    assert_eq!(v["name"].as_str(), Some("jwc"));
    assert_eq!(v["port"].as_i64(), Some(8080));
}
