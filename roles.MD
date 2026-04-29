#!/usr/bin/env python3
"""
Cache benchmark runner for portfolio query caching.

Runs the ignored Rust benchmark-style test and extracts:
- get_portfolio cold vs warm average latency
- get_top_traders cold vs warm average latency
- cache hit/miss counters and hit ratio

Usage:
  python3 scripts/benchmark_cache.py
"""

import re
import shutil
import subprocess
import sys

BENCH_CMD = [
    "cargo",
    "test",
    "-p",
    "counter",
    "benchmark_cache_latency_and_hit_ratio",
    "--release",
    "--",
    "--ignored",
    "--nocapture",
]

LINE_PATTERN = re.compile(
    r"CACHE_BENCH\s+"
    r"portf_cold_ms=(?P<portf_cold>[0-9.]+)\s+"
    r"portf_warm_ms=(?P<portf_warm>[0-9.]+)\s+"
    r"portf_delta_pct=(?P<portf_delta>-?[0-9.]+)\s+"
    r"top_cold_ms=(?P<top_cold>[0-9.]+)\s+"
    r"top_warm_ms=(?P<top_warm>[0-9.]+)\s+"
    r"top_delta_pct=(?P<top_delta>-?[0-9.]+)\s+"
    r"hits=(?P<hits>[0-9]+)\s+"
    r"misses=(?P<misses>[0-9]+)\s+"
    r"hit_ratio_pct=(?P<hit_ratio>[0-9.]+)"
)


def _print_setup_hint() -> None:
    print("cargo is not installed or not on PATH.")
    print("Install Rust toolchain and retry:")
    print("  curl https://sh.rustup.rs -sSf | sh")
    print("  source $HOME/.cargo/env")


def _evaluate_target(delta_pct: float) -> str:
    if 30.0 <= delta_pct <= 50.0:
        return "PASS"
    return "WARN"


def main() -> int:
    if shutil.which("cargo") is None:
        _print_setup_hint()
        return 2

    print("Running cache benchmark test...")
    proc = subprocess.run(BENCH_CMD, capture_output=True, text=True)
    output = proc.stdout + "\n" + proc.stderr

    if proc.returncode != 0:
        print("Benchmark command failed.")
        print(output)
        return proc.returncode

    match = LINE_PATTERN.search(output)
    if not match:
        print("Could not find CACHE_BENCH output line.")
        print(output)
        return 3

    portf_cold = float(match.group("portf_cold"))
    portf_warm = float(match.group("portf_warm"))
    portf_delta = float(match.group("portf_delta"))
    top_cold = float(match.group("top_cold"))
    top_warm = float(match.group("top_warm"))
    top_delta = float(match.group("top_delta"))
    hits = int(match.group("hits"))
    misses = int(match.group("misses"))
    hit_ratio = float(match.group("hit_ratio"))

    print("Cache benchmark summary")
    print(f"  get_portfolio cold avg: {portf_cold:.6f} ms")
    print(f"  get_portfolio warm avg: {portf_warm:.6f} ms")
    print(f"  get_portfolio reduction: {portf_delta:.2f}% [{_evaluate_target(portf_delta)}]")
    print(f"  get_top_traders cold avg: {top_cold:.6f} ms")
    print(f"  get_top_traders warm avg: {top_warm:.6f} ms")
    print(f"  get_top_traders reduction: {top_delta:.2f}% [{_evaluate_target(top_delta)}]")
    print(f"  cache hits: {hits}")
    print(f"  cache misses: {misses}")
    print(f"  cache hit ratio: {hit_ratio:.2f}%")

    return 0


if __name__ == "__main__":
    sys.exit(main())
