#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, BitboardBoard, SimdBitboard};
use shogi_engine::evaluation::tactical_patterns_simd::SimdPatternMatcher;
use shogi_engine::types::{PieceType, Player, Position};

fn bench_count_attack_targets(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let attack_pattern = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F);
    let target_mask = SimdBitboard::from_u128(0x3333_3333_3333_3333);

    c.bench_function("simd_count_attack_targets", |b| {
        b.iter(|| {
            black_box(
                matcher.count_attack_targets(black_box(attack_pattern), black_box(target_mask)),
            );
        });
    });
}

fn bench_count_attack_targets_batch(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let attack_patterns = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F),
        SimdBitboard::from_u128(0x3333_3333_3333_3333),
        SimdBitboard::from_u128(0x5555_5555_5555_5555),
        SimdBitboard::from_u128(0xAAAA_AAAA_AAAA_AAAA),
    ]);
    let target_mask = SimdBitboard::from_u128(0xFFFF_FFFF_FFFF_FFFF);

    c.bench_function("simd_count_attack_targets_batch", |b| {
        b.iter(|| {
            black_box(
                matcher.count_attack_targets_batch(
                    black_box(&attack_patterns),
                    black_box(target_mask),
                ),
            );
        });
    });
}

fn bench_detect_forks_batch(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();
    let pieces = vec![
        (Position::new(4, 4), PieceType::Rook),
        (Position::new(4, 5), PieceType::Bishop),
        (Position::new(5, 4), PieceType::Knight),
        (Position::new(5, 5), PieceType::Gold),
    ];

    c.bench_function("simd_detect_forks_batch", |b| {
        b.iter(|| {
            black_box(matcher.detect_forks_batch(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_detect_patterns_batch(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();
    let positions: Vec<Position> =
        (0..16).map(|i| Position::new((i / 9) as u8, (i % 9) as u8)).collect();

    c.bench_function("simd_detect_patterns_batch", |b| {
        b.iter(|| {
            black_box(matcher.detect_patterns_batch(
                black_box(&board),
                black_box(&positions),
                black_box(PieceType::Rook),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_detect_pins_batch(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();
    let pieces = vec![
        (Position::new(4, 4), PieceType::Rook),
        (Position::new(4, 5), PieceType::Bishop),
        (Position::new(5, 4), PieceType::Lance),
        (Position::new(5, 5), PieceType::PromotedRook),
    ];

    c.bench_function("simd_detect_pins_batch", |b| {
        b.iter(|| {
            black_box(matcher.detect_pins_batch(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_detect_skewers_batch(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();
    let pieces = vec![
        (Position::new(4, 4), PieceType::Rook),
        (Position::new(4, 5), PieceType::Bishop),
        (Position::new(5, 4), PieceType::PromotedRook),
        (Position::new(5, 5), PieceType::PromotedBishop),
    ];

    c.bench_function("simd_detect_skewers_batch", |b| {
        b.iter(|| {
            black_box(matcher.detect_skewers_batch(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
            ));
        });
    });
}

fn bench_detect_discovered_attacks_batch(c: &mut Criterion) {
    let matcher = SimdPatternMatcher::new();
    let board = BitboardBoard::new();
    let pieces = vec![
        (Position::new(4, 4), PieceType::Rook),
        (Position::new(4, 5), PieceType::Bishop),
        (Position::new(5, 4), PieceType::Knight),
        (Position::new(5, 5), PieceType::Gold),
    ];
    let target_pos = Position::new(8, 8);

    c.bench_function("simd_detect_discovered_attacks_batch", |b| {
        b.iter(|| {
            black_box(matcher.detect_discovered_attacks_batch(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
                black_box(target_pos),
            ));
        });
    });
}

criterion_group!(
    benches,
    bench_count_attack_targets,
    bench_count_attack_targets_batch,
    bench_detect_forks_batch,
    bench_detect_patterns_batch,
    bench_detect_pins_batch,
    bench_detect_skewers_batch,
    bench_detect_discovered_attacks_batch
);
criterion_main!(benches);
