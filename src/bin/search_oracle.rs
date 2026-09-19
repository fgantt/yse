//! Reference search: plain alpha-beta with a captures-only quiescence, the
//! engine's evaluation and move generator, and nothing else (no transposition
//! table, pruning, reductions, extensions or aspiration windows). Compared with
//! the engine's search at the same depth it shows whether the engine's search
//! machinery changes results: the two should agree closely, and the engine's
//! moves should not get worse with depth (they did: YaneuraOu-scored move loss
//! 258 cp at depth 1, 326 cp at depth 4).

use clap::Parser;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::search::IterativeDeepening;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{Move, Player};
use std::io::{BufRead, BufReader};

#[derive(Parser)]
struct Cli {
    /// JSONL with an `sfen` field per line.
    #[arg(long)]
    positions: String,
    #[arg(long, default_value_t = 30)]
    count: usize,
    #[arg(long, default_value_t = 2)]
    depth: u8,
    #[arg(long, default_value = "nnue/default.bin")]
    nnue: String,
}

const MATE: i32 = 100_000;

struct Oracle {
    ev: PositionEvaluator,
    mg: MoveGenerator,
    nodes: u64,
}

impl Oracle {
    fn qsearch(&mut self, b: &BitboardBoard, c: &CapturedPieces, p: Player, mut alpha: i32, beta: i32, qd: u8) -> i32 {
        self.nodes += 1;
        let stand = self.ev.evaluate(b, p, c);
        if stand >= beta || qd == 0 {
            return stand;
        }
        alpha = alpha.max(stand);
        let moves = self.mg.generate_legal_moves(b, p, c);
        for m in moves.iter().filter(|m| m.is_capture) {
            let (nb, nc) = apply(b, c, m);
            let s = -self.qsearch(&nb, &nc, p.opposite(), -beta, -alpha, qd - 1);
            if s >= beta {
                return s;
            }
            alpha = alpha.max(s);
        }
        alpha
    }

    fn search(&mut self, b: &BitboardBoard, c: &CapturedPieces, p: Player, d: u8, mut alpha: i32, beta: i32) -> i32 {
        if d == 0 {
            return self.qsearch(b, c, p, alpha, beta, 6);
        }
        self.nodes += 1;
        let moves = self.mg.generate_legal_moves(b, p, c);
        if moves.is_empty() {
            return -MATE;
        }
        let mut best = -MATE - 1;
        for m in ordered(&moves) {
            let (nb, nc) = apply(b, c, &m);
            let s = -self.search(&nb, &nc, p.opposite(), d - 1, -beta, -alpha);
            best = best.max(s);
            alpha = alpha.max(s);
            if alpha >= beta {
                break;
            }
        }
        best
    }

    fn root(&mut self, b: &BitboardBoard, c: &CapturedPieces, p: Player, d: u8) -> (Move, i32) {
        let moves = self.mg.generate_legal_moves(b, p, c);
        let mut best = (moves[0].clone(), -MATE - 1);
        let mut alpha = -MATE - 1;
        for m in ordered(&moves) {
            let (nb, nc) = apply(b, c, &m);
            let s = -self.search(&nb, &nc, p.opposite(), d - 1, -MATE - 1, -alpha);
            if s > best.1 {
                best = (m.clone(), s);
                alpha = s;
            }
        }
        best
    }
}

/// Captures first (most valuable victim), then the rest: only affects speed.
fn ordered(moves: &[Move]) -> Vec<Move> {
    let mut v = moves.to_vec();
    v.sort_by_key(|m| -(m.captured_piece.map_or(0, |x| x.piece_type.as_index() as i32 + 1)));
    v
}

fn apply(b: &BitboardBoard, c: &CapturedPieces, m: &Move) -> (BitboardBoard, CapturedPieces) {
    let mut nb = b.clone();
    let mut nc = c.clone();
    nb.make_move_with_hand(m, &mut nc);
    (nb, nc)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut ev = PositionEvaluator::new();
    ev.enable_nnue_with_weights(&cli.nnue)?;
    let mut oracle = Oracle { ev, mg: MoveGenerator::new(), nodes: 0 };
    let (mut agree, mut n, mut diffs) = (0, 0, Vec::new());
    for line in BufReader::new(std::fs::File::open(&cli.positions)?).lines().take(cli.count) {
        let line = line?;
        let i = line.find("\"sfen\":\"").ok_or("no sfen")? + 8;
        let sfen = &line[i..i + line[i..].find('"').unwrap()];
        let (b, p, c) = BitboardBoard::from_fen(sfen)?;
        let (om, os) = oracle.root(&b, &c, p, cli.depth);
        let mut engine = SearchEngine::new(None, 16);
        engine.get_evaluator_mut().enable_nnue_with_weights(&cli.nnue)?;
        let mut id = IterativeDeepening::new(cli.depth, 600_000, None);
        let (em, es) = id.search(&mut engine, &b, &c, p).ok_or("no move")?;
        n += 1;
        if em.to_usi_string() == om.to_usi_string() {
            agree += 1;
        }
        diffs.push((es - os).abs());
        println!("PICK {} {} {}", sfen, om.to_usi_string(), em.to_usi_string());
        println!(
            "{:>3} oracle {:<7} {:>7} | engine {:<7} {:>7} | {}",
            n,
            om.to_usi_string(),
            os,
            em.to_usi_string(),
            es,
            if em.to_usi_string() == om.to_usi_string() { "" } else { "DIFFERENT" }
        );
    }
    diffs.sort();
    println!(
        "depth {}: same move {}/{}; |score diff| median {} max {}",
        cli.depth,
        agree,
        n,
        diffs[diffs.len() / 2],
        diffs.last().unwrap()
    );
    Ok(())
}
