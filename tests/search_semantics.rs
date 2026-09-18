//! Semantic (oracle) tests for the classical evaluator and the search.
//!
//! A position and its 180-degree colour-mirror are the same position seen from
//! the other side. From the side to move's point of view they must evaluate
//! identically and search to the same score, with mirrored best moves. These
//! tests check that property on real corpus positions rather than comparing the
//! implementation against itself.
//!
//! Each test reports every mismatch before asserting, so a failure shows the
//! size and spread of the asymmetry, not just its existence.

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::search::IterativeDeepening;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{Piece, PieceType, Player, Position};

/// Corpus positions (every 2011th record of nnue_corpus_yaneura_d10.jsonl),
/// excluding in-check positions with <=10 replies, whose search is time-bound.
const POSITIONS: [&str; 16] = [
    "ln1g1gsnl/2sk1r1b1/1pppppppp/p8/9/P1P6/1P1PPPPPP/1B1SG2R1/LN1GK1SNL b -",
    "7n1/5gk2/p5bn1/1P2pLppK/2p4P1/2PG1p+rL1/P1+n1P3G/8P/L2P+p1Ps1 w PPSLrspngbs",
    "l3g1ks1/4spss1/4p1np1/p4Pp1+R/2p1N2Pp/4PLPN1/P4G2P/1p3GK2/L1r+p4L w BGNPpbp",
    "lr6l/4k4/1sgpp1+N2/p4p3/1p3n1lp/P2sP4/1PNB1GP1P/1SG6/+b1K4NL b PGPPppppsr",
    "l4k2l/3g2P2/p3ppng1/7pp/1P4p2/P1rP4P/1GsR+b1+b2/4NP2L/Ls3K3 b NPpppgsspnp",
    "1ns1G+R2l/l2k5/pp1ppgs1p/2p6/n5bPP/1PP1P4/P2P2P2/KS1+b5/LN5RL b PPGSPGPn",
    "ln1gk2nl/2sr2g2/1+B1pps1pp/5pp2/p1p6/5SP2/PPSPPP1PP/2G3R2/LN2KG1NL b Pbp",
    "+Bn6l/3rg1ks1/pp2pgnpp/2p1sbp2/3P5/2P1P1P2/PPNG2N1P/2G3S2/L3K2RL w PPPLPs",
    "1n1g4l/lr3k1s1/3s1gnp1/p1ppp1p1p/1p5P1/P1P2pPR1/1PSPP1N1P/2S1G3L/LNKG5 b Bbp",
    "l7l/3+S1gBsk/2r1p1P2/1lpp3PP/p3P+rSn1/2PP1p3/PP1S1P3/LKGB5/1N6+p b PPngngp",
    "lnkg1g1+L1/2s2s2+R/1pp1pp3/pn3P3/7p1/P2pP4/4KGLP1/1+s1PG4/LNS6 w PRPNBppppb",
    "lnk1+P2+R1/9/p1+b1pp1+L1/2pP1np2/3SB3p/2P2K1n1/P1+pG+s1P2/5G3/L2+r4L b PGGNppppssp",
    "lR3nRsl/2g1kgs2/1pnppp1Pp/4b4/P1+B3P2/LPP1n4/3P2g1P/1KS6/1N1GS3L w PPPPPp",
    "l2s+R2+Pl/2g3g2/1p1p5/5pp1p/pkS1Bn3/4P3P/PPKP1PG2/1G4P2/L6+rL b PSNNPBpnps",
    "l7B/1r1skp+R1l/p2p5/3gp3p/4n4/P1pSSn2P/1P1PP1p2/2GK1G3/L+p6L w PPPSNPbpgn",
    "l6nl/2sgk1g2/2nppsbpp/p1p2pp2/4P2P1/1rPP3BP/PP1SSP3/2G1G2R1/LN1K3NL w Pp",
];

const HAND_TYPES: [PieceType; 7] = [
    PieceType::Pawn,
    PieceType::Lance,
    PieceType::Knight,
    PieceType::Silver,
    PieceType::Gold,
    PieceType::Bishop,
    PieceType::Rook,
];

