//! SwapTrade Contract Performance Benchmark Runner
//! 
//! This executable runs comprehensive performance benchmarks on the SwapTrade contract
//! to establish performance baselines and detect regressions.

use std::time::Instant;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use serde_json::Value;

mod performance_benchmark;
use performance_benchmark::{run_benchmarks, BenchmarkRunner, BenchmarkResult};

/// Configuration for the benchmark runner
struct BenchmarkConfig {
    iterations: usize,
    warmup_runs: usize,
    output_format: OutputFormat,
}

#[derive(Debug)]
enum OutputFormat {
    Console,
    Json,
    Csv,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 10,
            warmup_runs: 3,
            output_format: OutputFormat::Console,
        }
    }
}

/// Performance regression detector
struct RegressionDetector {
    baseline_data: HashMap<String, f64>, // Stores baseline average times
    threshold_percent: f64,              // Threshold percentage for regression detection
}

impl RegressionDetector {
    fn new(threshold_percent: f64) -> Self {
        Self {
            baseline_data: HashMap::new(),
            threshold_percent,
        }
    }

    fn load_baseline_from_file(&mut self, filepath: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(filepath)?;
        let data: HashMap<String, f64> = serde_json::from_str(&content)?;
        self.baseline_data = data;
        Ok(())
    }

    fn detect_regression(&self, current_result: &BenchmarkResult) -> bool {
        if let Some(baseline_time) = self.baseline_data.get(&current_result.name) {
            let percent_increase = ((current_result.avg_time_ms - baseline_time) / baseline_time) * 100.0;
            percent_increase > self.threshold_percent
        } else {
            // No baseline, so no regression detected
            false
        }
    }

    fn format_regression_report(&self, current_results: &[BenchmarkResult]) -> String {
        let mut report = String::new();
        report.push_str("=== PERFORMANCE REGRESSION REPORT ===\n");

        for result in current_results {
            if self.detect_regression(result) {
                if let Some(baseline_time) = self.baseline_data.get(&result.name) {
                    let percent_increase = ((result.avg_time_ms - baseline_time) / baseline_time) * 100.0;
                    report.push_str(&format!(
                        "⚠️  REGRESSION DETECTED: {} increased by {:.2}% (baseline: {:.3}ms, current: {:.3}ms)\n",
                        result.name, percent_increase, baseline_time, result.avg_time_ms
                    ));
                }
            } else {
                report.push_str(&format!("✅ {}: No regression detected\n", result.name));
            }
        }

        report.push_str("==================================\n");
        report
    }
}

/// Optimization opportunity analyzer
struct OptimizationAnalyzer;

impl OptimizationAnalyzer {
    fn analyze_results(results: &[BenchmarkResult]) -> Vec<String> {
        let mut opportunities = Vec::new();
        
        for result in results {
            // Identify slow operations
            if result.avg_time_ms > 10.0 {
                opportunities.push(format!(
                    "Slow operation detected: {} averages {:.3}ms - consider optimization", 
                    result.name, result.avg_time_ms
                ));
            }
            
            // High instruction count analysis
            if result.instructions > 100_000 {
                opportunities.push(format!(
                    "High instruction count: {} uses {} instructions - potential optimization target", 
                    result.name, result.instructions
                ));
            }
            
            // High variance analysis
            let variance = result.max_time_ms - result.min_time_ms;
            if variance > result.avg_time_ms * 0.5 {  // If variance is more than 50% of average
                opportunities.push(format!(
                    "High timing variance: {} varies from {:.3}ms to {:.3}ms - investigate inconsistency", 
                    result.name, result.min_time_ms, result.max_time_ms
                ));
            }
        }
        
        opportunities
    }
}

fn main() {
    println!("🚀 Starting SwapTrade Contract Performance Benchmark Suite...");
    
    // Run the benchmarks
    run_benchmarks();
    
    // Additional analysis could be added here
    println!("✅ Benchmarks completed successfully!");
    
    // Example of how to use the benchmark runner programmatically
    run_detailed_analysis();
}

