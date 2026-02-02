use jwc::{JwcDeserializable, JwcSerializable, Node, Value, parser};
use jwcc_derive::{JwcDeserializable, JwcSerializable};
use std::collections::HashMap;

#[derive(JwcSerializable, JwcDeserializable, Debug, PartialEq)]
struct Config {
    debug: bool,
    timeout: i32,
    server: String,
    metadata: HashMap<String, String>,
}

fn main() {
    println!("--- JWC Usage Demo ---");

    // 1. Parsing JSONC (JSON with comments)
    let input = r#"
    {
        // Debug mode enabled
        "debug": true,
        /* Connection timeout in ms */
        "timeout": 5000,
        "server": "localhost",
        "metadata": {
            "env": "dev"
        }, // Initial setup
    }
    "#;

    let mut parser = parser::Parser::new(input);
    let node: Node = parser.parse().expect("Parsing failed");

    println!("\n[Parsed AST]");
    println!("{node}");

    // 2. Deserialize into Struct
    println!("\n[Deserialized Struct]");
    let config = Config::from_jwc(node.value.clone()).expect("Deserialization failed");
    println!("{config:?}");

    // 3. Modify AST (Add comments and fields)
    println!("\n[Modifying AST]");
    let mut node_modified = node;

    // Check if it's an object and modify
    if let Value::Object(ref mut members) = node_modified.value {
        // Add a new field 'retry_count' with a key comment
        let retry_node = Node::new(Value::from(3));
        let mut entry = jwc::ObjectEntry::new("retry_count".to_string(), retry_node);
        entry.key_comment(jwc::Trivia::line(" Number of retries"));

        members.push(entry);
    }

    println!("{node_modified}");

    // 4. Serialize Struct back to JWC
    println!("\n[Serialized Struct]");
    let config_val = config.to_jwc();
    // Wrap in Node to convert to string (no comments preserved from struct unless manually added)
    println!("{}", Node::new(config_val));
}
