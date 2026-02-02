use jwc::SinglePassParser;

#[cfg_attr(feature = "profiling", hotpath::main)]
fn main() {
    #[cfg(not(feature = "profiling"))]
    {
        println!("To run with profiling enabled:");
        println!("cargo run --example profile --features profiling");
        println!("\nRunning standard workload without profiling...");
    }
    run_profiling();
}

fn run_profiling() {
    println!("Generating sample data...");
    // Generate a large JSONC string to give the parser some work
    let mut large_json = String::with_capacity(1024 * 1024);
    large_json.push_str("[\n");
    for i in 0..5000 {
        large_json.push_str(&format!("  // Item comment {i}\n"));
        large_json.push_str(&format!(
            "  {{ \"id\": {i}, \"name\": \"item_{i}\", \"values\": [1, 2, 3, 4, 5], \"active\": true }},\n"
        ));
    }
    large_json.push_str("  { \"id\": 9999, \"name\": \"last\" }\n");
    large_json.push(']');

    println!("Parsing {} iterations...", 50);
    // Parse it multiple times to generate load
    for _ in 0..50 {
        let mut parser = SinglePassParser::new(&large_json);
        match parser.parse() {
            Ok(_) => {}
            Err(e) => eprintln!("Error: {e}"),
        }
    }
    println!("Done.");
}
