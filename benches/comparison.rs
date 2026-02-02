use criterion::{Criterion, criterion_group, criterion_main};
use jwc::SinglePassParser;
use std::hint::black_box;

fn benchmark_parsing(c: &mut Criterion) {
    let json_data = r#"
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

    let mut group = c.benchmark_group("JSON Parsing");

    // JWC
    group.bench_function("jwc", |b| {
        b.iter(|| {
            let mut parser = SinglePassParser::new(black_box(json_data));
            let _ = parser.parse().unwrap();
        });
    });

    // Serde JSON
    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let _: serde_json::Value = serde_json::from_str(black_box(json_data)).unwrap();
        });
    });

    // Simd JSON (requires mutable bytes)
    group.bench_function("simd-json", |b| {
        b.iter(|| {
            let mut bytes = json_data.as_bytes().to_vec();
            let _: simd_json::BorrowedValue =
                simd_json::to_borrowed_value(black_box(&mut bytes)).unwrap();
        });
    });

    // Sonic RS
    group.bench_function("sonic-rs", |b| {
        b.iter(|| {
            let _: sonic_rs::Value = sonic_rs::from_str(black_box(json_data)).unwrap();
        });
    });

    // JSON (json-rust)
    group.bench_function("json-rust", |b| {
        b.iter(|| {
            let _ = json::parse(black_box(json_data)).unwrap();
        });
    });

    // TinyJSON
    group.bench_function("tinyjson", |b| {
        b.iter(|| {
            let _: tinyjson::JsonValue = black_box(json_data).parse().unwrap();
        });
    });

    // GJSON
    group.bench_function("gjson", |b| {
        b.iter(|| {
            let _ = gjson::valid(black_box(json_data));
        });
    });

    // AJSON
    group.bench_function("ajson", |b| {
        b.iter(|| {
            let _ = ajson::parse(black_box(json_data)).unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_parsing);
criterion_main!(benches);
