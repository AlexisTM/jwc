use jwc::single_pass_parser;
use std::hint::black_box;

fn main() {
    let json_data = r#"{"string":"Hello","number":12345,"float":123.456,"bool_true":true,"bool_false":false,"null":null,"array":[1,2,3,4,5],"object":{"key1":"value1","key2":"value2"}}"#;

    // Run parsing many times to get a good profile
    for _ in 0..1_000_000 {
        let mut parser = single_pass_parser::SinglePassParser::new(black_box(json_data));
        black_box(parser.parse().unwrap());
    }
}
