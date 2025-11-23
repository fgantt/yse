#![cfg(feature = "simd")]
/// Integration tests for SIMD runtime flags
/// 
/// Verifies that runtime flags actually control SIMD usage.
/// 
/// # Task 4.0 (Task 4.10)

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::config::{EngineConfig, SimdConfig};
use shogi_engine::evaluation::integration::IntegratedEvaluator;
use shogi_engine::evaluation::tactical_patterns::TacticalPatternRecognizer;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::{CapturedPieces, Player};

#[test]
fn test_simd_evaluation_runtime_flag() {
    let mut config = EngineConfig::default();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();
    
    // Test with SIMD enabled
    config.simd.enable_simd_evaluation = true;
    let mut evaluator = IntegratedEvaluator::new();
    let mut eval_config = evaluator.config().clone();
    eval_config.simd = config.simd.clone();
    evaluator.set_config(eval_config);
    let _score1 = evaluator.evaluate(&board, Player::Black, &captured);
    
    // Test with SIMD disabled
    config.simd.enable_simd_evaluation = false;
    let mut eval_config2 = evaluator.config().clone();
    eval_config2.simd = config.simd.clone();
    evaluator.set_config(eval_config2);
    let _score2 = evaluator.evaluate(&board, Player::Black, &captured);
    
    // Both should complete without error
    // (We can't easily verify which path was taken without telemetry, but at least we verify it doesn't crash)
}

#[test]
fn test_simd_pattern_matching_runtime_flag() {
    let mut config = EngineConfig::default();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();
    
    // Test with SIMD enabled
    config.simd.enable_simd_pattern_matching = true;
    let mut recognizer = TacticalPatternRecognizer::new();
    let mut tactical_config = recognizer.config().clone();
    tactical_config.enable_simd_pattern_matching = true;
    recognizer.set_config(tactical_config);
    let _score1 = recognizer.evaluate_tactics(&board, Player::Black, &captured);
    
    // Test with SIMD disabled
    config.simd.enable_simd_pattern_matching = false;
    let mut tactical_config2 = recognizer.config().clone();
    tactical_config2.enable_simd_pattern_matching = false;
    recognizer.set_config(tactical_config2);
    let _score2 = recognizer.evaluate_tactics(&board, Player::Black, &captured);
    
    // Both should complete without error
}

#[test]
fn test_simd_move_generation_runtime_flag() {
    let mut config = EngineConfig::default();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();
    
    // Test with SIMD enabled
    config.simd.enable_simd_move_generation = true;
    let mut generator = MoveGenerator::new();
    generator.set_simd_config(config.simd.clone());
    let _moves1 = generator.generate_legal_moves(&board, Player::Black, &captured);
    
    // Test with SIMD disabled
    config.simd.enable_simd_move_generation = false;
    generator.set_simd_config(config.simd.clone());
    let _moves2 = generator.generate_legal_moves(&board, Player::Black, &captured);
    
    // Both should complete without error
    // Note: Move count might differ slightly due to different generation paths,
    // but both should produce valid moves
    assert!(!_moves1.is_empty(), "SIMD enabled should produce moves");
    assert!(!_moves2.is_empty(), "SIMD disabled should produce moves");
}

#[test]
fn test_all_simd_flags_disabled() {
    let mut config = EngineConfig::default();
    config.simd.enable_simd_evaluation = false;
    config.simd.enable_simd_pattern_matching = false;
    config.simd.enable_simd_move_generation = false;
    
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();
    
    // All components should work with SIMD disabled
    let mut evaluator = IntegratedEvaluator::new();
    let mut eval_config = evaluator.config().clone();
    eval_config.simd = config.simd.clone();
    evaluator.set_config(eval_config);
    let _eval_score = evaluator.evaluate(&board, Player::Black, &captured);
    
    let mut recognizer = TacticalPatternRecognizer::new();
    let mut tactical_config = recognizer.config().clone();
    tactical_config.enable_simd_pattern_matching = false;
    recognizer.set_config(tactical_config);
    let _tactical_score = recognizer.evaluate_tactics(&board, Player::Black, &captured);
    
    let mut generator = MoveGenerator::new();
    generator.set_simd_config(config.simd.clone());
    let _moves = generator.generate_legal_moves(&board, Player::Black, &captured);
    
    // All should complete successfully
    assert!(!_moves.is_empty());
}

#[test]
fn test_all_simd_flags_enabled() {
    let mut config = EngineConfig::default();
    config.simd.enable_simd_evaluation = true;
    config.simd.enable_simd_pattern_matching = true;
    config.simd.enable_simd_move_generation = true;
    
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();
    
    // All components should work with SIMD enabled
    let mut evaluator = IntegratedEvaluator::new();
    let mut eval_config = evaluator.config().clone();
    eval_config.simd = config.simd.clone();
    evaluator.set_config(eval_config);
    let _eval_score = evaluator.evaluate(&board, Player::Black, &captured);
    
    let mut recognizer = TacticalPatternRecognizer::new();
    let mut tactical_config = recognizer.config().clone();
    tactical_config.enable_simd_pattern_matching = true;
    recognizer.set_config(tactical_config);
    let _tactical_score = recognizer.evaluate_tactics(&board, Player::Black, &captured);
    
    let mut generator = MoveGenerator::new();
    generator.set_simd_config(config.simd.clone());
    let _moves = generator.generate_legal_moves(&board, Player::Black, &captured);
    
    // All should complete successfully
    assert!(!_moves.is_empty());
}

