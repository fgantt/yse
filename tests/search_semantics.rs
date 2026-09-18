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
#[ignore = "Defect: evaluate() routes through IntegratedEvaluator, which is not colour-symmetric \
            (15/16 positions differ from their mirror by 10-521 cp) and runs ~10x the scale of \
            the tapered evaluator. See docs/nnue-phase2/SESSION_LOG_022.md."]
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
#[ignore = "Defect: pruning uses the asymmetric IntegratedEvaluator static eval, so search scores \
            differ from the colour-mirror. See docs/nnue-phase2/SESSION_LOG_022.md."]
fn search_is_colour_symmetric_depth_1() {
    search_symmetry(1);
}

#[test]
#[ignore = "Defect: pruning uses the asymmetric IntegratedEvaluator static eval, so search scores \
            differ from the colour-mirror. See docs/nnue-phase2/SESSION_LOG_022.md."]
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

/// The search must score positions with a single evaluation function. Today it
/// does not: `evaluate()` (IntegratedEvaluator, or NNUE when enabled) drives the
/// static eval used for pruning, while quiescence scores every leaf with the
/// tapered evaluator in `evaluate_with_context()` — which never consults NNUE.
/// Checked with NNUE off and on.
#[test]
#[ignore = "Defect: two evaluators in one search; NNUE never reaches leaf evaluation. \
            See docs/nnue-phase2/SESSION_LOG_022.md."]
fn search_uses_one_evaluation_function() {
    let weights = std::path::Path::new("weights/session21/nnue_weights_s21_pov_final.bin");
    let mut configs: Vec<(&str, PositionEvaluator)> = vec![("classical", PositionEvaluator::new())];
    if weights.exists() {
        let mut ev = PositionEvaluator::new();
        ev.enable_nnue_with_weights(weights).expect("load weights");
        ev.nnue_set_use_pov(true);
        ev.nnue_set_use_hand_features(true);
        configs.push(("nnue-pov", ev));
    }
    let mut failures = Vec::new();
    for (name, ev) in configs.iter_mut() {
        for (i, sfen) in POSITIONS.iter().enumerate() {
            let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
            let static_eval = ev.evaluate(&b, stm, &c);
            let leaf_eval = ev.evaluate_with_context(&b, stm, &c, 1, false, false, false, true);
            if static_eval != leaf_eval {
                failures.push(format!("{name} #{i}: evaluate {static_eval} vs leaf {leaf_eval}"));
            }
        }
    }
    for f in &failures {
        eprintln!("  {f}");
    }
    assert!(failures.is_empty(), "{} evaluator disagreements", failures.len());
}
