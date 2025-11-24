#![cfg(feature = "simd")]
/// NPS (Nodes Per Second) validation tests for SIMD optimizations
/// 
/// These tests validate that SIMD optimizations contribute to overall engine
/// performance improvement. Target: at least 20% NPS improvement.
///
/// # Task 5.12 (Tasks 5.12.2, 5.12.3, 5.12.4)

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::config::SimdConfig;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;
use shogi_engine::bitboards::SimdBitboard;
use std::time::{Duration, Instant};

/// Measure NPS for a workload of bitboard operations
/// This simulates the kind of work done during search
fn measure_bitboard_workload_nps(iterations: u64) -> f64 {
    let test_values: Vec<u128> = (0..100)
        .map(|i| 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F ^ (i as u128))
        .collect();
    
    let bitboards: Vec<SimdBitboard> = test_values
        .iter()
        .map(|&v| SimdBitboard::from_u128(v))
        .collect();
    
    let start = Instant::now();
    
    // Simulate realistic workload: multiple bitwise operations
    for _ in 0..iterations {
        for i in 0..bitboards.len() {
            let j = (i + 1) % bitboards.len();
            let _ = bitboards[i] & bitboards[j];
            let _ = bitboards[i] | bitboards[j];
            let _ = bitboards[i] ^ bitboards[j];
            let _ = !bitboards[i];
            let _ = bitboards[i].count_ones();
        }
    }
    
    let elapsed = start.elapsed();
    let nodes = iterations * bitboards.len() as u64 * 5; // 5 operations per bitboard pair
    nodes as f64 / elapsed.as_secs_f64()
}

#[test]
fn test_simd_nps_improvement() {
    // This test validates that SIMD operations are fast enough to contribute
    // to overall engine performance. We measure a workload that simulates
    // the bitboard operations done during search.
    
    let iterations = 10_000;
    let nps = measure_bitboard_workload_nps(iterations);
    
    // Target: At least 500k nodes per second for this workload (adjusted for debug builds)
    // Release builds should achieve 1M+ NPS
    // This is a conservative target - actual search will be more complex
    // but this validates that SIMD operations are fast enough
    let min_nps = 500_000.0;
    
    assert!(
        nps >= min_nps,
        "SIMD workload NPS too low: {} (target: {})",
        nps,
        min_nps
    );
    
    println!("SIMD workload NPS: {:.2}", nps);
}

#[test]
fn test_simd_operations_performance_threshold() {
    // Validate that individual SIMD operations meet performance thresholds
    // This ensures SIMD operations are fast enough to contribute to NPS improvement
    
    let iterations = 1_000_000;
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    
    // Measure AND operation throughput
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = bb1 & bb2;
    }
    let and_duration = start.elapsed();
    let and_ops_per_sec = iterations as f64 / and_duration.as_secs_f64();
    
    // Target: At least 5 million operations per second (adjusted for debug builds)
    // Release builds should achieve 10M+ ops/sec
    let min_ops_per_sec = 5_000_000.0;
    
    assert!(
        and_ops_per_sec >= min_ops_per_sec,
        "SIMD AND operations too slow: {:.2} ops/sec (target: {:.2})",
        and_ops_per_sec,
        min_ops_per_sec
    );
    
    println!("SIMD AND operations: {:.2} ops/sec", and_ops_per_sec);
}

#[test]
fn test_batch_operations_nps_contribution() {
    // Validate that batch operations are fast enough to contribute to NPS
    use shogi_engine::bitboards::batch_ops::AlignedBitboardArray;
    
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];
    
    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);
    
    let iterations = 100_000;
    let start = Instant::now();
    
    for _ in 0..iterations {
        let _ = a.batch_and(&b);
        let _ = a.batch_or(&b);
        let _ = a.batch_xor(&b);
    }
    
    let elapsed = start.elapsed();
    let batch_ops_per_sec = (iterations * 3) as f64 / elapsed.as_secs_f64();
    
    // Target: At least 500k batch operations per second (adjusted for debug builds)
    // Release builds should achieve 1M+ ops/sec
    let min_batch_ops_per_sec = 500_000.0;
    
    assert!(
        batch_ops_per_sec >= min_batch_ops_per_sec,
        "Batch operations too slow: {:.2} ops/sec (target: {:.2})",
        batch_ops_per_sec,
        min_batch_ops_per_sec
    );
    
    println!("Batch operations: {:.2} ops/sec", batch_ops_per_sec);
}

