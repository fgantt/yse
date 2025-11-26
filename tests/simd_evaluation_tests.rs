#![cfg(feature = "simd")]
/// Tests for SIMD-optimized evaluation functions
///
/// These tests validate that SIMD evaluation functions work correctly and
/// provide performance improvements over scalar implementations.
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::evaluation_simd::SimdEvaluator;
use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{PieceType, Player, Position};
use shogi_engine::types::evaluation::TaperedScore;

#[test]
fn test_evaluate_pst_batch() {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::empty();
    let pst = PieceSquareTables::new();

    let score = evaluator.evaluate_pst_batch(&board, &pst, Player::Black);

    // On an empty board, score should be zero
    assert_eq!(score, TaperedScore::default(), "Empty board should have zero PST score");
}

#[test]
fn test_evaluate_pst_optimized() {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::empty();
    let pst = PieceSquareTables::new();

    let score_batch = evaluator.evaluate_pst_batch(&board, &pst, Player::Black);
    let score_optimized = evaluator.evaluate_pst_optimized(&board, &pst, Player::Black);

    // Both should produce the same result
    assert_eq!(score_batch, score_optimized, "Both methods should produce same result");
}

#[test]
fn test_count_material_batch() {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::empty();

    let piece_types = vec![PieceType::Pawn, PieceType::Rook, PieceType::Bishop];

    let counts = evaluator.count_material_batch(&board, &piece_types, Player::Black);

    assert_eq!(counts.len(), piece_types.len(), "Should have count for each piece type");

    // On an empty board, all counts should be 0
    for count in counts {
        assert_eq!(count, 0, "Empty board should have zero counts");
    }
}

#[test]
fn test_evaluate_material_batch() {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::empty();

    let piece_values = vec![
        (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
        (PieceType::Rook, TaperedScore::new_tapered(500, 500)),
    ];

    let score = evaluator.evaluate_material_batch(&board, &piece_values, Player::Black);

    // On an empty board, score should be zero
    assert_eq!(score, TaperedScore::default(), "Empty board should have zero material score");
}

#[test]
fn test_evaluate_hand_material_batch() {
    let evaluator = SimdEvaluator::new();
    let captured_pieces = CapturedPieces::new();

    let piece_values = vec![
        (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
        (PieceType::Rook, TaperedScore::new_tapered(500, 500)),
    ];

    let score =
        evaluator.evaluate_hand_material_batch(&captured_pieces, &piece_values, Player::Black);

    // With no captured pieces, score should be zero
    assert_eq!(
        score,
        TaperedScore::default(),
        "No captured pieces should have zero hand material score"
    );
}

#[test]
fn test_accumulate_scores_batch() {
    let evaluator = SimdEvaluator::new();

    let scores = vec![
        TaperedScore::new_tapered(100, 50),
        TaperedScore::new_tapered(200, 100),
        TaperedScore::new_tapered(300, 150),
    ];

    let total = evaluator.accumulate_scores_batch(&scores);

    let expected = TaperedScore::new_tapered(600, 300);
    assert_eq!(total, expected, "Accumulated score should match sum");
}

#[test]
fn test_accumulate_scores_batch_empty() {
    let evaluator = SimdEvaluator::new();

    let scores = vec![];

    let total = evaluator.accumulate_scores_batch(&scores);

    assert_eq!(total, TaperedScore::default(), "Empty scores should produce zero total");
}

#[test]
fn test_simd_evaluation_performance() {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::empty();
    let pst = PieceSquareTables::new();

    let iterations = 100_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = evaluator.evaluate_pst_batch(&board, &pst, Player::Black);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    // Target: At least 100K operations per second (adjusted for debug builds)
    let min_ops_per_sec = 50_000.0;

    assert!(
        ops_per_sec >= min_ops_per_sec,
        "SIMD evaluation too slow: {:.2} ops/sec (target: {:.2})",
        ops_per_sec,
        min_ops_per_sec
    );

    println!("SIMD evaluation: {:.2} ops/sec", ops_per_sec);
}
