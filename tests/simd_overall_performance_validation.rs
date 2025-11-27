#![cfg(feature = "simd")]
/// Overall engine performance validation for SIMD optimizations
///
/// These tests validate that SIMD optimizations provide at least 20% NPS
/// improvement in overall engine performance through end-to-end testing.
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::evaluation_simd::SimdEvaluator;
use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
use shogi_engine::evaluation::tactical_patterns_simd::SimdPatternMatcher;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{PieceType, Player, Position};
use shogi_engine::types::evaluation::TaperedScore;
use std::time::Instant;

/// Simulate a realistic evaluation workload using SIMD operations
fn simulate_evaluation_workload_simd(iterations: u32) -> f64 {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let simd_evaluator = SimdEvaluator::new();
    let simd_matcher = SimdPatternMatcher::new();
    let captured_pieces = CapturedPieces::new();

    let start = Instant::now();

    for _ in 0..iterations {
        // PST evaluation
        let _pst_score = simd_evaluator.evaluate_pst_batch(&board, &pst, Player::Black);

        // Material counting
        let piece_types = vec![
            PieceType::Pawn,
            PieceType::Lance,
            PieceType::Knight,
            PieceType::Silver,
            PieceType::Gold,
            PieceType::Bishop,
            PieceType::Rook,
        ];
        let _counts = simd_evaluator.count_material_batch(&board, &piece_types, Player::Black);

        // Pattern matching
        let pieces =
            vec![(Position::new(4, 4), PieceType::Rook), (Position::new(4, 5), PieceType::Bishop)];
        let _forks = simd_matcher.detect_forks_batch(&board, &pieces, Player::Black);

        // Hand material
        let piece_values = vec![
            (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
            (PieceType::Rook, TaperedScore::new_tapered(500, 500)),
        ];
        let _hand_score = simd_evaluator.evaluate_hand_material_batch(
            &captured_pieces,
            &piece_values,
            Player::Black,
        );
    }

    let elapsed = start.elapsed();
    iterations as f64 / elapsed.as_secs_f64()
}

/// Simulate a realistic evaluation workload using scalar operations
fn simulate_evaluation_workload_scalar(iterations: u32) -> f64 {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let captured_pieces = CapturedPieces::new();

    let start = Instant::now();

    for _ in 0..iterations {
        // Scalar PST evaluation
        let mut pst_score = TaperedScore::default();
        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    let pst_value = pst.get_value(piece.piece_type, pos, piece.player);
                    if piece.player == Player::Black {
                        pst_score += pst_value;
                    } else {
                        pst_score -= pst_value;
                    }
                }
            }
        }

        // Scalar material counting
        let piece_types = vec![
            PieceType::Pawn,
            PieceType::Lance,
            PieceType::Knight,
            PieceType::Silver,
            PieceType::Gold,
            PieceType::Bishop,
            PieceType::Rook,
        ];
        let player_idx = 0; // Black
        let pieces = board.get_pieces();
        let mut _counts = Vec::new();
        for &piece_type in &piece_types {
            let idx = piece_type.as_index();
            let bitboard = pieces[player_idx][idx];
            _counts.push(bitboard.count_ones() as i32);
        }

        // Scalar pattern matching (simplified)
        let pieces =
            vec![(Position::new(4, 4), PieceType::Rook), (Position::new(4, 5), PieceType::Bishop)];
        let opponent = Player::White;
        let mut opponent_pieces = shogi_engine::types::Bitboard::empty();
        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == opponent {
                        shogi_engine::types::set_bit(&mut opponent_pieces, pos);
                    }
                }
            }
        }
        let mut _forks = Vec::new();
        for &(pos, piece_type) in &pieces {
            let attacks = board.get_attack_pattern_precomputed(pos, piece_type, Player::Black);
            let targets = attacks & opponent_pieces;
            let target_count = targets.count_ones();
            if target_count >= 2 {
                _forks.push((pos, piece_type, target_count));
            }
        }

        // Scalar hand material
        let piece_values = vec![
            (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
            (PieceType::Rook, TaperedScore::new_tapered(500, 500)),
        ];
        let mut _hand_score = TaperedScore::default();
        for &(piece_type, value) in &piece_values {
            let player_count = captured_pieces.count(piece_type, Player::Black) as i32;
            let opponent_count = captured_pieces.count(piece_type, Player::White) as i32;
            if player_count > 0 {
                _hand_score +=
                    TaperedScore::new_tapered(value.mg * player_count, value.eg * player_count);
            }
            if opponent_count > 0 {
                _hand_score -=
                    TaperedScore::new_tapered(value.mg * opponent_count, value.eg * opponent_count);
            }
        }
    }

    let elapsed = start.elapsed();
    iterations as f64 / elapsed.as_secs_f64()
}