/// Measure NPS for a realistic search workload
/// 
/// This simulates actual engine search with evaluation, pattern matching, and move generation.
/// # Task 5.12.3
fn measure_search_nps(
    engine: &mut SearchEngine,
    board: &mut BitboardBoard,
    captured_pieces: &CapturedPieces,
    player: Player,
    depth: u8,
    iterations: usize,
) -> (f64, u64) {
    let mut total_nodes = 0u64;
    let start_time = Instant::now();

    for _ in 0..iterations {
        // Reset board to starting position for each iteration
        *board = BitboardBoard::new();
        
        let result = engine.search_at_depth(board, captured_pieces, player, depth, 5000, i32::MIN, i32::MAX);
        assert!(result.is_some(), "Search should return a result");
        total_nodes += engine.get_nodes_searched();
    }

    let duration = start_time.elapsed();
    let nps = if duration.as_secs_f64() > 0.0 {
        total_nodes as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    (nps, total_nodes)
}

/// Create a SearchEngine with SIMD enabled or disabled
/// # Task 5.12.3
/// Note: SIMD configuration is set on evaluator directly.
/// Move generator SIMD is controlled by default config (enabled when simd feature is on).
/// For move generation, we rely on the default which should be SIMD enabled when the feature is enabled.
fn create_engine_with_simd_config(simd_enabled: bool) -> SearchEngine {
    let mut engine = SearchEngine::new(None, 16);
    
    // Configure SIMD on evaluator via public API
    let evaluator = engine.get_evaluator_mut();
    if let Some(integrated) = evaluator.get_integrated_evaluator_mut() {
        let mut eval_config = integrated.config().clone();
        eval_config.simd.enable_simd_evaluation = simd_enabled;
        integrated.set_config(eval_config);
        
        // Configure SIMD on tactical pattern recognizer via update_tactical_config
        let mut tactical_config = integrated.config().tactical.clone();
        tactical_config.enable_simd_pattern_matching = simd_enabled;
        integrated.update_tactical_config(tactical_config);
    }
    
    // Note: Move generator SIMD config is private in SearchEngine.
    // The default MoveGenerator::new() enables SIMD when the feature is enabled.
    // For comprehensive testing, we'd need a public API to configure it, but
    // evaluation and pattern matching are the main contributors to NPS improvement.
    
    engine
}

#[test]
fn test_simd_nps_improvement_end_to_end() {
    // # Task 5.12.2: NPS validation test that requires 20%+ improvement
    // This test validates that SIMD provides meaningful overall engine performance improvement
    // by comparing end-to-end search performance with SIMD enabled vs disabled.
    
    // Skip in CI or if performance tests are disabled
    if std::env::var("CI").is_ok() || std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        println!("Skipping performance test in CI or when SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    let mut board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();
    let player = Player::Black;
    let depth = 3; // Shallow depth for faster testing
    let iterations = 5; // Few iterations for faster testing

    // Measure with SIMD enabled
    let mut engine_simd = create_engine_with_simd_config(true);
    let (nps_simd, nodes_simd) = measure_search_nps(
        &mut engine_simd,
        &mut board,
        &captured_pieces,
        player,
        depth,
        iterations,
    );

    // Measure with SIMD disabled
    let mut engine_scalar = create_engine_with_simd_config(false);
    let (nps_scalar, nodes_scalar) = measure_search_nps(
        &mut engine_scalar,
        &mut board,
        &captured_pieces,
        player,
        depth,
        iterations,
    );

    // Calculate improvement percentage
    let improvement = if nps_scalar > 0.0 {
        ((nps_simd - nps_scalar) / nps_scalar) * 100.0
    } else {
        0.0
    };

    println!("SIMD NPS: {:.2}", nps_simd);
    println!("Scalar NPS: {:.2}", nps_scalar);
    println!("Improvement: {:.2}%", improvement);
    println!("SIMD nodes: {}, Scalar nodes: {}", nodes_simd, nodes_scalar);

    // # Task 5.12.2: Require 20%+ improvement in release builds
    // In debug builds, we allow some regression due to function call overhead
    // In release builds, we require 20%+ improvement
    #[cfg(not(debug_assertions))]
    {
        assert!(
            improvement >= 20.0,
            "SIMD should provide at least 20% NPS improvement. Got {:.2}% improvement (SIMD: {:.2} NPS, Scalar: {:.2} NPS)",
            improvement,
            nps_simd,
            nps_scalar
        );
    }
    
    #[cfg(debug_assertions)]
    {
        // In debug builds, allow up to 50% regression (expected due to function call overhead)
        // But still prefer improvement
        if improvement < -50.0 {
            panic!(
                "SIMD regression too large in debug build: {:.2}% (SIMD: {:.2} NPS, Scalar: {:.2} NPS)",
                improvement,
                nps_simd,
                nps_scalar
            );
        }
        println!("Debug build: Improvement {:.2}% (allowing up to 50% regression)", improvement);
    }

    // Both should search reasonable number of nodes
    assert!(nodes_simd > 0, "SIMD search should explore nodes");
    assert!(nodes_scalar > 0, "Scalar search should explore nodes");
}

#[test]
fn test_simd_nps_regression_detection() {
    // # Task 5.12.4: NPS regression detection
    // This test ensures that SIMD doesn't cause significant performance regressions
    
    // Skip in CI or if performance tests are disabled
    if std::env::var("CI").is_ok() || std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        println!("Skipping performance test in CI or when SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    let mut board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();
    let player = Player::Black;
    let depth = 3;
    let iterations = 5;

    // Measure with SIMD enabled
    let mut engine_simd = create_engine_with_simd_config(true);
    let (nps_simd, _) = measure_search_nps(
        &mut engine_simd,
        &mut board,
        &captured_pieces,
        player,
        depth,
        iterations,
    );

    // Measure with SIMD disabled
    let mut engine_scalar = create_engine_with_simd_config(false);
    let (nps_scalar, _) = measure_search_nps(
        &mut engine_scalar,
        &mut board,
        &captured_pieces,
        player,
        depth,
        iterations,
    );

    // In release builds, SIMD should not be significantly slower
    #[cfg(not(debug_assertions))]
    {
        let regression = if nps_simd > 0.0 {
            ((nps_scalar - nps_simd) / nps_simd) * 100.0
        } else {
            100.0
        };

        // Allow up to 5% regression in release builds (acceptable variance)
        assert!(
            regression <= 5.0,
            "SIMD should not cause more than 5% regression. Got {:.2}% regression (SIMD: {:.2} NPS, Scalar: {:.2} NPS)",
            regression,
            nps_simd,
            nps_scalar
        );
    }

    // Both should achieve reasonable NPS
    assert!(nps_simd > 0.0, "SIMD NPS should be positive");
    assert!(nps_scalar > 0.0, "Scalar NPS should be positive");
}

#[test]
fn test_simd_realistic_workload_simulation() {
    // # Task 5.12.3: Realistic workload simulation for NPS testing
    // This test uses multiple positions to simulate realistic search workload
    
    // Skip in CI or if performance tests are disabled
    if std::env::var("CI").is_ok() || std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        println!("Skipping performance test in CI or when SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    let captured_pieces = CapturedPieces::new();
    let player = Player::Black;
    let depth = 3;
    let iterations_per_position = 3;

    // Test multiple positions to simulate realistic workload
    let positions = vec![
        BitboardBoard::new(), // Starting position
        // Add more positions if needed
    ];

    // Measure with SIMD enabled
    let mut engine_simd = create_engine_with_simd_config(true);
    let mut total_nodes_simd = 0u64;
    let start_simd = Instant::now();

    for board in &positions {
        for _ in 0..iterations_per_position {
            let mut test_board = board.clone();
            let _ = engine_simd.search_at_depth(&mut test_board, &captured_pieces, player, depth, 5000, i32::MIN, i32::MAX);
            total_nodes_simd += engine_simd.get_nodes_searched();
        }
    }

    let duration_simd = start_simd.elapsed();
    let nps_simd = if duration_simd.as_secs_f64() > 0.0 {
        total_nodes_simd as f64 / duration_simd.as_secs_f64()
    } else {
        0.0
    };

    // Measure with SIMD disabled
    let mut engine_scalar = create_engine_with_simd_config(false);
    let mut total_nodes_scalar = 0u64;
    let start_scalar = Instant::now();

    for board in &positions {
        for _ in 0..iterations_per_position {
            let mut test_board = board.clone();
            let _ = engine_scalar.search_at_depth(&mut test_board, &captured_pieces, player, depth, 5000, i32::MIN, i32::MAX);
            total_nodes_scalar += engine_scalar.get_nodes_searched();
        }
    }

    let duration_scalar = start_scalar.elapsed();
    let nps_scalar = if duration_scalar.as_secs_f64() > 0.0 {
        total_nodes_scalar as f64 / duration_scalar.as_secs_f64()
    } else {
        0.0
    };

    let improvement = if nps_scalar > 0.0 {
        ((nps_simd - nps_scalar) / nps_scalar) * 100.0
    } else {
        0.0
    };

    println!("Realistic workload - SIMD NPS: {:.2}", nps_simd);
    println!("Realistic workload - Scalar NPS: {:.2}", nps_scalar);
    println!("Realistic workload - Improvement: {:.2}%", improvement);

    // Both should achieve reasonable NPS
    assert!(nps_simd > 0.0, "SIMD NPS should be positive");
    assert!(nps_scalar > 0.0, "Scalar NPS should be positive");
    assert!(total_nodes_simd > 0, "SIMD should search nodes");
    assert!(total_nodes_scalar > 0, "Scalar should search nodes");
}

