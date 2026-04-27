//! NNUE vs PST ELO Tester (Phase 4 validation)
//!
//! Plays N head-to-head games between two engine configurations built on top
//! of the same SearchEngine / IterativeDeepening stack:
//!   * Engine A: NNUE-enabled (loads a trained weights file)
//!   * Engine B: PST-only   (NNUE disabled)
//!
//! Colors alternate each game. Each game begins from a shared random opening
//! (seeded) so both sides face the same start, but each game's opening is
//! unique. Draws are declared on 3-fold repetition or move-limit.
//!
//! Reports W/D/L, score, ELO difference and a ~95% confidence interval, and
//! writes a per-game CSV.
//!
//! Usage:
//!     cargo run --release --bin elo-tester -- \
//!         --nnue-weights nnue_weights_trained.json \
//!         --games 40 --depth 3 --time-ms 100 --seed 42

use clap::Parser;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::search::shogi_hash::ShogiHashHandler;
use shogi_engine::search::IterativeDeepening;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;
use shogi_engine::BitboardBoard;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write as IoWrite;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "elo-tester")]
#[command(about = "NNUE vs PST head-to-head ELO test (Phase 4 validation)")]
struct Cli {
    /// Path to trained NNUE weights (JSON)
    #[arg(long, default_value = "nnue_weights_trained.json")]
    nnue_weights: String,

    /// Number of games to play
    #[arg(short, long, default_value_t = 20)]
    games: u32,

    /// Max search depth (iterative deepening)
    #[arg(short, long, default_value_t = 3)]
    depth: u8,

    /// Time per move in milliseconds. Default 500 ms — Session 8 found that
    /// 100-150 ms is *intrinsically* weight-blind at depth 3 because the
    /// iterative-deepening search bails out before producing a scored PV
    /// and both engines return `eval=0` for every move.
    #[arg(long, default_value_t = 500)]
    time_ms: u32,

    /// If set, disable the per-move time cutoff and let iterative deepening
    /// run to the requested `--depth` for every move. Equivalent to passing
    /// a very large `--time-ms` and recommended for weight-comparison runs.
    #[arg(long)]
    fixed_depth: bool,

    /// Max moves per game before declaring draw
    #[arg(long, default_value_t = 200)]
    max_moves: u32,

    /// Number of random opening plies (shared by both sides each game)
    #[arg(long, default_value_t = 6)]
    random_plies: u8,

    /// RNG seed for opening randomness (0 = use system entropy)
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Transposition-table size in MB for each engine
    #[arg(long, default_value_t = 32)]
    tt_mb: usize,

    /// Output CSV with per-game results
    #[arg(long, default_value = "elo_results.csv")]
    output: String,

    /// Verbose per-move output
    #[arg(long)]
    verbose: bool,

    /// Session 14: enable the side-to-move feature on the loaded NNUE
    /// weights. The feature occupies row `STM_FEATURE_INDEX` in
    /// `input_weights_1` and is active for Black-to-move positions. Set
    /// this when loading weights trained with `nnue-offline-trainer
    /// --use-stm-feature`; pre-Session-14 weights have a zero stm row,
    /// so leaving this off keeps backward compatibility. The flag is
    /// ignored for the PST engine.
    #[arg(long)]
    use_stm_feature: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    NnueWin,
    PstWin,
    Draw,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::NnueWin => "nnue_win",
            Outcome::PstWin => "pst_win",
            Outcome::Draw => "draw",
        }
    }
}

/// Play `num_plies` random legal moves. Mutates board/captured/player in place.
fn play_random_opening(
    board: &mut BitboardBoard,
    captured: &mut CapturedPieces,
    player: &mut Player,
    rng: &mut StdRng,
    num_plies: u8,
) -> u32 {
    let mg = MoveGenerator::new();
    let mut plies: u32 = 0;
    for _ in 0..num_plies {
        let legal = mg.generate_legal_moves(board, *player, captured);
        if legal.is_empty() {
            break;
        }
        let idx = rng.gen_range(0..legal.len());
        let mv = legal[idx].clone();
        if let Some(cap) = board.make_move(&mv) {
            captured.add_piece(cap.piece_type, *player);
        }
        *player = player.opposite();
        plies += 1;
    }
    plies
}