fn run_detailed_analysis() {
    println!("\n🔍 Running detailed analysis...");
    
    // This is a placeholder for more advanced analysis
    // In a real implementation, you would run the benchmarks and collect results
    // for detailed analysis
    
    let config = BenchmarkConfig::default();
    
    // Simulate some benchmark results for demonstration
    let mut results = Vec::new();
    
    // Add sample results
    results.push(BenchmarkResult {
        name: "swap_basic".to_string(),
        avg_time_ms: 2.5,
        min_time_ms: 1.2,
        max_time_ms: 4.8,
        iterations: 10,
        total_time_ms: 25.0,
        instructions: 50_000,
    });
    
    results.push(BenchmarkResult {
        name: "query_get_portfolio".to_string(),
        avg_time_ms: 1.2,
        min_time_ms: 0.8,
        max_time_ms: 2.1,
        iterations: 100,
        total_time_ms: 120.0,
        instructions: 25_000,
    });
    
    // Run regression detection
    let mut detector = RegressionDetector::new(10.0); // 10% threshold
    
    // Try to load baseline data (if it exists)
    if let Err(e) = detector.load_baseline_from_file("baseline_performance.json") {
        println!("⚠️  Could not load baseline data: {}", e);
        println!("   This is expected on first run. Baseline will be created after this run.");
    }
    
    // Print regression report
    println!("{}", detector.format_regression_report(&results));
    
    // Run optimization analysis
    let opportunities = OptimizationAnalyzer::analyze_results(&results);
    if !opportunities.is_empty() {
        println!("💡 OPTIMIZATION OPPORTUNITIES:");
        for opp in opportunities {
            println!("   • {}", opp);
        }
    } else {
        println!("✅ No obvious optimization opportunities detected in sample data.");
    }
    
    // Save current results as baseline
    save_current_results_as_baseline(&results);
}

fn save_current_results_as_baseline(results: &[BenchmarkResult]) {
    let mut baseline_data = HashMap::new();
    for result in results {
        baseline_data.insert(result.name.clone(), result.avg_time_ms);
    }
    
    match serde_json::to_string_pretty(&baseline_data) {
        Ok(json) => {
            match File::create("baseline_performance.json") {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(json.as_bytes()) {
                        eprintln!("❌ Error writing baseline file: {}", e);
                    } else {
                        println!("💾 Baseline performance data saved to baseline_performance.json");
                    }
                },
                Err(e) => eprintln!("❌ Error creating baseline file: {}", e),
            }
        },
        Err(e) => eprintln!("❌ Error serializing baseline data: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regression_detector() {
        let mut detector = RegressionDetector::new(10.0); // 10% threshold
        
        // Add a baseline
        detector.baseline_data.insert("fast_op".to_string(), 1.0);
        
        // Create a result that's within threshold (no regression)
        let good_result = BenchmarkResult {
            name: "fast_op".to_string(),
            avg_time_ms: 1.05, // 5% increase - within threshold
            min_time_ms: 0.9,
            max_time_ms: 1.2,
            iterations: 10,
            total_time_ms: 10.5,
            instructions: 1000,
        };
        
        assert!(!detector.detect_regression(&good_result));
        
        // Create a result that exceeds threshold (regression)
        let bad_result = BenchmarkResult {
            name: "fast_op".to_string(),
            avg_time_ms: 1.5, // 50% increase - exceeds threshold
            min_time_ms: 1.2,
            max_time_ms: 1.8,
            iterations: 10,
            total_time_ms: 15.0,
            instructions: 1000,
        };
        
        assert!(detector.detect_regression(&bad_result));
    }

    #[test]
    fn test_optimization_analyzer() {
        let results = vec![
            BenchmarkResult {
                name: "slow_op".to_string(),
                avg_time_ms: 15.0, // Slow operation
                min_time_ms: 10.0,
                max_time_ms: 20.0,
                iterations: 10,
                total_time_ms: 150.0,
                instructions: 150_000, // High instruction count
            }
        ];
        
        let opportunities = OptimizationAnalyzer::analyze_results(&results);
        assert!(!opportunities.is_empty());
        assert!(opportunities.iter().any(|o| o.contains("Slow operation")));
        assert!(opportunities.iter().any(|o| o.contains("High instruction count")));
    }
}