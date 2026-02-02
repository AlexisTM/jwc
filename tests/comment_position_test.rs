//! Comments keep their position: before a key or value (leading), after a value
//! on the same line (trailing), before a closing bracket (dangling), after the
//! document (root trailing).

use jwc::{CommentPolicy, FormatOptions, Indentation, Trivia, Value};

fn line(text: &str) -> Trivia {
    Trivia::LineComment(text.to_string())
}

fn block(text: &str) -> Trivia {
    Trivia::BlockComment(text.to_string())
}

fn members(node: &jwc::Node) -> &Vec<jwc::ObjectEntry> {
    match &node.value {
        Value::Object(m) => m,
        other => panic!("expected object, got {other:?}"),
    }
}

fn elements(node: &jwc::Node) -> &Vec<jwc::Node> {
    match &node.value {
        Value::Array(e) => e,
        other => panic!("expected array, got {other:?}"),
    }
}

const CANONICAL: &str = "\
// header
{
\t\"groups\": {
\t\t\"group:admin\": [
\t\t\t\"a@example.com\"
\t\t], // owners
\t},
\t\"tagOwners\": {
\t\t\"tag:infra\": [
\t\t\t\"group:admin\"
\t\t],
\t\t// BEGIN managed:site-tags
\t\t\"tag:a\": [
\t\t\t\"group:admin\"
\t\t],
\t\t// END managed:site-tags
\t},
\t\"grants\": [
\t\t// first
\t\t{
\t\t\t\"src\": [
\t\t\t\t\"x\"
\t\t\t], /* inline */
\t\t},
\t\t// BEGIN managed:site-grants
\t\t{
\t\t\t\"src\": [
\t\t\t\t\"y\"
\t\t\t],
\t\t},
\t\t// END managed:site-grants
\t],
\t\"empty\": {
\t\t// nothing here yet
\t},
}
// tail";

#[test]
fn canonical_layout_round_trips_byte_for_byte() {
    let node = jwc::from_str(CANONICAL).unwrap();
    let out = jwc::to_string_pretty(&node, Some("\t")).unwrap();
    assert_eq!(out, CANONICAL);
}

#[test]
fn markers_before_a_closing_bracket_are_dangling_on_the_container() {
    let node = jwc::from_str(CANONICAL).unwrap();
    let root = members(&node);

    let tag_owners = &root[1].value;
    let tag_a = &members(tag_owners)[1];
    assert_eq!(tag_a.key_trivia, vec![line(" BEGIN managed:site-tags")]);
    assert!(tag_a.value.trivia.is_empty());
    assert!(tag_a.value.trailing.is_empty());
    assert_eq!(tag_owners.dangling, vec![line(" END managed:site-tags")]);

    let grants = &root[2].value;
    assert_eq!(elements(grants)[0].trivia, vec![line(" first")]);
    assert_eq!(
        elements(grants)[1].trivia,
        vec![line(" BEGIN managed:site-grants")]
    );
    assert_eq!(grants.dangling, vec![line(" END managed:site-grants")]);

    assert_eq!(root[3].value.dangling, vec![line(" nothing here yet")]);
    assert_eq!(node.trivia, vec![line(" header")]);
    assert_eq!(node.trailing, vec![line(" tail")]);
}

#[test]
fn same_line_comments_are_trailing_of_their_value() {
    let node = jwc::from_str("[1, // one\n 2 /* two */, 3 // three\n]").unwrap();
    let e = elements(&node);
    assert_eq!(e[0].trailing, vec![line(" one")]);
    assert_eq!(e[1].trailing, vec![block(" two ")]);
    assert_eq!(e[2].trailing, vec![line(" three")]);
    assert!(e.iter().all(|n| n.trivia.is_empty()));
    assert!(node.dangling.is_empty());

    let out = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    assert_eq!(out, "[\n  1, // one\n  2, /* two */\n  3 // three\n]");
}

#[test]
fn comment_on_the_next_line_leads_the_next_element() {
    let node = jwc::from_str("[1,\n// two\n2]").unwrap();
    let e = elements(&node);
    assert!(e[0].trailing.is_empty());
    assert_eq!(e[1].trivia, vec![line(" two")]);
}

