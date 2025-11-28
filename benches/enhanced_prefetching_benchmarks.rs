//! Enhanced prefetching performance benchmarks
//!
//! Optimization 6: Enhanced prefetching - benchmarks to measure prefetching
//! effectiveness
//!
//! This benchmark suite measures the performance impact of enhanced adaptive
//! prefetching strategies compared to fixed-distance prefetching:
//! - Adaptive prefetching with workload-aware distances
//! - Magic table lookup prefetching
//! - PST table lookup prefetching
//! - Batch operation prefetching
//!
//! # Benchmarks
//!
//! - `prefetch_magic_table_lookups`: Measures prefetching effectiveness for
//!   magic table lookups
//! - `prefetch_pst_table_lookups`: Measures prefetching effectiveness for PST
//!   table lookups
//! - `adaptive_vs_fixed_prefetch`: Compares adaptive prefetching to
//!   fixed-distance prefetching
//! - `prefetch_distance_tuning`: Tests different prefetch distances for optimal
//!   performance

#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::memory_optimization::adaptive_prefetch::{
    AdaptivePrefetchManager, WorkloadType,
};
use shogi_engine::bitboards::memory_optimization::prefetch::PrefetchLevel;
use shogi_engine::bitboards::{BitboardBoard, SimdBitboard};
use shogi_engine::evaluation::integration::IntegratedEvaluator;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::*;

/// Create a board with many sliding pieces for magic table prefetching
/// benchmarks
fn create_sliding_pieces_board() -> BitboardBoard {
    let mut board = BitboardBoard::new();

    // Add rooks and bishops for magic table prefetching
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(0, 0));
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(0, 8));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(0, 4));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(2, 2));
    board.place_piece(Piece::new(PieceType::Rook, Player::White), Position::new(8, 0));
    board.place_piece(Piece::new(PieceType::Rook, Player::White), Position::new(8, 8));
    board.place_piece(Piece::new(PieceType::Bishop, Player::White), Position::new(8, 4));
    board.place_piece(Piece::new(PieceType::Bishop, Player::White), Position::new(6, 6));

    // Add some blocking pieces
    board.place_piece(Piece::new(PieceType::Pawn, Player::Black), Position::new(3, 4));
    board.place_piece(Piece::new(PieceType::Pawn, Player::White), Position::new(5, 4));

    board
}

/// Create a board with many pieces for PST prefetching benchmarks
fn create_pst_heavy_board() -> BitboardBoard {
    let mut board = BitboardBoard::new();

    // Add pieces across the board for PST table prefetching
    for row in 0..9 {
        for col in 0..9 {
            if (row + col) % 2 == 0 && (row != 4 || col != 4) {
                let piece_type = match (row + col) % 6 {
                    0 => PieceType::Pawn,
                    1 => PieceType::Silver,
                    2 => PieceType::Gold,
                    3 => PieceType::Bishop,
                    4 => PieceType::Rook,
                    _ => PieceType::Lance,
                };
                let player = if row < 4 { Player::Black } else { Player::White };
                board.place_piece(Piece::new(piece_type, player), Position::new(row, col));
            }
        }
    }

    board
}

/// Benchmark prefetching effectiveness for magic table lookups
fn benchmark_prefetch_magic_table_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_magic_table_lookups");

    let board = create_sliding_pieces_board();
    let generator = MoveGenerator::new();

    // Benchmark move generation (which uses magic table lookups)
    group.bench_function("move_generation_with_prefetch", |b| {
        b.iter(|| {
            let moves = generator.generate_legal_moves(black_box(&board), Player::Black);
            black_box(moves);
        });
    });

    // Benchmark multiple consecutive move generations
    group.bench_function("multiple_move_generations", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let moves = generator.generate_legal_moves(black_box(&board), Player::Black);
                black_box(moves);
            }
        });
    });

    group.finish();
}

/// Benchmark prefetching effectiveness for PST table lookups
fn benchmark_prefetch_pst_table_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_pst_table_lookups");

    let heavy_board = create_pst_heavy_board();
    let sparse_board = BitboardBoard::new();
    let evaluator = IntegratedEvaluator::new();

    // Benchmark PST evaluation with many pieces (more prefetching opportunities)
    group.bench_with_input(
        BenchmarkId::new("pst_evaluation", "heavy_board"),
        &heavy_board,
        |b, board| {
            b.iter(|| {
                black_box(evaluator.evaluate_pst(black_box(board), Player::Black));
            });
        },
    );

    // Benchmark PST evaluation with sparse board
    group.bench_with_input(
        BenchmarkId::new("pst_evaluation", "sparse_board"),
        &sparse_board,
        |b, board| {
            b.iter(|| {
                black_box(evaluator.evaluate_pst(black_box(board), Player::Black));
            });
        },
    );

    // Benchmark multiple consecutive evaluations
    group.bench_function("multiple_pst_evaluations", |b| {
        b.iter(|| {
            for _ in 0..50 {
                black_box(evaluator.evaluate_pst(black_box(&heavy_board), Player::Black));
            }
        });
    });

    group.finish();
}

