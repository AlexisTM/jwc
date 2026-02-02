use jwc::parser;

fn main() {
    let json_data = r#"
    {
        "string": "Hello, world!",
        "number": 12345,
        "float": 123.456,
        "bool_true": true,
        "bool_false": false,
        "null": null,
        "array": [1, 2, 3, 4, 5],
        "object": {
            "key1": "value1",
            "key2": "value2"
        },
        "nested": {
            "a": {
                "b": {
                    "c": "deep"
                }
            }
        }
    }
    "#;

    // Run parsing many times to get a good profile
    for _ in 0..100_000 {
        let mut parser = parser::Parser::new(json_data);
        let _ = parser.parse().unwrap();
    }
}
