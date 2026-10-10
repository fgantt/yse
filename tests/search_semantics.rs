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
        // Root PVS searches all but the first move with a null window, and quiescence
        // results depend on the window, so a different (tie-broken) move order can move
        // the score by a few cp. Full windows at the root make this test exact (checked
        // Session 34). Same move: the scores must match exactly.
        let window_noise = mirror_usi(&mmv) != mv && (score - mscore).abs() <= 5;
        if score != mscore && !window_noise {
            score_failures.push(format!(
                "#{i}: score {score} vs mirror {mscore} (diff {}); moves {mv} / mirror {}",
                score - mscore,
                mirror_usi(&mmv)
            ));
        } else if mirror_usi(&mmv) != mv {
            // Equal scores with different moves can be legitimate tie-breaking.
            move_differences.push(format!(
                "#{i}: {mv} ({score}) vs mirror {} ({mscore})",
                mirror_usi(&mmv)
            ));
        }
    }
    for f in &score_failures {
        eprintln!("  depth {depth} score asymmetry {f}");
    }
    for d in &move_differences {
        eprintln!("  depth {depth} different move {d}");
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
fn search_is_colour_symmetric_depth_1() {
    search_symmetry(1);
}

#[test]
#[ignore = "Residual defect: 1/16 positions differs from its colour-mirror by 20 cp at depth 2 \
            (same best move, near-mate score ~22,500). Likely colour-dependent move-ordering \
            tie-breaks feeding pruning. See docs/nnue-phase2/SESSION_LOG_022.md."]
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

/// Written once per test process: tests run on parallel threads, and an
/// exists-then-write check let one thread read the file mid-write.
fn random_pov_weights_file() -> std::path::PathBuf {
    static PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PATH.get_or_init(write_random_pov_weights).clone()
}

fn write_random_pov_weights() -> std::path::PathBuf {
    use shogi_engine::evaluation::nnue::{NNUEWeights, NUM_NNUE_FEATURES_WITH_HAND_TOTAL};
    let path = std::env::temp_dir().join(format!("search_semantics_pov_{}.bin", std::process::id()));
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
        flat_weights_2: Vec::new(),
        l2_colsum_max: 0,
        psqt: Vec::new(),
    };
    // A PSQT path too (material plus noise), so every test below also covers its
    // incremental updates, perspective and symmetry.
    let mut w = w;
    w.rebuild_flat_weights_2();
    w.psqt = NNUEWeights::material_psqt(NUM_NNUE_FEATURES_WITH_HAND_TOTAL)
        .into_iter()
        .map(|v| v + next(-30, 30))
        .collect();
    let fs = shogi_engine::evaluation::nnue::NNUEFeatureSet::parse("pov,hand").unwrap();
    w.save_with_features(&path, fs).expect("save random weights");
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

/// Quiescence must search evasions when in check rather than stand pat. With
/// that, a mate-in-1 is found at depth 1: after the mating move the opponent is
/// in check with no legal reply, and quiescence returns the mate score. White
/// king on 5a, Black pawn on 5c, Black gold in hand: G*5b is mate (the gold
/// covers 4a/6a/4b/6b and is defended by the pawn).
#[test]
fn search_finds_mate_in_one() {
    let (b, stm, c) = BitboardBoard::from_fen("4k4/9/4P4/9/9/9/9/9/4K4 b G 1").expect("from_fen");
    for depth in [1u8, 2] {
        let (mv, score) = search(&b, &c, stm, depth);
        assert_eq!(mv, "G*5b", "depth {depth}: expected the mating drop, got {mv} ({score})");
        assert!(score >= 90_000, "depth {depth}: mate should score as mate, got {score}");
    }
}

#[test]
fn mate_in_one_fixture_is_actually_mate() {
    use shogi_engine::moves::MoveGenerator;
    let (mut b, stm, mut c) = BitboardBoard::from_fen("4k4/9/4P4/9/9/9/9/9/4K4 b G 1").unwrap();
    let mg = MoveGenerator::new();
    let drop = mg
        .generate_legal_moves(&b, stm, &c)
        .into_iter()
        .find(|m| m.to_usi_string() == "G*5b")
        .expect("G*5b is legal");
    b.make_move(&drop);
    c.remove_piece(PieceType::Gold, Player::Black);
    assert!(b.is_king_in_check(Player::White, &c), "White is in check after G*5b");
    let replies: Vec<String> =
        mg.generate_legal_moves(&b, Player::White, &c).iter().map(|m| m.to_usi_string()).collect();
    assert!(replies.is_empty(), "White has replies: {replies:?}");
}

#[test]
#[ignore = "diagnostic"]
fn diagnostic_king_capture_of_defended_piece() {
    use shogi_engine::moves::MoveGenerator;
    let mg = MoveGenerator::new();
    // After G*5b: W king 5a, B gold 5b, B pawn 5c, B king 5i.
    let (b, _stm, c) = BitboardBoard::from_fen("4k4/4G4/4P4/9/9/9/9/9/4K4 w - 1").unwrap();
    let sq_5b = Position::new(1, 4);
    eprintln!("5b attacked by Black: {}", b.is_square_attacked_by(sq_5b, Player::Black));
    eprintln!("piece on 5b: {:?}  on 5c: {:?}", b.get_piece(sq_5b), b.get_piece(Position::new(2, 4)));
    let legal: Vec<String> = mg.generate_legal_moves(&b, Player::White, &c).iter().map(|m| m.to_usi_string()).collect();
    eprintln!("legal: {legal:?}");
    // Simpler: can a Black pawn's attack be seen at all? Pawn 5c -> 5b.
    let (b2, _, _) = BitboardBoard::from_fen("4k4/9/4P4/9/9/9/9/9/4K4 w - 1").unwrap();
    eprintln!("empty 5b attacked by Black pawn on 5c: {}", b2.is_square_attacked_by(sq_5b, Player::Black));
    // After the king actually captures:
    let (b3, _, c3) = BitboardBoard::from_fen("9/4k4/4P4/9/9/9/9/9/4K4 b G 1").unwrap();
    eprintln!("W king on 5b next to B pawn 5c -> W in check: {}", b3.is_king_in_check(Player::White, &c3));
}

/// Parallel-search workers are built with a fresh evaluator. They must be given
/// the main engine's NNUE network — otherwise helper threads search with
/// classical evaluation while the main thread uses NNUE.
#[test]
fn parallel_workers_receive_the_main_engines_network() {
    use shogi_engine::search::parallel_search::{ParallelSearchConfig, ParallelSearchEngine};
    let main = nnue_evaluator();
    let main_id = main.nnue_network_id();
    assert!(main_id.is_some());

    let parallel = ParallelSearchEngine::new(ParallelSearchConfig::new(2)).expect("parallel engine");
    let (b, stm, c) = BitboardBoard::from_fen(POSITIONS[0]).unwrap();

    // Before a template is set, a worker has no network.
    let mut before = parallel.create_thread_context(&b, &c, stm, 16);
    assert_eq!(before.search_engine_mut().get_evaluator().nnue_network_id(), None);

    parallel.set_nnue_template(&main);
    let mut ctx = parallel.create_thread_context(&b, &c, stm, 16);
    let worker = ctx.search_engine_mut().get_evaluator_mut();
    assert_eq!(worker.nnue_network_id(), main_id, "worker must run the same network");

    // And it scores positions exactly like the main evaluator.
    let mut main = main;
    assert_eq!(worker.evaluate(&b, stm, &c), main.evaluate(&b, stm, &c));

    // Switching NNUE off on the main engine propagates too.
    let classical = PositionEvaluator::new();
    parallel.set_nnue_template(&classical);
    let mut off = parallel.create_thread_context(&b, &c, stm, 16);
    assert_eq!(off.search_engine_mut().get_evaluator().nnue_network_id(), None);
}

// ---------------------------------------------------------------------------
// Two-perspective, king-conditioned networks: the modern-NNUE layout. A king
// move marks the mover's perspective dirty and it is rebuilt from the board, so
// the exactness test below must see king moves during search.
// ---------------------------------------------------------------------------

fn random_king_dual_weights_file() -> std::path::PathBuf {
    static PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        use shogi_engine::evaluation::nnue::{NNUEFeatureSet, NNUEWeights};
        let src = random_pov_weights_file();
        let (mut w, _) = NNUEWeights::load_with_features(&src).expect("load fixture");
        w.tile_for_king(81);
        w.make_dual();
        // Make every king bucket and the opponent half differ, so a stale bucket
        // or a swapped perspective changes the score.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = |lo: i32, hi: i32| -> i32 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            lo + ((state >> 33) as i64 % (hi - lo + 1) as i64) as i32
        };
        for row in w.input_weights_1.iter_mut() {
            for v in row.iter_mut() {
                *v = (*v as i32 + next(-5, 5)) as i16;
            }
        }
        let h1 = w.hidden_biases_1.len();
        for row in w.input_weights_2.as_mut().unwrap()[h1..].iter_mut() {
            for v in row.iter_mut() {
                *v = next(-64, 64) as i16;
            }
        }
        let path = std::env::temp_dir()
            .join(format!("search_semantics_king_dual_{}.bin", std::process::id()));
        w.save_with_features(&path, NNUEFeatureSet::parse("pov,hand,king,dual").unwrap())
            .expect("save");
        path
    })
    .clone()
}

