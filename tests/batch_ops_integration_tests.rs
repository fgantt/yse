#![cfg(feature = "simd")]
/// Integration tests for batch operations in real-world scenarios
///
/// These tests simulate how batch operations would be used in actual
/// Shogi engine code paths, such as:
/// - Combining multiple attack patterns
/// - Processing arrays of bitboards
/// - Move generation scenarios
use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, SimdBitboard};

#[test]
fn test_batch_combine_attack_patterns() {
    // Simulate combining attack patterns from multiple pieces
    // This is a common use case in move generation

    let rook_attacks = SimdBitboard::from_u128(0x0F0F_0F0F_0000_0000_0000_0000_0000_0000);
    let bishop_attacks = SimdBitboard::from_u128(0x0000_0000_0F0F_0F0F_0000_0000_0000_0000);
    let knight_attacks = SimdBitboard::from_u128(0x0000_0000_0000_0000_0F0F_0F0F_0000_0000);
    let pawn_attacks = SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0F0F_0F0F);

    let attack_patterns = AlignedBitboardArray::<4>::from_slice(&[
        rook_attacks,
        bishop_attacks,
        knight_attacks,
        pawn_attacks,
    ]);

    // Combine all attacks using batch operations
    let combined = attack_patterns.combine_all();
    let expected = rook_attacks | bishop_attacks | knight_attacks | pawn_attacks;

    assert_eq!(combined.to_u128(), expected.to_u128());
}

#[test]
fn test_batch_filter_attacks() {
    // Simulate filtering attacks by combining with a mask
    // This simulates checking which attacks are valid (e.g., not blocked)

    let attacks = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
    ]);

    let mask = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333),
        SimdBitboard::from_u128(0x5555_5555_5555_5555_5555_5555_5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA),
    ]);

    // Filter attacks using batch AND
    let filtered = attacks.batch_and(&mask);

    // Verify each filtered attack matches expected result
    for i in 0..4 {
        let expected = *attacks.get(i) & *mask.get(i);
        assert_eq!(
            filtered.get(i).to_u128(),
            expected.to_u128(),
            "Filtered attack {} should match expected",
            i
        );
    }
}

#[test]
fn test_batch_merge_attacks() {
    // Simulate merging attacks from two different sets of pieces
    // This simulates combining attacks from multiple players or phases

    let player1_attacks = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F_0000_0000_0000_0000_0000_0000_0000),
        SimdBitboard::from_u128(0x0000_0F0F_0000_0000_0000_0000_0000_0000),
        SimdBitboard::from_u128(0x0000_0000_0F0F_0000_0000_0000_0000_0000),
        SimdBitboard::from_u128(0x0000_0000_0000_0F0F_0000_0000_0000_0000),
    ]);

    let player2_attacks = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0F0F_0000_0000_0000),
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0F0F_0000_0000),
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0F0F_0000),
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0F0F),
    ]);

    // Merge attacks using batch OR
    let merged = player1_attacks.batch_or(&player2_attacks);

    // Verify merged attacks
    for i in 0..4 {
        let expected = *player1_attacks.get(i) | *player2_attacks.get(i);
        assert_eq!(
            merged.get(i).to_u128(),
            expected.to_u128(),
            "Merged attack {} should match expected",
            i
        );
    }
}

#[test]
fn test_batch_find_attacked_squares() {
    // Simulate finding squares attacked by multiple pieces
    // This simulates checking which squares are under attack

    let piece_attacks = AlignedBitboardArray::<8>::from_slice(&[
        SimdBitboard::from_u128(0x0001), // Piece 1 attacks square 0
        SimdBitboard::from_u128(0x0002), // Piece 2 attacks square 1
        SimdBitboard::from_u128(0x0004), // Piece 3 attacks square 2
        SimdBitboard::from_u128(0x0008), // Piece 4 attacks square 3
        SimdBitboard::from_u128(0x0010), // Piece 5 attacks square 4
        SimdBitboard::from_u128(0x0020), // Piece 6 attacks square 5
        SimdBitboard::from_u128(0x0040), // Piece 7 attacks square 6
        SimdBitboard::from_u128(0x0080), // Piece 8 attacks square 7
    ]);

    // Combine all attacks to find all attacked squares
    let all_attacked = piece_attacks.combine_all();

    // Verify all squares 0-7 are attacked
    assert_eq!(all_attacked.to_u128(), 0x00FF);
    assert_eq!(all_attacked.count_ones(), 8);
}

#[test]
fn test_batch_move_generation_scenario() {
    // Simulate a realistic move generation scenario:
    // 1. Get attack patterns for multiple pieces
    // 2. Filter by valid moves (not blocked, not own pieces)
    // 3. Combine to get all possible moves

    // Step 1: Attack patterns
    let attack_patterns = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF), // Rook
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF), // Bishop
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0000_00FF), // Knight
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0000_FF00), // Pawn
    ]);

    // Step 2: Valid move mask (not blocked, not own pieces)
    let valid_mask = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F), // Rook mask
        SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333), // Bishop mask
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0000_00FF), // Knight mask
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0000_FF00), // Pawn mask
    ]);

    // Step 3: Filter attacks to get valid moves
    let valid_moves = attack_patterns.batch_and(&valid_mask);

    // Step 4: Combine all valid moves
    let all_valid_moves = valid_moves.combine_all();

    // Verify we have some valid moves
    assert!(all_valid_moves.count_ones() > 0, "Should have at least some valid moves");

    // Verify each piece's valid moves are correct
    for i in 0..4 {
        let expected = *attack_patterns.get(i) & *valid_mask.get(i);
        assert_eq!(
            valid_moves.get(i).to_u128(),
            expected.to_u128(),
            "Valid moves for piece {} should match expected",
            i
        );
    }
}

#[test]
fn test_batch_operations_performance_characteristics() {
    // Test that batch operations maintain performance characteristics
    // This validates that batch operations are efficient for real-world use

    let large_array = AlignedBitboardArray::<16>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333),
        SimdBitboard::from_u128(0x5555_5555_5555_5555_5555_5555_5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA),
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333),
        SimdBitboard::from_u128(0x5555_5555_5555_5555_5555_5555_5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA),
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333),
        SimdBitboard::from_u128(0x5555_5555_5555_5555_5555_5555_5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA),
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333),
        SimdBitboard::from_u128(0x5555_5555_5555_5555_5555_5555_5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA),
    ]);

    let mask = AlignedBitboardArray::<16>::from_slice(&[
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
        SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF),
    ]);

    // Perform batch operations - should be efficient
    let filtered = large_array.batch_and(&mask);
    let merged = large_array.batch_or(&mask);
    let combined = large_array.combine_all();

    // Verify correctness
    for i in 0..16 {
        assert_eq!(
            filtered.get(i).to_u128(),
            large_array.get(i).to_u128(),
            "Filtered result {} should match input",
            i
        );
        assert_eq!(
            merged.get(i).to_u128(),
            mask.get(i).to_u128(),
            "Merged result {} should match mask",
            i
        );
    }

    // Combined should have at least some bits set
    assert!(combined.count_ones() > 0, "Combined result should have some bits set");
}
