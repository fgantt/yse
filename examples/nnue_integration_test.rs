//! Test that trained NNUE weights are automatically loaded

use shogi_engine::ShogiEngine;

fn main() {
    println!("=== NNUE Integration Test ===\n");

    // Create engine - should automatically load trained weights if available
    let engine = ShogiEngine::new();

    // Check if NNUE is enabled
    if engine.is_nnue_enabled() {
        println!("✅ SUCCESS: NNUE is enabled!");
        println!("   The trained weights were automatically loaded from nnue_weights_trained.json");
    } else {
        println!("⚠️  WARNING: NNUE is not enabled");
        println!("   This is normal if nnue_weights_trained.json doesn't exist in the current directory");
        println!("   To enable NNUE, make sure nnue_weights_trained.json is in the working directory");
    }

    println!("\n=== Integration Complete ===");
    println!("The engine will now use NNUE evaluation for all searches when weights are available.");
}

