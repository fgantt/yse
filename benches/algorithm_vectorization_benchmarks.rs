#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, BitboardBoard, SimdBitboard};
use shogi_engine::evaluation::evaluation_simd::SimdEvaluator;
use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
use shogi_engine::evaluation::tactical_patterns_simd::SimdPatternMatcher;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{PieceType, Player, Position};
use shogi_engine::types::evaluation::TaperedScore;

/// Scalar implementation of PST evaluation for comparison
fn scalar_evaluate_pst(
    board: &BitboardBoard,
    pst: &PieceSquareTables,
    player: Player,
) -> TaperedScore {
    let mut score = TaperedScore::default();

    for row in 0..9 {
        for col in 0..9 {
            let pos = Position::new(row, col);
            if let Some(piece) = board.get_piece(pos) {
                let pst_value = pst.get_value(piece.piece_type, pos, piece.player);

                if piece.player == player {
                    score += pst_value;
                } else {
                    score -= pst_value;
                }
            }
        }
    }

    score
}

/// Scalar implementation of material counting for comparison
fn scalar_count_material(
    board: &BitboardBoard,
    piece_types: &[PieceType],
    player: Player,
) -> Vec<i32> {
    let mut counts = vec![0; piece_types.len()];
    let player_idx = if player == Player::Black { 0 } else { 1 };
    let pieces = board.get_pieces();

    for (i, &piece_type) in piece_types.iter().enumerate() {
        let idx = piece_type.as_index();
        let bitboard = pieces[player_idx][idx];
        counts[i] = bitboard.count_ones() as i32;
    }

    counts
}

/// Scalar implementation of fork detection for comparison
fn scalar_detect_forks(
    board: &BitboardBoard,
    pieces: &[(Position, PieceType)],
    player: Player,
) -> Vec<(Position, PieceType, u32)> {
    let mut forks = Vec::new();
    let opponent = player.opposite();

    // Build opponent pieces bitboard
    let mut opponent_pieces_bitboard = shogi_engine::types::Bitboard::empty();
    for row in 0..9 {
        for col in 0..9 {
            let pos = Position::new(row, col);
            if let Some(piece) = board.get_piece(pos) {
                if piece.player == opponent {
                    shogi_engine::types::set_bit(&mut opponent_pieces_bitboard, pos);
                }
            }
        }
    }

    for &(pos, piece_type) in pieces {
        let attacks = board.get_attack_pattern_precomputed(pos, piece_type, player);
        let targets = attacks & opponent_pieces_bitboard;
        let target_count = targets.count_ones();

        if target_count >= 2 {
            forks.push((pos, piece_type, target_count));
        }
    }

    forks
}

fn bench_pst_evaluation_comparison(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let simd_evaluator = SimdEvaluator::new();

    let mut group = c.benchmark_group("pst_evaluation");
    group.sample_size(1000);

    group.bench_function("simd_pst_batch", |b| {
        b.iter(|| {
            black_box(simd_evaluator.evaluate_pst_batch(
                black_box(&board),
                black_box(&pst),
                black_box(Player::Black),
            ));
        });
    });

    group.bench_function("scalar_pst", |b| {
        b.iter(|| {
            black_box(scalar_evaluate_pst(
                black_box(&board),
                black_box(&pst),
                black_box(Player::Black),
            ));
        });
    });

    group.finish();
}

fn bench_material_counting_comparison(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let simd_evaluator = SimdEvaluator::new();
    let piece_types = vec![
        PieceType::Pawn,
        PieceType::Lance,
        PieceType::Knight,
        PieceType::Silver,
        PieceType::Gold,
        PieceType::Bishop,
        PieceType::Rook,
    ];

    let mut group = c.benchmark_group("material_counting");
    group.sample_size(1000);

    group.bench_function("simd_count_batch", |b| {
        b.iter(|| {
            black_box(simd_evaluator.count_material_batch(
                black_box(&board),
                black_box(&piece_types),
                black_box(Player::Black),
            ));
        });
    });

    group.bench_function("scalar_count", |b| {
        b.iter(|| {
            black_box(scalar_count_material(
                black_box(&board),
                black_box(&piece_types),
                black_box(Player::Black),
            ));
        });
    });

    group.finish();
}

fn bench_pattern_matching_comparison(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let simd_matcher = SimdPatternMatcher::new();
    let pieces = vec![
        (Position::new(4, 4), PieceType::Rook),
        (Position::new(4, 5), PieceType::Bishop),
        (Position::new(5, 4), PieceType::Knight),
        (Position::new(5, 5), PieceType::Gold),
    ];

    let mut group = c.benchmark_group("pattern_matching");
    group.sample_size(1000);

    group.bench_function("simd_detect_forks", |b| {
        b.iter(|| {
            black_box(simd_matcher.detect_forks_batch(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
            ));
        });
    });

    group.bench_function("scalar_detect_forks", |b| {
        b.iter(|| {
            black_box(scalar_detect_forks(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
            ));
        });
    });

    group.finish();
}

