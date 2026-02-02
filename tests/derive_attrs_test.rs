//! Tests for `#[jwc(...)]` field attributes on the derive macros.

use jwc::{Error, JwcDeserializable, JwcSerializable, Value};
use jwcc_derive::{JwcDeserializable, JwcSerializable};

#[test]
fn rename_changes_json_key() {
    #[derive(JwcSerializable, JwcDeserializable, Debug, PartialEq)]
    struct Config {
        #[jwc(rename = "log-level")]
        log_level: String,
    }

    let c = Config {
        log_level: "debug".into(),
    };
    let v = c.to_jwc();
    let s = Value::String(String::new()); // just to ensure type
    let _ = s;
    let out = jwc::to_string(&jwc::Node::new(v.clone())).unwrap();
    assert!(
        out.contains("\"log-level\":\"debug\""),
        "expected renamed key in output, got: {out}"
    );
    assert!(!out.contains("log_level"));

    // Round-trip
    let parsed = Config::from_jwc(v).unwrap();
    assert_eq!(parsed, c);
}

#[test]
fn default_fills_missing_field() {
    #[derive(JwcDeserializable, Debug, PartialEq, Default)]
    struct Settings {
        name: String,
        #[jwc(default)]
        retries: u32,
    }

    // Missing `retries` uses Default.
    let v = jwc::from_str(r#"{"name":"svc"}"#).unwrap().value;
    let s = Settings::from_jwc(v).unwrap();
    assert_eq!(
        s,
        Settings {
            name: "svc".into(),
            retries: 0
        }
    );

    // Present `retries` overrides default.
    let v = jwc::from_str(r#"{"name":"svc","retries":5}"#)
        .unwrap()
        .value;
    let s = Settings::from_jwc(v).unwrap();
    assert_eq!(s.retries, 5);
}

#[test]
fn missing_field_without_default_errors_structurally() {
    #[derive(JwcDeserializable, Debug)]
    #[allow(dead_code)]
    struct Strict {
        required: String,
    }

    let v = jwc::from_str(r#"{}"#).unwrap().value;
    let err = Strict::from_jwc(v).unwrap_err();
    match err {
        Error::MissingField { name, .. } => assert_eq!(name, "required"),
        other => panic!("expected MissingField, got {other:?}"),
    }
}

#[test]
fn wrong_shape_errors_with_type_error() {
    #[derive(JwcDeserializable, Debug)]
    #[allow(dead_code)]
    struct S {
        x: i32,
    }

    let err = S::from_jwc(Value::from("not an object")).unwrap_err();
    match err {
        Error::Type {
            expected,
            got,
            path,
        } => {
            assert_eq!(expected, "object");
            assert_eq!(got, "string");
            assert!(path.contains("S"));
        }
        other => panic!("expected Type, got {other:?}"),
    }
}

#[test]
fn skip_serializing_omits_field() {
    #[derive(JwcSerializable)]
    #[allow(dead_code)]
    struct Creds {
        user: String,
        #[jwc(skip_serializing)]
        password: String,
    }

    let c = Creds {
        user: "alice".into(),
        password: "secret".into(),
    };
    let out = jwc::to_string(&jwc::Node::new(c.to_jwc())).unwrap();
    assert!(out.contains("\"user\":\"alice\""));
    assert!(!out.contains("password"));
    assert!(!out.contains("secret"));
}

#[test]
fn skip_deserializing_uses_default() {
    #[derive(JwcDeserializable, Debug, PartialEq)]
    struct Partial {
        name: String,
        #[jwc(skip_deserializing)]
        cache: Vec<i32>,
    }

    let v = jwc::from_str(r#"{"name":"x","cache":[99]}"#).unwrap().value;
    let p = Partial::from_jwc(v).unwrap();
    // cache is ignored from input; defaults to empty.
    assert_eq!(
        p,
        Partial {
            name: "x".into(),
            cache: vec![]
        }
    );
}

#[test]
fn skip_both_directions() {
    #[derive(JwcSerializable, JwcDeserializable, Debug, PartialEq)]
    struct Compact {
        id: i32,
        #[jwc(skip)]
        transient: String,
    }

    let c = Compact {
        id: 7,
        transient: "ignored".into(),
    };
    let out = jwc::to_string(&jwc::Node::new(c.to_jwc())).unwrap();
    assert!(!out.contains("transient"));
    assert!(!out.contains("ignored"));

    // On deserialize, transient is absent but the struct still builds via Default.
    let v = jwc::from_str(r#"{"id": 7}"#).unwrap().value;
    let back = Compact::from_jwc(v).unwrap();
    assert_eq!(
        back,
        Compact {
            id: 7,
            transient: String::new()
        }
    );
}

#[test]
fn rename_combined_with_default() {
    #[derive(JwcDeserializable, Debug, PartialEq, Default)]
    struct Opts {
        #[jwc(rename = "max-retries", default)]
        max_retries: u32,
    }

    let v = jwc::from_str(r#"{}"#).unwrap().value;
    assert_eq!(Opts::from_jwc(v).unwrap(), Opts { max_retries: 0 });

    let v = jwc::from_str(r#"{"max-retries":9}"#).unwrap().value;
    assert_eq!(Opts::from_jwc(v).unwrap(), Opts { max_retries: 9 });
}
