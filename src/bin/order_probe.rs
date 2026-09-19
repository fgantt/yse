//! Diagnostic: print the move order the search would use at a position.
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::search::search_engine::SearchEngine;

fn main() {
    let sfen = std::env::args().nth(1).expect("sfen");
    let depth: u8 = std::env::args().nth(2).map_or(2, |d| d.parse().unwrap());
    let (board, player, captured) = BitboardBoard::from_fen(&sfen).expect("sfen");
    let mut engine = SearchEngine::new(None, 16);
    let _ = engine.get_evaluator_mut().enable_nnue_with_weights("nnue/default.bin");
    let legal = MoveGenerator::new().generate_legal_moves(&board, player, &captured);
    let ordered = engine.order_moves_for_negamax(&legal, &board, &captured, player, depth, -30000, 30000, None, None);
    println!("{} legal moves; first 20 in search order:", ordered.len());
    for (i, m) in ordered.iter().enumerate() {
        let comps = engine.debug_move_score_components(m, &board);
        println!("{:>3} {:<8} {}", i + 1, m.to_usi_string(), comps);
    }
    let caps: Vec<usize> = ordered.iter().enumerate().filter(|(_, m)| board.get_piece(m.to).map_or(false, |p| p.player != player)).map(|(i, _)| i + 1).collect();
    println!("captures at positions: {:?}", caps);
}
