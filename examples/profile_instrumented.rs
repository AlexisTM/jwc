use jwc::single_pass_parser;
use std::hint::black_box;
use std::time::Instant;

#[cfg_attr(feature = "profiling", hotpath::main)]
#[cfg_attr(feature = "profiling", hotpath::main)]
fn main() {
    let json_data = r#"{"string":"Hello","number":12345,"float":123.456,"bool_true":true,"bool_false":false,"null":null,"array":[1,2,3,4,5],"object":{"key1":"value1","key2":"value2"}}"#;

    let iterations = 100_000;

    println!("Running {iterations} iterations...");
    let start = Instant::now();

    for _ in 0..iterations {
        let mut parser = single_pass_parser::SinglePassParser::new(black_box(json_data));
        black_box(parser.parse().unwrap());
    }

    let elapsed = start.elapsed();
    println!("Total time: {elapsed:?}");
    println!("Average time per parse: {:?}", elapsed / iterations);
    println!(
        "Throughput: {:.2} parses/sec",
        f64::from(iterations) / elapsed.as_secs_f64()
    );
}
