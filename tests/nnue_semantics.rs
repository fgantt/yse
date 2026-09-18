//! Semantic (oracle) tests for the NNUE feature spaces.
//!
//! The pre-existing NNUE tests in `src/evaluation/nnue.rs` are self-consistency
//! checks: they assert that the incremental accumulator agrees with a full
//! refresh, and that two engine-side evaluation paths agree with each other. A
//! feature encoding that is uniformly wrong passes all of them.
//!
//! The tests here instead assert properties the encoding must satisfy to be
//! *correct*, independent of how it happens to be implemented.

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{
    feature_index, feature_index_halfkp, hand_feature_index, NNUEAccumulator, NNUEWeights,
    HAND_FEATURE_BASE_FLAT, HAND_PIECE_TYPES, MAX_HAND_COUNT, NUM_HAND_FEATURES,
    NUM_NNUE_FEATURES, NUM_NNUE_FEATURES_HALFKP, NUM_NNUE_FEATURES_TOTAL,
    NUM_NNUE_FEATURES_WITH_HAND_TOTAL,
};
use shogi_engine::evaluation::nnue_training::extract_active_features_with_stm;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{Piece, PieceType, Player, Position};
use std::collections::HashSet;

const ALL_PIECE_TYPES: [PieceType; 14] = [
    PieceType::Pawn,
    PieceType::Lance,
    PieceType::Knight,
    PieceType::Silver,
    PieceType::Gold,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::King,
    PieceType::PromotedPawn,
    PieceType::PromotedLance,
    PieceType::PromotedKnight,
    PieceType::PromotedSilver,
    PieceType::PromotedBishop,
    PieceType::PromotedRook,
];

/// Deterministic pseudo-random weights, so these tests are hermetic and do not
/// depend on `nnue_weights_trained.json` being present on disk.
fn det_weights(num_features: usize, h1: usize, h2: usize) -> NNUEWeights {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = |lo: i32, hi: i32| -> i32 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = ((state >> 33) as i64) % ((hi - lo + 1) as i64);
        lo + r as i32
    };

    // Input weights use the same +/-127 range the trainer's quantizer clamps to.
    let input_weights_1: Vec<Vec<i16>> =
        (0..num_features).map(|_| (0..h1).map(|_| next(-127, 127) as i16).collect()).collect();
    let hidden_biases_1: Vec<i32> = (0..h1).map(|_| next(-50, 50)).collect();
    let input_weights_2: Vec<Vec<i16>> =
        (0..h1).map(|_| (0..h2).map(|_| next(-2000, 2000) as i16).collect()).collect();
    let hidden_biases_2: Vec<i32> = (0..h2).map(|_| next(-50, 50)).collect();
    let output_weights: Vec<i16> = (0..h2).map(|_| next(-2000, 2000) as i16).collect();

    NNUEWeights {
        input_weights_1,
        hidden_biases_1,
        input_weights_2: Some(input_weights_2),
        hidden_biases_2: Some(hidden_biases_2),
        output_weights,
        output_bias: 0,
    }
}

/// Rotate a board 180 degrees and swap piece colours. The result is the
/// strategically identical position seen from the other side.
fn mirror_board(board: &BitboardBoard) -> BitboardBoard {
    let mut out = BitboardBoard::empty();
    for sq in 0..81u8 {
        let pos = Position::from_u8(sq);
        if let Some(piece) = board.get_piece(pos) {
            let mirrored = Position::from_u8(80 - sq);
            out.place_piece(Piece::new(piece.piece_type, piece.player.opposite()), mirrored);
        }
    }
    out
}

/// Swap the two players' hands.
fn mirror_hand(captured: &CapturedPieces) -> CapturedPieces {
    let mut out = CapturedPieces::new();
    for &player in &[Player::Black, Player::White] {
        for &pt in HAND_PIECE_TYPES.iter() {
            for _ in 0..captured.count(pt, player) {
                out.add_piece(pt, player.opposite());
            }
        }
    }
    out
}

fn eval_flat_stm(board: &BitboardBoard, stm: Player, weights: &NNUEWeights) -> i32 {
    let (h1, h2) = weights.hidden_sizes();
    let mut acc = NNUEAccumulator::new(h1, h2);
    acc.refresh_with_stm(board, stm, weights);
    acc.evaluate(weights)
}

fn eval_pov(board: &BitboardBoard, stm: Player, weights: &NNUEWeights) -> i32 {
    let (h1, h2) = weights.hidden_sizes();
    let mut acc = NNUEAccumulator::new(h1, h2);
    acc.refresh_pov(board, stm, weights);
    acc.evaluate(weights)
}

// ---------------------------------------------------------------------------
// Index-space integrity. These should pass; they pin the layout against drift.
// ---------------------------------------------------------------------------

