//! Parse-only throughput on a node-dense fixture (256 small objects in
//! an outer array). Stresses per-node allocation.
//!
//! Run: `cargo bench --bench parse_nested`

#[path = "common/mod.rs"]
mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn run(c: &mut Criterion) {
    let json = common::build_nested_json();
    let mut g = c.benchmark_group("parse_nested");

    g.bench_function("jwc", |b| {
        b.iter(|| jwc::from_str(black_box(&json)).unwrap());
    });
    g.bench_function("jwc-lazy", |b| {
        b.iter(|| jwc::from_str_lazy(black_box(&json)).unwrap());
    });
    g.bench_function("serde_json", |b| {
        b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(&json)).unwrap());
    });
    g.bench_function("simd-json", |b| {
        b.iter(|| {
            let mut bytes = json.as_bytes().to_vec();
            let _: simd_json::BorrowedValue =
                simd_json::to_borrowed_value(black_box(&mut bytes)).unwrap();
        });
    });
    g.bench_function("sonic-rs", |b| {
        b.iter(|| sonic_rs::from_str::<sonic_rs::Value>(black_box(&json)).unwrap());
    });

    g.finish();
}

criterion_group!(benches, run);
criterion_main!(benches);
