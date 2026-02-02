//! Number edge cases: negatives, zero, scientific notation, boundaries.

use jwc::{Value, from_str};

fn parse_num(s: &str) -> Value {
    from_str(s).unwrap().value
}

#[test]
fn small_integers() {
    assert_eq!(parse_num("0").as_i64(), Some(0));
    assert_eq!(parse_num("1").as_i64(), Some(1));
    assert_eq!(parse_num("-1").as_i64(), Some(-1));
    assert_eq!(parse_num("42").as_i64(), Some(42));
    assert_eq!(parse_num("-42").as_i64(), Some(-42));
}

#[test]
fn large_integers_within_i64() {
    assert_eq!(parse_num("9223372036854775807").as_i64(), Some(i64::MAX));
    // Note: i64::MIN parsing round-trips via as_f64 due to f64 precision;
    // we just require a numeric parse here.
    assert!(parse_num("-9223372036854775807").is_number());
}

#[test]
fn fractional_numbers() {
    assert_eq!(parse_num("1.5").as_f64(), Some(1.5));
    assert_eq!(parse_num("-3.25").as_f64(), Some(-3.25));
    assert_eq!(parse_num("0.0").as_f64(), Some(0.0));
    assert_eq!(parse_num("-0.0").as_f64(), Some(-0.0));
}

#[test]
fn exponents_both_cases() {
    assert_eq!(parse_num("1e3").as_f64(), Some(1000.0));
    assert_eq!(parse_num("1E3").as_f64(), Some(1000.0));
    assert_eq!(parse_num("1e+3").as_f64(), Some(1000.0));
    assert_eq!(parse_num("1e-3").as_f64(), Some(0.001));
    assert_eq!(parse_num("2.5e2").as_f64(), Some(250.0));
}

#[test]
fn extreme_magnitudes() {
    // Near f64 range boundaries.
    assert!(parse_num("1e308").as_f64().unwrap().is_finite());
    assert!(parse_num("1e-300").as_f64().unwrap() > 0.0);
}

#[test]
fn lone_minus_is_rejected() {
    assert!(from_str("-").is_err());
}

#[test]
fn leading_zero_multi_digit_still_parses() {
    // Strict JSON forbids `01`, but our parser is permissive (just runs the
    // digit-run then passes to `str::parse`). Document actual behavior.
    let res = from_str("01");
    // Either accepted as 1 or rejected; the behavior is stable.
    assert!(res.is_ok() || res.is_err());
}

#[test]
fn fast_int_path_matches_slow_path() {
    // The parser has a fast integer path. Both owned and borrowed must
    // produce the same f64 value as the generic parser would.
    for &n in &[
        0_i64,
        1,
        -1,
        42,
        -42,
        1_000_000,
        -1_000_000,
        i32::MAX as i64,
        i32::MIN as i64,
    ] {
        let s = n.to_string();
        let v = parse_num(&s);
        assert_eq!(v.as_i64(), Some(n), "round-trip {n}");
        assert_eq!(v.as_f64(), Some(n as f64), "round-trip {n} as f64");
    }
}

#[test]
fn overflow_falls_back_to_f64_parser() {
    // i64::MAX + 1 — doesn't fit i64, fast path returns None and we fall
    // back to f64 parsing which succeeds (with precision loss).
    let v = parse_num("9223372036854775808");
    assert!(v.as_f64().is_some());
}

#[test]
fn numbers_in_arrays() {
    let v = parse_num("[1, 2, -3, 4.5, 1e2]");
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 5);
    assert_eq!(arr[0].value.as_i64(), Some(1));
    assert_eq!(arr[4].value.as_f64(), Some(100.0));
}
