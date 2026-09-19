//! Engine-vs-engine match over USI, for measuring playing strength at a real
//! time control (or a node/depth handicap) between two engine binaries.
//!
//! `elo-tester` plays two evaluators inside one process with one search, so it
//! cannot compare two versions of the search, and it plays at fixed depth, so it
//! says nothing about time management. This tool runs two external engines.
//!
//! * Openings: `--random-plies` random legal moves, each opening played twice
//!   with colours swapped (the pair is the independent unit for the CI).
//! * Legality and game end are adjudicated with our move generator, which
//!   agrees with YaneuraOu on every move of the training corpus. An illegal
//!   move loses. `resign` loses; `win` (entering-king declaration) wins.
//! * Four-fold repetition is a draw (perpetual-check losses are not modelled),
//!   as is reaching `--max-moves`.
//! * Reports each engine's longest think time, so a clock overrun is visible.

use clap::Parser;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::search::shogi_hash::ShogiHashHandler;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;
use shogi_engine::usi_client::UsiEngine;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "usi-match", about = "Paired-opening match between two USI engines")]
struct Cli {
    #[arg(long)]
    engine_a: PathBuf,
    #[arg(long)]
    engine_b: PathBuf,
    /// Working directory for engine A (defaults to the current directory, so an
    /// engine that loads `nnue/default.bin` finds it in this worktree).
    #[arg(long)]
    cwd_a: Option<PathBuf>,
    #[arg(long)]
    cwd_b: Option<PathBuf>,
    /// `setoption` pairs for A, `Name=Value;Name=Value`.
    #[arg(long, default_value = "")]
    options_a: String,
    #[arg(long, default_value = "")]
    options_b: String,
    /// `go` arguments for A; default `btime 0 wtime 0 byoyomi <byoyomi>`.
    #[arg(long)]
    go_a: Option<String>,
    #[arg(long)]
    go_b: Option<String>,
    /// Per-move time (ms) when `--go-*` is not given.
    #[arg(long, default_value_t = 1000)]
    byoyomi: u32,
    /// Number of games (rounded up to an even number: openings come in pairs).
    #[arg(long, default_value_t = 20)]
    games: u32,
    #[arg(long, default_value_t = 8)]
    random_plies: u32,
    #[arg(long, default_value_t = 256)]
    max_moves: u32,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, default_value = "usi_match.csv")]
    output: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Res {
    AWin,
    BWin,
    Draw,
}

struct Side {
    engine: UsiEngine,
    go: String,
    max_think_ms: u128,
}

fn start(path: &Path, cwd: &Path, options: &str) -> Result<UsiEngine, Box<dyn std::error::Error>> {
    let mut e = UsiEngine::spawn(path, cwd)?;
    e.handshake()?;
    for kv in options.split(';').filter(|s| !s.trim().is_empty()) {
        let (k, v) = kv.split_once('=').ok_or(format!("bad option '{kv}'"))?;
        e.set_option(k.trim(), v.trim())?;
    }
    e.isready()?;
    Ok(e)
}

/// Play one game; `a_black` says whether engine A has the first move.
fn play(
    a: &mut Side,
    b: &mut Side,
    a_black: bool,
    opening: &[String],
    max_moves: u32,
) -> Result<(Res, u32, String, Vec<String>), Box<dyn std::error::Error>> {
    let mg = MoveGenerator::new();
    let hasher = ShogiHashHandler::new_default();
    let mut board = BitboardBoard::new();
    let mut captured = CapturedPieces::new();
    let mut player = Player::Black;
    let mut moves: Vec<String> = Vec::new();
    let mut seen: HashMap<u64, u8> = HashMap::new();
    for m in opening {
        let legal = mg.generate_legal_moves(&board, player, &captured);
        let mv = legal.iter().find(|x| &x.to_usi_string() == m).ok_or("bad opening")?.clone();
        board.make_move_with_hand(&mv, &mut captured);
        moves.push(m.clone());
        player = player.opposite();
    }
    a.engine.new_game()?;
    b.engine.new_game()?;
    let a_is = |p: Player| (p == Player::Black) == a_black;
    let win_for = |p: Player| if a_is(p) { Res::AWin } else { Res::BWin };
    loop {
        let legal = mg.generate_legal_moves(&board, player, &captured);
        if legal.is_empty() {
            return Ok((win_for(player.opposite()), moves.len() as u32, "no legal moves".into(), moves));
        }
        let h = hasher.get_position_hash(&board, player, &captured);
        let c = seen.entry(h).or_insert(0);
        *c += 1;
        if *c >= 4 {
            return Ok((Res::Draw, moves.len() as u32, "repetition".into(), moves));
        }
        if moves.len() as u32 >= max_moves {
            return Ok((Res::Draw, moves.len() as u32, "move limit".into(), moves));
        }
        let side = if a_is(player) { &mut *a } else { &mut *b };
        side.engine.set_position_startpos(&moves)?;
        let t0 = Instant::now();
        let result = side.engine.go_with(&side.go)?;
        let took = t0.elapsed().as_millis();
        side.max_think_ms = side.max_think_ms.max(took);
        if took > 2000 && side.go.contains("byoyomi") {
            eprintln!(
                "SLOW MOVE {} ms ({}): position startpos moves {}",
                took,
                if a_is(player) { "A" } else { "B" },
                moves.join(" ")
            );
        }
        let bm = result.bestmove.trim().to_string();
        match bm.as_str() {
            "resign" => {
                return Ok((win_for(player.opposite()), moves.len() as u32, "resign".into(), moves))
            }
            "win" => {
                return Ok((win_for(player), moves.len() as u32, "declared win".into(), moves))
            }
            _ => {}
        }
        let mv = match legal.iter().find(|x| x.to_usi_string() == bm) {
            Some(m) => m.clone(),
            None => {
                return Ok((
                    win_for(player.opposite()),
                    moves.len() as u32,
                    format!("illegal move {bm}"),
                    moves,
                ))
            }
        };
        board.make_move_with_hand(&mv, &mut captured);
        moves.push(bm);
        player = player.opposite();
    }
}

