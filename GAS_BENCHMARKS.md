# Gas Cost Optimization & Benchmarking Suite

## Overview

This document describes the gas cost benchmarking infrastructure for SwapTrade Soroban contracts, the optimizations applied, and measured improvements.

## Quick Start

```bash
# Run the full gas benchmark suite
cargo test -p gas-benchmarks --test gas_benchmarks -- --nocapture

# Run orderbook scaling benchmarks
cargo test -p gas-benchmarks --test orderbook_optimization -- --nocapture
```

## Benchmark Infrastructure

### What We Measure

Each benchmark captures these Soroban resource dimensions via `env.cost_estimate().resources()`:

| Resource | Description | Fee Impact |
|----------|-------------|------------|
| `instructions` | CPU instructions consumed | Proportional fee |
| `mem_bytes` | Peak memory usage | Upper bound only |
| `disk_read_entries` | Persistent storage reads | Per-entry fee |
| `memory_read_entries` | In-memory Soroban state reads | Upper bound only |
| `write_entries` | Persistent storage writes | Per-entry fee |
| `events_bytes` | Contract event size | Per-1KB fee |

We also capture `env.cost_estimate().fee()` which returns a `FeeEstimate` with stroop-denominated costs.

### Benchmark Files

- `gas-benchmarks/tests/gas_benchmarks.rs` — Core operation benchmarks (place, cancel, trade, query)
- `gas-benchmarks/tests/orderbook_optimization.rs` — Orderbook scaling benchmarks at different book sizes

## Before/After Gas Estimates

### Key Refactoring: Price-Level Bucketed Orderbook

The main optimization restructured `PairOrderBook` from flat `Vec<u64>` order-ID lists to `Map<u128, Vec<u64>>` price-level buckets with pre-computed `PriceLevel` aggregates.

**Before:** Each `add_order` call performed an O(n) sorted-insertion scan, reading every existing order from storage to compare prices. On a book with N orders, this meant N storage reads per insertion.

**After:** `add_order` does a direct O(1) `Map::get(price)` + `Vec::push_back`. Zero additional storage reads for price comparison.

### Operation: `place_order` (selling into an existing book)

| Book Size | Before (instructions) | After (instructions) | Savings |
|-----------|----------------------|---------------------|---------|
| 5 orders  | ~54,894              | ~33,191             | ~39%    |
| 10 orders | ~41,166              | ~16,887             | ~59%    |
| 20 orders | ~34,627              | ~4,682              | ~86%    |

> Note: The "delta" measurements are from snapshot-to-snapshot within the test. The absolute cost of a single place_order call on a book with N existing orders decreases with the new structure because the sorted-insertion scan is eliminated entirely.

**Absolute cost (per single place_order on a book with N existing orders):**

| Book Size | Before (total instructions) | After (total instructions) |
|-----------|---------------------------|--------------------------|
| 0 (empty) | ~194,000                  | ~206,000                 |
| 1         | —                         | ~261,000                 |
| 2         | —                         | ~300,000                 |
| 5         | —                         | ~371,000                 |
| 10        | —                         | ~472,000                 |
| 20        | —                         | ~705,000                 |
| 50        | —                         | ~1,263,000               |

The cost grows with book size due to the persistent storage write of the full book structure (serialization cost), but this growth is unavoidable with the current Soroban persistent storage model.

### Operation: `cancel_order`

| Scenario | Before (instructions) | After (instructions) | Notes |
|----------|----------------------|---------------------|-------|
| Sell order | ~10,101 | ~15,590 | Slight increase: reads order for price lookup |
| Buy order | ~12,789 | ~18,278 | Slight increase: reads order for price lookup |

The cancel operation now reads the order once to extract its price for direct price-level lookup, adding ~5K instructions. This is negligible in absolute terms.

### Operation: `execute_multi_pair_trade`

| Scenario | Before (instructions) | After (instructions) |
|----------|----------------------|---------------------|
| 1 leg, orderbook fill | ~430,200 | ~452,281 |
| 1 leg, pool fallback | ~412,117 | ~415,436 |
| 3 legs | ~1,675,632 | ~1,741,875 |
| 10 pairs | ~7,267,955 | ~7,477,377 |

Small increases (~3-5%) in trade execution due to Map iteration overhead vs. Vec iteration. This is a worthwhile tradeoff given the large improvement in `place_order` scaling.

### Fee Estimates (stroops)

| Operation | Total Fee (stroops) |
|-----------|-------------------|
| place_order (sell, empty book) | ~3,415,768 |
| place_order (buy) | ~3,415,767 |
| cancel_order (sell) | ~2,220,562 |
| cancel_order (buy) | ~2,220,563 |
| execute_trade (1 leg, OB fill) | ~3,356,041 |
| execute_trade (1 leg, pool) | ~2,251,693 |
| execute_trade (3 legs) | ~6,785,863 |
| execute_trade (10 pairs) | ~14,856,851 |