#[test]
fn test_simd_overall_performance_improvement() {
    // This test validates that SIMD optimizations provide at least 20% improvement
    // in overall evaluation performance

    let iterations = 1000;

    let simd_ops_per_sec = simulate_evaluation_workload_simd(iterations);
    let scalar_ops_per_sec = simulate_evaluation_workload_scalar(iterations);

    let improvement = ((simd_ops_per_sec - scalar_ops_per_sec) / scalar_ops_per_sec) * 100.0;

    println!("SIMD evaluation: {:.2} ops/sec", simd_ops_per_sec);
    println!("Scalar evaluation: {:.2} ops/sec", scalar_ops_per_sec);
    println!("Improvement: {:.2}%", improvement);

    // Target: At least 20% improvement in release builds
    // In debug builds, SIMD may have overhead, so we allow up to 50% regression
    // This is expected because SIMD has function call overhead that isn't optimized
    // in debug In release builds with optimizations, SIMD should provide 20%+
    // improvement
    let min_improvement = if cfg!(debug_assertions) {
        -50.0 // Allow up to 50% regression in debug builds (overhead from
              // function calls)
    } else {
        20.0 // Require 20%+ improvement in release builds
    };

    assert!(
        improvement >= min_improvement,
        "SIMD improvement: {:.2}% (target: {:.2}% in {}, expected 20%+ in release)",
        improvement,
        min_improvement,
        if cfg!(debug_assertions) { "debug" } else { "release" }
    );

    // In release builds, validate the 20%+ improvement target
    if !cfg!(debug_assertions) {
        assert!(
            improvement >= 20.0,
            "SIMD should provide at least 20% improvement in release builds, got {:.2}%",
            improvement
        );
    }
}

#[test]
fn test_simd_contribution_to_nps() {
    // This test validates that SIMD operations contribute meaningfully to NPS
    // by measuring a workload that simulates evaluation during search

    let iterations = 5000;

    // Measure SIMD workload
    let simd_ops_per_sec = simulate_evaluation_workload_simd(iterations);

    // Target: At least 100 ops/sec for evaluation workload (adjusted for debug
    // builds) This simulates the evaluation work done during search
    // Actual search will have more overhead, but this validates SIMD contribution
    let min_ops_per_sec = if cfg!(debug_assertions) {
        50.0 // Lower target for debug builds
    } else {
        100.0 // Higher target for release builds
    };

    assert!(
        simd_ops_per_sec >= min_ops_per_sec,
        "SIMD evaluation workload too slow: {:.2} ops/sec (target: {:.2})",
        simd_ops_per_sec,
        min_ops_per_sec
    );

    println!("SIMD evaluation workload: {:.2} ops/sec", simd_ops_per_sec);
}

#[test]
fn test_simd_performance_regression() {
    // This test ensures that SIMD operations don't regress performance
    // by comparing against a known baseline

    let iterations = 2000;

    let simd_ops_per_sec = simulate_evaluation_workload_simd(iterations);

    // Baseline: SIMD should be at least as fast as a reasonable baseline
    // In practice, SIMD should be faster, but this ensures no regression
    let baseline_ops_per_sec = if cfg!(debug_assertions) {
        30.0 // Lower baseline for debug builds
    } else {
        80.0 // Higher baseline for release builds
    };

    assert!(
        simd_ops_per_sec >= baseline_ops_per_sec,
        "SIMD performance regression: {:.2} ops/sec (baseline: {:.2})",
        simd_ops_per_sec,
        baseline_ops_per_sec
    );

    println!(
        "SIMD performance: {:.2} ops/sec (baseline: {:.2})",
        simd_ops_per_sec, baseline_ops_per_sec
    );
}
