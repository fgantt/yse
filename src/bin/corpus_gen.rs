//! NNUE Training Corpus Generator
//!
//! Spawns an external USI teacher engine (YaneuraOu or Apery) and has it play
//! self-play games. For each ply we record:
//!     (sfen, eval_cp, player_to_move, game_id, ply)
//! and backfill `outcome` (Black-POV: +1 / 0 / -1) once the game ends.
//!
//! Positions come in three flavors:
//!   1. Random opening plies from our own MoveGenerator (for diversity).
//!      These are still recorded but eval_cp is None because no search was
//!      performed on them.
//!   2. Teacher-driven mid-game plies. eval_cp set to the teacher's last
//!      reported score from the to-move player's perspective.
//!   3. Game-ending positions (no-legal-moves / repetition / move-cap). Not
//!      recorded as samples.
//!
//! Output format: JSON Lines. One JSON object per position per line.

use clap::Parser;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use shogi_engine::moves::MoveGenerator;
use shogi_engine::search::shogi_hash::ShogiHashHandler;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::Player;
use shogi_engine::usi_client::{UsiEngine, UsiError};
use shogi_engine::BitboardBoard;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "corpus-gen")]
#[command(about = "Generate NNUE training corpus via external USI teacher self-play")]
struct Cli {
    /// Path to the teacher engine binary (e.g. YaneuraOu / Apery).
    #[arg(long)]
    engine_path: PathBuf,

    /// Working directory for the teacher (needed for engines that resolve
    /// eval/book paths relative to cwd, e.g. YaneuraOu). Defaults to the
    /// directory containing the engine binary.
    #[arg(long)]
    engine_cwd: Option<PathBuf>,

    /// Number of self-play games to generate.
    #[arg(short, long, default_value_t = 5)]
    games: u32,

    /// Search depth per ply for the teacher (go depth N).
    #[arg(short, long, default_value_t = 8)]
    depth: u32,

    /// Number of random legal plies to play at the start of each game
    /// (from our MoveGenerator, for opening diversity).
    #[arg(long, default_value_t = 8)]
    random_plies: u8,

    /// Maximum moves per game before declaring a draw.
    #[arg(long, default_value_t = 256)]
    max_moves: u32,

    /// RNG seed for random openings (0 = system entropy).
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Output JSONL path.
    #[arg(long, default_value = "nnue_corpus.jsonl")]
    output: PathBuf,

    /// Passed as `setoption name Threads value N`. 1 is most reproducible.
    #[arg(long, default_value_t = 1)]
    threads: u32,

    /// Teacher hash size in MB (`setoption name USI_Hash value N`).
    #[arg(long, default_value_t = 256)]
    hash_mb: u32,

    /// Magnitude used to map mate-in-N scores to cp (mate_magnitude - |N|).
    #[arg(long, default_value_t = 30000)]
    mate_magnitude: i32,

    /// Skip the initial random-opening positions in the output (they have no
    /// teacher eval and many training pipelines don't want them).
    #[arg(long, default_value_t = true)]
    skip_random_opening_positions: bool,

    /// Verbose per-ply output.
    #[arg(long)]
    verbose: bool,

    /// After the teacher labels a position, play a uniformly random legal move
    /// instead of its best move with this probability. The position is still
    /// labelled; the random move sends the game somewhere the teacher would not
    /// go, typically a material imbalance.
    ///
    /// Why: in pure teacher self-play material almost never hangs, so a network
    /// trained on it never sees what a piece is worth, and the search spends
    /// most of its time exactly in positions after captures.
    #[arg(long, default_value_t = 0.0)]
    random_move_prob: f64,

    /// Play the moves with this engine instead of the teacher, so the corpus
    /// follows the positions this engine reaches. The teacher still labels every
    /// position. Both sides are played by it (self-play).
    #[arg(long)]
    player_engine: Option<PathBuf>,

    /// Working directory for `--player-engine` (default: the current directory).
    #[arg(long)]
    player_cwd: Option<PathBuf>,

