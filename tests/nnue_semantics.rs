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
use shogi_engine::evaluation::nnue_training::{
    extract_active_features_pov_with_hand, extract_active_features_with_stm,
};
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

    let mut w = NNUEWeights {
        input_weights_1,
        hidden_biases_1,
        input_weights_2: Some(input_weights_2),
        hidden_biases_2: Some(hidden_biases_2),
        output_weights,
        output_bias: 0,
        flat_weights_2: Vec::new(),
        psqt: Vec::new(),
    };
    w.rebuild_flat_weights_2();
    w
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

/// Same parity check for the perspective-normalised path. The trainer extracts
/// via `extract_active_features_pov_with_hand`; the engine accumulates via
/// `refresh_pov` + `add_hand_contributions_pov`. If these disagree, the network
/// is trained on different inputs than the engine evaluates.
#[test]
fn trainer_pov_extraction_matches_engine_refresh() {
    let weights = det_weights(NUM_NNUE_FEATURES_WITH_HAND_TOTAL, 16, 8);
    let fens = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL w - 1",
        "4k4/9/4S4/9/9/9/9/9/4K4 b GGBB 1",
        "4k4/9/4s4/9/9/9/9/9/4K4 w 2P3r 1",
    ];
    for fen in fens {
        let (board, stm, cap) = BitboardBoard::from_fen(fen).expect("from_fen");

        let (h1, h2) = weights.hidden_sizes();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.refresh_pov(&board, stm, &weights);
        acc.add_hand_contributions_pov(&cap, stm, &weights, HAND_FEATURE_BASE_FLAT);

        let feats = extract_active_features_pov_with_hand(&board, stm, &cap);
        let mut expected = vec![0i32; h1];
        for f in feats {
            for (i, &w) in weights.input_weights_1[f].iter().enumerate() {
                expected[i] += w as i32;
            }
        }

        assert_eq!(
            acc.hidden_1, expected,
            "trainer POV extraction and engine refresh_pov disagree on {}",
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
            h2[j] += act * (w as f32);
        }
    }
    for v in h2.iter_mut() {
        *v /= 64.0;
    }
    let mut dot = 0.0f32;
    for (j, &b) in b2.iter().enumerate() {
        let a = (h2[j] + b as f32).max(0.0);
        dot += a * (weights.output_weights[j] as f32);
    }
    let raw = weights.output_bias as f32 + dot / 64.0;
    raw * 400.0 / 4080.0
}

