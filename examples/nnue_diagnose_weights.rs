//! Diagnose NNUE weight magnitudes
//!
//! This example checks the magnitude of weights in trained vs random NNUE models

use shogi_engine::evaluation::nnue::NNUEWeights;

fn main() {
    println!("=== NNUE Weight Diagnosis ===\n");

    // Analyze random weights
    println!("1. Random Weights Analysis:");
    println!("{:-<80}", "");
    let random_weights = NNUEWeights::new(256, 32);
    analyze_weights(&random_weights, "Random");

    // Analyze trained weights
    println!("\n2. Trained Weights Analysis:");
    println!("{:-<80}", "");
    match NNUEWeights::load("nnue_weights_trained.json") {
        Ok(trained_weights) => {
            analyze_weights(&trained_weights, "Trained");
            
            // Compare
            println!("\n3. Comparison:");
            println!("{:-<80}", "");
            compare_weights(&random_weights, &trained_weights);
        }
        Err(e) => {
            println!("Could not load trained weights: {}", e);
        }
    }
}

fn analyze_weights(weights: &NNUEWeights, label: &str) {
    // Analyze output bias
    println!("{} Output Bias: {}", label, weights.output_bias);
    println!("  Magnitude: {}", weights.output_bias.abs());
    
    // Analyze output weights
    let output_weights_stats = compute_stats_i16(&weights.output_weights);
    println!("{} Output Weights:", label);
    println!("  Count: {}", weights.output_weights.len());
    println!("  Min: {}, Max: {}, Avg: {:.2}, StdDev: {:.2}", 
             output_weights_stats.min, 
             output_weights_stats.max,
             output_weights_stats.avg,
             output_weights_stats.stddev);
    
    // Analyze input weights (sample)
    if !weights.input_weights_1.is_empty() {
        let mut all_input_weights = Vec::new();
        for feature_weights in &weights.input_weights_1 {
            all_input_weights.extend(feature_weights.iter().cloned());
        }
        let input_weights_stats = compute_stats_i16(&all_input_weights);
        println!("{} Input Weights (sample):", label);
        println!("  Total weights: {}", all_input_weights.len());
        println!("  Min: {}, Max: {}, Avg: {:.2}, StdDev: {:.2}", 
                 input_weights_stats.min, 
                 input_weights_stats.max,
                 input_weights_stats.avg,
                 input_weights_stats.stddev);
    }
    
    // Analyze hidden biases
    let bias_1_stats = compute_stats_i32(&weights.hidden_biases_1);
    println!("{} Hidden Layer 1 Biases:", label);
    println!("  Count: {}", weights.hidden_biases_1.len());
    println!("  Min: {}, Max: {}, Avg: {:.2}, StdDev: {:.2}", 
             bias_1_stats.min, 
             bias_1_stats.max,
             bias_1_stats.avg,
             bias_1_stats.stddev);
    
    if let Some(ref biases_2) = weights.hidden_biases_2 {
        let bias_2_stats = compute_stats_i32(biases_2);
        println!("{} Hidden Layer 2 Biases:", label);
        println!("  Count: {}", biases_2.len());
        println!("  Min: {}, Max: {}, Avg: {:.2}, StdDev: {:.2}", 
                 bias_2_stats.min, 
                 bias_2_stats.max,
                 bias_2_stats.avg,
                 bias_2_stats.stddev);
    }
}

fn compare_weights(random: &NNUEWeights, trained: &NNUEWeights) {
    let bias_diff = (trained.output_bias - random.output_bias).abs();
    println!("Output Bias Difference: {} (trained: {}, random: {})", 
             bias_diff, trained.output_bias, random.output_bias);
    
    if bias_diff > 10000 {
        println!("⚠️  WARNING: Output bias difference is very large!");
        println!("   This suggests weights may have grown too large during training.");
    }
    
    // Compare output weights
    if trained.output_weights.len() == random.output_weights.len() {
        let mut total_diff = 0i64;
        let mut max_diff = 0i64;
        for (t, r) in trained.output_weights.iter().zip(random.output_weights.iter()) {
            let diff = ((*t as i32) - (*r as i32)).abs() as i64;
            total_diff += diff;
            max_diff = max_diff.max(diff);
        }
        let avg_diff = total_diff / trained.output_weights.len() as i64;
        println!("Output Weights Average Difference: {}", avg_diff);
        println!("Output Weights Max Difference: {}", max_diff);
    }
}

struct Stats {
    min: i32,
    max: i32,
    avg: f64,
    stddev: f64,
}

fn compute_stats_i16(values: &[i16]) -> Stats {
    if values.is_empty() {
        return Stats { min: 0, max: 0, avg: 0.0, stddev: 0.0 };
    }
    
    let min = *values.iter().min().unwrap() as i32;
    let max = *values.iter().max().unwrap() as i32;
    let sum: i64 = values.iter().map(|&v| v as i64).sum();
    let avg = sum as f64 / values.len() as f64;
    
    let variance: f64 = values.iter()
        .map(|&v| {
            let diff = (v as f64) - avg;
            diff * diff
        })
        .sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();
    
    Stats { min, max, avg, stddev }
}

fn compute_stats_i32(values: &[i32]) -> Stats {
    if values.is_empty() {
        return Stats { min: 0, max: 0, avg: 0.0, stddev: 0.0 };
    }
    
    let min = *values.iter().min().unwrap();
    let max = *values.iter().max().unwrap();
    let sum: i64 = values.iter().map(|&v| v as i64).sum();
    let avg = sum as f64 / values.len() as f64;
    
    let variance: f64 = values.iter()
        .map(|&v| {
            let diff = (v as f64) - avg;
            diff * diff
        })
        .sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();
    
    Stats { min, max, avg, stddev }
}