fn random_opening(rng: &mut StdRng, plies: u32) -> Vec<String> {
    let mg = MoveGenerator::new();
    let mut board = BitboardBoard::new();
    let mut captured = CapturedPieces::new();
    let mut player = Player::Black;
    let mut out = Vec::new();
    for _ in 0..plies {
        let legal = mg.generate_legal_moves(&board, player, &captured);
        if legal.is_empty() {
            break;
        }
        let mv = legal[rng.gen_range(0..legal.len())].clone();
        out.push(mv.to_usi_string());
        board.make_move_with_hand(&mv, &mut captured);
        player = player.opposite();
    }
    out
}

fn elo(score: f64) -> f64 {
    let s = score.clamp(1e-6, 1.0 - 1e-6);
    -400.0 * (1.0 / s - 1.0).log10()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let here = std::env::current_dir()?;
    let default_go = format!("btime 0 wtime 0 byoyomi {}", cli.byoyomi);
    let mut a = Side {
        engine: start(&cli.engine_a, cli.cwd_a.as_deref().unwrap_or(&here), &cli.options_a)?,
        go: cli.go_a.clone().unwrap_or_else(|| default_go.clone()),
        max_think_ms: 0,
    };
    let mut b = Side {
        engine: start(&cli.engine_b, cli.cwd_b.as_deref().unwrap_or(&here), &cli.options_b)?,
        go: cli.go_b.clone().unwrap_or_else(|| default_go.clone()),
        max_think_ms: 0,
    };
    println!("A: {} (go {})", cli.engine_a.display(), a.go);
    println!("B: {} (go {})", cli.engine_b.display(), b.go);

    let mut rng = StdRng::seed_from_u64(cli.seed);
    let mut csv = std::fs::File::create(&cli.output)?;
    writeln!(csv, "game,a_color,result,moves,reason,record")?;
    let pairs = (cli.games + 1) / 2;
    let (mut w, mut d, mut l) = (0u32, 0u32, 0u32);
    let mut pair_points: Vec<f64> = Vec::new();
    for p in 0..pairs {
        let opening = random_opening(&mut rng, cli.random_plies);
        let mut pts = 0.0;
        for k in 0..2 {
            let a_black = k == 0;
            let (res, n, why, record) = play(&mut a, &mut b, a_black, &opening, cli.max_moves)?;
            match res {
                Res::AWin => {
                    w += 1;
                    pts += 1.0
                }
                Res::BWin => l += 1,
                Res::Draw => {
                    d += 1;
                    pts += 0.5
                }
            }
            let g = p * 2 + k + 1;
            writeln!(
                csv,
                "{},{},{:?},{},{},{}",
                g,
                if a_black { "black" } else { "white" },
                res,
                n,
                why,
                record.join(" ")
            )?;
            csv.flush()?;
            println!(
                "Game {:>3}: A {} {:?} in {} moves ({}) | W{} D{} L{}",
                g,
                if a_black { "black" } else { "white" },
                res,
                n,
                why,
                w,
                d,
                l
            );
        }
        pair_points.push(pts);
    }
    let n = pair_points.len() as f64;
    let scores: Vec<f64> = pair_points.iter().map(|p| p / 2.0).collect();
    let mean = scores.iter().sum::<f64>() / n;
    let var = scores.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let se = (var / n).sqrt();
    println!("=== RESULT ===");
    println!("  A: {} wins, {} draws, {} losses", w, d, l);
    println!(
        "  Paired Elo (A): {:+.1} ± {:.1} over {} opening pairs",
        elo(mean),
        (elo(mean + 1.96 * se) - elo(mean - 1.96 * se)) / 2.0,
        pair_points.len()
    );
    println!("  Longest think: A {} ms, B {} ms", a.max_think_ms, b.max_think_ms);
    let _ = a.engine.quit();
    let _ = b.engine.quit();
    Ok(())
}
