#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::evaluation_simd::SimdEvaluator;
use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
use shogi_engine::evaluation::tactical_patterns_simd::SimdPatternMatcher;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{PieceType, Player, Position};
use shogi_engine::types::evaluation::TaperedScore;

/// Scalar implementation for comparison
fn scalar_evaluation_workload(
    board: &BitboardBoard,
    pst: &PieceSquareTables,
    captured_pieces: &CapturedPieces,
) -> TaperedScore {
    let mut score = TaperedScore::default();

    // Scalar PST evaluation
    for row in 0..9 {
        for col in 0..9 {
            let pos = Position::new(row, col);
            if let Some(piece) = board.get_piece(pos) {
                let pst_value = pst.get_value(piece.piece_type, pos, piece.player);
                if piece.player == Player::Black {
                    score += pst_value;
                } else {
                    score -= pst_value;
                }
            }
        }
    }

    // Scalar material counting
    let piece_types = vec![
        PieceType::Pawn,
        PieceType::Lance,
        PieceType::Knight,
        PieceType::Silver,
        PieceType::Gold,
        PieceType::Bishop,
        PieceType::Rook,
    ];
    let player_idx = 0;
    let pieces = board.get_pieces();
    for &piece_type in &piece_types {
        let idx = piece_type.as_index();
        let _count = pieces[player_idx][idx].count_ones();
    }

    score
}

fn bench_overall_evaluation_simd_vs_scalar(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let simd_evaluator = SimdEvaluator::new();
    let captured_pieces = CapturedPieces::new();

    let mut group = c.benchmark_group("overall_evaluation");
    group.sample_size(500);

    // SIMD evaluation
    group.bench_function("simd_evaluation", |b| {
        b.iter(|| {
            let _pst = simd_evaluator.evaluate_pst_batch(
                black_box(&board),
                black_box(&pst),
                black_box(Player::Black),
            );
            let piece_types = vec![
                PieceType::Pawn,
                PieceType::Lance,
                PieceType::Knight,
                PieceType::Silver,
                PieceType::Gold,
                PieceType::Bishop,
                PieceType::Rook,
            ];
            let _counts = simd_evaluator.count_material_batch(
                black_box(&board),
                black_box(&piece_types),
                black_box(Player::Black),
            );
            black_box(_pst);
        });
    });

    // Scalar evaluation
    group.bench_function("scalar_evaluation", |b| {
        b.iter(|| {
            black_box(scalar_evaluation_workload(
                black_box(&board),
                black_box(&pst),
                black_box(&captured_pieces),
            ));
        });
    });

    group.finish();
}

fn bench_full_evaluation_pipeline(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let simd_evaluator = SimdEvaluator::new();
    let simd_matcher = SimdPatternMatcher::new();
    let captured_pieces = CapturedPieces::new();

    let mut group = c.benchmark_group("full_evaluation_pipeline");
    group.sample_size(200);

    // Full SIMD pipeline
    group.bench_function("simd_pipeline", |b| {
        b.iter(|| {
            // PST evaluation
            let _pst_score = simd_evaluator.evaluate_pst_batch(&board, &pst, Player::Black);

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

            // Pattern matching
            let pieces = vec![
                (Position::new(4, 4), PieceType::Rook),
                (Position::new(4, 5), PieceType::Bishop),
            ];
            let _forks = simd_matcher.detect_forks_batch(&board, &pieces, Player::Black);

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

            black_box(_pst_score);
        });
    });

    group.finish();
}

fn bench_evaluation_components(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let pst = PieceSquareTables::new();
    let simd_evaluator = SimdEvaluator::new();
    let simd_matcher = SimdPatternMatcher::new();
    let captured_pieces = CapturedPieces::new();

    let mut group = c.benchmark_group("evaluation_components");
    group.sample_size(1000);

    // PST evaluation
    group.bench_function("pst_evaluation", |b| {
        b.iter(|| {
            black_box(simd_evaluator.evaluate_pst_batch(
                black_box(&board),
                black_box(&pst),
                black_box(Player::Black),
            ));
        });
    });

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
    group.bench_function("material_counting", |b| {
        b.iter(|| {
            black_box(simd_evaluator.count_material_batch(
                black_box(&board),
                black_box(&piece_types),
                black_box(Player::Black),
            ));
        });
    });

    // Pattern matching
    let pieces =
        vec![(Position::new(4, 4), PieceType::Rook), (Position::new(4, 5), PieceType::Bishop)];
    group.bench_function("pattern_matching", |b| {
        b.iter(|| {
            black_box(simd_matcher.detect_forks_batch(
                black_box(&board),
                black_box(&pieces),
                black_box(Player::Black),
            ));
        });
    });

    // Hand material
    let piece_values = vec![
        (PieceType::Pawn, TaperedScore::new_tapered(100, 100)),
        (PieceType::Rook, TaperedScore::new_tapered(500, 500)),
    ];
    group.bench_function("hand_material", |b| {
        b.iter(|| {
            black_box(simd_evaluator.evaluate_hand_material_batch(
                black_box(&captured_pieces),
                black_box(&piece_values),
                black_box(Player::Black),
            ));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_overall_evaluation_simd_vs_scalar,
    bench_full_evaluation_pipeline,
    bench_evaluation_components
);
criterion_main!(benches);