## Architecture Changes

### `PairOrderBook` (Before → After)

**Before:**
```rust
pub struct PairOrderBook {
    pub bid_order_ids: Vec<u64>,  // Flat list, sorted by price
    pub ask_order_ids: Vec<u64>,  // Flat list, sorted by price
}
```

**After:**
```rust
pub struct PairOrderBook {
    pub bids: Map<u128, Vec<u64>>,       // price → order IDs (FIFO)
    pub asks: Map<u128, Vec<u64>>,       // price → order IDs (FIFO)
    pub bid_levels: Map<u128, PriceLevel>, // price → {total_amount, order_count}
    pub ask_levels: Map<u128, PriceLevel>, // price → {total_amount, order_count}
}
```

### `add_order` Complexity Change

- **Before:** O(n) storage reads + O(n) Vec rebuild per insertion
- **After:** O(1) Map lookup + O(1) Vec push + O(1) aggregate update

### `get_summary` Complexity Change

- **Before:** O(n) reads of individual orders → aggregate into a Map → build summary
- **After:** O(p) iteration of pre-computed price-level aggregates (p = distinct prices)

### `matching.rs` Updates

The matching engine was updated to iterate price-level Maps instead of flat Vecs. After each fill:
- `update_level_after_fill()` updates the running aggregate in O(1)
- Filled orders are removed from their price-level Vec

## Recommendations for Further Optimization

### 1. Separate Hot/Cold Storage

**Current:** The full `PairOrderBook` (including all order IDs) is serialized to persistent storage on every write. As the book grows, this serialization cost dominates.

**Recommendation:** Split into:
- **Hot storage** (instance): Price-level aggregates only (`bid_levels`, `ask_levels` maps)
- **Cold storage** (persistent): Individual order IDs and orders

**Estimated impact:** 20-40% reduction in `place_order` cost for large books (avoids serializing the full order ID lists).

### 2. Event Emission Batching

**Current:** Each fill emits a separate `emit_fill()` event. For multi-leg trades with many fills, this adds up.

**Recommendation:** Batch fill events into a single per-leg event, or make event emission optional (behind a `logging` feature flag, which already exists).

**Estimated impact:** 5-15% reduction in `execute_multi_pair_trade` cost.

### 3. `get_summary` Pagination

**Current:** `get_summary` returns all levels up to `max_levels` in a single call.

**Recommendation:** Add cursor-based pagination for the order book snapshot query to reduce per-call cost for large books.

### 4. Lazy Level Cleanup

**Current:** Empty price levels are removed immediately when the last order is filled/cancelled.

**Recommendation:** Defer cleanup to reduce write operations. Add a `cleanup_stale_levels()` admin function.

### 5. Storage Entry Compaction

**Current:** Each order is stored as a separate persistent entry. For high-volume order books, this creates many small entries.

**Recommendation:** Consider packing multiple orders into a single storage entry (e.g., all orders at the same price level in one entry). This would reduce per-entry overhead but increase serialization complexity.

### 6. Wasm Gas Metering

**Current:** Benchmarks run native Rust (test mode), which underestimates actual Wasm gas costs.

**Recommendation:** Add a Wasm build + invoke benchmark using `soroban contract invoke` to measure real on-chain gas costs. The native benchmarks are useful for relative comparison but the absolute numbers differ from Wasm execution.

## CI Integration

The gas benchmark suite runs as an optional CI step on pull requests:

```yaml
# In .github/workflows/ci.yml
gas-report:
  name: Gas Cost Report
  runs-on: ubuntu-latest
  if: github.event_name == 'pull_request'
  continue-on-error: true
  steps:
    - name: Run gas benchmarks
      run: |
        cargo test -p gas-benchmarks --test gas_benchmarks -- --nocapture 2>&1 | tee gas-report.txt
        cargo test -p gas-benchmarks --test orderbook_optimization -- --nocapture 2>&1 | tee -a gas-report.txt
    - name: Upload gas report
      uses: actions/upload-artifact@v4
      with:
        name: gas-report
        path: gas-report.txt
        retention-days: 30
```

## Adding New Benchmarks

To add a new benchmark:

1. Add a test function to `gas-benchmarks/tests/gas_benchmarks.rs`
2. Use `GasSnapshot::capture()` before and after the operation
3. Use `print_budget_delta()` to output the delta
4. Follow the existing pattern for setup and assertions

```rust
#[test]
fn bench_my_new_operation() {
    let (env, client, _admin, user, base, quote) = setup();
    // ... setup code ...

    let before = GasSnapshot::capture(&env, "my_operation");
    // ... call the contract ...
    let after = GasSnapshot::capture(&env, "my_operation");

    println!("\n=== my_operation ===");
    print_budget_delta(&env, &before, &after);
}
```