fn bench_batch_operations_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations_speedup");
    group.sample_size(1000);

    // Test different batch sizes
    bench_batch_size::<4>(&mut group);
    bench_batch_size::<8>(&mut group);
    bench_batch_size::<16>(&mut group);
    bench_batch_size::<32>(&mut group);

    group.finish();
}

fn bench_batch_size<const N: usize>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
) {
    let mut a_data = [SimdBitboard::empty(); N];
    let mut b_data = [SimdBitboard::empty(); N];

    for i in 0..N {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }

    let a = AlignedBitboardArray::<N>::from_slice(&a_data);
    let b = AlignedBitboardArray::<N>::from_slice(&b_data);

    // SIMD batch AND
    group.bench_function(&format!("simd_batch_and_{}", N), |bencher| {
        bencher.iter(|| {
            black_box(a.batch_and(&b));
        });
    });

    // Scalar batch AND
    group.bench_function(&format!("scalar_batch_and_{}", N), |bencher| {
        bencher.iter(|| {
            let mut result = AlignedBitboardArray::<N>::new();
            for i in 0..N {
                result.set(i, *a.get(i) & *b.get(i));
            }
            black_box(result);
        });
    });
}

fn bench_evaluation_pipeline(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let simd_evaluator = SimdEvaluator::new();
    let captured_pieces = CapturedPieces::new();

    let mut group = c.benchmark_group("evaluation_pipeline");
    group.sample_size(500);

    // Simulate full evaluation pipeline with SIMD
    group.bench_function("simd_pipeline", |b| {
        b.iter(|| {
            // PST evaluation
            let pst_score = simd_evaluator.evaluate_pst_batch(&board, &pst, Player::Black);

            // Material counting
            let piece_types = vec![
                PieceType::Pawn,
                PieceType::Lance,
                PieceType::Knight,
                PieceType::Silver,
                PieceType::Gold,
                PieceType::Bishop,
                PieceType::Rook,
            ];
            let _counts = simd_evaluator.count_material_batch(&board, &piece_types, Player::Black);

            // Hand material
            let piece_values = vec![
                (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
                (PieceType::Rook, TaperedScore::new_tapered(500, 500)),
            ];
            let _hand_score = simd_evaluator.evaluate_hand_material_batch(
                &captured_pieces,
                &piece_values,
                Player::Black,
            );

            black_box(pst_score);
        });
    });

    // Scalar pipeline
    group.bench_function("scalar_pipeline", |b| {
        b.iter(|| {
            // PST evaluation
            let pst_score = scalar_evaluate_pst(&board, &pst, Player::Black);

            // Material counting
            let piece_types = vec![
                PieceType::Pawn,
                PieceType::Lance,
                PieceType::Knight,
                PieceType::Silver,
                PieceType::Gold,
                PieceType::Bishop,
                PieceType::Rook,
            ];
            let _counts = scalar_count_material(&board, &piece_types, Player::Black);

            black_box(pst_score);
        });
    });

    group.finish();
}

fn bench_move_generation_vectorized(c: &mut Criterion) {
    use shogi_engine::bitboards::sliding_moves::SlidingMoveGenerator;
    use std::sync::Arc;

    let board = BitboardBoard::new();

    // Create magic table for generator (use default for benchmarks)
    let magic_table = Arc::new(shogi_engine::types::MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);

    // Collect pieces for batch processing
    let mut vectorized_pieces = Vec::new();
    let mut scalar_pieces = Vec::new();
    for row in 0..9 {
        for col in 0..9 {
            let pos = Position::new(row, col);
            if let Some(piece) = board.get_piece(pos) {
                if matches!(
                    piece.piece_type,
                    PieceType::Rook
                        | PieceType::Bishop
                        | PieceType::PromotedRook
                        | PieceType::PromotedBishop
                ) {
                    vectorized_pieces.push((pos, piece));
                    scalar_pieces.push((pos, piece.piece_type));
                }
            }
        }
    }

    let mut group = c.benchmark_group("move_generation");
    group.sample_size(500);

    // Vectorized batch generation
    #[cfg(feature = "simd")]
    {
        group.bench_function("vectorized_batch", |b| {
            b.iter(|| {
                black_box(generator.generate_sliding_moves_batch_vectorized(
                    &board,
                    &vectorized_pieces,
                    Player::Black,
                ));
            });
        });
    }

    // Regular batch generation
    group.bench_function("regular_batch", |b| {
        b.iter(|| {
            black_box(generator.generate_sliding_moves_batch(
                &board,
                &scalar_pieces,
                Player::Black,
            ));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pst_evaluation_comparison,
    bench_material_counting_comparison,
    bench_pattern_matching_comparison,
    bench_batch_operations_speedup,
    bench_evaluation_pipeline,
    bench_move_generation_vectorized
);
criterion_main!(benches);
