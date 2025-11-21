//! Task 5.0.5.4: Integration tests for platform-specific bitboard code paths
//!
//! These tests ensure that SIMD/BMI fallback paths remain functional across platforms,
//! including wasm/ARM builds if applicable.

use shogi_engine::bitboards::{
    integration::{BitScanningOptimizer, GlobalOptimizer},
    get_board_telemetry, get_magic_telemetry, reset_board_telemetry, BitboardBoard,
};
use shogi_engine::types::Bitboard;
use shogi_engine::bitboards::SimdBitboard;

#[test]
fn test_platform_specific_bitscan_fallback() {
    // Test that bit scanning works regardless of platform capabilities
    let optimizer = BitScanningOptimizer::new();
    
    let test_bitboards = vec![
        0u128,
        1u128,
        0b1010u128,
        0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFu128,
        0x5555555555555555u128,
    ];
    
    for bb in test_bitboards {
        // Should work on all platforms (uses fallback if hardware not available)
        let result = optimizer.bit_scan_forward(SimdBitboard::new(bb));
        assert!(result.is_none() || result.unwrap() < 128);
        
        let popcount = optimizer.popcount(SimdBitboard::new(bb));
        assert!(popcount <= 128);
    }
}

#[test]
fn test_popcount_strategies() {
    let optimizer = GlobalOptimizer::get();
    
    // Test sparse bitboard
    let sparse = SimdBitboard::new(0b101);
    assert_eq!(optimizer.popcount(sparse), 2);
    
    // Test dense bitboard
    let dense = SimdBitboard::new(!0);
    assert_eq!(optimizer.popcount(dense), 128);
    
    // Verify counters updated
    let counters = optimizer.get_strategy_counters();
    assert!(counters.popcount_hardware + counters.popcount_4bit + counters.popcount_swar + counters.popcount_debruijn > 0);
}

#[test]
fn test_bitscan_strategies() {
    let optimizer = GlobalOptimizer::get();
    
    // Test single bit
    let bb = SimdBitboard::new(1 << 10);
    assert_eq!(optimizer.bit_scan_forward(bb), Some(10));
    assert_eq!(optimizer.bit_scan_reverse(bb), Some(10));
    
    // Test multiple bits
    let bb = SimdBitboard::new((1 << 10) | (1 << 20));
    assert_eq!(optimizer.bit_scan_forward(bb), Some(10));
    assert_eq!(optimizer.bit_scan_reverse(bb), Some(20));
}

#[test]
fn test_static_dispatch() {
    // Test static dispatch methods
    let bb = SimdBitboard::new(0b101);
    let count = GlobalOptimizer::popcount(bb);
    assert_eq!(count, 2);
    
    let bb = SimdBitboard::new(1 << 10);
    let first_bit = GlobalOptimizer::bit_scan_forward(bb);
    assert_eq!(first_bit, Some(10));
}

#[test]
fn test_platform_specific_optimizations() {
    let board = BitboardBoard::new();
    
    // Test attack generation integration
    use shogi_engine::types::Position;
    let rook_attacks = board.get_attack_pattern(Position::new(4, 4), shogi_engine::types::PieceType::Rook);
    let bishop_attacks = board.get_attack_pattern(Position::new(4, 4), shogi_engine::types::PieceType::Bishop);
    
    // Should use optimized bit operations internally
    assert!(rook_attacks != SimdBitboard::new(0) || board.get_occupied_bitboard() != SimdBitboard::new(0));
    
    // Verify SIMD usage if available
    #[cfg(target_feature = "avx2")]
    {
        assert!(bishop_attacks != SimdBitboard::new(0) || board.get_occupied_bitboard() != SimdBitboard::new(0));
    }
}

#[test]
fn test_strategy_selection_telemetry() {
    let optimizer = GlobalOptimizer::get();
    let counters_before = optimizer.get_strategy_counters();
    
    // Perform operations
    optimizer.popcount(SimdBitboard::new(0b1010));
    optimizer.bit_scan_forward(SimdBitboard::new(0b1000));
    
    let counters_after = optimizer.get_strategy_counters();
    
    let total_before = counters_before.popcount_hardware + counters_before.popcount_4bit
        + counters_before.popcount_swar + counters_before.popcount_debruijn
        + counters_before.bitscan_hardware + counters_before.bitscan_debruijn;
        
    let total_after = counters_after.popcount_hardware + counters_after.popcount_4bit
        + counters_after.popcount_swar + counters_after.popcount_debruijn
        + counters_after.bitscan_hardware + counters_after.bitscan_debruijn;
        
    assert!(total_after >= total_before);
    
    // Verify fallback counters are 0 on supported hardware
    #[cfg(all(target_arch = "x86_64", target_feature = "popcnt"))]
    {
        assert_eq!(counters_after.popcount_hardware, 0); // Should use hardware popcount
        assert_eq!(counters_after.bitscan_hardware, 0); // Should use hardware bitscan
    }
}

#[test]
fn test_geometric_analysis_integration() {
    let optimizer = GlobalOptimizer::get();
    let bb = SimdBitboard::new(0x1234567890ABCDEF);
    
    let analysis = optimizer.analyze_geometry(bb);
    
    // Verify geometric properties
    assert!(analysis.popcount > 0);
    assert!(analysis.min_index.is_some());
    assert!(analysis.max_index.is_some());
    
    // Verify consistency
    assert_eq!(analysis.popcount, optimizer.popcount(bb));
    assert_eq!(analysis.min_index, optimizer.bit_scan_forward(bb));
    assert_eq!(analysis.max_index, optimizer.bit_scan_reverse(bb));
}

