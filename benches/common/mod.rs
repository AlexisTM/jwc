//! Shared fixtures used by the split bench binaries. Each `benches/*.rs`
//! includes this module via `#[path = "common/mod.rs"] mod common;`.

#![allow(dead_code)]

pub const SMALL_JSON: &str = r#"
    {
        "string": "Hello, world!",
        "number": 12345,
        "float": 123.456,
        "bool_true": true,
        "bool_false": false,
        "null": null,
        "array": [1, 2, 3, 4, 5],
        "object": {
            "key1": "value1",
            "key2": "value2"
        },
        "nested": {
            "a": {
                "b": {
                    "c": "deep"
                }
            }
        }
    }
"#;

pub fn build_large_json() -> String {
    let mut s = String::from("{\n");
    for i in 0..64 {
        s.push_str(&format!(
            "    \"key_{i:03}\": \"lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua - entry {i}\",\n"
        ));
    }
    s.push_str("    \"tail\": 0\n}\n");
    s
}

pub fn build_nested_json() -> String {
    let mut s = String::new();
    s.push('[');
    for i in 0..256 {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{{\"k\":{i}}}"));
    }
    s.push(']');
    s
}

pub const ACCESS_JSON: &str = r#"{
    "id": 42,
    "name": "service-alpha",
    "port": 8080,
    "tls": true,
    "database": {
        "host": "db.internal",
        "port": 5432,
        "pool_size": 16,
        "ssl": false,
        "timeout_ms": 30000,
        "retries": 3
    },
    "tags": ["web", "rust", "production", "v2", "fast"],
    "limits": {
        "rps": 1000,
        "burst": 200,
        "max_conn": 500
    },
    "metadata": {
        "owner": "platform",
        "team": "infra",
        "region": "us-east-1"
    }
}"#;

pub fn sum_numbers_jwc_value(v: &jwc::Value) -> i64 {
    match v {
        jwc::Value::Number(_) => v.as_i64().unwrap_or(0),
        jwc::Value::Array(a) => a.iter().map(|n| sum_numbers_jwc_value(&n.value)).sum(),
        jwc::Value::Object(o) => o
            .iter()
            .map(|e| sum_numbers_jwc_value(&e.value.value))
            .sum(),
        _ => 0,
    }
}

pub fn sum_numbers_lazy(n: &jwc::LazyNode<'_>) -> i64 {
    if let Some(x) = n.as_i64() {
        return x;
    }
    if let Some(arr) = n.as_array() {
        return arr.iter().map(sum_numbers_lazy).sum();
    }
    if let Some(obj) = n.as_object() {
        return obj.iter().map(|e| sum_numbers_lazy(&e.value)).sum();
    }
    0
}

pub fn sum_numbers_serde(v: &serde_json::Value) -> i64 {
    match v {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        serde_json::Value::Array(a) => a.iter().map(sum_numbers_serde).sum(),
        serde_json::Value::Object(o) => o.values().map(sum_numbers_serde).sum(),
        _ => 0,
    }
}

pub fn sum_numbers_sonic(v: &sonic_rs::Value) -> i64 {
    use sonic_rs::{JsonContainerTrait, JsonValueTrait};
    if v.is_number() {
        return v.as_i64().unwrap_or(0);
    }
    if let Some(arr) = v.as_array() {
        return arr.iter().map(sum_numbers_sonic).sum();
    }
    if let Some(obj) = v.as_object() {
        return obj.iter().map(|(_, x)| sum_numbers_sonic(x)).sum();
    }
    0
}
