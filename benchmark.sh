#!/bin/bash
# Benchmark script for measuring the integrated test in release mode
# with all debug messages disabled

echo -e "\033[36mBuilding project in release mode...\033[0m"
cargo build --release

# Set the environment variables to disable logging
export RUST_LOG=error

echo -e "\033[36mRunning integrated test benchmark...\033[0m"
echo -e "\033[33mDebug messages are disabled (RUST_LOG=error)\033[0m"

# Get current timestamp
start_time=$(date +%s.%N)

# Run the test with output
cargo test --release test_integrated_scanning_and_parsing -- --nocapture

# Calculate elapsed time
end_time=$(date +%s.%N)
duration=$(echo "$end_time - $start_time" | bc)

echo -e "\n\033[32m=== OVERALL BENCHMARK RESULTS ===\033[0m"
echo -e "\033[32mTotal execution time (including cargo overhead): $(printf "%.2f" $duration) seconds\033[0m"
echo -e "\033[32m=================================\033[0m" 