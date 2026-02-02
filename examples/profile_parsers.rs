//! Profile the two main parser paths on the nested fixture. Run:
//!
//! ```
//! cargo run --release --features profiling --example profile_parsers
//! ```
//!
//! Requires `hotpath::measure` attributes on the hot functions. Output
//! shows call counts + self-time per function, letting us see where each
//! parser spends its time without guessing.

#[cfg_attr(feature = "profiling", hotpath::main)]
fn main() {
    let nested = build_nested_fixture();
    let iterations = if cfg!(feature = "profiling") {
        2_000
    } else {
        20_000
    };

    println!("== jwc::from_str_lazy (nested, {iterations} iters) ==");
    for _ in 0..iterations {
        let _ = jwc::from_str_lazy(&nested).unwrap();
    }
}

fn build_nested_fixture() -> String {
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