/// Play a single game. Returns (outcome, move_count).
///
/// Both engines are given a fresh copy of the same random opening position.
/// Which color NNUE plays is controlled by `nnue_plays_black`.
#[allow(clippy::too_many_arguments)]
fn play_game(
    nnue_engine: &mut SearchEngine,
    pst_engine: &mut SearchEngine,
    nnue_plays_black: bool,
    depth: u8,
    time_ms: u32,
    max_moves: u32,
    random_plies: u8,
    rng: &mut StdRng,
    verbose: bool,
) -> (Outcome, u32) {
    let mut board = BitboardBoard::new();
    let mut captured = CapturedPieces::new();
    let mut player = Player::Black;
    let mg = MoveGenerator::new();
    let hash_calc = ShogiHashHandler::new_default();
    let mut pos_counts: HashMap<u64, u8> = HashMap::new();

    let opening_plies = play_random_opening(
        &mut board,
        &mut captured,
        &mut player,
        rng,
        random_plies,
    );
    let mut move_count: u32 = opening_plies;

    loop {
        let legal = mg.generate_legal_moves(&board, player, &captured);
        if legal.is_empty() {
            let in_check = board.is_king_in_check(player, &captured);
            if !in_check {
                return (Outcome::Draw, move_count);
            }
            let winner = player.opposite();
            let nnue_color = if nnue_plays_black { Player::Black } else { Player::White };
            let outcome = if winner == nnue_color {
                Outcome::NnueWin
            } else {
                Outcome::PstWin
            };
            return (outcome, move_count);
        }

        let pos_hash = hash_calc.get_position_hash(&board, player, &captured);
        let c = pos_counts.entry(pos_hash).or_insert(0);
        *c += 1;
        if *c >= 3 {
            return (Outcome::Draw, move_count);
        }

        if move_count >= max_moves {
            return (Outcome::Draw, move_count);
        }

        let nnue_to_move = (player == Player::Black) == nnue_plays_black;
        let engine: &mut SearchEngine = if nnue_to_move { nnue_engine } else { pst_engine };
        let mut id = IterativeDeepening::new(depth, time_ms, None);
        let best = id.search(engine, &board, &captured, player);

        match best {
            Some((mv, score)) => {
                if verbose {
                    println!(
                        "  move {:3} {:?} ({}): {} eval={}",
                        move_count + 1,
                        player,
                        if nnue_to_move { "NNUE" } else { "PST " },
                        mv.to_usi_string(),
                        score
                    );
                }
                if let Some(cap) = board.make_move(&mv) {
                    captured.add_piece(cap.piece_type, player);
                }
                player = player.opposite();
                move_count += 1;
            }
            None => return (Outcome::Draw, move_count),
        }
    }
}

/// Compute ELO difference from the perspective of NNUE.
/// score = (wins + 0.5*draws) / total
/// elo  = -400 * log10(1/score - 1)
fn elo_from_score(wins: u32, draws: u32, losses: u32) -> f64 {
    let total = (wins + draws + losses) as f64;
    if total <= 0.0 {
        return 0.0;
    }
    let score = (wins as f64 + 0.5 * draws as f64) / total;
    if score <= 0.0 {
        return -800.0;
    }
    if score >= 1.0 {
        return 800.0;
    }
    -400.0 * (1.0 / score - 1.0).log10()
}

