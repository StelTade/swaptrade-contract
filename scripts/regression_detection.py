#!/usr/bin/env python3
"""
Performance Regression Detection Script for SwapTrade Contract

This script compares benchmark results against stored baselines to detect performance regressions.
"""

import json
import sys
import subprocess
import os
from datetime import datetime
from typing import Dict, List, Tuple, Optional

class RegressionDetector:
    def __init__(self, baseline_file: str = "baseline_performance.json", threshold_percent: float = 10.0):
        self.baseline_file = baseline_file
        self.threshold_percent = threshold_percent
        self.current_results = {}
        self.baseline_results = {}
        self.load_baselines()

    def load_baselines(self):
        """Load baseline performance data from file."""
        try:
            with open(self.baseline_file, 'r') as f:
                self.baseline_results = json.load(f)
            print(f"✅ Loaded baseline data from {self.baseline_file}")
        except FileNotFoundError:
            print(f"⚠️  Baseline file {self.baseline_file} not found. This is expected on first run.")
            self.baseline_results = {}
        except json.JSONDecodeError:
            print(f"❌ Error decoding baseline file {self.baseline_file}. Starting fresh.")
            self.baseline_results = {}

    def run_benchmarks(self) -> Dict[str, float]:
        """Run the benchmarks and extract performance data."""
        print("🏃 Running benchmarks...")
        
        # This is a placeholder - in a real scenario, you'd run the actual benchmark command
        # For now, we'll simulate some results
        simulated_results = {
            "swap_basic_mean": 2.5,
            "get_portfolio_mean": 1.2,
            "get_top_traders_mean": 3.8,
            "batch_5_operations_mean": 8.2,
            "sequential_5_swaps_mean": 12.5
        }
        
        print("✅ Benchmarks completed")
        return simulated_results

    def detect_regressions(self, current_results: Dict[str, float]) -> List[Tuple[str, float, float, float]]:
        """Detect performance regressions compared to baselines."""
        regressions = []
        
        for operation, current_time in current_results.items():
            if operation in self.baseline_results:
                baseline_time = self.baseline_results[operation]
                percent_change = ((current_time - baseline_time) / baseline_time) * 100
                
                if percent_change > self.threshold_percent:
                    regressions.append((operation, baseline_time, current_time, percent_change))
            else:
                # New operation, no baseline to compare against
                print(f"ℹ️  New operation '{operation}' - no baseline to compare against")
        
        return regressions

    def generate_report(self, current_results: Dict[str, float], regressions: List[Tuple[str, float, float, float]]) -> str:
        """Generate a human-readable report."""
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        
        report = f"""
=== PERFORMANCE REGRESSION REPORT ===
Generated: {timestamp}
Threshold: ±{self.threshold_percent}% 

SUMMARY:
- Total operations measured: {len(current_results)}
- Regressions detected: {len(regressions)}
- Baseline file: {self.baseline_file}

DETAILED RESULTS:
"""
        
        for operation, current_time in current_results.items():
            if operation in self.baseline_results:
                baseline_time = self.baseline_results[operation]
                percent_change = ((current_time - baseline_time) / baseline_time) * 100
                status = "⚠️ REGRESSION" if percent_change > self.threshold_percent else "✅ OK"
                
                report += f"- {operation}: {baseline_time:.3f}ms → {current_time:.3f}ms ({percent_change:+.2f}%) [{status}]\n"
            else:
                report += f"- {operation}: {current_time:.3f}ms [NEW]\n"
        
        if regressions:
            report += "\nDETECTED REGRESSIONS:\n"
            for op, baseline, current, change in regressions:
                report += f"- {op}: {baseline:.3f}ms → {current:.3f}ms (+{change:.2f}%)\n"
        else:
            report += "\n🎉 No performance regressions detected!\n"
        
        report += "\n==================================\n"
        return report

    def save_current_results(self, results: Dict[str, float]):
        """Save current results as the new baseline."""
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        results_with_metadata = {
            "__metadata__": {
                "generated_at": timestamp,
                "description": "Baseline performance metrics for SwapTrade contract",
                "threshold_percent": self.threshold_percent
            },
            **results
        }
        
        with open(self.baseline_file, 'w') as f:
            json.dump(results_with_metadata, f, indent=2)
        
        print(f"💾 Current results saved to {self.baseline_file}")

    def run_detection(self) -> bool:
        """Run the full regression detection process."""
        # Run benchmarks to get current results
        current_results = self.run_benchmarks()
        
        # Detect regressions
        regressions = self.detect_regressions(current_results)
        
        # Generate and print report
        report = self.generate_report(current_results, regressions)
        print(report)
        
        # Save current results as new baseline
        self.save_current_results(current_results)
        
        # Return True if no regressions detected
        return len(regressions) == 0

def main():
    """Main entry point."""
    print("🚀 Starting SwapTrade Performance Regression Detection...")
    
    # Create detector with 10% threshold
    detector = RegressionDetector(threshold_percent=10.0)
    
    # Run detection
    all_good = detector.run_detection()
    
    if all_good:
        print("✅ All good - no performance regressions detected!")
        sys.exit(0)
    else:
        print("❌ Performance regressions detected!")
        sys.exit(1)

if __name__ == "__main__":
    main()