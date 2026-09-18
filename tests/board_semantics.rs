//! Board make/unmake must be an exact round trip.
//!
//! The search makes and unmakes moves on one board rather than copying it, so
//! any difference after `unmake_move` silently corrupts the position for every
//! sibling move searched afterwards.

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::core::Player;

/// Every legal move from positions containing promoted pieces must round-trip,
/// including captures of those promoted pieces.
#[test]
fn make_unmake_round_trips_every_legal_move() {
    let fens = [
        // White tokin (+p) on 5e capturable by Black's rook on 5h; White +B and +R
        // on the board, Black +P elsewhere.
        "4k4/9/9/9/4+p4/9/9/4R4/4K4 b - 1",
        "4k4/9/2+R6/9/4+b4/9/9/1B2R4/4K4 b - 1",
        "l6nl/3+P2gk1/2n1ps1p1/p1pp2p1p/1r3+B3/P1P1P1P1P/1P1PSP1P1/1+b2G2R1/LN1K3NL b Gg 1",
        "lnsgk2nl/1r4gb1/p1pppp1pp/6p2/1p7/2P6/PPBPPPPPP/7R1/LN+sGKGSNL w Pp 1",
    ];
    let mg = MoveGenerator::new();
    let mut failures = Vec::new();
    for fen in fens {
        let (board, stm, captured) = BitboardBoard::from_fen(fen).expect("from_fen");
        let before = board.to_fen(stm, &captured);
        for mv in mg.generate_legal_moves(&board, stm, &captured) {
            let mut b = board.clone();
            let info = b.make_move_with_info(&mv);
            b.unmake_move(&info);
            let after = b.to_fen(stm, &captured);
            if after != before {
                failures.push(format!("{} from {}\n    got {}", mv.to_usi_string(), before, after));
            }
        }
    }
    for f in &failures {
        eprintln!("  round-trip failure: {f}");
    }
    assert!(failures.is_empty(), "{} moves did not round-trip", failures.len());
}

#[test]
fn capturing_a_promoted_piece_records_it_for_the_hand_unpromoted() {
    let (mut b, _stm, _c) = BitboardBoard::from_fen("4k4/9/9/9/4+p4/9/9/4R4/4K4 b - 1").unwrap();
    let mg = MoveGenerator::new();
    let captured_empty = shogi_engine::types::board::CapturedPieces::new();
    let mv = mg
        .generate_legal_moves(&b, Player::Black, &captured_empty)
        .into_iter()
        .find(|m| m.to_usi_string().starts_with("5h5e"))
        .expect("5h5e capture of the tokin");
    let info = b.make_move_with_info(&mv);
    let cap = info.captured_piece.expect("a capture");
    assert_eq!(
        cap.piece_type,
        shogi_engine::types::core::PieceType::Pawn,
        "a captured tokin goes to hand as a pawn"
    );
}

/// In shogi a piece attacks exactly the squares it can move to on an otherwise
/// empty board (pawns capture straight ahead). So for every piece type, colour
/// and square, `is_square_attacked_by` must agree with the move generator's
/// destinations for that piece. Squares where the piece could not legally stand
/// (pawn/lance on the last rank, knight on the last two) are skipped.
#[test]
fn attacks_match_move_destinations_for_every_piece() {
    use shogi_engine::types::board::CapturedPieces;
    use shogi_engine::types::core::{Piece, PieceType, Position};
    use std::collections::{BTreeMap, BTreeSet};
    let types = [
        PieceType::Pawn, PieceType::Lance, PieceType::Knight, PieceType::Silver, PieceType::Gold,
        PieceType::Bishop, PieceType::Rook, PieceType::King, PieceType::PromotedPawn,
        PieceType::PromotedLance, PieceType::PromotedKnight, PieceType::PromotedSilver,
        PieceType::PromotedBishop, PieceType::PromotedRook,
    ];
    let mg = MoveGenerator::new();
    let captured = CapturedPieces::new();
    let mut bad: BTreeMap<String, usize> = BTreeMap::new();
    let mut example: BTreeMap<String, String> = BTreeMap::new();
    for &player in &[Player::Black, Player::White] {
        for &pt in &types {
            for sq in 0..81u8 {
                let pos = Position::from_u8(sq);
                // rows advance toward 0 for Black (7g7f is row 6 -> 5)
                let ranks_from_far_edge = if player == Player::Black { pos.row } else { 8 - pos.row };
                let illegal_home = match pt {
                    PieceType::Pawn | PieceType::Lance => ranks_from_far_edge == 0,
                    PieceType::Knight => ranks_from_far_edge <= 1,
                    _ => false,
                };
                if illegal_home {
                    continue;
                }
                let mut b = BitboardBoard::empty();
                b.place_piece(Piece::new(pt, player), pos);
                let moves: BTreeSet<u8> = mg
                    .generate_legal_moves(&b, player, &captured)
                    .iter()
                    .filter(|m| m.from == Some(pos))
                    .map(|m| m.to.to_u8())
                    .collect();
                let attacks: BTreeSet<u8> =
                    (0..81u8).filter(|&t| t != sq && b.is_square_attacked_by(Position::from_u8(t), player)).collect();
                if moves != attacks {
                    let key = format!("{:?} {:?}", player, pt);
                    *bad.entry(key.clone()).or_default() += 1;
                    example.entry(key).or_insert_with(|| {
                        format!("sq {} moves {:?} attacks {:?}", sq, moves, attacks)
                    });
                }
            }
        }
    }
    for (k, n) in &bad {
        eprintln!("  {k:<24} {n:>2} squares disagree, e.g. {}", example[k]);
    }
    assert!(bad.is_empty(), "attack detection disagrees with move generation for {} piece/colour pairs", bad.len());
}