#[test]
fn flat_feature_index_is_injective_and_in_range() {
    let mut seen = HashSet::new();
    for &player in &[Player::Black, Player::White] {
        for &pt in ALL_PIECE_TYPES.iter() {
            for sq in 0..81u8 {
                let idx = feature_index(player, pt, sq);
                assert!(
                    idx < NUM_NNUE_FEATURES,
                    "flat index {} out of range for ({:?},{:?},{})",
                    idx,
                    player,
                    pt,
                    sq
                );
                assert!(seen.insert(idx), "flat index collision at {}", idx);
            }
        }
    }
    assert_eq!(seen.len(), NUM_NNUE_FEATURES, "flat space is not fully covered");
}

#[test]
fn halfkp_feature_index_is_injective_and_in_range() {
    // Full cross product is 81 * 2268 = 183_708; walk a deterministic subset of
    // king squares to keep the test fast while still proving non-overlap.
    let mut seen = HashSet::new();
    for king_sq in [0u8, 1, 40, 79, 80] {
        for &player in &[Player::Black, Player::White] {
            for &pt in ALL_PIECE_TYPES.iter() {
                for sq in 0..81u8 {
                    let idx = feature_index_halfkp(king_sq, player, pt, sq);
                    assert!(
                        idx < NUM_NNUE_FEATURES_HALFKP,
                        "halfkp index {} out of range (king {})",
                        idx,
                        king_sq
                    );
                    assert!(seen.insert(idx), "halfkp index collision at {}", idx);
                }
            }
        }
    }
}

#[test]
fn hand_feature_index_is_injective_and_bounded() {
    let mut seen = HashSet::new();
    for &player in &[Player::Black, Player::White] {
        for (ti, &pt) in HAND_PIECE_TYPES.iter().enumerate() {
            for k in 1..=MAX_HAND_COUNT[ti] {
                let idx = hand_feature_index(HAND_FEATURE_BASE_FLAT, player, pt, k)
                    .expect("valid hand feature");
                assert!(
                    idx >= HAND_FEATURE_BASE_FLAT && idx < NUM_NNUE_FEATURES_WITH_HAND_TOTAL,
                    "hand index {} outside hand region",
                    idx
                );
                assert!(seen.insert(idx), "hand index collision at {}", idx);
            }
            // Out-of-range levels must be rejected, not silently aliased.
            assert!(hand_feature_index(HAND_FEATURE_BASE_FLAT, player, pt, 0).is_none());
            let over = MAX_HAND_COUNT[ti] + 1;
            assert!(hand_feature_index(HAND_FEATURE_BASE_FLAT, player, pt, over).is_none());
        }
    }
    assert_eq!(seen.len(), NUM_HAND_FEATURES, "hand region not fully covered");
    // The hand region must not overlap the piece-square + stm region.
    assert_eq!(HAND_FEATURE_BASE_FLAT, NUM_NNUE_FEATURES_TOTAL);
}

#[test]
fn king_and_promoted_pieces_are_not_hand_eligible() {
    for bad in [
        PieceType::King,
        PieceType::PromotedPawn,
        PieceType::PromotedLance,
        PieceType::PromotedKnight,
        PieceType::PromotedSilver,
        PieceType::PromotedBishop,
        PieceType::PromotedRook,
    ] {
        assert!(
            hand_feature_index(HAND_FEATURE_BASE_FLAT, Player::Black, bad, 1).is_none(),
            "{:?} must not have a hand feature",
            bad
        );
    }
}

// ---------------------------------------------------------------------------
// Trainer / engine parity. The trainer extracts features in nnue_training.rs;
// the engine accumulates them in nnue.rs. Nothing currently tests that these
// two independent implementations agree.
// ---------------------------------------------------------------------------

#[test]
fn trainer_feature_extraction_matches_engine_refresh() {
    let weights = det_weights(NUM_NNUE_FEATURES_TOTAL, 16, 8);
    let fens = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
        "4k4/9/4S4/9/9/9/9/9/4K4 w - 1",
    ];
    for fen in fens {
        let (board, stm, _cap) = BitboardBoard::from_fen(fen).expect("from_fen");

        // Engine path.
        let (h1, h2) = weights.hidden_sizes();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.refresh_with_stm(&board, stm, &weights);

        // Trainer path: sum the extracted feature rows by hand.
        let feats = extract_active_features_with_stm(&board, stm);
        let mut expected = vec![0i32; h1];
        for f in feats {
            for (i, &w) in weights.input_weights_1[f].iter().enumerate() {
                expected[i] += w as i32;
            }
        }

        assert_eq!(
            acc.hidden_1, expected,
            "trainer extraction and engine refresh disagree on {}",
            fen
        );
    }
}

// ---------------------------------------------------------------------------
// The load-bearing semantic property: colour symmetry.
//
// A position and its 180-degree colour-mirror are the same position seen from
// the other side. The training target is to-move-POV (the teacher's eval_cp is
// mover-relative), so the network's output for a position and for its mirror
// must be identical. Any encoding that does not normalise for perspective
// fails this, and must learn the two colours as independent problems.
// ---------------------------------------------------------------------------

const MIRROR_FENS: [&str; 3] = [
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
    "4k4/9/4S4/9/9/9/9/9/4K4 b - 1",
];

