# SwapTrade Contract Performance Benchmarking

This document describes the performance benchmarking framework for the SwapTrade contract, including benchmarks, targets, and regression detection.

## Overview

The SwapTrade contract performance benchmarking suite measures execution time and gas-like costs for critical operations to:
- Establish performance baselines
- Detect performance regressions
- Guide optimization efforts
- Ensure consistent user experience

## Benchmark Suite Structure

The benchmark suite is located in the `benches/` directory and includes:

### 1. Core Operations Benchmarks

#### `swap()` - Token Exchange
- **Purpose**: Measure time to execute token swaps
- **Target**: <10ms average execution time
- **Metrics**: Execution time, instruction count
- **Parameters**: Standard swap (XLM ↔ USDCSIM) with 1000 units

#### `add_liquidity()` - Liquidity Provision
- **Purpose**: Measure time to add liquidity to pools
- **Target**: <10ms average execution time
- **Metrics**: Execution time, instruction count

#### `get_portfolio()` - User Portfolio Query
- **Purpose**: Measure query latency for user portfolio data
- **Target**: <5ms average execution time
- **Metrics**: Query time, instruction count

#### `get_top_traders()` - Leaderboard Query
- **Purpose**: Measure query time for top traders (up to 100 users)
- **Target**: <5ms average execution time
- **Metrics**: Query time, instruction count

### 2. Batch Operations Benchmarks

#### Batch Execution vs Sequential
- **Purpose**: Compare performance of batch operations vs sequential calls
- **Configuration**: 5 operations in batch vs 5 sequential calls
- **Target**: Batch operations should be 20-30% faster than sequential
- **Operations**: Multiple swaps in a single batch

## Performance Targets

| Operation | Target Time | Critical |
|-----------|-------------|----------|
| Swap | <10ms | Yes |
| Query Operations | <5ms | Yes |
| Batch (5 ops) | <20ms | Yes |
| Sequential (5 ops) | <50ms | Yes |

## Benchmark Execution

### Running Benchmarks

```bash
# Navigate to the benches directory
cd benches/

# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench contract_benchmark swap
```

### Regression Detection

The regression detection script compares current performance against stored baselines:

```bash
# Run regression detection
python3 scripts/regression_detection.py
```

## Framework Components

### 1. Criterion-Based Benchmarking
Uses the Criterion framework for statistical benchmarking with:
- Statistical analysis of performance data
- Noise reduction through repeated measurements
- Automated regression detection

### 2. Performance Regression Detection
Python script that:
- Loads baseline performance data
- Runs current benchmarks
- Compares results against baselines
- Reports regressions exceeding threshold (default: 10%)

### 3. Baseline Management
- Stores performance baselines in `baseline_performance.json`
- Updates baselines after each successful run
- Tracks metadata including generation timestamp

## Expected vs Actual Performance

### Current Baseline (Sample)
These are example values - actual baselines will be generated during first run:

| Operation | Baseline (ms) | Current (ms) | Status |
|-----------|---------------|--------------|--------|
| swap_basic | 2.5 | TBD | TBD |
| get_portfolio | 1.2 | TBD | TBD |
| get_top_traders | 3.8 | TBD | TBD |
| batch_5_operations | 8.2 | TBD | TBD |
| sequential_5_swaps | 12.5 | TBD | TBD |

## Integration Testing

### Load Testing
The framework supports load testing with concurrent users through stress testing utilities that can be added as needed.

### Validation Checks
- Benchmarks are reproducible and consistent
- Regression detection is accurate
- Performance meets or exceeds targets
- Documentation is clear for future optimization

## Optimization Opportunities

Based on performance analysis, potential optimization areas include:

1. **Storage Access Patterns**: Optimize frequently accessed data structures
2. **Batch Processing**: Improve efficiency of multiple operations
3. **Query Optimization**: Reduce complexity of data retrieval operations
4. **Code Path Efficiency**: Streamline common execution paths

## Usage in CI/CD

The benchmark suite can be integrated into CI/CD pipelines to:
- Run benchmarks on pull requests
- Block merges that introduce performance regressions
- Track performance trends over time
- Generate performance reports

## Maintenance

### Updating Baselines
Baselines are automatically updated after each benchmark run. Manual updates should only be done when intentional performance changes occur.

### Adding New Benchmarks
To add new benchmarks:
1. Add the benchmark function to `benches/contract_benchmark.rs`
2. Register it in the `criterion_group!` macro
3. Run `cargo bench` to establish new baseline

## Troubleshooting

### Common Issues
- **High Variance**: May indicate system load or interference; run benchmarks on isolated systems
- **Inconsistent Results**: May indicate non-deterministic code paths; review contract logic
- **Memory Effects**: Previous runs may affect performance; restart environment between runs

### Best Practices
- Run benchmarks on dedicated hardware for consistent results
- Close unnecessary applications during benchmarking
- Run benchmarks multiple times to establish confidence intervals
- Compare against known-good baselines before accepting new ones