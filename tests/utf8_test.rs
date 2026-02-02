//! Unicode / UTF-8 round-trips: strings, keys, comments with non-ASCII.

use jwc::{Value, from_str, from_str_lazy};

#[test]
fn multibyte_chars_in_string_values() {
    let cases = [
        ("ascii", r#""hello""#, "hello"),
        ("latin", r#""café""#, "café"),
        ("cjk", r#""日本語""#, "日本語"),
        ("emoji", r#""hi 🚀""#, "hi 🚀"),
        ("mixed", r#""Привет, 世界 🌍""#, "Привет, 世界 🌍"),
    ];
    for (tag, src, expected) in cases {
        let v = from_str(src).unwrap().value;
        assert_eq!(v.as_str(), Some(expected), "case {tag}");
        // Lazy parser should agree after decode.
        let b = from_str_lazy(src).unwrap();
        assert_eq!(b.as_str().as_deref(), Some(expected), "case {tag} (lazy)");
    }
}

#[test]
fn surrogate_pair_escape_decodes_to_astral() {
    // Rocket: U+1F680, UTF-16 surrogate pair D83D DE80.
    let src = r#""🚀""#;
    let v = from_str(src).unwrap().value;
    assert_eq!(v.as_str(), Some("🚀"));
}

#[test]
fn unpaired_high_surrogate_errors() {
    // High surrogate not followed by low surrogate → error.
    let src = r#""\uD83D""#;
    assert!(from_str(src).is_err(), "lone high surrogate should fail");
    // from_str_lazy doesn't validate escape sequences upfront — escape
    // validity is checked lazily in `as_str()`.
    let lazy = from_str_lazy(src).unwrap();
    assert!(
        lazy.as_str().is_none(),
        "lazy decode should fail for lone surrogate"
    );
}

#[test]
fn lone_low_surrogate_errors() {
    let src = r#""\uDE80""#;
    assert!(from_str(src).is_err(), "lone low surrogate should fail");
}

#[test]
fn unicode_escape_for_ascii_char_works() {
    // `A` == 'A'.
    let v = from_str(r#""ABC""#).unwrap().value;
    assert_eq!(v.as_str(), Some("ABC"));
}

#[test]
fn unicode_in_object_keys() {
    let v = from_str(r#"{"ключ": 1, "キー": 2, "🔑": 3}"#)
        .unwrap()
        .value;
    assert_eq!(v.get("ключ").and_then(Value::as_i64), Some(1));
    assert_eq!(v.get("キー").and_then(Value::as_i64), Some(2));
    assert_eq!(v.get("🔑").and_then(Value::as_i64), Some(3));
}

#[test]
fn utf8_inside_comments_preserved() {
    let src = "// commentaire avec café 🚀\n42";
    let n = from_str(src).unwrap();
    assert_eq!(n.trivia.len(), 1);
    assert_eq!(n.trivia[0].text(), " commentaire avec café 🚀");
}

#[test]
fn reject_unescaped_control_character() {
    // Raw newline inside a string (0x0A) must be rejected by strict parsers.
    let src = "\"line\nbreak\"";
    assert!(from_str(src).is_err());
    assert!(from_str_lazy(src).is_err());
}

#[test]
fn escape_all_rfc_controls() {
    let v = from_str(r#""\" \\ \/ \b \f \n \r \t""#).unwrap().value;
    assert_eq!(v.as_str(), Some("\" \\ / \u{08} \u{0c} \n \r \t"));
}
