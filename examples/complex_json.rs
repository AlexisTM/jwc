fn main() {
    let json = r#"{
  "duplicate_key": "first_value",
  "duplicate_key": "second_value",
  "": "This key is a perfectly valid empty string",
  " \t\n ": "This key contains raw whitespace but is valid",
  "escapes": "\" \\ \/ \b \f \n \r \t \u0000",
  "unicode_surrogates": "\uD83D\uDE80",
  "max_int64": 9223372036854775807,
  "precision_loss": 9007199254740993,
  "floats": [
    1.23456789e+308,
    5e-324,
    -0.0,
    0.0000000000000000000000000000000000000001
  ],
  "keywords_as_keys": {
    "true": false,
    "null": "null",
    "false": true
  },
  "deep_nesting": [[[[[[[[[[[[[[[[[[[[ { "buried": null } ]]]]]]]]]]]]]]]]]]]],
"chaotic_whitespace"	: 
	[
		1	,    
	2
		]
}"#;
    let node = jwc::from_str(json).unwrap();
    let serialized = jwc::to_string_pretty(&node, Some("  ")).unwrap();
    let reparsed = jwc::from_str(&serialized).unwrap();

    assert_eq!(node, reparsed, "AST round-trip failed");

    println!("Round-trip AST preserved successfully.");
    println!("Serialized output:\n{serialized}");
}
