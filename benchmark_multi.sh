#!/bin/bash
# Multi-run benchmark script for measuring the integrated test in release mode
# with all debug messages disabled

# Default iterations if not specified
ITERATIONS=${1:-3}

echo -e "\033[36mBuilding project in release mode...\033[0m"
cargo build --release

# Set the environment variables to disable logging
export RUST_LOG=error

# Array to store timings
declare -a timings

echo -e "\033[36mRunning $ITERATIONS benchmark iterations...\033[0m"
echo -e "\033[33mDebug messages are disabled (RUST_LOG=error)\033[0m"

for ((i=1; i<=$ITERATIONS; i++)); do
    echo -e "\n\033[35mIteration $i of $ITERATIONS\033[0m"
    
    # Get current timestamp
    start_time=$(date +%s.%N)
    
    # Run the test with output
    cargo test --release test_integrated_scanning_and_parsing -- --nocapture
    
    # Calculate elapsed time
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc)
    timings[$i-1]=$duration
    
    echo -e "\033[35mIteration $i completed in: $(printf "%.2f" $duration) seconds\033[0m"
done

# Calculate statistics
total=0
for t in "${timings[@]}"; do
    total=$(echo "$total + $t" | bc)
done

avg=$(echo "scale=4; $total / $ITERATIONS" | bc)
min=$(printf "%.4f" $(echo "${timings[0]}" | bc))
max=$(printf "%.4f" $(echo "${timings[0]}" | bc))

# Find min and max
for t in "${timings[@]}"; do
    t_formatted=$(printf "%.4f" $(echo "$t" | bc))
    if (( $(echo "$t_formatted < $min" | bc -l) )); then
        min=$t_formatted
    fi
    if (( $(echo "$t_formatted > $max" | bc -l) )); then
        max=$t_formatted
    fi
done

# Calculate standard deviation
sum_squared_diff=0
for t in "${timings[@]}"; do
    diff=$(echo "$t - $avg" | bc)
    squared_diff=$(echo "$diff * $diff" | bc)
    sum_squared_diff=$(echo "$sum_squared_diff + $squared_diff" | bc)
done
std_dev=$(echo "scale=4; sqrt($sum_squared_diff / $ITERATIONS)" | bc)

echo -e "\n\033[32m=== BENCHMARK STATISTICS ($ITERATIONS runs) ===\033[0m"
echo -e "\033[32mAverage execution time: $(printf "%.2f" $avg) seconds\033[0m"
echo -e "\033[32mMinimum execution time: $(printf "%.2f" $min) seconds\033[0m"
echo -e "\033[32mMaximum execution time: $(printf "%.2f" $max) seconds\033[0m"
echo -e "\033[32mStandard deviation: $(printf "%.2f" $std_dev) seconds\033[0m"
echo -e "\033[32m===========================================\033[0m" 