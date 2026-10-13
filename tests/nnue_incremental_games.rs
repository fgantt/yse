//! The shipped network's incremental accumulator against a full refresh, over
//! real handicap games: drops, captures into hand, promotions and king moves.
//!
//! At every position of each game, every legal move is made and unmade the way
//! the search does it (`make_move_with_info`, then the evaluator's
//! `nnue_make_move`); after the move the incremental evaluation must equal a
//! freshly refreshed evaluator's, and after the unmake it must equal the
//! evaluation before the move. Then the game move is played and the walk goes on.
//!
//! The unit tests in `nnue.rs` check one move from the start position; the
//! search makes millions of other kinds.

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::{CapturedPieces, Move, Piece, Player};

const NET: &str = "nnue/default.bin";
const START: &str = "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1";

fn game_moves(path: &str) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap();
    let line = text.lines().find(|l| !l.starts_with('#') && !l.trim().is_empty()).unwrap();
    line.split_whitespace().map(str::to_string).collect()
}

fn evaluator() -> PositionEvaluator {
    let mut e = PositionEvaluator::new();
    e.enable_nnue_with_weights(NET).expect("network loads");
    e
}

/// A second evaluator rebuilt from scratch for each position (refresh, no history).
fn fresh_eval(
    fresh: &mut PositionEvaluator,
    board: &BitboardBoard,
    player: Player,
    captured: &CapturedPieces,
) -> i32 {
    fresh.nnue_refresh(board, player, captured);
    fresh.evaluate(board, player, captured)
}

/// Make `mv` on the board and the incremental accumulator; returns the hands after it.
fn make(
    e: &mut PositionEvaluator,
    board: &mut BitboardBoard,
    captured: &CapturedPieces,
    player: Player,
    mv: &Move,
) -> (shogi_engine::bitboards::MoveInfo, CapturedPieces) {
    let info = board.make_move_with_info(mv);
    let original = Piece::new(info.original_piece_type, info.player);
    let moved = if info.was_promotion {
        info.original_piece_type
            .promoted_version()
            .map_or(original, |p| Piece::new(p, info.player))
    } else {
        original
    };
    e.nnue_make_move(info.from, info.to, moved, original, info.captured_on_board, info.was_promotion);
    let mut after = captured.clone();
    if let Some(ref c) = info.captured_piece {
        after.add_piece(c.piece_type, player);
    } else if mv.from.is_none() {
        after.remove_piece(mv.piece_type, player);
    }
    (info, after)
}

fn check_game(path: &str) -> (usize, usize) {
    if !std::path::Path::new(NET).exists() || !std::path::Path::new(path).exists() {
        eprintln!("skipping {path}: network or game file missing");
        return (0, 0);
    }
    let (mut board, mut player, mut captured) = BitboardBoard::from_fen(START).unwrap();
    let mut e = evaluator();
    let mut fresh = evaluator();
    e.nnue_refresh(&board, player, &captured);
    let gen = MoveGenerator::new();
    let (mut positions, mut moves_checked) = (0, 0);
    for (ply, usi) in game_moves(path).iter().enumerate() {
        let before = e.evaluate(&board, player, &captured);
        assert_eq!(before, fresh_eval(&mut fresh, &board, player, &captured), "{path} ply {ply}: walk drifted");
        for mv in gen.generate_legal_moves(&board, player, &captured) {
            let (info, after) = make(&mut e, &mut board, &captured, player, &mv);
            let inc = e.evaluate(&board, player.opposite(), &after);
            let full = fresh_eval(&mut fresh, &board, player.opposite(), &after);
            assert_eq!(
                inc,
                full,
                "{path} ply {ply}: {} incremental {inc} != refresh {full}",
                mv.to_usi_string()
            );
            e.nnue_unmake_move();
            board.unmake_move(&info);
            assert_eq!(
                e.evaluate(&board, player, &captured),
                before,
                "{path} ply {ply}: {} did not unmake",
                mv.to_usi_string()
            );
            moves_checked += 1;
        }
        let mv = Move::from_usi_string(usi, player, &board).unwrap();
        let (_, after) = make(&mut e, &mut board, &captured, player, &mv);
        captured = after;
        player = player.opposite();
        positions += 1;
    }
    (positions, moves_checked)
}

#[test]
fn incremental_matches_refresh_over_handicap_losses() {
    let mut total = (0, 0);
    for g in [
        "matches/user_games/2026-10-10_4mai_loss76.txt",
        "matches/user_games/2026-10-11_4mai_loss70.txt",
        "matches/user_games/2026-10-12_4mai_loss84.txt",
    ] {
        let (p, m) = check_game(g);
        total.0 += p;
        total.1 += m;
    }
    eprintln!("checked {} positions, {} moves", total.0, total.1);
}