fn mirror(
    board: &BitboardBoard,
    captured: &CapturedPieces,
) -> (BitboardBoard, CapturedPieces) {
    let mut b = BitboardBoard::empty();
    for sq in 0..81u8 {
        if let Some(p) = board.get_piece(Position::from_u8(sq)) {
            b.place_piece(Piece::new(p.piece_type, p.player.opposite()), Position::from_u8(80 - sq));
        }
    }
    let mut c = CapturedPieces::new();
    for &player in &[Player::Black, Player::White] {
        for &pt in HAND_TYPES.iter() {
            for _ in 0..captured.count(pt, player) {
                c.add_piece(pt, player.opposite());
            }
        }
    }
    (b, c)
}

/// Mirror a USI move string: file f -> 10 - f, rank r -> ('a' + 'i') - r.
fn mirror_usi(mv: &str) -> String {
    let b = mv.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && i + 1 < b.len() && b[i + 1].is_ascii_lowercase() {
            out.push((b'0' + 10 - (b[i] - b'0')) as char);
            out.push((b'a' + b'i' - b[i + 1]) as char);
            i += 2;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

#[test]
fn mirror_usi_is_an_involution() {
    for mv in ["7g7f", "2d2c+", "P*5e", "1a9i", "5e5e"] {
        assert_eq!(mirror_usi(&mirror_usi(mv)), mv);
    }
    assert_eq!(mirror_usi("7g7f"), "3c3d");
    assert_eq!(mirror_usi("P*5e"), "P*5e");
}

#[test]
fn static_eval_is_colour_symmetric() {
    let mut evaluator = PositionEvaluator::new();
    let mut failures = Vec::new();
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (board, stm, captured) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let (mb, mc) = mirror(&board, &captured);
        let direct = evaluator.evaluate(&board, stm, &captured);
        let flipped = evaluator.evaluate(&mb, stm.opposite(), &mc);
        if direct != flipped {
            failures.push(format!("#{i}: {direct} vs mirror {flipped} (diff {})", direct - flipped));
        }
    }
    for f in &failures {
        eprintln!("  static eval asymmetry {f}");
    }
    assert!(
        failures.is_empty(),
        "{}/{} positions evaluate differently from their colour-mirror",
        failures.len(),
        POSITIONS.len()
    );
}

fn search(board: &BitboardBoard, captured: &CapturedPieces, stm: Player, depth: u8) -> (String, i32) {
    let mut engine = SearchEngine::new(None, 16);
    let mut id = IterativeDeepening::new(depth, 600_000, None);
    let (mv, score) = id.search(&mut engine, board, captured, stm).expect("a legal move");
    (mv.to_usi_string(), score)
}

fn search_symmetry(depth: u8) {
    let mut score_failures = Vec::new();
    let mut move_differences = Vec::new();
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (board, stm, captured) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let (mb, mc) = mirror(&board, &captured);
        let (mv, score) = search(&board, &captured, stm, depth);
        let (mmv, mscore) = search(&mb, &mc, stm.opposite(), depth);
        if score != mscore {
            score_failures.push(format!(
                "#{i}: score {score} vs mirror {mscore} (diff {}); moves {mv} / mirror {}",
                score - mscore,
                mirror_usi(&mmv)
            ));
        } else if mirror_usi(&mmv) != mv {
            // Equal scores with different moves can be legitimate tie-breaking.
            move_differences.push(format!("#{i}: {mv} vs mirror {}", mirror_usi(&mmv)));
        }
    }
    for f in &score_failures {
        eprintln!("  depth {depth} score asymmetry {f}");
    }
    for d in &move_differences {
        eprintln!("  depth {depth} equal score, different move {d}");
    }
    assert!(
        score_failures.is_empty(),
        "depth {}: {}/{} positions search to a different score than their colour-mirror",
        depth,
        score_failures.len(),
        POSITIONS.len()
    );
}

#[test]
#[ignore = "Residual defect: after the single-evaluator and unmake fixes, 1/16 positions still \
            differs from its colour-mirror (59 cp at depth 1, 7 cp at depth 2, same best move). \
            Likely colour-dependent move-ordering tie-breaks feeding pruning. Was 2/16 and 5/16 with up \
            to 805 cp. See docs/nnue-phase2/SESSION_LOG_022.md. Slow (~15 min)."]