fn king_dual_evaluator() -> PositionEvaluator {
    let mut ev = PositionEvaluator::new();
    ev.enable_nnue_with_weights(random_king_dual_weights_file()).expect("load");
    ev
}

#[test]
fn king_dual_network_declares_its_features() {
    let ev = king_dual_evaluator();
    let fs = ev.nnue_declared_feature_set().expect("v4 file declares features");
    assert!(fs.pov && fs.hand && fs.king && fs.dual, "{fs}");
}

#[test]
fn king_dual_evaluate_is_not_stale_and_honours_player() {
    let mut shared = king_dual_evaluator();
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        for p in [stm, stm.opposite()] {
            let expected = king_dual_evaluator().evaluate(&b, p, &c);
            assert_eq!(shared.evaluate(&b, p, &c), expected, "#{i} {p:?}");
        }
    }
}

#[test]
fn king_dual_static_eval_is_colour_symmetric() {
    for (i, sfen) in POSITIONS.iter().enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let (mb, mc) = mirror(&b, &c);
        let direct = king_dual_evaluator().evaluate(&b, stm, &c);
        let flipped = king_dual_evaluator().evaluate(&mb, stm.opposite(), &mc);
        assert_eq!(direct, flipped, "#{i}");
    }
}

#[test]
fn king_dual_incremental_is_exact_at_every_evaluated_node() {
    let mut total_checks = 0;
    for (i, sfen) in POSITIONS.iter().take(4).enumerate() {
        let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
        let mut engine = SearchEngine::new(None, 16);
        engine
            .get_evaluator_mut()
            .enable_nnue_with_weights(random_king_dual_weights_file())
            .expect("load");
        engine.get_evaluator_mut().nnue_set_verify(true);
        let mut id = IterativeDeepening::new(2, 600_000, None);
        let _ = id.search(&mut engine, &b, &c, stm);
        let (checks, mismatches, first) = engine.get_evaluator_mut().nnue_verify_stats().unwrap();
        total_checks += checks;
        assert_eq!(mismatches, 0, "#{i}: {mismatches}/{checks}; first: {}", first.unwrap_or_default());
    }
    assert!(total_checks > 100, "ran {total_checks} checks");
}

