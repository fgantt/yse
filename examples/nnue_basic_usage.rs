//! Basic example of using NNUE evaluation in the Shogi engine
//!
//! This example demonstrates how to:
//! - Enable NNUE evaluation with default random weights
//! - Load NNUE weights from a file
//! - Evaluate positions using NNUE
//! - Save NNUE weights to a file

use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::{BitboardBoard, CapturedPieces, Player};

fn main() {
    println!("=== NNUE Evaluation Usage Examples ===\n");

    // Example 1: Enable NNUE with default random weights
    println!("Example 1: Enable NNUE with random weights");
    let mut evaluator = PositionEvaluator::new();

    // Enable NNUE with architecture: 256 hidden neurons in first layer, 32 in second
    evaluator.enable_nnue(256, 32);
    println!("✓ NNUE enabled with architecture: 2268 features -> 256 -> 32 -> 1");
    println!("  (This uses randomly initialized weights - not trained yet)\n");

    // Evaluate a position
    let board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();
    let score = evaluator.evaluate(&board, Player::Black, &captured_pieces);
    println!("  Initial position evaluation: {} centipawns\n", score);

    // Example 2: Load trained NNUE weights from a file
    println!("Example 2: Load trained NNUE weights from file");
    let mut evaluator2 = PositionEvaluator::new();
    
    // Uncomment and provide path to your trained weights file:
    /*
    match evaluator2.enable_nnue_with_weights("path/to/nnue_weights.json") {
        Ok(()) => {
            println!("✓ NNUE weights loaded successfully");
            let score = evaluator2.evaluate(&board, Player::Black, &captured_pieces);
            println!("  Position evaluation with trained weights: {} centipawns", score);
        }
        Err(e) => eprintln!("✗ Failed to load NNUE weights: {}", e),
    }
    */
    println!("  (Uncomment code above and provide weights file path)\n");

    // Example 3: Save current NNUE weights
    println!("Example 3: Save NNUE weights to file");
    let mut evaluator3 = PositionEvaluator::new();
    evaluator3.enable_nnue(512, 128); // Larger architecture
    
    // Uncomment to save:
    /*
    match evaluator3.save_nnue_weights("my_nnue_weights.json") {
        Ok(()) => println!("✓ NNUE weights saved to my_nnue_weights.json"),
        Err(e) => eprintln!("✗ Failed to save NNUE weights: {}", e),
    }
    */
    println!("  (Uncomment code above to save weights)\n");

    // Example 4: Check status and toggle
    println!("Example 4: Check NNUE status");
    if evaluator.is_nnue_enabled() {
        println!("✓ NNUE is currently enabled");
    }

    // Disable NNUE (falls back to traditional evaluation)
    evaluator.disable_nnue();
    println!("✓ NNUE disabled - now using traditional evaluation");

    let score_traditional = evaluator.evaluate(&board, Player::Black, &captured_pieces);
    println!("  Traditional evaluation: {} centipawns\n", score_traditional);

    println!("=== Integration with Search Engine ===");
    println!("\nThe SearchEngine automatically uses NNUE when enabled:");
    println!("  use shogi_engine::search::search_engine::SearchEngine;");
    println!("  let mut search = SearchEngine::new(None, 16);");
    println!("  search.evaluator.enable_nnue(256, 32);");
    println!("\nAll searches will now use NNUE evaluation automatically!");
}
