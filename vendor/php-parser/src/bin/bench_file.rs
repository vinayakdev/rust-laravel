use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::env;
use std::fs;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let code = fs::read_to_string(path).expect("Failed to read file");
    let bytes = code.as_bytes();

    println!("Benchmarking: {}", path);
    println!("File size: {:.2} KB", bytes.len() as f64 / 1024.0);

    // Warmup
    println!("Warming up...");
    for _ in 0..50 {
        let bump = Bump::new();
        let lexer = Lexer::new(bytes);
        let mut parser = Parser::new(lexer, &bump);
        let _ = parser.parse_program();
    }

    // Benchmark
    let iterations = 200;
    println!("Running {} iterations...", iterations);

    let start = Instant::now();

    for _ in 0..iterations {
        let bump = Bump::new();
        let lexer = Lexer::new(bytes);
        let mut parser = Parser::new(lexer, &bump);
        let _ = parser.parse_program();
    }

    let duration = start.elapsed();
    let avg_time = duration / iterations as u32;
    let throughput =
        (bytes.len() as f64 * iterations as f64) / duration.as_secs_f64() / 1_024.0 / 1_024.0;

    println!("Total time: {:?}", duration);
    println!("Average time: {:?}", avg_time);
    println!("Throughput: {:.2} MB/s", throughput);
}
