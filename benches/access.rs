//! Parse **and access** — measures realistic "pull a field out of a
//! config blob" workloads, where lazy parsers win big.
//!
//! Run: `cargo bench --bench access`
//!
//! Three groups:
//! - `access_one_field`        — parse + read `database.port` only.
//! - `access_full_walk`        — parse + sum every number leaf.
//! - `access_repeated_lookups` — parse once, hit 20 keys on a wide object.

#[path = "common/mod.rs"]
mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use sonic_rs::JsonValueTrait;
use std::hint::black_box;

fn access_one_field(c: &mut Criterion) {
    let json = common::ACCESS_JSON;
    let mut g = c.benchmark_group("access_one_field");

    g.bench_function("jwc-lazy", |b| {
        b.iter(|| {
            let v = jwc::from_str_lazy(black_box(json)).unwrap();
            v.get("database")
                .and_then(|db| db.get("port"))
                .and_then(|p| p.as_i64())
                .unwrap()
        });
    });
    g.bench_function("jwc-owned", |b| {
        b.iter(|| {
            let v = jwc::from_str(black_box(json)).unwrap().value;
            v["database"]["port"].as_i64().unwrap()
        });
    });
    g.bench_function("serde_json", |b| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_str(black_box(json)).unwrap();
            v["database"]["port"].as_i64().unwrap()
        });
    });
    g.bench_function("sonic-rs", |b| {
        b.iter(|| {
            let v: sonic_rs::Value = sonic_rs::from_str(black_box(json)).unwrap();
            v["database"]["port"].as_i64().unwrap()
        });
    });

    g.finish();
}

fn access_full_walk(c: &mut Criterion) {
    let json = common::ACCESS_JSON;
    let mut g = c.benchmark_group("access_full_walk");

    g.bench_function("jwc-lazy", |b| {
        b.iter(|| {
            let v = jwc::from_str_lazy(black_box(json)).unwrap();
            common::sum_numbers_lazy(&v)
        });
    });
    g.bench_function("jwc-owned", |b| {
        b.iter(|| {
            let v = jwc::from_str(black_box(json)).unwrap().value;
            common::sum_numbers_jwc_value(&v)
        });
    });
    g.bench_function("serde_json", |b| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_str(black_box(json)).unwrap();
            common::sum_numbers_serde(&v)
        });
    });
    g.bench_function("sonic-rs", |b| {
        b.iter(|| {
            let v: sonic_rs::Value = sonic_rs::from_str(black_box(json)).unwrap();
            common::sum_numbers_sonic(&v)
        });
    });

    g.finish();
}

fn access_repeated_lookups(c: &mut Criterion) {
    let mut json = String::from("{");
    for i in 0..100 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("\"key_{i:03}\":{i}"));
    }
    json.push('}');

    let keys: Vec<String> = (0..100).step_by(5).map(|i| format!("key_{i:03}")).collect();

    let mut g = c.benchmark_group("access_repeated_lookups");

    g.bench_function("jwc-lazy-indexed", |b| {
        // LazyVal::Object is pre-sorted at parse time — .get() is already
        // O(log m), no explicit .index() step.
        b.iter(|| {
            let v = jwc::from_str_lazy(black_box(&json)).unwrap();
            let mut sum = 0i64;
            for k in &keys {
                sum += v.get(k).and_then(|v| v.as_i64()).unwrap();
            }
            sum
        });
    });
    g.bench_function("jwc-owned", |b| {
        b.iter(|| {
            let v = jwc::from_str(black_box(&json)).unwrap().value;
            let mut sum = 0i64;
            for k in &keys {
                sum += v[k.as_str()].as_i64().unwrap();
            }
            sum
        });
    });
    g.bench_function("sonic-rs", |b| {
        b.iter(|| {
            let v: sonic_rs::Value = sonic_rs::from_str(black_box(&json)).unwrap();
            let mut sum = 0i64;
            for k in &keys {
                sum += v[k.as_str()].as_i64().unwrap();
            }
            sum
        });
    });
    g.bench_function("serde_json", |b| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_str(black_box(&json)).unwrap();
            let mut sum = 0i64;
            for k in &keys {
                sum += v[k.as_str()].as_i64().unwrap();
            }
            sum
        });
    });

    g.finish();
}

criterion_group!(
    benches,
    access_one_field,
    access_full_walk,
    access_repeated_lookups
);
criterion_main!(benches);
