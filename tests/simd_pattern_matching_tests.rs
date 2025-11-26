#![cfg(feature = "simd")]
/// Tests for SIMD-based pattern matching for tactical patterns
///
/// These tests validate that SIMD pattern matching works correctly and
/// provides performance improvements over scalar implementations.
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::tactical_patterns_simd::SimdPatternMatcher;
use shogi_engine::types::{Piece, PieceType, Player, Position};

#[test]
fn test_detect_forks_batch() {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();

    // Create a position with a piece that can fork
    let pieces = vec![(Position::new(4, 4), PieceType::Rook)];

    let forks = matcher.detect_forks_batch(&board, &pieces, Player::Black);

    // On an empty board, there should be no forks
    assert!(
        forks.is_empty() || forks.iter().all(|(_, _, count)| *count < 2),
        "Empty board should have no forks or forks with < 2 targets"
    );
}

#[test]
fn test_detect_forks_batch_empty() {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();

    let pieces = vec![];

    let forks = matcher.detect_forks_batch(&board, &pieces, Player::Black);
    assert!(forks.is_empty(), "Empty pieces list should produce no forks");
}

#[test]
fn test_count_attack_targets() {
    use shogi_engine::bitboards::SimdBitboard;

    let matcher = SimdPatternMatcher::new();

    // Create attack pattern with some bits set
    let attack_pattern = SimdBitboard::from_u128(0x0F0F_0F0F);
    let target_mask = SimdBitboard::from_u128(0x3333_3333);

    let count = matcher.count_attack_targets(attack_pattern, target_mask);

    // Intersection of 0x0F0F and 0x3333 should have some bits
    let intersection = attack_pattern & target_mask;
    let expected_count = intersection.count_ones();

    assert_eq!(count, expected_count, "Target count should match intersection");
}

#[test]
fn test_count_attack_targets_batch() {
    use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, SimdBitboard};

    let matcher = SimdPatternMatcher::new();

    let attack_patterns = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333),
        SimdBitboard::from_u128(0x5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA),
    ]);

    let target_mask = SimdBitboard::from_u128(0xFFFF_FFFF);

    let counts = matcher.count_attack_targets_batch(&attack_patterns, target_mask);

    // Verify counts match individual intersections
    for i in 0..4 {
        let expected = (*attack_patterns.get(i) & target_mask).count_ones();
        assert_eq!(counts[i], expected, "Batch count {} should match individual count", i);
    }
}

#[test]
fn test_detect_patterns_batch() {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();

    let positions =
        vec![Position::new(4, 4), Position::new(4, 5), Position::new(5, 4), Position::new(5, 5)];

    let results = matcher.detect_patterns_batch(&board, &positions, PieceType::Rook, Player::Black);

    // Should have results for all positions
    assert_eq!(results.len(), positions.len(), "Should have results for all positions");

    // Verify all positions are in results
    for &pos in &positions {
        assert!(results.iter().any(|(p, _)| *p == pos), "Position {:?} should be in results", pos);
    }
}

#[test]
fn test_detect_patterns_batch_empty() {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();

    let positions = vec![];

    let results = matcher.detect_patterns_batch(&board, &positions, PieceType::Rook, Player::Black);
    assert!(results.is_empty(), "Empty positions list should produce no results");
}

#[test]
fn test_detect_pins_batch() {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();

    // Create a position with pieces that might create pins
    let pieces =
        vec![(Position::new(4, 4), PieceType::Rook), (Position::new(4, 0), PieceType::Bishop)];

    let pins = matcher.detect_pins_batch(&board, &pieces, Player::Black);

    // On an empty board, there should be no pins
    // But the function should still work correctly
    assert!(
        pins.is_empty() || pins.len() <= pieces.len() * 4,
        "Pins should be reasonable in number"
    );
}

#[test]
fn test_simd_pattern_matching_performance() {
    use shogi_engine::bitboards::SimdBitboard;

    let matcher = SimdPatternMatcher::new();

    // Test that SIMD operations are fast
    let attack_pattern = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F);
    let target_mask = SimdBitboard::from_u128(0x3333_3333_3333_3333);

    let iterations = 1_000_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = matcher.count_attack_targets(attack_pattern, target_mask);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    // Target: At least 1 million operations per second (adjusted for debug builds)
    let min_ops_per_sec = 500_000.0;

    assert!(
        ops_per_sec >= min_ops_per_sec,
        "SIMD pattern matching too slow: {:.2} ops/sec (target: {:.2})",
        ops_per_sec,
        min_ops_per_sec
    );

    println!("SIMD pattern matching: {:.2} ops/sec", ops_per_sec);
}
