//! Evaluate trained NNUE weights
//!
//! This example compares random NNUE weights vs trained weights on benchmark positions

use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::evaluation::nnue::NNUEWeights;
use shogi_engine::BitboardBoard;
use shogi_engine::search::performance_tuning::load_standard_positions;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;

fn main() {
    println!("=== NNUE Weight Evaluation ===\n");

    // Load benchmark positions
    let benchmark_positions = match load_standard_positions() {
        Ok(positions) => positions,
        Err(e) => {
            eprintln!("Failed to load benchmark positions: {}", e);
            eprintln!("Using default starting position instead.");
            vec![]
        }
    };

    // Prepare test positions
    let mut positions = Vec::new();
    
    if benchmark_positions.is_empty() {
        // Fallback to starting position
        positions.push(("Starting Position", BitboardBoard::new(), CapturedPieces::new(), Player::Black));
    } else {
        // Use benchmark positions
        for bp in &benchmark_positions {
            match BitboardBoard::from_fen(&bp.fen) {
                Ok((board, player, captured)) => {
                    positions.push((&bp.name, board, captured, player));
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse FEN for {}: {}", bp.name, e);
                }
            }
        }
    }

    if positions.is_empty() {
        eprintln!("No valid positions to evaluate!");
        return;
    }

    println!("Evaluating {} positions...\n", positions.len());

    // Test 1: Random weights
    println!("1. Testing with random weights:");
    println!("{:-<80}", "");
    let random_weights = NNUEWeights::new(256, 32);
    let mut evaluator_random = PositionEvaluator::new();
    evaluator_random.enable_nnue_with_weights_internal(random_weights);

    for (name, board, captured, player) in &positions {
        let score = evaluator_random.evaluate(board, *player, captured);
        println!("  {:30} | Player: {:?} | Score: {:6} centipawns", name, player, score);
    }

    // Test 2: Trained weights (if available)
    println!("\n2. Testing with trained weights:");
    println!("{:-<80}", "");
    let mut evaluator_trained = PositionEvaluator::new();
    
    match NNUEWeights::load("nnue_weights_trained.json") {
        Ok(weights) => {
            evaluator_trained.enable_nnue_with_weights_internal(weights);
            
            for (name, board, captured, player) in &positions {
                let score = evaluator_trained.evaluate(board, *player, captured);
                println!("  {:30} | Player: {:?} | Score: {:6} centipawns", name, player, score);
            }
        }
        Err(e) => {
            println!("  Could not load trained weights: {}", e);
            println!("  Run training first: cargo run --bin nnue_trainer");
            println!("  Expected file: nnue_weights_trained.json");
        }
    }

    // Test 3: Traditional evaluation for comparison
    println!("\n3. Testing with traditional evaluation:");
    println!("{:-<80}", "");
    let mut evaluator_traditional = PositionEvaluator::new();
    evaluator_traditional.disable_nnue();

    for (name, board, captured, player) in &positions {
        let score = evaluator_traditional.evaluate(board, *player, captured);
        println!("  {:30} | Player: {:?} | Score: {:6} centipawns", name, player, score);
    }

    // Comparison summary
    println!("\n=== Comparison Summary ===");
    println!("{:-<80}", "");
    
    if let Ok(trained_weights) = NNUEWeights::load("nnue_weights_trained.json") {
        let mut evaluator_random = PositionEvaluator::new();
        evaluator_random.enable_nnue_with_weights_internal(NNUEWeights::new(256, 32));
        
        let mut evaluator_trained = PositionEvaluator::new();
        evaluator_trained.enable_nnue_with_weights_internal(trained_weights);
        
        let mut evaluator_traditional = PositionEvaluator::new();
        evaluator_traditional.disable_nnue();

        let mut total_diff_random_trained = 0i64;
        let mut total_diff_trained_traditional = 0i64;
        let mut count = 0;

        for (_name, board, captured, player) in &positions {
            let random_score = evaluator_random.evaluate(board, *player, captured);
            let trained_score = evaluator_trained.evaluate(board, *player, captured);
            let traditional_score = evaluator_traditional.evaluate(board, *player, captured);

            let diff_random_trained = (trained_score - random_score).abs() as i64;
            let diff_trained_traditional = (trained_score - traditional_score).abs() as i64;

            total_diff_random_trained += diff_random_trained;
            total_diff_trained_traditional += diff_trained_traditional;
            count += 1;
        }

        if count > 0 {
            let avg_diff_random_trained = total_diff_random_trained / count;
            let avg_diff_trained_traditional = total_diff_trained_traditional / count;

            println!("Average difference (Random vs Trained): {} centipawns", avg_diff_random_trained);
            println!("Average difference (Trained vs Traditional): {} centipawns", avg_diff_trained_traditional);
            println!("\nInterpretation:");
            println!("  - Large difference (Random vs Trained): Training changed the model");
            println!("  - Small difference (Trained vs Traditional): Model agrees with traditional eval");
            println!("  - Large difference (Trained vs Traditional): Model learned different patterns");
        }
    }
}

