//! Layout-preserving output: untouched nodes and members come back verbatim.

use jwc::Value;

const POLICY: &str = "// Example ACLs
{
\t\"groups\": {
\t\t\"group:admin\": [\"alexis@example.com\", \"kabir@example.com\"],

\t\t\"group:dev\":   [\"parker@example.com\"],
\t},

\t\"tagOwners\": {
\t\t\"tag:infra\":      [\"group:admin\"],
\t\t\"tag:msi-caor\":   [\"group:dev\", \"tag:dock\"], // prod
\t},

\t// Per-site nest subnets
\t\"ipsets\": {
\t\t\"ipset:msi-caor\":     [\"10.0.89.112/28\", \"10.0.6.96/28\"],
\t\t\"ipset:msi-gasv\":     [\"10.0.81.144/28\"],
\t},
}
";

#[test]
fn unchanged_document_is_byte_identical() {
    let node = jwc::from_str(POLICY).unwrap();
    assert_eq!(
        jwc::to_string_preserving(&node, POLICY, None).unwrap(),
        POLICY
    );
}

#[test]
fn editing_one_member_reflows_only_its_container() {
    let mut node = jwc::from_str(POLICY).unwrap();
    let list = node.value.pointer_mut("/ipsets/ipset:msi-caor").unwrap();
    *list = Value::Array(vec![jwc::Node::new(Value::from("10.0.89.112/28"))]);

    let out = jwc::to_string_preserving(&node, POLICY, None).unwrap();

    // Everything before and after the ipsets object is untouched, alignment,
    // blank lines and comments included.
    let head = &POLICY[..POLICY.find("\t\"ipsets\": {").unwrap()];
    assert!(out.starts_with(head));
    assert!(out.ends_with("\t},\n}\n"));
    // The untouched sibling keeps its own spacing...
    assert!(out.contains("\t\t\"ipset:msi-gasv\":     [\"10.0.81.144/28\"],\n"));
    // ...while the edited member is laid out again with the detected indentation.
    assert!(out.contains("\t\t\"ipset:msi-caor\": [\n\t\t\t\"10.0.89.112/28\"\n\t\t],\n"));
    assert_eq!(jwc::from_str(&out).unwrap(), node);
}

#[test]
fn appending_a_member_keeps_the_others_verbatim() {
    let mut node = jwc::from_str(POLICY).unwrap();
    let Value::Object(tag_owners) = node.value.pointer_mut("/tagOwners").unwrap() else {
        panic!()
    };
    tag_owners.last_mut().unwrap().value.comma = true;
    tag_owners.push(jwc::ObjectEntry::new(
        "tag:new-site".to_string(),
        jwc::Node::new(Value::Array(vec![jwc::Node::new(Value::from("group:dev"))])),
    ));

    let out = jwc::to_string_preserving(&node, POLICY, None).unwrap();
    assert!(out.contains("\t\t\"tag:infra\":      [\"group:admin\"],\n"));
    assert!(out.contains("\t\t\"tag:msi-caor\":   [\"group:dev\", \"tag:dock\"], // prod\n"));
    assert!(out.contains("\t\t\"tag:new-site\": [\n\t\t\t\"group:dev\"\n\t\t]\n\t},"));
    // Untouched siblings of tagOwners are still verbatim, blank lines included.
    assert!(out.contains("\t},\n\n\t// Per-site nest subnets\n\t\"ipsets\": {"));
}

#[test]
fn comment_edits_count_as_changes() {
    let mut node = jwc::from_str(POLICY).unwrap();
    node.value.pointer_mut("/groups").map(|_| ()).unwrap();
    let Value::Object(root) = &mut node.value else {
        panic!()
    };
    root[0].value.add_dangling_comment(" more groups here");
    let out = jwc::to_string_preserving(&node, POLICY, None).unwrap();
    assert!(out.contains("\t\t// more groups here\n\t},"));
    assert!(out.contains("\t\t\"group:dev\":   [\"parker@example.com\"],\n"));
}

#[test]
fn blank_lines_survive_a_reflow_and_plain_pretty_output() {
    let src = "[\n  1,\n\n  // two\n  2,\n  3\n]";
    let mut node = jwc::from_str(src).unwrap();
    *node.value.pointer_mut("/2").unwrap() = Value::from(4);
    assert_eq!(
        jwc::to_string_preserving(&node, src, None).unwrap(),
        "[\n  1,\n\n  // two\n  2,\n  4\n]"
    );
    assert_eq!(
        jwc::to_string_pretty(&jwc::from_str(src).unwrap(), Some("  ")).unwrap(),
        "[\n  1,\n\n  // two\n  2,\n  3\n]"
    );
    // Trailing spaces are not layout: they go, blank lines stay.
    let sloppy = "{\n  \"a\": 1,   \n\n  \"b\": 2  \n}\n";
    let mut node = jwc::from_str(sloppy).unwrap();
    *node.value.pointer_mut("/a").unwrap() = Value::from(9);
    assert_eq!(
        jwc::to_string_preserving(&node, sloppy, None).unwrap(),
        "{\n  \"a\": 9,\n\n  \"b\": 2\n}\n"
    );
}

#[test]
fn spaces_are_detected_and_a_wrong_source_falls_back_to_pretty() {
    let src = "{\n  \"a\": [1,   2],\n  \"b\": 3\n}";
    let mut node = jwc::from_str(src).unwrap();
    *node.value.pointer_mut("/b").unwrap() = Value::from(4);
    assert_eq!(
        jwc::to_string_preserving(&node, src, None).unwrap(),
        "{\n  \"a\": [1,   2],\n  \"b\": 4\n}"
    );
    // Spans that do not fit the text given are ignored rather than trusted.
    let out = jwc::to_string_preserving(&node, "{}", Some("  ")).unwrap();
    assert_eq!(out, "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": 4\n}");
}
