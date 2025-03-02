# Multi-run benchmark script for measuring the integrated test in release mode
# with all debug messages disabled

param (
    [int]$Iterations = 3
)

Write-Host "Building project in release mode..." -ForegroundColor Cyan
cargo build --release

# Set the environment variables to disable logging
$env:RUST_LOG = "error"

$timings = @()

Write-Host "Running $Iterations benchmark iterations..." -ForegroundColor Cyan
Write-Host "Debug messages are disabled (RUST_LOG=error)" -ForegroundColor Yellow

for ($i = 1; $i -le $Iterations; $i++) {
    Write-Host "`nIteration $i of $Iterations" -ForegroundColor Magenta
    
    # Get current timestamp
    $startTime = Get-Date
    
    # Run the test with output
    cargo test --release test_integrated_scanning_and_parsing -- --nocapture
    
    # Calculate elapsed time
    $endTime = Get-Date
    $duration = $endTime - $startTime
    $timings += $duration.TotalSeconds
    
    Write-Host "Iteration $i completed in: $($duration.TotalSeconds.ToString("0.00")) seconds" -ForegroundColor Magenta
}

# Calculate statistics
$average = ($timings | Measure-Object -Average).Average
$min = ($timings | Measure-Object -Minimum).Minimum
$max = ($timings | Measure-Object -Maximum).Maximum
$stdDev = [Math]::Sqrt(($timings | ForEach-Object { [Math]::Pow($_ - $average, 2) } | Measure-Object -Average).Average)

Write-Host "`n=== BENCHMARK STATISTICS ($Iterations runs) ===" -ForegroundColor Green
Write-Host "Average execution time: $($average.ToString("0.00")) seconds" -ForegroundColor Green
Write-Host "Minimum execution time: $($min.ToString("0.00")) seconds" -ForegroundColor Green
Write-Host "Maximum execution time: $($max.ToString("0.00")) seconds" -ForegroundColor Green
Write-Host "Standard deviation: $($stdDev.ToString("0.00")) seconds" -ForegroundColor Green
Write-Host "===========================================" -ForegroundColor Green 