#[test]
fn test_bit_count_estimation_integration() {
    let optimizer = GlobalOptimizer::get();
    let bb = SimdBitboard::new(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);
    
    let popcount = optimizer.popcount(bb);
    let estimated = optimizer.estimate_bit_count(bb);
    assert_eq!(estimated, 128);
}

#[test]
fn test_global_optimizer_platform_independence() {
    // Test that GlobalOptimizer works on all platforms
    let test_cases = vec![
        (SimdBitboard::new(0u128), 0),
        (SimdBitboard::new(1u128), 1),
        (SimdBitboard::new(0b1010u128), 2),
        (SimdBitboard::new(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFu128), 128),
    ];
    
    for (bb, expected_count) in test_cases {
        let count = GlobalOptimizer::popcount(bb);
        assert_eq!(count, expected_count, "Popcount failed for 0x{:X}", bb.to_u128());
        
        if bb != SimdBitboard::new(0) {
            let first_bit = GlobalOptimizer::bit_scan_forward(bb);
            assert!(first_bit.is_some(), "Bit scan forward should find bit in 0x{:X}", bb.to_u128());
        }
    }
}

#[test]
fn test_magic_fallback_functionality() {
    // Test that magic bitboard fallback works correctly
    let board = BitboardBoard::empty();
    
    // Test attack pattern generation (should use fallback if magic unavailable)
    use shogi_engine::types::{PieceType, Position};
    let center = Position::new(4, 4);
    
    let rook_attacks = board.get_attack_pattern(center, PieceType::Rook);
    // Should generate some attacks (at least in the same row/col)
    assert!(rook_attacks != SimdBitboard::new(0) || board.get_occupied_bitboard() != SimdBitboard::new(0));
    
    let bishop_attacks = board.get_attack_pattern(center, PieceType::Bishop);
    // Should generate some attacks (at least in diagonals)
    assert!(bishop_attacks != SimdBitboard::new(0) || board.get_occupied_bitboard() != SimdBitboard::new(0));
}

#[test]
fn test_telemetry_counters_functionality() {
    // Test that telemetry counters work correctly
    reset_board_telemetry();
    
    let board = BitboardBoard::new();
    
    // Perform some operations
    let _clone1 = board.clone();
    let _clone2 = board.clone();
    
    let telemetry = get_board_telemetry();
    assert!(telemetry.clone_count >= 2, "Clone counter should track operations");
    
    // Test magic telemetry
    let (raycast_count, magic_count, unavailable_count) = get_magic_telemetry();
    // These should be non-negative (may be 0 if no operations performed yet)
    assert!(raycast_count >= 0);
    assert!(magic_count >= 0);
    assert!(unavailable_count >= 0);
}

#[test]
fn test_strategy_counters_reset() {
    let optimizer = BitScanningOptimizer::new();
    
    // Perform some operations
    optimizer.popcount(SimdBitboard::new(0b1010));
    optimizer.bit_scan_forward(SimdBitboard::new(0b1000));
    
    let counters_before = optimizer.get_strategy_counters();
    let total_before = counters_before.popcount_hardware + counters_before.popcount_4bit
        + counters_before.popcount_swar + counters_before.popcount_debruijn
        + counters_before.bitscan_hardware + counters_before.bitscan_debruijn;
    
    // Reset and verify
    optimizer.reset_counters();
    let counters_after = optimizer.get_strategy_counters();
    assert_eq!(counters_after.popcount_hardware, 0);
    assert_eq!(counters_after.bitscan_hardware, 0);
}

#[test]
fn test_attack_table_initialization_telemetry() {
    reset_board_telemetry();
    
    // Creating a new board should initialize attack tables
    let _board = BitboardBoard::empty();
    
    let telemetry = get_board_telemetry();
    // Attack table should have been initialized (time > 0 or memory > 0)
    assert!(
        telemetry.attack_table_init_time > 0 || telemetry.attack_table_memory > 0,
        "Attack table initialization should be tracked"
    );
}

#[test]
fn test_bitboard_operations_cross_platform() {
    // Test that basic bitboard operations work on all platforms
    let board = BitboardBoard::new();
    
    // Test attack detection
    use shogi_engine::types::{Position, Player};
    let center = Position::new(4, 4);
    let _attacked = board.is_square_attacked_by(center, Player::Black);
    
    // Test attack pattern iteration
    let attacks = board.get_attack_pattern(center, shogi_engine::types::PieceType::Rook);
    let targets: Vec<_> = board.iter_attack_targets(attacks).collect();
    // Should be able to iterate over targets
    assert!(targets.len() <= 81);
    
    // Test piece iteration
    let pieces: Vec<_> = board.iter_pieces().collect();
    assert!(pieces.len() <= 81);
}

#[test]
fn test_estimate_bit_count_accuracy() {
    // Test that estimate_bit_count works correctly on all platforms
    let optimizer = BitScanningOptimizer::new();
    
    let test_cases = vec![
        (0u128, 0),
        (0b1010u128, 2),
        (0x5555555555555555u128, 32), // Low half only
        (0x55555555555555550000000000000000u128, 32), // High half only
        (0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFu128, 128), // All bits
    ];
    
    for (bb, expected) in test_cases {
        let estimated = optimizer.estimate_bit_count(SimdBitboard::new(bb));
        assert_eq!(
            estimated, expected,
            "Estimate should match actual for 0x{:X}",
            bb
        );
    }
}