#[test]
#[ignore = "diagnostic"]
fn diagnostic_knight_and_dragon_moves() {
    use shogi_engine::types::board::CapturedPieces;
    let mg = MoveGenerator::new();
    let c = CapturedPieces::new();
    // Black knight on 5e (row 4) should go to 4c/6c (row 2).
    let (b, _, _) = BitboardBoard::from_fen("4k4/9/9/9/4N4/9/9/9/4K4 b - 1").unwrap();
    let mut v: Vec<String> = mg.generate_legal_moves(&b, Player::Black, &c).iter().map(|m| m.to_usi_string()).filter(|s| s.starts_with("5e")).collect();
    v.sort(); eprintln!("Black knight 5e: {v:?}   (expect 5e4c, 5e6c)");
    // Black dragon (+R) on 5e with kings on 5a/5i.
    let (b, _, _) = BitboardBoard::from_fen("k8/9/9/9/4+R4/9/9/9/8K b - 1").unwrap();
    let mut v: Vec<String> = mg.generate_legal_moves(&b, Player::Black, &c).iter().map(|m| m.to_usi_string()).filter(|s| s.starts_with("5e")).collect();
    v.sort(); eprintln!("Black dragon 5e: {} moves {v:?}   (expect 20: 16 slides + 4 diagonal steps)", v.len());
    let (b, _, _) = BitboardBoard::from_fen("k8/9/9/9/4+B4/9/9/9/8K b - 1").unwrap();
    let v: Vec<String> = mg.generate_legal_moves(&b, Player::Black, &c).iter().map(|m| m.to_usi_string()).filter(|s| s.starts_with("5e")).collect();
    eprintln!("Black horse 5e: {} moves   (expect 20: 16 slides + 4 orthogonal steps)", v.len());
}

fn perft(
    mg: &MoveGenerator,
    b: &mut BitboardBoard,
    player: Player,
    captured: &mut shogi_engine::types::board::CapturedPieces,
    depth: u32,
) -> u64 {
    let moves = mg.generate_legal_moves(b, player, captured);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut total = 0;
    for mv in moves {
        let info = b.make_move_with_info(&mv);
        if let Some(cap) = info.captured_piece {
            captured.add_piece(cap.piece_type, player);
        } else if mv.from.is_none() {
            captured.remove_piece(mv.piece_type, player);
        }
        total += perft(mg, b, player.opposite(), captured, depth - 1);
        b.unmake_move(&info);
        if let Some(cap) = info.captured_piece {
            captured.remove_piece(cap.piece_type, player);
        } else if mv.from.is_none() {
            captured.add_piece(mv.piece_type, player);
        }
    }
    total
}

/// Perft from the start position against the published shogi values. The
/// definitive check of move generation: every legal move sequence counted.
#[test]
fn perft_from_start_position() {
    let expected = [30u64, 900, 25_470];
    let mg = MoveGenerator::new();
    let (mut b, stm, mut c) =
        BitboardBoard::from_fen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1").unwrap();
    let mut got = Vec::new();
    for (i, &want) in expected.iter().enumerate() {
        got.push((i + 1, perft(&mg, &mut b, stm, &mut c, i as u32 + 1), want));
    }
    for (d, g, w) in &got {
        eprintln!("  perft({d}) = {g:>8}   expected {w:>8}{}", if g == w { "" } else { "   <-- WRONG" });
    }
    assert!(got.iter().all(|(_, g, w)| g == w), "perft mismatch");
}

/// Deeper perft: depth 4 has no drops yet; depth 5 is the first with drops
/// (double-pawn and drop-pawn-mate rules). Slow, so opt-in.
#[test]
#[ignore = "slow; run with --ignored"]
fn perft_from_start_position_deep() {
    let mg = MoveGenerator::new();
    let (mut b, stm, mut c) =
        BitboardBoard::from_fen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1").unwrap();
    for (depth, want) in [(4u32, 719_731u64), (5, 19_861_490)] {
        let t = std::time::Instant::now();
        let got = perft(&mg, &mut b, stm, &mut c, depth);
        eprintln!("  perft({depth}) = {got:>10}   expected {want:>10}   ({:.1?}){}", t.elapsed(), if got == want { "" } else { "   <-- WRONG" });
        assert_eq!(got, want, "perft({depth})");
    }
}