#[test]
fn comments_between_value_and_comma_stay_with_the_value() {
    let node = jwc::from_str("{\"a\": 1 /* x */, \"b\": 2}").unwrap();
    let m = members(&node);
    assert_eq!(m[0].value.trailing, vec![block(" x ")]);
    assert!(m[1].key_trivia.is_empty());
    assert_eq!(
        jwc::to_string_pretty(&node, Some("  ")).unwrap(),
        "{\n  \"a\": 1, /* x */\n  \"b\": 2\n}"
    );
}

#[test]
fn line_comment_between_colon_and_value_reindents_the_value() {
    let node = jwc::from_str("{\n  \"a\": // note\n  1\n}").unwrap();
    let out = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    assert_eq!(out, "{\n  \"a\": // note\n  1\n}");
    assert_eq!(jwc::from_str(&out).unwrap(), node);
}

#[test]
fn leading_line_comments_are_indented_at_every_depth() {
    let node = jwc::from_str("{\"a\": {\"b\": [\n// deep\n1]}}").unwrap();
    let out = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    assert_eq!(
        out,
        "{\n  \"a\": {\n    \"b\": [\n      // deep\n      1\n    ]\n  }\n}"
    );
}

#[test]
fn empty_containers_keep_their_dangling_comments() {
    let node = jwc::from_str("{\"a\": [\n// none\n], \"b\": {/* todo */}}").unwrap();
    let m = members(&node);
    assert_eq!(m[0].value.dangling, vec![line(" none")]);
    assert_eq!(m[1].value.dangling, vec![block(" todo ")]);
    assert_eq!(
        jwc::to_string_pretty(&node, Some("  ")).unwrap(),
        "{\n  \"a\": [\n    // none\n  ],\n  \"b\": {\n    /* todo */\n  }\n}"
    );
    assert_eq!(
        jwc::to_string(&node).unwrap(),
        "{\"a\":[// none\n],\"b\":{/* todo */}}"
    );
}

#[test]
fn comments_after_the_document_trail_the_root() {
    let node = jwc::from_str("{\"x\":1} // same line\n// end\n/* block */").unwrap();
    assert_eq!(
        node.trailing,
        vec![line(" same line"), line(" end"), block(" block ")]
    );
    let out = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    assert_eq!(out, "{\n  \"x\": 1\n}\n// same line\n// end\n/* block */");
    assert_eq!(jwc::from_str(&out).unwrap(), node);
}

#[test]
fn minified_output_stays_parseable_with_line_comments() {
    let node = jwc::from_str("// head\n[1, // one\n2]\n// tail").unwrap();
    let out = jwc::to_string(&node).unwrap();
    assert_eq!(out, "// head\n[1,// one\n2]\n// tail");
    assert_eq!(jwc::from_str(&out).unwrap(), node);
}

#[test]
fn minify_policy_turns_line_comments_into_block_comments() {
    let node = jwc::from_str("{\n// c\n\"a\": 1, // d\n// e\n}").unwrap();
    let out = node.to_formatted_string(FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Minify,
    });
    assert_eq!(out, "{/* c*/\"a\":1,/* d*//* e*/}");
    assert!(jwc::from_str(&out).is_ok());
}

#[test]
fn remove_policy_drops_comments_in_every_position() {
    let node = jwc::from_str("// h\n{\n// c\n\"a\": 1, // d\n// e\n}\n// t").unwrap();
    let out = node.to_formatted_string(FormatOptions {
        indentation: Indentation::Spaces(2),
        comment_policy: CommentPolicy::Remove,
    });
    assert_eq!(out, "{\n  \"a\": 1,\n}");
}

#[test]
fn programmatic_comments_land_where_asked() {
    let mut node = jwc::from_str("{\"a\": 1}").unwrap();
    node.add_dangling_comment(" end");
    match &mut node.value {
        Value::Object(m) => m[0].value.add_trailing_comment("// same line"),
        _ => unreachable!(),
    }
    assert_eq!(
        jwc::to_string_pretty(&node, Some("  ")).unwrap(),
        "{\n  \"a\": 1 // same line\n  // end\n}"
    );
}
