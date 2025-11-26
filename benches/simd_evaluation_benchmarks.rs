#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::evaluation_simd::SimdEvaluator;
use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{PieceType, Player};
use shogi_engine::types::evaluation::TaperedScore;

fn bench_evaluate_pst_batch(c: &mut Criterion) {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();

    c.bench_function("simd_evaluate_pst_batch", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate_pst_batch(
                black_box(&board),
                black_box(&pst),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_evaluate_pst_optimized(c: &mut Criterion) {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();

    c.bench_function("simd_evaluate_pst_optimized", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate_pst_optimized(
                black_box(&board),
                black_box(&pst),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_count_material_batch(c: &mut Criterion) {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::new();
    let piece_types = vec![
        PieceType::Pawn,
        PieceType::Lance,
        PieceType::Knight,
        PieceType::Silver,
        PieceType::Gold,
        PieceType::Bishop,
        PieceType::Rook,
    ];

    c.bench_function("simd_count_material_batch", |b| {
        b.iter(|| {
            black_box(evaluator.count_material_batch(
                black_box(&board),
                black_box(&piece_types),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_evaluate_material_batch(c: &mut Criterion) {
    let evaluator = SimdEvaluator::new();
    let board = BitboardBoard::new();
    let piece_values = vec![
        (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
        (PieceType::Lance, TaperedScore::new_tapered(200, 200)),
        (PieceType::Knight, TaperedScore::new_tapered(300, 300)),
        (PieceType::Silver, TaperedScore::new_tapered(400, 400)),
        (PieceType::Gold, TaperedScore::new_tapered(500, 500)),
        (PieceType::Bishop, TaperedScore::new_tapered(600, 600)),
        (PieceType::Rook, TaperedScore::new_tapered(700, 700)),
    ];

    c.bench_function("simd_evaluate_material_batch", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate_material_batch(
                black_box(&board),
                black_box(&piece_values),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_evaluate_hand_material_batch(c: &mut Criterion) {
    let evaluator = SimdEvaluator::new();
    let captured_pieces = CapturedPieces::new();
    let piece_values = vec![
        (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
        (PieceType::Lance, TaperedScore::new_tapered(200, 200)),
        (PieceType::Knight, TaperedScore::new_tapered(300, 300)),
        (PieceType::Silver, TaperedScore::new_tapered(400, 400)),
        (PieceType::Gold, TaperedScore::new_tapered(500, 500)),
        (PieceType::Bishop, TaperedScore::new_tapered(600, 600)),
        (PieceType::Rook, TaperedScore::new_tapered(700, 700)),
    ];

    c.bench_function("simd_evaluate_hand_material_batch", |b| {
        b.iter(|| {
            black_box(evaluator.evaluate_hand_material_batch(
                black_box(&captured_pieces),
                black_box(&piece_values),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_accumulate_scores_batch(c: &mut Criterion) {
    let evaluator = SimdEvaluator::new();
    let scores: Vec<TaperedScore> =
        (0..100).map(|i| TaperedScore::new_tapered(i * 10, i * 5)).collect();

    c.bench_function("simd_accumulate_scores_batch", |b| {
        b.iter(|| {
            black_box(evaluator.accumulate_scores_batch(black_box(&scores)));
        });
    });
}

criterion_group!(
    benches,
    bench_evaluate_pst_batch,
    bench_evaluate_pst_optimized,
    bench_count_material_batch,
    bench_evaluate_material_batch,
    bench_evaluate_hand_material_batch,
    bench_accumulate_scores_batch
);
criterion_main!(benches);
