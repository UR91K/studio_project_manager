# Benchmark script for measuring the integrated test in release mode
# with all debug messages disabled

Write-Host "Building project in release mode..." -ForegroundColor Cyan
cargo build --release

# Set the environment variables to disable logging
$env:RUST_LOG = "error"

Write-Host "Running integrated test benchmark..." -ForegroundColor Cyan
Write-Host "Debug messages are disabled (RUST_LOG=error)" -ForegroundColor Yellow

# Get current timestamp
$startTime = Get-Date

# Run the test with output
cargo test --release test_integrated_scanning_and_parsing -- --nocapture

# Calculate elapsed time
$endTime = Get-Date
$duration = $endTime - $startTime

Write-Host "`n=== OVERALL BENCHMARK RESULTS ===" -ForegroundColor Green
Write-Host "Total execution time (including cargo overhead): $($duration.TotalSeconds.ToString("0.00")) seconds" -ForegroundColor Green
Write-Host "=================================" -ForegroundColor Green 