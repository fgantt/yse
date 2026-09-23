//! Sibling-ranking corpus generator.
//!
//! Every corpus we have so far answers "what is this position worth?". This one
//! answers "which of these moves is better, and by how much?" — the question a
//! search actually asks at every node.
//!
//! Session 28 established why that distinction matters: our network correlates
//! r = 0.81 with the teacher on position *values* while choosing moves roughly
//! twice as badly as the teacher (median loss 121 cp vs 64). Correlation on
//! values does not deliver correct ranking between siblings, and ranking is what
//! a search consumes.
//!
//! For each sampled position we ask the teacher for its top-K moves with scores
//! (`MultiPV`), apply each move with our own move generator, and record the
//! resulting child position plus the teacher's score for it. The trainer can
//! then push the network's ordering of those children towards the teacher's.
//!
//! Output format: JSON Lines, one group per line:
//! ```text
//! {"sfen":"<parent>","player":"black","ply":42,
//!  "moves":["2h3g","2g2f"],"scores":[108,102],
//!  "children":["<child sfen>","<child sfen>"]}
//! ```
//! Child SFENs are written out rather than re-derived at training time so that
//! a move our generator cannot reproduce is dropped here, once, instead of
//! failing silently inside an epoch.

use clap::Parser;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::core::Player;
use shogi_engine::usi_client::{UsiEngine, UsiError};
use shogi_engine::BitboardBoard;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write as IoWrite};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "rank-gen")]
#[command(about = "Generate sibling-ranking training data via teacher MultiPV")]
struct Cli {
    /// Path to the teacher engine binary.
    #[arg(long)]
    engine_path: PathBuf,

    /// Working directory for the teacher (YaneuraOu resolves `eval/` from cwd).
    /// Defaults to the directory containing the binary.
    #[arg(long)]
    engine_cwd: Option<PathBuf>,

    /// Source of positions: any corpus JSONL with `sfen` and `player` fields.
    #[arg(long)]
    input: PathBuf,

    /// Output JSONL path.
    #[arg(long)]
    output: PathBuf,

    /// Take every Nth eligible position (after filtering).
    #[arg(long, default_value_t = 1)]
    stride: usize,

    /// Stop after this many groups have been written (0 = no limit).
    #[arg(long, default_value_t = 0)]
    limit: usize,

    /// Teacher search depth per position.
    #[arg(long, default_value_t = 8)]
    depth: u32,

    /// How many sibling moves to ask for.
    #[arg(long, default_value_t = 8)]
    multipv: usize,

    /// Teacher hash size in MB. Small is fine — searches are shallow and the
    /// table is cleared between positions anyway.
    #[arg(long, default_value_t = 64)]
    hash_mb: usize,

    /// Shard index for parallel runs (0-based); each shard takes every
    /// `shards`-th eligible position.
    #[arg(long, default_value_t = 0)]
    shard: usize,

    /// Total number of shards.
    #[arg(long, default_value_t = 1)]
    shards: usize,

    /// Skip positions the corpus marked as random or opening-random: they are
    /// off-distribution and their rankings teach nothing we need.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    skip_random: bool,

    /// Skip positions whose best move already scores beyond this magnitude.
    /// Ranking is uninformative once every move wins; 56.9% of our self-play
    /// positions sit past 2000 cp (Session 28).
    #[arg(long, default_value_t = 3000)]
    max_abs_cp: i32,

    /// Drop groups with fewer than this many usable siblings. A group of one
    /// carries no ordering information at all.
    #[arg(long, default_value_t = 3)]
    min_siblings: usize,

    /// Skip records whose `game_id` is divisible by this, protecting a held-out
    /// set from contaminating training. 0 disables.
    #[arg(long, default_value_t = 0)]
    exclude_gameid_mod: u32,

    /// Mate scores map to this magnitude in centipawns.
    #[arg(long, default_value_t = 30_000)]
    mate_magnitude: i32,

    #[arg(long, default_value_t = false)]
    verbose: bool,
}

/// A minimal view of a source corpus record — only the fields we sample on.
struct SourcePosition {
    sfen: String,
    player: Player,
    ply: u32,
}

fn find_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\":", key);
    let start = line.find(&pat)? + pat.len();
    let rest = &line[start..];
    let end = rest.find(|c| c == ',' || c == '}')?;
    Some(rest[..end].trim())
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches('"')
}

fn parse_source_line(line: &str, cli: &Cli) -> Option<SourcePosition> {
    if cli.skip_random {
        if find_field(line, "random_move") == Some("true")
            || find_field(line, "from_random_opening") == Some("true")
        {
            return None;
        }
    }
    if cli.exclude_gameid_mod > 0 {
        let gid: u32 = find_field(line, "game_id")?.parse().ok()?;
        if gid % cli.exclude_gameid_mod == 0 {
            return None;
        }
    }
    let sfen = unquote(find_field(line, "sfen")?).to_string();
    let player = match unquote(find_field(line, "player")?) {
        "black" => Player::Black,
        "white" => Player::White,
        _ => return None,
    };
    let ply = find_field(line, "ply").and_then(|v| v.parse().ok()).unwrap_or(0);
    Some(SourcePosition { sfen, player, ply })
}