/// Approximate 95% CI half-width (in ELO). Uses the sample score variance
/// over X in {1, 0.5, 0}; ignores game-to-game correlation (fine for i.i.d.
/// alternating-color matches).
fn elo_ci95(wins: u32, draws: u32, losses: u32) -> f64 {
    let n = (wins + draws + losses) as f64;
    if n <= 1.0 {
        return f64::INFINITY;
    }
    let w = wins as f64 / n;
    let d = draws as f64 / n;
    let score = w + 0.5 * d;
    let e_x2 = w + 0.25 * d;
    let var = (e_x2 - score * score).max(0.0) / n;
    let std = var.sqrt();
    let lo = (score - 1.96 * std).clamp(1e-6, 1.0 - 1e-6);
    let hi = (score + 1.96 * std).clamp(1e-6, 1.0 - 1e-6);
    let elo_lo = -400.0 * (1.0 / lo - 1.0).log10();
    let elo_hi = -400.0 * (1.0 / hi - 1.0).log10();
    (elo_hi - elo_lo) / 2.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let effective_time_ms: u32 = if cli.fixed_depth { u32::MAX } else { cli.time_ms };

    println!("=== NNUE vs PST ELO Tester ===");
    println!("  Weights:       {}", cli.nnue_weights);
    println!("  Games:         {}", cli.games);
    println!("  Depth:         {}", cli.depth);
    if cli.fixed_depth {
        println!("  Time/move:     fixed-depth (no cutoff)");
    } else {
        println!("  Time/move:     {} ms", cli.time_ms);
    }
    println!("  Max moves:     {}", cli.max_moves);
    println!("  Random plies:  {}", cli.random_plies);
    println!("  TT size:       {} MB", cli.tt_mb);
    println!("  Seed:          {}", cli.seed);
    println!("  Output CSV:    {}", cli.output);
    println!();

    // Build NNUE engine: PositionEvaluator with NNUE weights loaded.
    let mut nnue_engine = SearchEngine::new(None, cli.tt_mb);
    {
        let eval: &mut PositionEvaluator = nnue_engine.get_evaluator_mut();
        eval.enable_nnue_with_weights(&cli.nnue_weights)?;
        assert!(eval.is_nnue_enabled(), "NNUE should be enabled after loading");
        if cli.use_stm_feature {
            eval.nnue_set_use_stm_feature(true);
            println!("  STM feature:   ON (Session 14)");
        }
    }

    // Build PST engine: NNUE explicitly disabled.
    let mut pst_engine = SearchEngine::new(None, cli.tt_mb);
    {
        let eval: &mut PositionEvaluator = pst_engine.get_evaluator_mut();
        eval.disable_nnue();
        assert!(!eval.is_nnue_enabled(), "NNUE should be disabled on PST engine");
    }

    let mut rng = if cli.seed == 0 {
        StdRng::from_entropy()
    } else {
        StdRng::seed_from_u64(cli.seed)
    };

    let mut wins: u32 = 0;
    let mut draws: u32 = 0;
    let mut losses: u32 = 0;

    let mut csv = File::create(&cli.output)?;
    writeln!(csv, "game,nnue_color,outcome,moves")?;

    let start = Instant::now();
    for g in 0..cli.games {
        let nnue_plays_black = g % 2 == 0;
        let game_start = Instant::now();
        let (outcome, moves) = play_game(
            &mut nnue_engine,
            &mut pst_engine,
            nnue_plays_black,
            cli.depth,
            effective_time_ms,
            cli.max_moves,
            cli.random_plies,
            &mut rng,
            cli.verbose,
        );
        match outcome {
            Outcome::NnueWin => wins += 1,
            Outcome::PstWin => losses += 1,
            Outcome::Draw => draws += 1,
        }

        let elo = elo_from_score(wins, draws, losses);
        let ci = elo_ci95(wins, draws, losses);
        println!(
            "Game {:3}/{:3}: NNUE={:5} moves={:3} t={:.1}s | W:{} D:{} L:{} ELO:{:+.1} ± {:.1}",
            g + 1,
            cli.games,
            outcome.as_str(),
            moves,
            game_start.elapsed().as_secs_f64(),
            wins,
            draws,
            losses,
            elo,
            ci,
        );

        let color = if nnue_plays_black { "black" } else { "white" };
        writeln!(csv, "{},{},{},{}", g + 1, color, outcome.as_str(), moves)?;
        csv.flush()?;
    }

    let elapsed = start.elapsed();
    let total = wins + draws + losses;
    let elo = elo_from_score(wins, draws, losses);
    let ci = elo_ci95(wins, draws, losses);

    println!();
    println!("=== FINAL RESULTS ===");
    println!("  Games:   {}", total);
    println!("  NNUE:    {} wins, {} draws, {} losses", wins, draws, losses);
    println!(
        "  Score:   {:.3}",
        if total == 0 {
            0.0
        } else {
            (wins as f64 + 0.5 * draws as f64) / total as f64
        }
    );
    println!("  ELO:     {:+.1} ± {:.1}  (95% CI)", elo, ci);
    println!("  Wall:    {:.1}s", elapsed.as_secs_f64());
    println!("  CSV:     {}", cli.output);

    Ok(())
}
