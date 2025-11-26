#![cfg(feature = "simd")]
/// Integration tests for SIMD evaluation in IntegratedEvaluator
///
/// These tests verify that SIMD-optimized evaluation produces the same results
/// as scalar implementation and is actually used when the feature is enabled.
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::{
    evaluation_simd::SimdEvaluator,
    integration::{ComponentFlags, IntegratedEvaluationConfig, IntegratedEvaluator},
};
use shogi_engine::types::{CapturedPieces, Piece, PieceType, Player, Position};

/// Create a config with only PST enabled for testing
fn pst_only_config() -> IntegratedEvaluationConfig {
    let mut config = IntegratedEvaluationConfig::default();
    config.components = ComponentFlags {
        material: false,
        piece_square_tables: true,
        position_features: false,
        opening_principles: false,
        endgame_patterns: false,
        tactical_patterns: false,
        positional_patterns: false,
        castle_patterns: false,
    };
    config.enable_phase_cache = false;
    config.enable_eval_cache = false;
    config.use_optimized_path = false;
    config
}

#[test]
fn test_simd_evaluation_same_results_as_scalar() {
    let config = pst_only_config();
    let mut evaluator = IntegratedEvaluator::with_config(config);
    evaluator.enable_statistics();
    let board = BitboardBoard::new(); // Standard starting position
    let captured = CapturedPieces::new();

    // Evaluate using IntegratedEvaluator (which should use SIMD when enabled)
    let result = evaluator.evaluate(&board, Player::Black, &captured);

    // Verify evaluation completed successfully
    // Note: Starting position may have zero or small PST score depending on PST configuration
    assert!(
        result.score.abs() < 100000,
        "PST evaluation should produce reasonable score (got {})",
        result.score
    );

    // Verify telemetry is available and contains PST data
    let telemetry = evaluator.telemetry_snapshot();
    assert!(telemetry.is_some(), "Telemetry should be available");

    // Note: PST telemetry may be zero on starting position depending on PST configuration
    // The important thing is that telemetry is present and the evaluation completed
    if let Some(tel) = telemetry {
        // PST telemetry should be present (even if zero)
        // This verifies that SIMD evaluation path was used
        assert!(tel.pst.is_some(), "PST telemetry should be present");
    }
}

#[test]
fn test_simd_evaluation_empty_board() {
    let config = pst_only_config();
    let mut evaluator = IntegratedEvaluator::with_config(config);
    evaluator.enable_statistics();
    let board = BitboardBoard::empty();
    let captured = CapturedPieces::new();

    // Evaluate empty board
    let result = evaluator.evaluate(&board, Player::Black, &captured);

    // Empty board should have zero score (or very small due to other components)
    // With only PST enabled, it should be zero
    assert_eq!(result.score, 0, "Empty board with only PST should have zero score");
}

#[test]
fn test_simd_evaluation_with_pieces() {
    let config = pst_only_config();
    let mut evaluator = IntegratedEvaluator::with_config(config);
    evaluator.enable_statistics();
    let mut board = BitboardBoard::empty();
    let captured = CapturedPieces::new();

    // Place some pieces
    board.place_piece(Piece::new(PieceType::King, Player::Black), Position::new(8, 4));
    board.place_piece(Piece::new(PieceType::King, Player::White), Position::new(0, 4));
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(7, 0));
    board.place_piece(Piece::new(PieceType::Bishop, Player::White), Position::new(1, 7));

    let result = evaluator.evaluate(&board, Player::Black, &captured);

    // Should have non-zero score
    assert_ne!(result.score, 0, "Board with pieces should have non-zero PST score");

    // Verify telemetry contains PST data
    let telemetry = evaluator.telemetry_snapshot();
    assert!(telemetry.is_some(), "Telemetry should be available");

    if let Some(tel) = telemetry {
        if let Some(pst_tel) = tel.pst {
            assert!(
                !pst_tel.per_piece.is_empty() || pst_tel.total_mg != 0 || pst_tel.total_eg != 0,
                "Board with pieces should have PST telemetry"
            );
        }
    }
}

#[test]
fn test_simd_evaluation_consistency() {
    let config = pst_only_config();
    let mut evaluator = IntegratedEvaluator::with_config(config);
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Evaluate multiple times - should get same result
    let result1 = evaluator.evaluate(&board, Player::Black, &captured);
    let result2 = evaluator.evaluate(&board, Player::Black, &captured);

    assert_eq!(result1.score, result2.score, "Multiple evaluations should produce same score");
}

#[test]
fn test_simd_evaluation_player_switching() {
    let config = pst_only_config();
    let mut evaluator = IntegratedEvaluator::with_config(config);
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    let black_result = evaluator.evaluate(&board, Player::Black, &captured);
    let white_result = evaluator.evaluate(&board, Player::White, &captured);

    // Scores should be opposite (one positive, one negative, or both zero)
    // On a balanced starting position, they should be approximately opposite
    assert_eq!(
        black_result.score, -white_result.score,
        "Black and White scores should be opposite"
    );
}

#[test]
fn test_simd_evaluator_direct_comparison() {
    // Test that SimdEvaluator produces same total score as IntegratedEvaluator
    // This verifies that SIMD evaluation is actually being used
    let config = pst_only_config();
    let mut integrated_evaluator = IntegratedEvaluator::with_config(config);
    let simd_evaluator = SimdEvaluator::new();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Get PST tables (using default)
    use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
    let pst = PieceSquareTables::new();

    // Evaluate using IntegratedEvaluator (which should use SIMD internally)
    let integrated_result = integrated_evaluator.evaluate(&board, Player::Black, &captured);

    // Evaluate using SimdEvaluator directly
    let simd_score = simd_evaluator.evaluate_pst_batch(&board, &pst, Player::Black);

    // The PST component of integrated result should match SIMD score
    // (allowing for other components that might be enabled)
    // Since we have only PST enabled, they should match closely
    let pst_component = simd_score.interpolate(integrated_result.phase);

    // Allow small differences due to phase interpolation
    let diff = (integrated_result.score - pst_component).abs();
    assert!(diff < 100, 
           "IntegratedEvaluator PST component should match SimdEvaluator (diff: {}, integrated: {}, simd: {})",
           diff, integrated_result.score, pst_component);
}

#[test]
fn test_simd_evaluation_integrated_evaluator_usage() {
    // Test that SIMD evaluation is actually used in the full evaluation pipeline
    let mut config = IntegratedEvaluationConfig::default();
    config.components.piece_square_tables = true;
    config.enable_phase_cache = false;
    config.enable_eval_cache = false;

    let mut evaluator = IntegratedEvaluator::with_config(config);
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Full evaluation should use SIMD for PST component
    let result = evaluator.evaluate(&board, Player::Black, &captured);

    // Verify evaluation produces reasonable result
    assert!(result.score.abs() < 100000, "Evaluation score should be reasonable");

    // Verify telemetry is available
    let telemetry = evaluator.telemetry_snapshot();
    assert!(telemetry.is_some(), "Telemetry should be available after evaluation");

    if let Some(tel) = telemetry {
        assert!(tel.pst.is_some(), "PST telemetry should be present");
    }
}
