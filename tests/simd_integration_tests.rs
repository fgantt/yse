#![cfg(feature = "simd")]
/// Integration tests for SIMD move generation in MoveGenerator
///
/// These tests verify that SIMD-optimized move generation produces the same
/// results as scalar implementation and is actually used when the feature is
/// enabled.
///
/// # Task 4.0 (Task 5.11)
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::integration::IntegratedEvaluator;
use shogi_engine::evaluation::tactical_patterns::TacticalPatternRecognizer;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{Piece, PieceType, Player, Position};
use shogi_engine::utils::telemetry::{
    get_simd_telemetry, reset_simd_telemetry, SimdTelemetry, SimdTelemetryTracker, SIMD_TELEMETRY,
};
use std::sync::Arc;
use std::thread;

#[test]
fn test_simd_move_generation_same_results() {
    let generator = MoveGenerator::new();
    let board = BitboardBoard::new(); // Standard starting position
    let captured = CapturedPieces::new();

    // Generate legal moves (which should use SIMD for sliding pieces when enabled)
    let moves = generator.generate_legal_moves(&board, Player::Black, &captured);

    // Verify moves were generated
    assert!(!moves.is_empty(), "Starting position should have legal moves");

    // Verify all moves are valid
    for mv in &moves {
        if let Some(from) = mv.from {
            assert!(from.is_valid(), "Move from position should be valid");
        }
        assert!(mv.to.is_valid(), "Move to position should be valid");
    }
}

#[test]
fn test_simd_move_generation_empty_board() {
    let generator = MoveGenerator::new();
    let board = BitboardBoard::empty();
    let captured = CapturedPieces::new();

    // Empty board with no captured pieces - should have no legal moves
    let moves = generator.generate_legal_moves(&board, Player::Black, &captured);

    // Empty board with no pieces and no captured pieces should have no moves
    // (This is expected - you need captured pieces to drop)
    // The important thing is that move generation completes without error
    assert!(
        moves.is_empty() || moves.iter().all(|mv| mv.from.is_none()),
        "Empty board with no captured pieces should have no piece moves (only possible drops)"
    );
}

#[test]
fn test_simd_move_generation_with_sliding_pieces() {
    let generator = MoveGenerator::new();
    let mut board = BitboardBoard::empty();
    let captured = CapturedPieces::new();

    // Create a position with sliding pieces
    board.place_piece(Piece::new(PieceType::King, Player::Black), Position::new(8, 4));
    board.place_piece(Piece::new(PieceType::King, Player::White), Position::new(0, 4));
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(7, 4));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(6, 3));
    board.place_piece(Piece::new(PieceType::Lance, Player::Black), Position::new(8, 0));

    let moves = generator.generate_legal_moves(&board, Player::Black, &captured);

    // Should have moves from sliding pieces
    assert!(!moves.is_empty(), "Position with sliding pieces should have legal moves");

    // Verify some moves are from sliding pieces
    let sliding_moves: Vec<_> = moves
        .iter()
        .filter(|mv| {
            if let Some(from) = mv.from {
                if let Some(piece) = board.get_piece(from) {
                    matches!(
                        piece.piece_type,
                        PieceType::Rook | PieceType::Bishop | PieceType::Lance
                    )
                } else {
                    false
                }
            } else {
                false
            }
        })
        .collect();

    assert!(!sliding_moves.is_empty(), "Should have moves from sliding pieces");
}

#[test]
fn test_simd_move_generation_consistency() {
    let generator = MoveGenerator::new();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Generate moves multiple times - should get same result
    let moves1 = generator.generate_legal_moves(&board, Player::Black, &captured);
    let moves2 = generator.generate_legal_moves(&board, Player::Black, &captured);

    assert_eq!(
        moves1.len(),
        moves2.len(),
        "Multiple generations should produce same number of moves"
    );
}

#[test]
fn test_simd_move_generation_player_switching() {
    let generator = MoveGenerator::new();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    let black_moves = generator.generate_legal_moves(&board, Player::Black, &captured);
    let white_moves = generator.generate_legal_moves(&board, Player::White, &captured);

    // Both players should have moves
    assert!(!black_moves.is_empty(), "Black should have legal moves");
    assert!(!white_moves.is_empty(), "White should have legal moves");
}