    /// `go` arguments for `--player-engine`.
    #[arg(long, default_value = "nodes 3000")]
    player_go: String,

    /// Added to every game id, so corpora generated in parallel can be
    /// concatenated without colliding (the trainer's held-out split is by id).
    #[arg(long, default_value_t = 0)]
    game_id_offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    BlackWin,
    WhiteWin,
    Draw,
}

impl Outcome {
    fn as_i8(self) -> i8 {
        match self {
            Outcome::BlackWin => 1,
            Outcome::Draw => 0,
            Outcome::WhiteWin => -1,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Outcome::BlackWin => "black_win",
            Outcome::WhiteWin => "white_win",
            Outcome::Draw => "draw",
        }
    }
}

/// One recorded position within a game.
#[derive(Debug, Clone)]
struct PositionRecord {
    sfen: String,
    player: Player,
    eval_cp: Option<i32>,
    from_random_opening: bool,
    ply: u32,
    /// The move played from this position was random, not the teacher's.
    random_move: bool,
}

fn player_str(p: Player) -> &'static str {
    match p {
        Player::Black => "black",
        Player::White => "white",
    }
}

fn write_jsonl_record(
    out: &mut impl IoWrite,
    game_id: u32,
    rec: &PositionRecord,
    outcome: Outcome,
) -> std::io::Result<()> {
    // Hand-built JSON (no serde) — schema is tiny and stable.
    let eval_str = match rec.eval_cp {
        Some(v) => v.to_string(),
        None => String::from("null"),
    };
    writeln!(
        out,
        "{{\"game_id\":{},\"ply\":{},\"sfen\":\"{}\",\"player\":\"{}\",\"eval_cp\":{},\"outcome\":{},\"outcome_str\":\"{}\",\"from_random_opening\":{},\"random_move\":{}}}",
        game_id,
        rec.ply,
        rec.sfen.replace('"', "\\\""),
        player_str(rec.player),
        eval_str,
        outcome.as_i8(),
        outcome.as_str(),
        rec.from_random_opening,
        rec.random_move,
    )
}

