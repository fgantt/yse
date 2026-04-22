//! NNUE Evaluation Speed Benchmarks
//!
//! Measures positions/second for NNUE evaluation under different conditions:
//! - Full refresh (current baseline)
//! - Incremental accumulator update after a move
//! - PST evaluation (comparison baseline)
//!
//! Run with: cargo bench --bench nnue_speed_benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{NNUEAccumulator, NNUEWeights};
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;

/// Benchmark full NNUE refresh evaluation (current implementation)
fn bench_nnue_full_refresh(c: &mut Criterion) {
    let weights = NNUEWeights::new(256, 32);
    let board = BitboardBoard::new();
    let (h1, h2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(h1, h2);

    c.bench_function("nnue_full_refresh_eval", |b| {
        b.iter(|| {
            accumulator.refresh(black_box(&board), black_box(&weights));
            black_box(accumulator.evaluate(black_box(&weights)))
        });
    });
}

/// Benchmark just the NNUE accumulator refresh (no forward pass)
fn bench_nnue_refresh_only(c: &mut Criterion) {
    let weights = NNUEWeights::new(256, 32);
    let board = BitboardBoard::new();
    let (h1, h2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(h1, h2);

    c.bench_function("nnue_refresh_only", |b| {
        b.iter(|| {
            accumulator.refresh(black_box(&board), black_box(&weights));
        });
    });
}

/// Benchmark just the NNUE forward pass (evaluate from pre-computed accumulator)
fn bench_nnue_forward_pass_only(c: &mut Criterion) {
    let weights = NNUEWeights::new(256, 32);
    let board = BitboardBoard::new();
    let (h1, h2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(h1, h2);
    accumulator.refresh(&board, &weights);

    c.bench_function("nnue_forward_pass_only", |b| {
        b.iter(|| {
            black_box(accumulator.evaluate(black_box(&weights)))
        });
    });
}

/// Benchmark NNUE incremental update (add + remove one piece)
fn bench_nnue_incremental_update(c: &mut Criterion) {
    let weights = NNUEWeights::new(256, 32);
    let board = BitboardBoard::new();
    let (h1, h2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(h1, h2);
    accumulator.refresh(&board, &weights);

    // Get a legal move to benchmark incremental update
    let move_generator = MoveGenerator::new();
    let captured_pieces = CapturedPieces::new();
    let legal_moves = move_generator.generate_legal_moves(&board, Player::Black, &captured_pieces);
    let mv = &legal_moves[0];

    // Get the piece info for the move
    let piece = board.get_piece(mv.from.unwrap()).unwrap();
    let captured = if mv.is_capture {
        board.get_piece(mv.to)
    } else {
        None
    };

    c.bench_function("nnue_incremental_update_and_eval", |b| {
        b.iter(|| {
            // Simulate: update for move, evaluate, then undo
            accumulator.update_move(
                black_box(mv.from),
                black_box(mv.to),
                black_box(piece),
                black_box(captured),
                black_box(&weights),
            );
            let score = accumulator.evaluate(black_box(&weights));
            // Undo (reverse operation)
            accumulator.remove_piece(piece, mv.to, &weights);
            if let Some(cap) = captured {
                accumulator.add_piece(cap, mv.to, &weights);
            }
            if let Some(from) = mv.from {
                accumulator.add_piece(piece, from, &weights);
            }
            black_box(score)
        });
    });
}

/// Benchmark PST evaluation for comparison
fn bench_pst_evaluation(c: &mut Criterion) {
    let mut evaluator = PositionEvaluator::new();
    let board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();

    c.bench_function("pst_evaluation", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate(
                black_box(&board),
                black_box(Player::Black),
                black_box(&captured_pieces),
            ))
        });
    });
}

/// Benchmark NNUE through PositionEvaluator (full stack)
fn bench_nnue_via_evaluator(c: &mut Criterion) {
    let mut evaluator = PositionEvaluator::new();
    evaluator.enable_nnue(256, 32);
    let board = BitboardBoard::new();
    let captured_pieces = CapturedPieces::new();

    c.bench_function("nnue_via_evaluator", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate(
                black_box(&board),
                black_box(Player::Black),
                black_box(&captured_pieces),
            ))
        });
    });
}

/// Compare refresh vs incremental across different board positions
fn bench_nnue_refresh_vs_incremental(c: &mut Criterion) {
    let weights = NNUEWeights::new(256, 32);
    let board = BitboardBoard::new();
    let (h1, h2) = weights.hidden_sizes();

    let mut group = c.benchmark_group("nnue_refresh_vs_incremental");

    // Full refresh
    group.bench_function("full_refresh", |b| {
        let mut accumulator = NNUEAccumulator::new(h1, h2);
        b.iter(|| {
            accumulator.refresh(black_box(&board), black_box(&weights));
            black_box(accumulator.evaluate(black_box(&weights)))
        });
    });

    // Incremental (pre-refreshed, then just evaluate)
    group.bench_function("evaluate_only", |b| {
        let mut accumulator = NNUEAccumulator::new(h1, h2);
        accumulator.refresh(&board, &weights);
        b.iter(|| {
            black_box(accumulator.evaluate(black_box(&weights)))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_nnue_full_refresh,
    bench_nnue_refresh_only,
    bench_nnue_forward_pass_only,
    bench_nnue_incremental_update,
    bench_pst_evaluation,
    bench_nnue_via_evaluator,
    bench_nnue_refresh_vs_incremental,
);
criterion_main!(benches);