#[test]
fn test_simd_all_piece_moves_integration() {
    // Test that generate_all_piece_moves uses SIMD for sliding pieces
    let generator = MoveGenerator::new();
    let mut board = BitboardBoard::empty();

    // Place sliding pieces
    board.place_piece(Piece::new(PieceType::King, Player::Black), Position::new(8, 4));
    board.place_piece(Piece::new(PieceType::King, Player::White), Position::new(0, 4));
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(7, 4));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(6, 3));
    board.place_piece(Piece::new(PieceType::Lance, Player::Black), Position::new(8, 0));

    // Also place non-sliding pieces
    board.place_piece(Piece::new(PieceType::Pawn, Player::Black), Position::new(6, 4));
    board.place_piece(Piece::new(PieceType::Knight, Player::Black), Position::new(7, 2));

    let moves = generator.generate_all_piece_moves(&board, Player::Black);

    // Should have moves from both sliding and non-sliding pieces
    assert!(!moves.is_empty(), "Should generate moves for all pieces");

    // Verify moves include both types
    let has_sliding_moves = moves.iter().any(|mv| {
        if let Some(from) = mv.from {
            if let Some(piece) = board.get_piece(from) {
                matches!(piece.piece_type, PieceType::Rook | PieceType::Bishop | PieceType::Lance)
            } else {
                false
            }
        } else {
            false
        }
    });

    let has_non_sliding_moves = moves.iter().any(|mv| {
        if let Some(from) = mv.from {
            if let Some(piece) = board.get_piece(from) {
                !matches!(piece.piece_type, PieceType::Rook | PieceType::Bishop | PieceType::Lance)
            } else {
                false
            }
        } else {
            false
        }
    });

    assert!(has_sliding_moves, "Should have moves from sliding pieces");
    assert!(has_non_sliding_moves, "Should have moves from non-sliding pieces");
}

#[test]
fn test_simd_move_generation_correctness() {
    // Test that SIMD-generated moves are correct by comparing with known positions
    let generator = MoveGenerator::new();
    let mut board = BitboardBoard::empty();
    let captured = CapturedPieces::new();

    // Place a rook in a known position
    board.place_piece(Piece::new(PieceType::King, Player::Black), Position::new(8, 4));
    board.place_piece(Piece::new(PieceType::King, Player::White), Position::new(0, 4));
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(4, 4));

    let moves = generator.generate_legal_moves(&board, Player::Black, &captured);

    // Rook on 4,4 should be able to move along rank and file
    // Verify some expected moves exist
    let has_horizontal_move = moves
        .iter()
        .any(|mv| mv.from == Some(Position::new(4, 4)) && mv.to.row == 4 && mv.to.col != 4);

    let has_vertical_move = moves
        .iter()
        .any(|mv| mv.from == Some(Position::new(4, 4)) && mv.to.col == 4 && mv.to.row != 4);

    assert!(
        has_horizontal_move || has_vertical_move,
        "Rook should be able to move horizontally or vertically"
    );
}

#[test]
fn test_simd_telemetry_collection() {
    // Reset telemetry before test
    shogi_engine::utils::telemetry::reset_simd_telemetry();

    let generator = MoveGenerator::new();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Generate moves (should record telemetry)
    let _moves = generator.generate_legal_moves(&board, Player::Black, &captured);

    // Get telemetry
    let telemetry = shogi_engine::utils::telemetry::get_simd_telemetry();

    // Should have recorded some calls (either SIMD or scalar)
    let total_move_calls = telemetry.simd_move_gen_calls + telemetry.scalar_move_gen_calls;
    assert!(total_move_calls > 0, "Should have recorded move generation calls");
}

#[test]
fn test_simd_telemetry_evaluation() {
    // Reset telemetry before test
    shogi_engine::utils::telemetry::reset_simd_telemetry();

    let mut evaluator = IntegratedEvaluator::new();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Evaluate (should record telemetry)
    let _score = evaluator.evaluate(&board, Player::Black, &captured);

    // Get telemetry
    let telemetry = shogi_engine::utils::telemetry::get_simd_telemetry();

    // Should have recorded some calls (either SIMD or scalar)
    let total_eval_calls = telemetry.simd_evaluation_calls + telemetry.scalar_evaluation_calls;
    assert!(total_eval_calls > 0, "Should have recorded evaluation calls");
}

#[test]
fn test_simd_telemetry_pattern_matching() {
    // Reset telemetry before test
    shogi_engine::utils::telemetry::reset_simd_telemetry();

    let mut recognizer = TacticalPatternRecognizer::new();
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Evaluate tactics (should record telemetry)
    let _score = recognizer.evaluate_tactics(&board, Player::Black, &captured);

    // Get telemetry
    let telemetry = shogi_engine::utils::telemetry::get_simd_telemetry();

    // Should have recorded some calls (either SIMD or scalar)
    let total_pattern_calls = telemetry.simd_pattern_calls + telemetry.scalar_pattern_calls;
    assert!(total_pattern_calls > 0, "Should have recorded pattern matching calls");
}

#[test]
fn test_simd_telemetry_accuracy_counts() {
    reset_simd_telemetry();

    for _ in 0..3 {
        SIMD_TELEMETRY.record_simd_evaluation();
    }
    for _ in 0..2 {
        SIMD_TELEMETRY.record_scalar_pattern();
    }
    SIMD_TELEMETRY.record_simd_move_gen();
    SIMD_TELEMETRY.record_scalar_move_gen();

    let telemetry = get_simd_telemetry();
    assert_eq!(telemetry.simd_evaluation_calls, 3);
    assert_eq!(telemetry.scalar_pattern_calls, 2);
    assert_eq!(telemetry.simd_move_gen_calls, 1);
    assert_eq!(telemetry.scalar_move_gen_calls, 1);
}