#[test]
fn mirrored_position_evaluates_identically_under_pov() {
    let weights = det_weights(NUM_NNUE_FEATURES_TOTAL, 16, 8);
    for fen in MIRROR_FENS {
        let (board, stm, _cap) = BitboardBoard::from_fen(fen).expect("from_fen");
        let mirrored = mirror_board(&board);

        let direct = eval_pov(&board, stm, &weights);
        let flipped = eval_pov(&mirrored, stm.opposite(), &weights);

        assert_eq!(
            direct, flipped,
            "position and its colour-mirror must evaluate identically under a \
             to-move-POV target, but got {} vs {} for {}",
            direct, flipped, fen
        );
    }
}

/// Pins the defect the POV encoding exists to fix, so that if someone points the
/// engine back at the colour-absolute path the cost is visible rather than silent.
#[test]
fn colour_absolute_encoding_is_not_mirror_invariant() {
    let weights = det_weights(NUM_NNUE_FEATURES_TOTAL, 16, 8);
    let mut any_differs = false;
    for fen in MIRROR_FENS {
        let (board, stm, _cap) = BitboardBoard::from_fen(fen).expect("from_fen");
        let mirrored = mirror_board(&board);
        if eval_flat_stm(&board, stm, &weights) != eval_flat_stm(&mirrored, stm.opposite(), &weights)
        {
            any_differs = true;
        }
    }
    assert!(
        any_differs,
        "expected the colour-absolute encoding to break mirror invariance; if this now \
         passes, the legacy path has gained perspective normalisation and this test \
         (and Finding 2) should be retired"
    );
}

// ---------------------------------------------------------------------------
// Train / inference arithmetic skew.
//
// The trainer's forward pass (nnue_training.rs) accumulates in f32 with an
// exact `* w / 64.0`. The engine (nnue.rs `evaluate`) accumulates in i32 with
// `(act * w) >> 6`, which floors every individual term. The optimiser therefore
// computes gradients for a function the engine never evaluates, and the floor
// biases every accumulation downward.
// ---------------------------------------------------------------------------

/// Replicates the trainer's f32 forward pass exactly (see
/// `nnue_training.rs` `train_position`), so the two arithmetic paths can be
/// compared on identical weights and inputs.
fn trainer_forward_cp(acc: &NNUEAccumulator, weights: &NNUEWeights) -> f32 {
    let h1_act: Vec<f32> = acc
        .hidden_1
        .iter()
        .zip(weights.hidden_biases_1.iter())
        .map(|(&h, &b)| (h + b).max(0) as f32)
        .collect();

    let w2 = weights.input_weights_2.as_ref().unwrap();
    let b2 = weights.hidden_biases_2.as_ref().unwrap();
    let mut h2 = vec![0.0f32; b2.len()];
    for (i, &act) in h1_act.iter().enumerate() {
        for (j, &w) in w2[i].iter().enumerate() {
            h2[j] += act * (w as f32) / 64.0;
        }
    }
    let mut raw = weights.output_bias as f32;
    for (j, &b) in b2.iter().enumerate() {
        let a = (h2[j] + b as f32).max(0.0);
        raw += a * (weights.output_weights[j] as f32) / 64.0;
    }
    raw * 400.0 / 4080.0
}

#[test]
#[ignore = "Finding 3: the trainer's f32 forward divides exactly while the engine floors every \
            term with >> 6, so the optimiser differentiates a different function. See \
            docs/nnue-phase2/FEATURE_SPEC_AND_AUDIT.md. Remove this attribute when fixed."]
fn trainer_forward_matches_engine_evaluate() {
    let weights = det_weights(NUM_NNUE_FEATURES_TOTAL, 256, 32);
    let fens = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
        "4k4/9/4S4/9/9/9/9/9/4K4 b - 1",
    ];
    let mut worst = 0.0f32;
    for fen in fens {
        let (board, stm, _cap) = BitboardBoard::from_fen(fen).expect("from_fen");
        let (h1, h2) = weights.hidden_sizes();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.refresh_with_stm(&board, stm, &weights);

        let engine = acc.evaluate(&weights) as f32;
        let trainer = trainer_forward_cp(&acc, &weights);
        let drift = (engine - trainer).abs();
        println!("  {:<58} engine={:>9.1} trainer={:>9.1} drift={:>8.1}", fen, engine, trainer, drift);
        worst = worst.max(drift);
    }
    assert!(
        worst < 5.0,
        "trainer f32 forward and engine i32 evaluate diverge by up to {:.1} cp; \
         the optimiser is not differentiating the function the engine computes",
        worst
    );
}

#[test]
fn mirrored_hand_is_symmetric() {
    let mut hand = CapturedPieces::new();
    hand.add_piece(PieceType::Pawn, Player::Black);
    hand.add_piece(PieceType::Pawn, Player::Black);
    hand.add_piece(PieceType::Rook, Player::White);

    let mirrored = mirror_hand(&hand);
    assert_eq!(mirrored.count(PieceType::Pawn, Player::White), 2);
    assert_eq!(mirrored.count(PieceType::Rook, Player::Black), 1);
    assert_eq!(mirrored.count(PieceType::Pawn, Player::Black), 0);
}