#[test]
fn trainer_forward_matches_engine_evaluate() {
    let weights = det_weights(NUM_NNUE_FEATURES_TOTAL, 256, 32);
    let fens = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
        "4k4/9/4S4/9/9/9/9/9/4K4 b - 1",
    ];
    // Asserted as *relative* error. Some divergence is irreducible — the engine
    // rounds once per layer where the trainer stays in f32 — and these synthetic
    // weights produce outputs around 1e5 cp, two orders above anything a trained
    // network emits, so an absolute cp bound would say nothing. What must hold is
    // that the two agree to within quantisation, not that they agree exactly.
    let mut worst_rel = 0.0f32;
    for fen in fens {
        let (board, stm, _cap) = BitboardBoard::from_fen(fen).expect("from_fen");
        let (h1, h2) = weights.hidden_sizes();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.refresh_with_stm(&board, stm, &weights);

        let engine = acc.evaluate(&weights) as f32;
        let trainer = trainer_forward_cp(&acc, &weights);
        let drift = (engine - trainer).abs();
        let rel = drift / engine.abs().max(1.0);
        println!(
            "  {:<58} engine={:>9.1} trainer={:>9.1} drift={:>7.1} rel={:.2e}",
            fen, engine, trainer, drift, rel
        );
        worst_rel = worst_rel.max(rel);
    }
    assert!(
        worst_rel < 1e-3,
        "trainer f32 forward and engine i32 evaluate diverge by up to {:.2e} relative; \
         the optimiser is not differentiating the function the engine computes",
        worst_rel
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

// ---------------------------------------------------------------------------
// PSQT path. Checked against a material count computed here from the board and
// hands, not through any feature index.
// ---------------------------------------------------------------------------

/// Material from `stm`'s side, counted directly: board pieces at their own
/// (promoted) value, hand pieces at their base value, kings at zero.
fn material_balance(board: &BitboardBoard, captured: &CapturedPieces, stm: Player) -> i32 {
    use shogi_engine::evaluation::nnue::MATERIAL_VALUES;
    let sign = |p: Player| if p == stm { 1 } else { -1 };
    let mut total = 0;
    for sq in 0..81u8 {
        if let Some(piece) = board.get_piece(Position::from_u8(sq)) {
            total += sign(piece.player) * MATERIAL_VALUES[piece.piece_type.as_index()];
        }
    }
    for &player in &[Player::Black, Player::White] {
        for &pt in HAND_PIECE_TYPES.iter() {
            total += sign(player) * captured.count(pt, player) as i32 * MATERIAL_VALUES[pt.as_index()];
        }
    }
    total
}

/// A network whose layers are all zero, so its output is the PSQT path alone.
fn material_only_weights() -> NNUEWeights {
    let (h1, h2) = (8, 4);
    let mut w = NNUEWeights {
        input_weights_1: vec![vec![0; h1]; NUM_NNUE_FEATURES_WITH_HAND_TOTAL],
        hidden_biases_1: vec![0; h1],
        input_weights_2: Some(vec![vec![0; h2]; h1]),
        hidden_biases_2: Some(vec![0; h2]),
        output_weights: vec![0; h2],
        output_bias: 0,
        flat_weights_2: Vec::new(),
        psqt: NNUEWeights::material_psqt(NUM_NNUE_FEATURES_WITH_HAND_TOTAL),
    };
    w.rebuild_flat_weights_2();
    w
}

const MATERIAL_FENS: &[&str] = &[
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
    // Black has taken a bishop and a pawn; White holds a pawn.
    "lnsgkgsnl/1r7/pppp1pppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w BPp 1",
    // Promoted pieces on the board, several pieces in hand on both sides.
    "4k4/9/4+P4/9/2+B3+R2/9/9/9/4K4 b 2G3Pr2s 1",
    "4k4/9/4+P4/9/2+B3+R2/9/9/9/4K4 w 2G3Pr2s 1",
];

/// With the network zeroed, the POV evaluation must equal the material balance
/// from the side to move, including pieces in hand and promoted pieces.
#[test]
fn material_psqt_scores_material_from_the_movers_side() {
    let weights = material_only_weights();
    for fen in MATERIAL_FENS {
        let (board, stm, captured) = BitboardBoard::from_fen(fen).expect("from_fen");
        let (h1, h2) = weights.hidden_sizes();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.refresh_pov(&board, stm, &weights);
        acc.add_hand_contributions_pov(&captured, stm, &weights, HAND_FEATURE_BASE_FLAT);
        assert_eq!(acc.evaluate(&weights), material_balance(&board, &captured, stm), "{fen}");
    }
}

/// The engine evaluator (dual accumulators) must agree
/// with the material count, and a v3 file must round-trip the PSQT vector.
#[test]
fn material_psqt_survives_save_load_and_the_engine_evaluator() {
    use shogi_engine::evaluation::nnue::{NNUEEvaluator, NNUEFeatureSet};
    let weights = material_only_weights();
    let path = std::env::temp_dir().join(format!("nnue_psqt_{}.bin", std::process::id()));
    weights.save_with_features(&path, NNUEFeatureSet::parse("pov,hand").unwrap()).unwrap();
    let (loaded, fs) = NNUEWeights::load_with_features(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.psqt, weights.psqt);
    assert_eq!(fs, Some(NNUEFeatureSet::parse("pov,hand").unwrap()));

    let mut ev = NNUEEvaluator::from_weights(loaded);
    ev.apply_feature_set(fs.unwrap());
    for fen in MATERIAL_FENS {
        let (board, stm, captured) = BitboardBoard::from_fen(fen).expect("from_fen");
        assert_eq!(
            ev.evaluate_for(&board, stm, &captured),
            material_balance(&board, &captured, stm),
            "{fen}"
        );
    }
    // The legacy format cannot hold a PSQT vector, so it must refuse.
    assert!(weights.save(std::env::temp_dir().join("nnue_psqt_legacy.bin")).is_err());
}

// ---------------------------------------------------------------------------
// Architecture expansions used to warm-start training: each must leave the
// function the network computes unchanged.
// ---------------------------------------------------------------------------

#[test]
fn make_dual_and_tile_for_king_preserve_the_network() {
    use shogi_engine::evaluation::nnue::{score_pov_scratch, KING_BUCKET_SCHEMES, KING_FEATURE_STRIDE};
    for nb in KING_BUCKET_SCHEMES {
        let mut w = det_weights(NUM_NNUE_FEATURES_WITH_HAND_TOTAL, 16, 8);
        w.psqt = NNUEWeights::material_psqt(NUM_NNUE_FEATURES_WITH_HAND_TOTAL);
        let score = |w: &NNUEWeights, king: bool, fen: &str| {
            let (b, stm, c) = BitboardBoard::from_fen(fen).unwrap();
            score_pov_scratch(w, king, true, &b, stm, &c)
        };
        let before: Vec<i32> = MATERIAL_FENS.iter().map(|f| score(&w, false, f)).collect();
        w.make_dual();
        assert!(w.is_dual());
        let dual: Vec<i32> = MATERIAL_FENS.iter().map(|f| score(&w, false, f)).collect();
        assert_eq!(before, dual, "make_dual must not change scores");
        w.tile_for_king(nb);
        assert_eq!(w.input_weights_1.len(), nb * KING_FEATURE_STRIDE);
        let king: Vec<i32> = MATERIAL_FENS.iter().map(|f| score(&w, true, f)).collect();
        assert_eq!(before, king, "tile_for_king({nb}) must not change scores");
    }
}

/// The trainer's feature lists must be exactly what the engine's accumulator
/// activates, for both perspectives and with king buckets.
#[test]
fn trainer_frame_features_match_engine_refresh_with_king_buckets() {
    use shogi_engine::evaluation::nnue::{KING_BUCKET_SCHEMES, KING_FEATURE_STRIDE};
    use shogi_engine::evaluation::nnue_training::extract_features_frame;
    for nb in KING_BUCKET_SCHEMES {
        let w = det_weights(nb * KING_FEATURE_STRIDE, 8, 4);
        for fen in MATERIAL_FENS {
            let (b, _stm, c) = BitboardBoard::from_fen(fen).unwrap();
            for frame in [Player::Black, Player::White] {
                let mut acc = NNUEAccumulator::new(8, 4);
                acc.king_mode = true;
                acc.refresh_pov(&b, frame, &w);
                acc.add_hand_contributions_pov(&c, frame, &w, HAND_FEATURE_BASE_FLAT);
                let mut sum = vec![0i32; 8];
                for f in extract_features_frame(&b, frame, &c, true, nb) {
                    for (s, &x) in sum.iter_mut().zip(&w.input_weights_1[f]) {
                        *s += x as i32;
                    }
                }
                assert_eq!(acc.hidden_1, sum, "{nb} buckets, {fen} {frame:?}");
            }
        }
    }
}

/// 9-bucket scheme: every square maps into 0..9, the home rank of each wing is
/// its own bucket, and the mapping is taken from each side's own view.
#[test]
fn nine_king_buckets_cover_the_board() {
    use shogi_engine::evaluation::nnue::king_bucket;
    let mut seen = HashSet::new();
    for sq in 0..81u8 {
        let b = king_bucket(sq, 9);
        assert!(b < 9);
        seen.insert(b);
    }
    assert_eq!(seen.len(), 9);
    // Home rank (row 8): left wing, centre, right wing are distinct buckets.
    assert_ne!(king_bucket(8 * 9 + 1, 9), king_bucket(8 * 9 + 4, 9));
    assert_ne!(king_bucket(8 * 9 + 4, 9), king_bucket(8 * 9 + 7, 9));
    for sq in 0..81u8 {
        assert_eq!(king_bucket(sq, 81), sq as usize);
    }
}

/// Enabling the king factoriser splits each row into a shared mean and a
/// per-bucket difference; the network it renders must be exactly the same.
#[test]
fn king_factorizer_split_preserves_the_network() {
    use shogi_engine::evaluation::nnue::KING_FEATURE_STRIDE;
    use shogi_engine::evaluation::nnue_training::{NNUETrainer, NNUETrainingConfig};
    let mut w = det_weights(KING_FEATURE_STRIDE, 8, 4);
    w.make_dual();
    w.tile_for_king(9);
    // Make buckets differ so the mean is not just a copy.
    for (r, row) in w.input_weights_1.iter_mut().enumerate() {
        for v in row.iter_mut() {
            *v = (*v as i32 + (r / KING_FEATURE_STRIDE) as i32 - 4).clamp(-127, 127) as i16;
        }
    }
    let mut config = NNUETrainingConfig::default();
    config.use_adam = true;
    let mut trainer = NNUETrainer::new(w.clone(), config);
    trainer.enable_king_factorizer();
    let mut rendered = w.clone();
    trainer.get_shadow().quantize_into(&mut rendered);
    assert_eq!(rendered.input_weights_1, w.input_weights_1);
}

/// The ranking loss must move a move's score towards the teacher's preference.
///
/// A child position is evaluated from the *opponent's* point of view, so the
/// mover's score for move i is `-eval(child_i)`. A sign error there would train
/// the network to prefer exactly the moves the teacher rejected while the loss
/// still fell, so the direction has to be asserted directly.
///
/// The assertion is differential — the same starting weights trained once with
/// the teacher preferring A and once preferring B — rather than "the gap to A
/// widens". That weaker form does not discriminate: cross-entropy also
/// calibrates *confidence*, so a network that already prefers A far more
/// strongly than the teacher does gets pulled back towards A's rival no matter
/// which way the sign points.
#[test]
fn ranking_loss_follows_the_teachers_preference() {
    use shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES;
    use shogi_engine::evaluation::nnue_training::{
        extract_active_features, NNUETrainer, NNUETrainingConfig, RankConfig, RankingGroup,
        TrainingPosition,
    };

    // Two real children of one position, from the ranking corpus. Black to move
    // in the parent, so both children have White to move.
    const CHILD_A: &str = "lns3snl/2r1gkgb1/ppppppppp/9/8P/8R/PPPPPPPP1/1BS6/LN1GKGSNL w -";
    const CHILD_B: &str = "lns3snl/2r1gkgb1/ppppppppp/9/8P/6P2/PPPPPP1P1/1BS5R/LN1GKGSNL w -";

    let make_position = |sfen: &str, w: &NNUEWeights| -> TrainingPosition {
        let (board, _p, _cap) = BitboardBoard::from_fen(sfen).expect("valid sfen");
        let features = extract_active_features(&board);
        let (h1, h2) = w.hidden_sizes();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.refresh(&board, w);
        let eval = acc.evaluate(w);
        TrainingPosition {
            active_features: features,
            accumulator: acc,
            evaluation: eval,
            pst_evaluation: 0,
            player: Player::White,
            move_made: None,
            outcome: None,
            td_target: None,
            active_features_nstm: Vec::new(),
            accumulator_nstm: None,
        }
    };

    // Train from identical weights under one teacher preference, and report
    // `eval(B) - eval(A)`. The mover's score for a move is the negation of its
    // child's evaluation, so preferring A means driving this quantity up.
    let train_with = |scores: Vec<f32>, cfg: RankConfig| -> f32 {
        let weights = det_weights(NUM_NNUE_FEATURES, 8, 4);
        let mut config = NNUETrainingConfig::default();
        config.use_adam = true;
        config.learning_rate = 0.01;
        let mut trainer = NNUETrainer::new(weights, config);
        for _ in 0..10 {
            let group = RankingGroup {
                children: vec![
                    make_position(CHILD_A, trainer.get_weights()),
                    make_position(CHILD_B, trainer.get_weights()),
                ],
                scores: scores.clone(),
            };
            // A temperature far above the usual 100 cp on purpose: these
            // random weights score the two children ~11600 cp apart, which at
            // 100 cp saturates the softmax to p = 1.0 and leaves the network
            // unable to move either way. The sign convention under test does
            // not depend on the temperature; the softmax's responsiveness does.
            trainer.train_batch_mixed(&[], std::slice::from_ref(&group), &cfg);
        }
        let w = trainer.get_weights();
        (make_position(CHILD_B, w).evaluation - make_position(CHILD_A, w).evaluation) as f32
    };

    // Both objectives must obey the same convention. The softmax needs an
    // unsaturated temperature to respond at all (see the comment above); the
    // hinge does not, since its gradient does not pass through a softmax.
    let cases = [
        ("softmax", RankConfig { weight: 1.0, temperature: 20_000.0, ..Default::default() }),
        (
            "pairwise",
            RankConfig {
                weight: 1.0,
                temperature: 100.0,
                pairwise: true,
                margin_cap: 400.0,
            },
        ),
    ];
    for (name, cfg) in cases {
        let gap_prefer_a = train_with(vec![20_000.0, -20_000.0], cfg);
        let gap_prefer_b = train_with(vec![-20_000.0, 20_000.0], cfg);
        assert!(
            gap_prefer_a > gap_prefer_b,
            "{name}: a teacher preferring the move to A must leave the network keener on A \
             than a teacher preferring B does: prefer_a={} prefer_b={}",
            gap_prefer_a,
            gap_prefer_b
        );
    }
}
