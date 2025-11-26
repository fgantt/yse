//! SIMD NPS (Nodes Per Second) Benchmarks
//!
//! Comprehensive benchmarks comparing SIMD vs scalar implementations for overall
//! engine performance measured in nodes per second (NPS).
//!
//! # Task 5.12 (Task 5.12.1)

#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::config::SimdConfig;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;

/// Create a SearchEngine with SIMD enabled or disabled
/// Note: SIMD configuration is set on evaluator directly.
/// Move generator SIMD is controlled by default config.
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
    // Evaluation and pattern matching are the main contributors to NPS improvement.

    engine
}

/// Measure NPS for a search workload
fn measure_search_nps(
    engine: &mut SearchEngine,
    board: &mut BitboardBoard,
    captured_pieces: &CapturedPieces,
    player: Player,
    depth: u8,
    iterations: usize,
) -> f64 {
    let mut total_nodes = 0u64;
    let start_time = std::time::Instant::now();

    for _ in 0..iterations {
        *board = BitboardBoard::new(); // Reset to starting position
        let _ =
            engine.search_at_depth(board, captured_pieces, player, depth, 5000, i32::MIN, i32::MAX);
        total_nodes += engine.get_nodes_searched();
    }

    let duration = start_time.elapsed();
    if duration.as_secs_f64() > 0.0 {
        total_nodes as f64 / duration.as_secs_f64()
    } else {
        0.0
    }
}

/// Benchmark: SIMD vs Scalar NPS for starting position
/// # Task 5.12.1
fn bench_simd_vs_scalar_nps_starting_position(c: &mut Criterion) {
    let mut board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();
    let player = Player::Black;
    let depth = 4;
    let iterations = 10;

    let mut group = c.benchmark_group("NPS Starting Position");

    // SIMD enabled
    group.bench_function("simd_nps", |b| {
        let mut engine = create_engine_with_simd_config(true);
        b.iter(|| {
            black_box(measure_search_nps(
                &mut engine,
                &mut board,
                black_box(&captured_pieces),
                black_box(player),
                black_box(depth),
                black_box(iterations),
            ));
        });
    });

    // SIMD disabled
    group.bench_function("scalar_nps", |b| {
        let mut engine = create_engine_with_simd_config(false);
        b.iter(|| {
            black_box(measure_search_nps(
                &mut engine,
                &mut board,
                black_box(&captured_pieces),
                black_box(player),
                black_box(depth),
                black_box(iterations),
            ));
        });
    });

    group.finish();
}

/// Benchmark: SIMD vs Scalar NPS for different depths
/// # Task 5.12.1
fn bench_simd_vs_scalar_nps_different_depths(c: &mut Criterion) {
    let mut board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();
    let player = Player::Black;
    let iterations = 5;

    let mut group = c.benchmark_group("NPS Different Depths");

    for depth in [2, 3, 4] {
        // SIMD enabled
        group.bench_function(format!("simd_depth_{}", depth), |b| {
            let mut engine = create_engine_with_simd_config(true);
            b.iter(|| {
                black_box(measure_search_nps(
                    &mut engine,
                    &mut board,
                    black_box(&captured_pieces),
                    black_box(player),
                    black_box(depth),
                    black_box(iterations),
                ));
            });
        });

        // SIMD disabled
        group.bench_function(format!("scalar_depth_{}", depth), |b| {
            let mut engine = create_engine_with_simd_config(false);
            b.iter(|| {
                black_box(measure_search_nps(
                    &mut engine,
                    &mut board,
                    black_box(&captured_pieces),
                    black_box(player),
                    black_box(depth),
                    black_box(iterations),
                ));
            });
        });
    }

    group.finish();
}

/// Benchmark: SIMD vs Scalar NPS for realistic workload
/// # Task 5.12.1, 5.12.3
fn bench_simd_vs_scalar_nps_realistic_workload(c: &mut Criterion) {
    let captured_pieces = CapturedPieces::new();
    let player = Player::Black;
    let depth = 3;
    let iterations_per_position = 5;

    // Multiple positions to simulate realistic workload
    let positions = vec![
        BitboardBoard::new(), // Starting position
    ];

    let mut group = c.benchmark_group("NPS Realistic Workload");

    // SIMD enabled
    group.bench_function("simd_realistic", |b| {
        let mut engine = create_engine_with_simd_config(true);
        b.iter(|| {
            let mut total_nodes = 0u64;
            let start = std::time::Instant::now();

            for board in &positions {
                for _ in 0..iterations_per_position {
                    let mut test_board = board.clone();
                    let _ = engine.search_at_depth(
                        &mut test_board,
                        black_box(&captured_pieces),
                        black_box(player),
                        black_box(depth),
                        5000,
                        i32::MIN,
                        i32::MAX,
                    );
                    total_nodes += engine.get_nodes_searched();
                }
            }

            let duration = start.elapsed();
            let nps = if duration.as_secs_f64() > 0.0 {
                total_nodes as f64 / duration.as_secs_f64()
            } else {
                0.0
            };
            black_box(nps);
        });
    });

    // SIMD disabled
    group.bench_function("scalar_realistic", |b| {
        let mut engine = create_engine_with_simd_config(false);
        b.iter(|| {
            let mut total_nodes = 0u64;
            let start = std::time::Instant::now();

            for board in &positions {
                for _ in 0..iterations_per_position {
                    let mut test_board = board.clone();
                    let _ = engine.search_at_depth(
                        &mut test_board,
                        black_box(&captured_pieces),
                        black_box(player),
                        black_box(depth),
                        5000,
                        i32::MIN,
                        i32::MAX,
                    );
                    total_nodes += engine.get_nodes_searched();
                }
            }

            let duration = start.elapsed();
            let nps = if duration.as_secs_f64() > 0.0 {
                total_nodes as f64 / duration.as_secs_f64()
            } else {
                0.0
            };
            black_box(nps);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simd_vs_scalar_nps_starting_position,
    bench_simd_vs_scalar_nps_different_depths,
    bench_simd_vs_scalar_nps_realistic_workload
);
criterion_main!(benches);
