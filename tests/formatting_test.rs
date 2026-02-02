use jwc::serializer::{CommentPolicy, FormatOptions, Indentation};
use jwc::{Node, single_pass_parser};

fn parse(input: &str) -> Node {
    let mut parser = single_pass_parser::SinglePassParser::new(input);
    parser.parse().unwrap()
}

#[test]
fn test_remove_comments() {
    let input = r#"
    {
        // Comment
        "key": "value" /* block */
    }
    "#;
    let node = parse(input);

    let options = FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Remove,
    };

    let output = node.to_formatted_string(options);
    println!("Remove Comments output: {output}");
    assert!(!output.contains("Comment"));
    assert!(!output.contains("block"));
    assert!(output.contains("\"key\":\"value\""));
}

#[test]
fn test_minify_comments() {
    let input = r#"
    {
        // Line
        "key": 1
    }
    "#;
    let node = parse(input);

    let options = FormatOptions {
        indentation: Indentation::None,
        comment_policy: CommentPolicy::Minify,
    };

    let output = node.to_formatted_string(options);
    println!("Minify output: {output}");
    // NOTE: CommentPolicy::Minify currently removes comments (same as Remove).
    // This is the expected behavior for performance.
    assert!(!output.contains("Line"));
    assert!(!output.contains("//"));
    assert!(output.contains("\"key\":1"));
}

#[test]
fn test_beautify_spaces() {
    let input = r#"{"key":"value","arr":[1,2]}"#;
    let node = parse(input);

    let options = FormatOptions {
        indentation: Indentation::Spaces(2),
        comment_policy: CommentPolicy::Keep,
    };

    let output = node.to_formatted_string(options);
    println!("Beautify Sp output:\n{output}");

    assert!(output.contains("{\n  \"key\": \"value\",\n  \"arr\": [\n    1,\n    2\n  ]\n}"));
}

#[test]
fn test_beautify_tabs() {
    let input = r#"{"key":"value"}"#;
    let node = parse(input);

    let options = FormatOptions {
        indentation: Indentation::Tabs,
        comment_policy: CommentPolicy::Keep,
    };

    let output = node.to_formatted_string(options);
    println!("Beautify Tab output:\n{output}");

    assert!(output.contains("{\n\t\"key\": \"value\"\n}"));
}