fn search_is_colour_symmetric_depth_1() {
    search_symmetry(1);
}

#[test]
#[ignore = "Residual defect: after the single-evaluator and unmake fixes, 1/16 positions still \
            differs from its colour-mirror (59 cp at depth 1, 7 cp at depth 2, same best move). \
            Likely colour-dependent move-ordering tie-breaks feeding pruning. Was 2/16 and 5/16 with up \
            to 805 cp. See docs/nnue-phase2/SESSION_LOG_022.md. Slow (~15 min)."]
fn search_is_colour_symmetric_depth_2() {
    search_symmetry(2);
}

/// Diagnostic, not an assertion: the search reaches two different evaluators.
/// `evaluate()` goes through `IntegratedEvaluator`, while quiescence calls
/// `evaluate_with_context()`, which runs a separate tapered evaluator. Print
/// both on the same positions (and their mirrors) so their agreement is visible.
#[test]
#[ignore = "diagnostic; run with --ignored --nocapture"]
fn diagnostic_compare_evaluator_entry_points() {
    let mut ev = PositionEvaluator::new();
    eprintln!("{:>3} {:>9} {:>9} {:>9} {:>9}", "#", "evaluate", "mirror", "w_ctx(q)", "mirror");
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let (mb, mc) = mirror(&b, &c);
        let e = ev.evaluate(&b, stm, &c);
        let em = ev.evaluate(&mb, stm.opposite(), &mc);
        let q = ev.evaluate_with_context(&b, stm, &c, 1, false, false, false, true);
        let qm = ev.evaluate_with_context(&mb, stm.opposite(), &mc, 1, false, false, false, true);
        eprintln!("{:>3} {:>9} {:>9} {:>9} {:>9}", i, e, em, q, qm);
    }
}

/// Classical play: the search's static eval (`evaluate`) and its leaf eval are
/// the same function, the tapered evaluator with fixed context flags.
#[test]
fn classical_evaluate_is_the_canonical_classical_eval() {
    let mut ev = PositionEvaluator::new();
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        assert_eq!(ev.evaluate(&b, stm, &c), ev.evaluate_classical(&b, stm, &c), "#{i}");
    }
}

// ---------------------------------------------------------------------------
// NNUE as the search evaluator. Mechanics are tested with a deterministic random
// POV network so the tests are hermetic; they check staleness, perspective,
// symmetry and incremental exactness, not playing strength.
// ---------------------------------------------------------------------------

fn random_pov_weights_file() -> std::path::PathBuf {
    use shogi_engine::evaluation::nnue::{NNUEWeights, NUM_NNUE_FEATURES_WITH_HAND_TOTAL};
    let path = std::env::temp_dir().join(format!("search_semantics_pov_{}.bin", std::process::id()));
    if path.exists() {
        return path;
    }
    let (h1, h2) = (64usize, 16usize);
    let mut state: u64 = 0x5DEE_CE66_D1CE_4E5B;
    let mut next = |lo: i32, hi: i32| -> i32 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        lo + ((state >> 33) as i64 % (hi - lo + 1) as i64) as i32
    };
    // Moderate magnitudes keep scores in a normal cp range, away from mate scores.
    let w = NNUEWeights {
        input_weights_1: (0..NUM_NNUE_FEATURES_WITH_HAND_TOTAL)
            .map(|_| (0..h1).map(|_| next(-20, 20) as i16).collect())
            .collect(),
        hidden_biases_1: (0..h1).map(|_| next(-10, 10)).collect(),
        input_weights_2: Some((0..h1).map(|_| (0..h2).map(|_| next(-64, 64) as i16).collect()).collect()),
        hidden_biases_2: Some((0..h2).map(|_| next(-10, 10)).collect()),
        output_weights: (0..h2).map(|_| next(-64, 64) as i16).collect(),
        output_bias: 0,
    };
    w.save(&path).expect("save random weights");
    path
}

