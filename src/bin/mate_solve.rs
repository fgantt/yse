//! Solve mates with the df-pn mate search: one SFEN per line on stdin (or
//! `--sfen`), printing the mate line, its length, the nodes used and the time.

use clap::Parser;
use shogi_engine::search::mate_search::MateSearch;
use shogi_engine::BitboardBoard;
use std::io::BufRead;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "mate-solve")]
struct Cli {
    /// Position to solve; otherwise SFENs are read from stdin, one per line.
    #[arg(long)]
    sfen: Option<String>,
    /// Node budget per position.
    #[arg(long, default_value_t = 1_000_000)]
    nodes: u64,
    /// With --sfen: instead of solving, replay these USI moves and print for
    /// each whether it gives check ("C") or not ("-"), or "?" if illegal.
    #[arg(long)]
    line: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    if let (Some(sfen), Some(line)) = (&cli.sfen, &cli.line) {
        let (mut board, mut player, mut captured) = BitboardBoard::from_fen(sfen.trim()).expect("sfen");
        let mg = shogi_engine::moves::MoveGenerator::new();
        let mut marks = Vec::new();
        for m in line.split_whitespace() {
            let legal = mg.generate_legal_moves(&board, player, &captured);
            match legal.iter().find(|x| x.to_usi_string() == m) {
                Some(mv) => {
                    marks.push(if mv.gives_check { "C" } else { "-" });
                    board.make_move_with_hand(mv, &mut captured);
                    player = player.opposite();
                }
                None => {
                    marks.push("?");
                    break;
                }
            }
        }
        println!("{}", marks.join(" "));
        return;
    }
    let sfens: Vec<String> = match &cli.sfen {
        Some(s) => vec![s.clone()],
        None => std::io::stdin().lock().lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).collect(),
    };
    for sfen in sfens {
        let (board, player, captured) = match BitboardBoard::from_fen(sfen.trim()) {
            Ok(x) => x,
            Err(e) => {
                println!("bad sfen: {e}");
                continue;
            }
        };
        let mut ms = MateSearch::new(cli.nodes);
        let t = Instant::now();
        let line = ms.solve(&board, &captured, player);
        let ms_time = t.elapsed().as_secs_f64() * 1000.0;
        match line {
            Some(l) => println!(
                "mate {:>2} nodes {:>8} ms {:>8.1} line {}",
                l.len(),
                ms.nodes(),
                ms_time,
                l.iter().map(|m| m.to_usi_string()).collect::<Vec<_>>().join(" ")
            ),
            None => println!("none    nodes {:>8} ms {:>8.1}", ms.nodes(), ms_time),
        }
    }
}
