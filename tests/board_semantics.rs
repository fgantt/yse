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