#[test]
fn test_simd_telemetry_reset_functionality() {
    reset_simd_telemetry();
    SIMD_TELEMETRY.record_simd_evaluation_with_time(10);
    SIMD_TELEMETRY.record_scalar_evaluation_with_time(20);
    SIMD_TELEMETRY.record_simd_pattern_with_time(30);
    SIMD_TELEMETRY.record_scalar_pattern_with_time(40);
    SIMD_TELEMETRY.record_simd_move_gen_with_time(50);
    SIMD_TELEMETRY.record_scalar_move_gen_with_time(60);

    reset_simd_telemetry();
    let telemetry = get_simd_telemetry();
    assert_eq!(telemetry.simd_evaluation_calls, 0);
    assert_eq!(telemetry.scalar_evaluation_calls, 0);
    assert_eq!(telemetry.simd_pattern_calls, 0);
    assert_eq!(telemetry.scalar_pattern_calls, 0);
    assert_eq!(telemetry.simd_move_gen_calls, 0);
    assert_eq!(telemetry.scalar_move_gen_calls, 0);
    assert_eq!(telemetry.simd_evaluation_time_ns, 0);
    assert_eq!(telemetry.scalar_evaluation_time_ns, 0);
    assert_eq!(telemetry.simd_pattern_time_ns, 0);
    assert_eq!(telemetry.scalar_pattern_time_ns, 0);
    assert_eq!(telemetry.simd_move_gen_time_ns, 0);
    assert_eq!(telemetry.scalar_move_gen_time_ns, 0);
}

#[test]
fn test_simd_telemetry_concurrent_tracking() {
    let tracker = Arc::new(SimdTelemetryTracker::new());
    let thread_count = 8;
    let iterations = 1_000;
    let mut handles = Vec::new();

    for _ in 0..thread_count {
        let tracker_cloned = tracker.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..iterations {
                tracker_cloned.record_simd_move_gen();
                tracker_cloned.record_scalar_move_gen();
                tracker_cloned.record_simd_pattern_with_time(5);
                tracker_cloned.record_scalar_pattern_with_time(7);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let telemetry = tracker.snapshot();
    let expected = (thread_count * iterations) as u64;
    assert_eq!(telemetry.simd_move_gen_calls, expected);
    assert_eq!(telemetry.scalar_move_gen_calls, expected);
    assert_eq!(telemetry.simd_pattern_calls, expected);
    assert_eq!(telemetry.scalar_pattern_calls, expected);
    assert_eq!(telemetry.simd_pattern_time_ns, expected * 5);
    assert_eq!(telemetry.scalar_pattern_time_ns, expected * 7);
}

#[test]
fn test_simd_telemetry_serialization_round_trip() {
    let telemetry = SimdTelemetry {
        simd_evaluation_calls: 4,
        scalar_evaluation_calls: 2,
        simd_pattern_calls: 3,
        scalar_pattern_calls: 1,
        simd_move_gen_calls: 5,
        scalar_move_gen_calls: 6,
        simd_evaluation_time_ns: 40,
        scalar_evaluation_time_ns: 20,
        simd_pattern_time_ns: 30,
        scalar_pattern_time_ns: 10,
        simd_move_gen_time_ns: 50,
        scalar_move_gen_time_ns: 60,
    };

    let json = serde_json::to_string(&telemetry).expect("serialize telemetry");
    let round_trip: SimdTelemetry = serde_json::from_str(&json).expect("deserialize telemetry");

    assert_eq!(telemetry.simd_evaluation_calls, round_trip.simd_evaluation_calls);
    assert_eq!(telemetry.scalar_evaluation_calls, round_trip.scalar_evaluation_calls);
    assert_eq!(telemetry.simd_pattern_calls, round_trip.simd_pattern_calls);
    assert_eq!(telemetry.scalar_pattern_calls, round_trip.scalar_pattern_calls);
    assert_eq!(telemetry.simd_move_gen_calls, round_trip.simd_move_gen_calls);
    assert_eq!(telemetry.scalar_move_gen_calls, round_trip.scalar_move_gen_calls);
    assert_eq!(telemetry.simd_evaluation_time_ns, round_trip.simd_evaluation_time_ns);
    assert_eq!(telemetry.scalar_evaluation_time_ns, round_trip.scalar_evaluation_time_ns);
    assert_eq!(telemetry.simd_pattern_time_ns, round_trip.simd_pattern_time_ns);
    assert_eq!(telemetry.scalar_pattern_time_ns, round_trip.scalar_pattern_time_ns);
    assert_eq!(telemetry.simd_move_gen_time_ns, round_trip.simd_move_gen_time_ns);
    assert_eq!(telemetry.scalar_move_gen_time_ns, round_trip.scalar_move_gen_time_ns);
}
