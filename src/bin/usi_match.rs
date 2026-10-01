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
//! * `--sprt-elo0/--sprt-elo1`: a sequential test (pentanomial GSPRT, the
//!   OpenBench/Fishtest normal approximation over opening pairs) that stops as
//!   soon as the result is decided; `--games` is then the cap. H0: A is
//!   `elo0` stronger than B; H1: `elo1`. Accepting H1 is a pass.

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
    /// Start every game from this SFEN instead of the initial position. Use it
    /// for handicap games: an opponent far stronger than us is uninformative at
    /// even material (we score ~3/24 against YaneuraOu whether it searches 1
    /// node or 50), but at the right handicap the win rate becomes sensitive to
    /// our own improvements again.
    #[arg(long)]
    start_sfen: Option<String>,
    #[arg(long, default_value = "usi_match.csv")]
    output: PathBuf,
    /// SPRT null hypothesis, logistic Elo of A over B (e.g. 0). Set both
    /// bounds to stop the match as soon as the test decides.
    #[arg(long, allow_hyphen_values = true, requires = "sprt_elo1")]
    sprt_elo0: Option<f64>,
    /// SPRT alternative hypothesis, logistic Elo (e.g. 40).
    #[arg(long, allow_hyphen_values = true, requires = "sprt_elo0")]
    sprt_elo1: Option<f64>,
    /// SPRT false-positive rate (passing a change that is no better than elo0).
    #[arg(long, default_value_t = 0.05)]
    sprt_alpha: f64,
    /// SPRT false-negative rate (failing a change that is worth elo1).
    #[arg(long, default_value_t = 0.05)]
    sprt_beta: f64,
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
    start_sfen: Option<&str>,
) -> Result<(Res, u32, String, Vec<String>), Box<dyn std::error::Error>> {
    let mg = MoveGenerator::new();
    let hasher = ShogiHashHandler::new_default();
    let (mut board, mut player, mut captured) = match start_sfen {
        Some(sfen) => BitboardBoard::from_fen(sfen)?,
        None => (BitboardBoard::new(), Player::Black, CapturedPieces::new()),
    };
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
        side.engine.set_position(start_sfen, &moves)?;
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

fn random_opening(rng: &mut StdRng, plies: u32, start_sfen: Option<&str>) -> Vec<String> {
    let mg = MoveGenerator::new();
    let (mut board, mut player, mut captured) = match start_sfen {
        Some(sfen) => BitboardBoard::from_fen(sfen).expect("valid --start-sfen"),
        None => (BitboardBoard::new(), Player::Black, CapturedPieces::new()),
    };
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

/// Expected score of a side `elo` stronger.
fn expected_score(elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-elo / 400.0))
}

/// Log-likelihood ratio of H1 (`elo1`) against H0 (`elo0`) from the per-pair
/// scores (0, 1/4, 1/2, 3/4, 1), by the normal approximation to the
/// pentanomial GSPRT: `n (s1 - s0) (2 mean - s0 - s1) / (2 var)`. Pairs, not
/// games, are the independent unit -- the two games of an opening share it.
fn sprt_llr(pair_scores: &[f64], elo0: f64, elo1: f64) -> f64 {
    let n = pair_scores.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mean = pair_scores.iter().sum::<f64>() / n;
    let var = pair_scores.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    // Too few pairs to estimate a spread (all the same result): wait.
    if var < 1e-9 {
        return 0.0;
    }
    let (s0, s1) = (expected_score(elo0), expected_score(elo1));
    n * (s1 - s0) * (2.0 * mean - s0 - s1) / (2.0 * var)
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
    let sprt = cli.sprt_elo0.zip(cli.sprt_elo1);
    // Wald's bounds on the log-likelihood ratio.
    let lower = (cli.sprt_beta / (1.0 - cli.sprt_alpha)).ln();
    let upper = ((1.0 - cli.sprt_beta) / cli.sprt_alpha).ln();
    if let Some((e0, e1)) = sprt {
        println!(
            "SPRT: H0 elo {e0:+} vs H1 elo {e1:+}, alpha {} beta {}, LLR bounds [{lower:.2}, {upper:.2}], at most {} games",
            cli.sprt_alpha,
            cli.sprt_beta,
            pairs * 2
        );
    }
    let mut verdict = None;
    for p in 0..pairs {
        let opening = random_opening(&mut rng, cli.random_plies, cli.start_sfen.as_deref());
        let mut pts = 0.0;
        for k in 0..2 {
            let a_black = k == 0;
            let (res, n, why, record) = play(&mut a, &mut b, a_black, &opening, cli.max_moves, cli.start_sfen.as_deref())?;
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
        if let Some((e0, e1)) = sprt {
            let scores: Vec<f64> = pair_points.iter().map(|x| x / 2.0).collect();
            let llr = sprt_llr(&scores, e0, e1);
            println!("SPRT: LLR {llr:+.2} [{lower:.2}, {upper:.2}] after {} pairs", pair_points.len());
            if llr >= upper {
                verdict = Some("PASS (H1 accepted)");
                break;
            }
            if llr <= lower {
                verdict = Some("FAIL (H0 accepted)");
                break;
            }
        }
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
    if let Some((e0, e1)) = sprt {
        let scores: Vec<f64> = pair_points.iter().map(|x| x / 2.0).collect();
        println!(
            "  SPRT [{e0:+}, {e1:+}]: {} (LLR {:+.2})",
            verdict.unwrap_or("INCONCLUSIVE (game cap reached)"),
            sprt_llr(&scores, e0, e1)
        );
    }
    let _ = a.engine.quit();
    let _ = b.engine.quit();
    Ok(())
}