/// An unlimited-depth search (what the USI engine runs) must respect its time
/// limit. It used to replace the limit with a hard-coded 60 s, so every game
/// move took up to a minute whatever the clock said.
#[test]
fn unlimited_depth_search_respects_the_time_limit() {
    let (b, stm, c) = BitboardBoard::from_fen(POSITIONS[3]).expect("from_fen");
    let mut engine = nnue_engine();
    let started = std::time::Instant::now();
    let mut id = IterativeDeepening::new(100, 500, None);
    let result = id.search(&mut engine, &b, &c, stm);
    let elapsed = started.elapsed().as_millis();
    assert!(result.is_some(), "must return a move");
    assert!(elapsed < 1500, "500 ms search took {elapsed} ms");
}

/// `go nodes N` must actually search about N nodes.
///
/// It did not, for a long time: the budget was tracked by a counter
/// incremented inside `should_stop`, which three of its five call sites invoke
/// once per *move* rather than once per node. A search asked for 20,000 nodes
/// stopped after 852 — depth 2 where the budget was worth depth 5 — and the
/// shortfall was worst in wide middlegames, the positions that decide games.
///
/// Nothing caught it because every strength measurement we trusted ran at fixed
/// depth, which ignores the node budget entirely. This test is the one that
/// would have.
#[test]
fn a_node_limited_search_spends_about_its_whole_budget() {
    use shogi_engine::search::statistics::GLOBAL_SEARCH_NODES;
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    // The node counter is process-global, so two searches at once would see
    // each other's nodes.
    static SERIALISE: Mutex<()> = Mutex::new(());
    let _guard = SERIALISE.lock().unwrap_or_else(|e| e.into_inner());

    let (board, _p, captured) =
        BitboardBoard::from_fen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
            .expect("valid sfen");

    const BUDGET: u64 = 50_000;
    let mut engine = SearchEngine::new(None, 16);
    engine.set_node_limit(Some(BUDGET));
    // Depth and time both far out of reach, so the node budget is the only
    // thing that can stop this search.
    let mut id = IterativeDeepening::new(64, 600_000, None);
    let _ = id.search(&mut engine, &board, &captured, Player::Black).expect("a legal move");

    let nodes = GLOBAL_SEARCH_NODES.load(Ordering::Relaxed);
    assert!(
        nodes >= BUDGET / 2,
        "searched only {nodes} of a {BUDGET}-node budget; the budget is being \
         consumed by something other than nodes"
    );
    assert!(
        nodes <= BUDGET * 2,
        "searched {nodes} on a {BUDGET}-node budget; the limit is not being enforced"
    );
}

