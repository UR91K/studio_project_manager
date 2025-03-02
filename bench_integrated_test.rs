use std::env;
use std::process::Command;
use std::time::{Duration, Instant};

fn main() {
    // Set env var to disable debug logging
    env::set_var("RUST_LOG", "error");
    
    println!("Running integrated test in release mode with debug messages disabled...");
    
    // Start timing
    let start = Instant::now();
    
    // Run the test with cargo
    let output = Command::new("cargo")
        .args([
            "test", 
            "--release", 
            "test_integrated_scanning_and_parsing", 
            "--", 
            "--nocapture"
        ])
        .output()
        .expect("Failed to execute cargo command");
    
    // Calculate elapsed time
    let elapsed = start.elapsed();
    
    // Print results
    println!("\n=== PERFORMANCE RESULTS ===");
    println!("Test completed in: {:.2?}", elapsed);
    println!("========================\n");
    
    // Print test output
    if !output.stdout.is_empty() {
        println!("Test Output:");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
    
    if !output.stderr.is_empty() {
        println!("Test Errors:");
        println!("{}", String::from_utf8_lossy(&output.stderr));
    }
} 