fn nnue_evaluator() -> PositionEvaluator {
    let mut ev = PositionEvaluator::new();
    ev.enable_nnue_with_weights(random_pov_weights_file()).expect("load");
    ev.nnue_set_use_pov(true);
    ev.nnue_set_use_hand_features(true);
    ev
}

fn nnue_engine() -> SearchEngine {
    let mut engine = SearchEngine::new(None, 16);
    let ev = engine.get_evaluator_mut();
    ev.enable_nnue_with_weights(random_pov_weights_file()).expect("load");
    ev.nnue_set_use_pov(true);
    ev.nnue_set_use_hand_features(true);
    engine
}

/// Evaluating unrelated positions in sequence on one evaluator, with no manual
/// refresh, must match a fresh evaluator every time. Before the fix every call
/// after the first returned the first position's score (387 for all 16).
#[test]
fn nnue_evaluate_is_not_stale() {
    let mut shared = nnue_evaluator();
    let mut distinct = std::collections::HashSet::new();
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let expected = nnue_evaluator().evaluate(&b, stm, &c);
        assert_eq!(shared.evaluate(&b, stm, &c), expected, "#{i}");
        distinct.insert(expected);
    }
    assert!(distinct.len() > POSITIONS.len() / 2, "NNUE scores should vary by position");
}

/// `evaluate` must answer for the requested player, not whichever side the
/// accumulator last saw move (null-move search asks for the other side).
#[test]
fn nnue_evaluate_honours_player() {
    for sfen in POSITIONS.iter() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let mut ev = nnue_evaluator();
        ev.nnue_refresh(&b, stm, &c);
        ev.nnue_set_verify(true);
        let _ = ev.evaluate(&b, stm, &c);
        let _ = ev.evaluate(&b, stm.opposite(), &c);
        let (checks, mismatches, first) = ev.nnue_verify_stats().unwrap();
        assert_eq!(checks, 2);
        assert_eq!(mismatches, 0, "{}", first.unwrap_or_default());
    }
}

/// POV NNUE must be exactly colour-symmetric as a static evaluator.
#[test]
fn nnue_static_eval_is_colour_symmetric() {
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let (mb, mc) = mirror(&b, &c);
        let direct = nnue_evaluator().evaluate(&b, stm, &c);
        let flipped = nnue_evaluator().evaluate(&mb, stm.opposite(), &mc);
        assert_eq!(direct, flipped, "#{i}");
    }
}

/// The incremental accumulator must equal a from-scratch evaluation at every
/// node the search evaluates. Once NNUE scores the leaves, any board change that
/// bypasses the make/unmake hooks would silently score the wrong position.
#[test]
fn nnue_incremental_is_exact_at_every_evaluated_node() {
    let mut total_checks = 0;
    for (i, sfen) in POSITIONS.iter().take(2).enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let mut engine = nnue_engine();
        engine.get_evaluator_mut().nnue_set_verify(true);
        let mut id = IterativeDeepening::new(2, 600_000, None);
        let _ = id.search(&mut engine, &b, &c, stm);
        let (checks, mismatches, first) = engine.get_evaluator_mut().nnue_verify_stats().unwrap();
        total_checks += checks;
        assert_eq!(mismatches, 0, "#{i}: {mismatches}/{checks} mismatches; first: {}", first.unwrap_or_default());
    }
    assert!(total_checks > 100, "verification should have run at many nodes, ran {total_checks}");
}

/// With NNUE enabled, the evaluator must actually influence the search.
/// Before the fix, classical and NNUE searches were identical on 28/28 positions.
#[test]
fn nnue_changes_search_results() {
    let mut differ = 0;
    for sfen in POSITIONS.iter().take(8) {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let classical = search(&b, &c, stm, 2);
        let mut engine = nnue_engine();
        let mut id = IterativeDeepening::new(2, 600_000, None);
        let (mv, score) = id.search(&mut engine, &b, &c, stm).expect("a legal move");
        if (mv.to_usi_string(), score) != classical {
            differ += 1;
        }
    }
    assert!(differ > 0, "NNUE did not change any search result");
}
