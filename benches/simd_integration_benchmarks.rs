//! SIMD Integration Benchmarks
//!
//! Benchmarks comparing SIMD-optimized implementations vs scalar implementations
//! for evaluation, pattern matching, and move generation.
//!
//! # Task 4.0 (Task 5.7-5.10)

#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::integration::IntegratedEvaluator;
use shogi_engine::evaluation::tactical_patterns::TacticalPatternRecognizer;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;

fn bench_simd_vs_scalar_evaluation(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Enable SIMD
    let mut evaluator_simd = IntegratedEvaluator::new();
    let mut config_simd = evaluator_simd.config().clone();
    config_simd.simd.enable_simd_evaluation = true;
    evaluator_simd.set_config(config_simd);

    // Disable SIMD
    let mut evaluator_scalar = IntegratedEvaluator::new();
    let mut config_scalar = evaluator_scalar.config().clone();
    config_scalar.simd.enable_simd_evaluation = false;
    evaluator_scalar.set_config(config_scalar);

    let mut group = c.benchmark_group("evaluation");
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(evaluator_simd.evaluate(
                black_box(&board),
                Player::Black,
                black_box(&captured),
            ));
        });
    });
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(evaluator_scalar.evaluate(
                black_box(&board),
                Player::Black,
                black_box(&captured),
            ));
        });
    });
    group.finish();
}

fn bench_simd_vs_scalar_pattern_matching(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Enable SIMD
    let mut recognizer_simd = TacticalPatternRecognizer::new();
    let mut config_simd = recognizer_simd.config().clone();
    config_simd.enable_simd_pattern_matching = true;
    recognizer_simd.set_config(config_simd);

    // Disable SIMD
    let mut recognizer_scalar = TacticalPatternRecognizer::new();
    let mut config_scalar = recognizer_scalar.config().clone();
    config_scalar.enable_simd_pattern_matching = false;
    recognizer_scalar.set_config(config_scalar);

    let mut group = c.benchmark_group("pattern_matching");
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(recognizer_simd.evaluate_tactics(
                black_box(&board),
                Player::Black,
                black_box(&captured),
            ));
        });
    });
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(recognizer_scalar.evaluate_tactics(
                black_box(&board),
                Player::Black,
                black_box(&captured),
            ));
        });
    });
    group.finish();
}

fn bench_simd_vs_scalar_move_generation(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();

    // Enable SIMD
    let mut generator_simd = MoveGenerator::new();
    let mut simd_config = generator_simd.simd_config().clone();
    simd_config.enable_simd_move_generation = true;
    generator_simd.set_simd_config(simd_config);

    // Disable SIMD
    let mut generator_scalar = MoveGenerator::new();
    let mut scalar_config = generator_scalar.simd_config().clone();
    scalar_config.enable_simd_move_generation = false;
    generator_scalar.set_simd_config(scalar_config);

    let mut group = c.benchmark_group("move_generation");
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(generator_simd.generate_legal_moves(
                black_box(&board),
                Player::Black,
                black_box(&captured),
            ));
        });
    });
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(generator_scalar.generate_legal_moves(
                black_box(&board),
                Player::Black,
                black_box(&captured),
            ));
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_simd_vs_scalar_evaluation,
    bench_simd_vs_scalar_pattern_matching,
    bench_simd_vs_scalar_move_generation
);
criterion_main!(benches);