fn play_one_game(
    engine: &mut UsiEngine,
    player: &mut Option<UsiEngine>,
    player_go: &str,
    depth: u32,
    random_plies: u8,
    max_moves: u32,
    random_move_prob: f64,
    rng: &mut StdRng,
    verbose: bool,
) -> Result<(Vec<PositionRecord>, Outcome), UsiError> {
    let mg = MoveGenerator::new();
    let hash_calc = ShogiHashHandler::new_default();

    let mut board = BitboardBoard::new();
    let mut captured = CapturedPieces::new();
    let mut side = Player::Black;

    let mut usi_moves: Vec<String> = Vec::new();
    let mut positions: Vec<PositionRecord> = Vec::new();
    let mut pos_counts: HashMap<u64, u8> = HashMap::new();

    let mut ply: u32 = 0;

    // Phase 1: random opening plies from our own MoveGenerator.
    for _ in 0..random_plies {
        let legal = mg.generate_legal_moves(&board, side, &captured);
        if legal.is_empty() {
            break;
        }
        let mv = legal[rng.gen_range(0..legal.len())].clone();
        let sfen = board.to_fen(side, &captured);
        positions.push(PositionRecord {
            sfen,
            player: side,
            eval_cp: None,
            from_random_opening: true,
            ply,
            random_move: true,
        });
        usi_moves.push(mv.to_usi_string());
        board.make_move_with_hand(&mv, &mut captured);
        side = side.opposite();
        ply += 1;
    }

    // Phase 2: teacher-driven play.
    engine.new_game()?;
    loop {
        // Our-side game-over checks.
        let legal = mg.generate_legal_moves(&board, side, &captured);
        if legal.is_empty() {
            let outcome = if board.is_king_in_check(side, &captured) {
                match side {
                    Player::Black => Outcome::WhiteWin,
                    Player::White => Outcome::BlackWin,
                }
            } else {
                Outcome::Draw
            };
            return Ok((positions, outcome));
        }

        let pos_hash = hash_calc.get_position_hash(&board, side, &captured);
        let c = pos_counts.entry(pos_hash).or_insert(0);
        *c += 1;
        if *c >= 3 {
            return Ok((positions, Outcome::Draw));
        }

        if ply >= max_moves {
            return Ok((positions, Outcome::Draw));
        }

        // Send position and ask teacher for its move.
        engine.set_position_startpos(&usi_moves)?;
        let search = engine.go_depth(depth)?;

        let bestmove_str = search.bestmove.trim().to_string();
        if bestmove_str == "resign" || bestmove_str == "0000" || bestmove_str.is_empty() {
            // Teacher resigns on behalf of the to-move side.
            let outcome = match side {
                Player::Black => Outcome::WhiteWin,
                Player::White => Outcome::BlackWin,
            };
            return Ok((positions, outcome));
        }
        if bestmove_str == "win" {
            // Declaration of win by the to-move side (Shogi entering-king rule).
            let outcome = match side {
                Player::Black => Outcome::BlackWin,
                Player::White => Outcome::WhiteWin,
            };
            return Ok((positions, outcome));
        }

        let sfen_before = board.to_fen(side, &captured);
        let eval_cp = search.as_cp_with_mate(30_000);

        if verbose {
            let eval_dbg = eval_cp.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string());
            println!(
                "    ply {:3} {:?} {} (cp {})",
                ply, side, bestmove_str, eval_dbg
            );
        }

        // The teacher's move must be legal by our move generator, which now
        // agrees with YaneuraOu on every move of the 500-game d10 corpus (see
        // `corpus_audit`). A disagreement means our board has diverged from the
        // teacher's, and every label after it would belong to another position.
        if !legal.iter().any(|m| m.to_usi_string() == bestmove_str) {
            return Err(UsiError::ProtocolError(format!(
                "teacher move '{}' is not legal by our move generator in {} \
                 (moves from startpos: {})",
                bestmove_str,
                sfen_before,
                usi_moves.join(" ")
            )));
        }
        // Which move actually gets played: a random legal one, the move of the
        // engine under training (`--player-engine`, so the corpus follows the
        // positions that engine reaches), or the teacher's.
        let random_move = random_move_prob > 0.0 && rng.gen_bool(random_move_prob);
        let mv = if random_move {
            legal[rng.gen_range(0..legal.len())].clone()
        } else if let Some(p) = player.as_mut() {
            p.set_position_startpos(&usi_moves)?;
            let pm = p.go_with(player_go)?.bestmove.trim().to_string();
            match legal.iter().find(|m| m.to_usi_string() == pm) {
                Some(m) => m.clone(),
                // Resignation or an illegal move ends the game; the side to move
                // loses, as in a real game.
                None => {
                    let outcome = match side {
                        Player::Black => Outcome::WhiteWin,
                        Player::White => Outcome::BlackWin,
                    };
                    return Ok((positions, outcome));
                }
            }
        } else {
            legal.iter().find(|m| m.to_usi_string() == bestmove_str).unwrap().clone()
        };

        positions.push(PositionRecord {
            sfen: sfen_before,
            player: side,
            eval_cp,
            from_random_opening: false,
            ply,
            random_move,
        });

        usi_moves.push(mv.to_usi_string());
        board.make_move_with_hand(&mv, &mut captured);
        side = side.opposite();
        ply += 1;
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let engine_cwd: PathBuf = match cli.engine_cwd.clone() {
        Some(p) => p,
        None => cli
            .engine_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    println!("=== NNUE Corpus Generator ===");
    println!("  Engine:       {}", cli.engine_path.display());
    println!("  Engine cwd:   {}", engine_cwd.display());
    println!("  Games:        {}", cli.games);
    println!("  Depth:        {}", cli.depth);
    println!("  Random plies: {}", cli.random_plies);
    println!("  Max moves:    {}", cli.max_moves);
    println!("  Seed:         {}", cli.seed);
    println!("  Threads:      {}", cli.threads);
    println!("  Hash MB:      {}", cli.hash_mb);
    println!("  Random moves: p={}", cli.random_move_prob);
    println!("  Game id base: {}", cli.game_id_offset);
    println!("  Output:       {}", cli.output.display());
    println!();

    let mut engine = UsiEngine::spawn(&cli.engine_path, &engine_cwd)?;
    engine.handshake()?;

    let mut player_engine = match &cli.player_engine {
        Some(path) => {
            let cwd = cli
                .player_cwd
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let mut p = UsiEngine::spawn(path, &cwd)?;
            p.handshake()?;
            p.isready()?;
            println!("  Player:       {} (go {})", path.display(), cli.player_go);
            Some(p)
        }
        None => None,
    };

    // Best-effort option setting. Unknown options are ignored by the engine;
    // YaneuraOu recognizes these, Apery ignores the ones it doesn't know.
    let _ = engine.set_option("Threads", &cli.threads.to_string());
    let _ = engine.set_option("USI_Hash", &cli.hash_mb.to_string());
    // YaneuraOu-specific noise suppressors:
    let _ = engine.set_option("USI_OwnBook", "false");
    let _ = engine.set_option("BookFile", "no_book");
    let _ = engine.set_option("MinimumThinkingTime", "0");
    let _ = engine.set_option("NetworkDelay", "0");
    let _ = engine.set_option("NetworkDelay2", "0");
    // Apery-specific:
    let _ = engine.set_option("Book_Enable", "false");

    engine.isready()?;

    let mut rng = if cli.seed == 0 {
        StdRng::from_entropy()
    } else {
        StdRng::seed_from_u64(cli.seed)
    };

    let out_file = File::create(&cli.output)?;
    let mut out = BufWriter::new(out_file);

    let mut total_positions: usize = 0;
    let mut written_positions: usize = 0;
    let mut black_wins: u32 = 0;
    let mut white_wins: u32 = 0;
    let mut draws: u32 = 0;

    let start = Instant::now();
    for game_id in 0..cli.games {
        let game_start = Instant::now();
        let (positions, outcome) = play_one_game(
            &mut engine,
            &mut player_engine,
            &cli.player_go,
            cli.depth,
            cli.random_plies,
            cli.max_moves,
            cli.random_move_prob,
            &mut rng,
            cli.verbose,
        )?;

        match outcome {
            Outcome::BlackWin => black_wins += 1,
            Outcome::WhiteWin => white_wins += 1,
            Outcome::Draw => draws += 1,
        }

        let mut game_written = 0_usize;
        for rec in &positions {
            total_positions += 1;
            if cli.skip_random_opening_positions && rec.from_random_opening {
                continue;
            }
            write_jsonl_record(&mut out, game_id + cli.game_id_offset, rec, outcome)?;
            game_written += 1;
        }
        written_positions += game_written;
        out.flush()?;

        println!(
            "Game {:3}/{:3}: {} in {:3} plies, {:4} positions ({} written) t={:.2}s",
            game_id + 1,
            cli.games,
            outcome.as_str(),
            positions.last().map(|p| p.ply + 1).unwrap_or(0),
            positions.len(),
            game_written,
            game_start.elapsed().as_secs_f64(),
        );
    }

    engine.quit()?;

    let elapsed = start.elapsed();
    let total_games = (black_wins + white_wins + draws) as f64;
    let decisive = (black_wins + white_wins) as f64;
    println!();
    println!("=== CORPUS SUMMARY ===");
    println!("  Games:              {}", cli.games);
    println!("  Black wins:         {}", black_wins);
    println!("  White wins:         {}", white_wins);
    println!("  Draws:              {}", draws);
    if total_games > 0.0 {
        println!(
            "  Decisive ratio:     {:.1}%",
            100.0 * decisive / total_games
        );
    }
    println!("  Positions total:    {}", total_positions);
    println!("  Positions written:  {}", written_positions);
    println!("  Wall time:          {:.1}s", elapsed.as_secs_f64());
    println!("  Output:             {}", cli.output.display());

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