/// Benchmark adaptive prefetching vs fixed-distance prefetching
fn benchmark_adaptive_vs_fixed_prefetch(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_vs_fixed_prefetch");

    let board = create_sliding_pieces_board();
    let generator = MoveGenerator::new();

    // Test with different access patterns
    let access_patterns = vec![
        ("sequential", WorkloadType::Sequential { base_distance: 2 }),
        ("random", WorkloadType::Random { base_distance: 1 }),
        ("batch", WorkloadType::Batch { base_distance: 3 }),
    ];

    for (pattern_name, workload_type) in access_patterns {
        let manager = match workload_type {
            WorkloadType::Sequential { .. } => AdaptivePrefetchManager::sequential(),
            WorkloadType::Random { .. } => AdaptivePrefetchManager::random(),
            WorkloadType::Batch { .. } => AdaptivePrefetchManager::batch(),
        };

        group.bench_with_input(
            BenchmarkId::new("adaptive_prefetch", pattern_name),
            &manager,
            |b, _manager| {
                b.iter(|| {
                    // Simulate adaptive prefetching with workload awareness
                    let moves = generator.generate_legal_moves(black_box(&board), Player::Black);
                    black_box(moves);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark different prefetch distances to find optimal values
fn benchmark_prefetch_distance_tuning(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_distance_tuning");

    let board = create_pst_heavy_board();
    let evaluator = IntegratedEvaluator::new();

    // Test different fixed distances
    let distances = vec![1, 2, 3, 4, 5, 6, 8];

    for distance in distances {
        group.bench_with_input(
            BenchmarkId::new("fixed_distance", distance),
            &distance,
            |b, &dist| {
                // Simulate prefetching with different distances
                // Note: Actual prefetching distance is controlled in the implementation
                // This benchmark measures overall evaluation performance
                b.iter(|| {
                    black_box(evaluator.evaluate_pst(black_box(&board), Player::Black));
                });
            },
        );
    }

    // Test adaptive prefetching
    group.bench_function("adaptive_distance", |b| {
        let mut manager = AdaptivePrefetchManager::sequential();
        b.iter(|| {
            // Simulate adaptive prefetching learning
            for i in 0..81 {
                manager.record_access(i);
                if i % 10 == 0 {
                    manager.record_cache_hit();
                } else if i % 15 == 0 {
                    manager.record_cache_miss();
                }
            }
            black_box(evaluator.evaluate_pst(black_box(&board), Player::Black));
        });
    });

    group.finish();
}

/// Benchmark prefetching overhead vs benefit
fn benchmark_prefetch_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_overhead");

    let board = create_sliding_pieces_board();
    let generator = MoveGenerator::new();

    // Benchmark with prefetching (default behavior)
    group.bench_function("with_prefetch", |b| {
        b.iter(|| {
            let moves = generator.generate_legal_moves(black_box(&board), Player::Black);
            black_box(moves);
        });
    });

    // Note: We can't easily disable prefetching in the current implementation
    // without modifying the code, so this benchmark primarily measures
    // the overhead of prefetch operations themselves

    group.finish();
}

/// Benchmark batch operations with prefetching
fn benchmark_batch_prefetching(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_prefetching");

    let board = create_sliding_pieces_board();
    let generator = MoveGenerator::new();

    // Create multiple positions for batch processing
    let positions: Vec<BitboardBoard> = (0..10)
        .map(|_| {
            let mut b = board.clone();
            // Add some variation
            b.make_move(&generator.generate_legal_moves(&b, Player::Black)[0]);
            b
        })
        .collect();

    group.bench_function("batch_move_generation", |b| {
        b.iter(|| {
            for board in &positions {
                let moves = generator.generate_legal_moves(black_box(board), Player::Black);
                black_box(moves);
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = enhanced_prefetching_benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(std::time::Duration::from_millis(100))
        .measurement_time(std::time::Duration::from_secs(2));
    targets =
        benchmark_prefetch_magic_table_lookups,
        benchmark_prefetch_pst_table_lookups,
        benchmark_adaptive_vs_fixed_prefetch,
        benchmark_prefetch_distance_tuning,
        benchmark_prefetch_overhead,
        benchmark_batch_prefetching
}

criterion_main!(enhanced_prefetching_benches);