/// Ask the teacher to rank the moves in one position, then replay each move on
/// our own board to get the child it leads to.
///
/// Returns `(moves, scores, child_sfens)`. A move the teacher names but our
/// generator does not produce is dropped rather than fatal: unlike `corpus_gen`,
/// where a divergence would mislabel every later ply of the game, here each
/// position stands alone and a missing sibling costs only that sibling. A
/// *systematic* divergence would show up as a collapsed sibling count, which is
/// reported at the end of the run.
fn rank_one_position(
    engine: &mut UsiEngine,
    mg: &MoveGenerator,
    pos: &SourcePosition,
    cli: &Cli,
) -> Result<Option<(Vec<String>, Vec<i32>, Vec<String>)>, UsiError> {
    let (board, _p, captured) = match BitboardBoard::from_fen(&pos.sfen) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    // The corpus stores SFENs without a move number; YaneuraOu wants one.
    let sfen_for_engine = format!("{} 1", pos.sfen);
    engine.set_position(Some(&sfen_for_engine), &[])?;
    let ranked = engine.go_multipv(cli.depth)?;
    if ranked.is_empty() {
        return Ok(None);
    }

    // Drop positions that are already decided: every move wins, so their
    // ordering carries almost no signal but would dominate a softmax target.
    let top = ranked[0].as_cp_with_mate(cli.mate_magnitude).unwrap_or(0);
    if top.abs() > cli.max_abs_cp {
        return Ok(None);
    }

    let legal = mg.generate_legal_moves(&board, pos.player, &captured);
    let mut moves = Vec::new();
    let mut scores = Vec::new();
    let mut children = Vec::new();
    for r in &ranked {
        let Some(cp) = r.as_cp_with_mate(cli.mate_magnitude) else { continue };
        let Some(mv) = legal.iter().find(|m| m.to_usi_string() == r.mv) else { continue };
        let mut child_board = board.clone();
        let mut child_captured = captured.clone();
        child_board.make_move_with_hand(mv, &mut child_captured);
        let child_sfen = child_board.to_fen(pos.player.opposite(), &child_captured);
        moves.push(r.mv.clone());
        scores.push(cp);
        children.push(child_sfen);
    }
    if moves.len() < cli.min_siblings {
        return Ok(None);
    }
    Ok(Some((moves, scores, children)))
}

fn write_group(
    out: &mut BufWriter<File>,
    pos: &SourcePosition,
    moves: &[String],
    scores: &[i32],
    children: &[String],
) -> std::io::Result<()> {
    let player = match pos.player {
        Player::Black => "black",
        Player::White => "white",
    };
    let quoted = |xs: &[String]| -> String {
        xs.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(",")
    };
    let nums = scores.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(",");
    writeln!(
        out,
        "{{\"sfen\":\"{}\",\"player\":\"{}\",\"ply\":{},\"moves\":[{}],\"scores\":[{}],\"children\":[{}]}}",
        pos.sfen,
        player,
        pos.ply,
        quoted(moves),
        nums,
        quoted(children)
    )
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cwd = cli
        .engine_cwd
        .clone()
        .or_else(|| cli.engine_path.parent().map(PathBuf::from))
        .ok_or("cannot determine engine cwd")?;

    let mut engine = UsiEngine::spawn(&cli.engine_path, &cwd)?;
    engine.handshake()?;
    engine.set_option("Threads", "1")?;
    engine.set_option("USI_Hash", &cli.hash_mb.to_string())?;
    engine.set_multipv(cli.multipv)?;
    engine.isready()?;
    engine.new_game()?;

    let mg = MoveGenerator::new();
    let reader = BufReader::new(File::open(&cli.input)?);
    let mut out = BufWriter::new(File::create(&cli.output)?);

    let start = Instant::now();
    let mut eligible = 0_usize; // positions passing the cheap filters
    let mut queried = 0_usize; // positions actually sent to the teacher
    let mut written = 0_usize;
    let mut sibling_total = 0_usize;

    for line in reader.lines() {
        let line = line?;
        let Some(pos) = parse_source_line(&line, &cli) else { continue };
        let idx = eligible;
        eligible += 1;
        // Shard first, then stride, so shards partition the same sample the
        // single-process run would have taken.
        if cli.shards > 1 && idx % cli.shards != cli.shard {
            continue;
        }
        if cli.stride > 1 && (idx / cli.shards.max(1)) % cli.stride != 0 {
            continue;
        }
        queried += 1;

        match rank_one_position(&mut engine, &mg, &pos, &cli)? {
            Some((moves, scores, children)) => {
                sibling_total += moves.len();
                write_group(&mut out, &pos, &moves, &scores, &children)?;
                written += 1;
                if cli.verbose && written % 1000 == 0 {
                    let rate = written as f64 / start.elapsed().as_secs_f64();
                    println!("  {} groups written ({:.1}/s)", written, rate);
                }
                if cli.limit > 0 && written >= cli.limit {
                    break;
                }
            }
            None => continue,
        }
    }
    out.flush()?;
    let _ = engine.quit();

    let secs = start.elapsed().as_secs_f64();
    println!("=== rank-gen shard {}/{} done ===", cli.shard, cli.shards);
    println!("  eligible positions: {}", eligible);
    println!("  queried:            {}", queried);
    println!("  groups written:     {}", written);
    if written > 0 {
        println!("  mean siblings:      {:.2}", sibling_total as f64 / written as f64);
    }
    println!("  elapsed:            {:.1}s ({:.1} groups/s)", secs, written as f64 / secs);
    println!("  output:             {}", cli.output.display());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("rank-gen error: {}", e);
        std::process::exit(1);
    }
}