fn total_pieces(b: &BitboardBoard, c: &shogi_engine::types::board::CapturedPieces) -> usize {
    use shogi_engine::types::core::Position;
    let on_board = (0..81u8).filter(|&s| b.get_piece(Position::from_u8(s)).is_some()).count();
    on_board + c.black.len() + c.white.len()
}

/// A drop must take the piece out of the mover's hand.
#[test]
fn drop_removes_the_piece_from_hand() {
    let (mut b, stm, mut c) = BitboardBoard::from_fen("4k4/9/9/9/9/9/9/9/4K4 b P 1").unwrap();
    let mg = MoveGenerator::new();
    let drop = mg
        .generate_legal_moves(&b, stm, &c)
        .into_iter()
        .find(|m| m.from.is_none())
        .expect("a pawn drop");
    b.make_move_with_hand(&drop, &mut c);
    assert_eq!(c.count(shogi_engine::types::core::PieceType::Pawn, Player::Black), 0);
    assert!(
        MoveGenerator::new().generate_legal_moves(&b, Player::Black, &c).iter().all(|m| m.from.is_some()),
        "no drops once the hand is empty"
    );
}

/// Random legal games must conserve material: 40 pieces between board and
/// hands at every ply, with both kings on the board. Before make_move_with_hand
/// the game loops never removed dropped pieces, so hands only grew.
#[test]
fn random_games_conserve_pieces() {
    use shogi_engine::types::core::{PieceType, Position};
    let mg = MoveGenerator::new();
    let mut state: u64 = 0x1234_5678_9ABC_DEF1;
    let mut drops = 0;
    for game in 0..40 {
        let (mut b, mut stm, mut c) =
            BitboardBoard::from_fen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1").unwrap();
        for ply in 0..160 {
            let legal = mg.generate_legal_moves(&b, stm, &c);
            if legal.is_empty() {
                break;
            }
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let mv = &legal[(state >> 33) as usize % legal.len()];
            if mv.from.is_none() {
                drops += 1;
            }
            b.make_move_with_hand(mv, &mut c);
            stm = stm.opposite();
            let kings = (0..81u8)
                .filter(|&s| matches!(b.get_piece(Position::from_u8(s)), Some(p) if p.piece_type == PieceType::King))
                .count();
            assert_eq!(total_pieces(&b, &c), 40, "game {game} ply {ply}: {}", b.to_fen(stm, &c));
            assert_eq!(kings, 2, "game {game} ply {ply}: {}", b.to_fen(stm, &c));
        }
    }
    assert!(drops > 50, "random games should include plenty of drops, saw {drops}");
}

fn legal_usi(sfen: &str) -> Vec<String> {
    let (b, stm, c) = BitboardBoard::from_fen(sfen).expect("from_fen");
    MoveGenerator::new().generate_legal_moves(&b, stm, &c).iter().map(|m| m.to_usi_string()).collect()
}

/// Dropping a pawn to give checkmate is illegal (uchifuzume), but a checking
/// pawn drop that some piece can capture is not mate and must stay legal.
#[test]
fn pawn_drop_mate_is_illegal_but_capturable_pawn_check_is_legal() {
    // White king 1a. Gold 2c covers 1b and 2b, silver 3b covers 2a: P*1b is mate.
    let mate = "8k/6S2/7G1/9/9/9/9/9/4K4 b P 1";
    assert!(!legal_usi(mate).contains(&"P*1b".to_string()), "P*1b is pawn-drop mate");
    // Same, but a White gold on 2a can take the pawn: not mate, so legal.
    let not_mate = "7gk/6S2/7G1/9/9/9/9/9/4K4 b P 1";
    assert!(legal_usi(not_mate).contains(&"P*1b".to_string()), "P*1b can be captured");
}

/// Checking pawn drops YaneuraOu played in the training corpus, which the old
/// pawn-drop-mate test (king steps only) rejected.
#[test]
fn checking_pawn_drops_from_teacher_games_are_legal() {
    let cases = [
        ("l1+Pp2S1l/4g1kS1/p1p1p2gp/4bp3/P5p1P/2n3PK1/3PPP3/3+r1NSbr/+s4G1N+n w PPGPpll 1", "P*2e"),
        ("l8/8l/p6+Rp/1SpSp1RK1/3s1p2L/P2n1B3/NkS1g1+pPg/3G5/L3G4 b PPPPNPPBPpnpp 1", "P*8h"),
        ("8l/2p4k1/2+PSBp1N1/4p1+rpp/2Gp5/1+R1sP2l1/PB1n1P1PP/1p1G5/Ks1P3NL w GGPPPLpsn 1", "P*9h"),
        ("3p2Snl/2s1+R1s1k/1+B5lp/1p1PlpBp1/K1P1g1G2/P1n2P1P1/+rP1GG3P/9/9 w LNSPNppppp 1", "P*9d"),
    ];
    for (sfen, mv) in cases {
        assert!(legal_usi(sfen).contains(&mv.to_string()), "{mv} should be legal in {sfen}");
    }
}
