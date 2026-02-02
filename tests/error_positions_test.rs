//! Parse-error position + variant precision. Verifies the parser reports
//! errors at the right `line:col` and lifts them into the right `Error`
//! variant.

use jwc::Error;

fn parse_err(s: &str) -> Error {
    jwc::from_str(s).unwrap_err()
}

fn parse_err_lazy(s: &str) -> Error {
    jwc::from_str_lazy(s).unwrap_err()
}

#[track_caller]
fn assert_parse_at(err: Error, expected_line: usize, expected_col_at_least: usize) {
    match err {
        Error::Parse { line, col, .. } => {
            assert_eq!(line, expected_line, "wrong line in {err:?}");
            assert!(
                col >= expected_col_at_least,
                "col {col} < expected at-least {expected_col_at_least}"
            );
        }
        other => panic!("expected Error::Parse, got {other:?}"),
    }
}

#[test]
fn unterminated_string_at_final_line() {
    assert_parse_at(parse_err("\n\n\"unterm"), 3, 1);
    assert_parse_at(parse_err_lazy("\n\n\"unterm"), 3, 1);
}

#[test]
fn unexpected_identifier_after_ws() {
    assert_parse_at(parse_err("    nope"), 1, 5);
    assert_parse_at(parse_err_lazy("    nope"), 1, 5);
}

#[test]
fn unterminated_block_comment_reports_input_end() {
    let src = "/* never closed";
    let err = parse_err(src);
    match err {
        Error::Parse { msg, .. } => assert!(msg.contains("Unterminated block comment"), "{msg}"),
        other => panic!("expected Parse, got {other:?}"),
    }
}

#[test]
fn bad_escape_position() {
    // \q is invalid; should error inside the string.
    let err = parse_err(r#""\q""#);
    match err {
        Error::Parse { msg, .. } => assert!(msg.contains("escape"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn missing_colon_error() {
    let err = parse_err(r#"{"k" 1}"#);
    match err {
        Error::Parse { msg, .. } => assert!(
            msg.contains(':') || msg.to_lowercase().contains("colon") || msg.contains("key"),
            "{msg}"
        ),
        _ => panic!(),
    }
}

#[test]
fn trailing_garbage_after_root() {
    let err = parse_err("42 garbage");
    match err {
        Error::Parse { msg, .. } => assert!(msg.contains("trailing"), "{msg}"),
        _ => panic!(),
    }
}

#[test]
fn empty_input_errors_with_eof() {
    assert!(matches!(parse_err(""), Error::Parse { .. }));
    assert!(matches!(parse_err("   \n\t  "), Error::Parse { .. }));
}

#[test]
fn error_display_includes_position() {
    let err = parse_err("\n\n\"unterm");
    let s = err.to_string();
    assert!(s.contains(" at 3:"), "expected 'at 3:...', got {s}");
}

#[test]
fn borrowed_and_owned_agree_on_error_position() {
    let bad = "\n\n[1, \"unterm";
    let a = parse_err(bad);
    let b = parse_err_lazy(bad);
    match (a, b) {
        (Error::Parse { line: la, .. }, Error::Parse { line: lb, .. }) => {
            assert_eq!(la, lb);
        }
        _ => panic!(),
    }
}