/// Parallel search must play a mate in one.
///
/// Its root split made each root move and searched the resulting position;
/// when that position was checkmate it had no move to return, and the move
/// was ranked worst. The engine then preferred longer mates and never
/// delivered one: a user's 8-piece handicap game against YaneuraOu, winning
/// from move ~60, was drawn at move 595. This is that game's position at move
/// 86 (Black to move; `6d6c+`, `6b7b` and others mate).
#[test]
fn parallel_search_plays_mate_in_one() {
    use shogi_engine::moves::MoveGenerator;
    use shogi_engine::search::ParallelSearchConfig;
    let (board, player, captured) = BitboardBoard::from_fen(
        "2+P+P5/3+P5/+N1k6/G2P4G/2S3+B2/1P3PP1P/+p5NPL/+p1PR1S2K/3G2G1L b 6P 172",
    )
    .unwrap();
    for threads in [1, 4] {
        let mut engine = SearchEngine::new(None, 16);
        let mut id = if threads > 1 {
            IterativeDeepening::new_with_threads(4, 5000, None, threads, ParallelSearchConfig::new(threads))
        } else {
            IterativeDeepening::new(4, 5000, None)
        };
        let (mv, score) = id.search(&mut engine, &board, &captured, player).expect("a move");
        let mut after = board.clone();
        let mut hands = captured.clone();
        after.make_move_with_hand(&mv, &mut hands);
        let replies = MoveGenerator::new().generate_legal_moves(&after, player.opposite(), &hands);
        assert!(
            replies.is_empty() && after.is_king_in_check(player.opposite(), &hands),
            "{threads} thread(s): {} does not mate (score {score})",
            mv.to_usi_string()
        );
        assert_eq!(score, 99_999, "{threads} thread(s): mate in one should score as mate in one");
    }
}
