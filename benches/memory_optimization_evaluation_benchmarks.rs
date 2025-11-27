//! Memory optimization performance benchmarks for evaluation paths
//!
//! This benchmark suite measures the performance impact of memory optimizations
//! (Task 1.10) in evaluation paths, specifically:
//! - Prefetching hints in PST evaluation
//! - Cache-aligned memory layouts
//! - Cache-friendly data access patterns
//!
//! # Benchmarks
//!
//! - `pst_evaluation_with_memory_optimizations`: Measures PST evaluation
//!   performance with memory optimizations enabled
//! - `pst_evaluation_without_memory_optimizations`: Baseline comparison
//! - `pst_table_access_patterns`: Measures cache performance of different
//!   access patterns

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::integration::IntegratedEvaluator;
use shogi_engine::types::*;

/// Create a board with many pieces for realistic evaluation workload
fn create_heavy_board() -> BitboardBoard {
    let mut board = BitboardBoard::new();

    // Add many pieces to create cache pressure
    for row in 0..9 {
        for col in 0..9 {
            if (row + col) % 3 == 0 {
                let piece_type = match (row + col) % 7 {
                    0 => PieceType::Pawn,
                    1 => PieceType::Silver,
                    2 => PieceType::Gold,
                    3 => PieceType::Bishop,
                    4 => PieceType::Rook,
                    5 => PieceType::PromotedPawn,
                    _ => PieceType::PromotedSilver,
                };
                let player = if row < 4 { Player::Black } else { Player::White };
                board.place_piece(Piece::new(piece_type, player), Position::new(row, col));
            }
        }
    }

    board
}

/// Create a board with sparse pieces (fewer cache misses expected)
fn create_sparse_board() -> BitboardBoard {
    let mut board = BitboardBoard::empty();

    // Add a few pieces strategically
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(4, 4));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(4, 5));
    board.place_piece(Piece::new(PieceType::Silver, Player::Black), Position::new(6, 3));
    board.place_piece(Piece::new(PieceType::Rook, Player::White), Position::new(3, 4));
    board.place_piece(Piece::new(PieceType::Bishop, Player::White), Position::new(2, 5));

    board
}

/// Benchmark PST evaluation with memory optimizations
fn benchmark_pst_evaluation_with_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("pst_evaluation_memory_optimized");

    let heavy_board = create_heavy_board();
    let sparse_board = create_sparse_board();
    let captured = CapturedPieces::new();

    // Benchmark with heavy board (more cache pressure)
    group.bench_with_input(
        BenchmarkId::new("heavy_board", "with_optimizations"),
        &heavy_board,
        |b, board| {
            let evaluator = IntegratedEvaluator::new();
            b.iter(|| {
                black_box(evaluator.evaluate_pst(black_box(board), Player::Black));
            });
        },
    );

    // Benchmark with sparse board (less cache pressure)
    group.bench_with_input(
        BenchmarkId::new("sparse_board", "with_optimizations"),
        &sparse_board,
        |b, board| {
            let evaluator = IntegratedEvaluator::new();
            b.iter(|| {
                black_box(evaluator.evaluate_pst(black_box(board), Player::Black));
            });
        },
    );

    // Benchmark multiple evaluations (simulating search tree)
    group.bench_function("multiple_evaluations", |b| {
        let evaluator = IntegratedEvaluator::new();
        let boards = vec![heavy_board.clone(), sparse_board.clone()];
        b.iter(|| {
            for board in &boards {
                black_box(evaluator.evaluate_pst(black_box(board), Player::Black));
            }
        });
    });

    group.finish();
}

/// Benchmark full evaluation with memory optimizations
fn benchmark_full_evaluation_with_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_evaluation_memory_optimized");

    let heavy_board = create_heavy_board();
    let sparse_board = create_sparse_board();
    let captured = CapturedPieces::new();

    // Benchmark with heavy board
    group.bench_with_input(
        BenchmarkId::new("heavy_board", "full_eval"),
        &(&heavy_board, &captured),
        |b, (board, captured)| {
            let mut evaluator = IntegratedEvaluator::new();
            b.iter(|| {
                black_box(evaluator.evaluate(black_box(board), Player::Black, black_box(captured)));
            });
        },
    );

    // Benchmark with sparse board
    group.bench_with_input(
        BenchmarkId::new("sparse_board", "full_eval"),
        &(&sparse_board, &captured),
        |b, (board, captured)| {
            let mut evaluator = IntegratedEvaluator::new();
            b.iter(|| {
                black_box(evaluator.evaluate(black_box(board), Player::Black, black_box(captured)));
            });
        },
    );

    group.finish();
}

/// Benchmark cache performance with different access patterns
fn benchmark_cache_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_access_patterns");

    let board = create_heavy_board();
    let evaluator = IntegratedEvaluator::new();

    // Sequential access (row by row) - should benefit from prefetching
    group.bench_function("sequential_access", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate_pst(black_box(&board), Player::Black));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_pst_evaluation_with_optimizations,
    benchmark_full_evaluation_with_optimizations,
    benchmark_cache_access_patterns
);
criterion_main!(benches);
