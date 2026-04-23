//! NNUE Offline Trainer
//!
//! Trains NNUE weights against a pre-generated corpus of teacher-evaluated
//! positions (produced by `corpus-gen`). Each corpus record is one JSONL line:
//!
//!     {"game_id", "ply", "sfen", "player", "eval_cp", "outcome", ...}
//!
//! For each record we reconstruct the `BitboardBoard`, extract NNUE active
//! features, refresh the accumulator, and supervise the student network
//! against a blended target:
//!
//!     target = (1 - α) * tanh(eval_cp / 600) + α * outcome_to_move_pov
//!
//! where `eval_cp` is already the teacher's evaluation from the to-move
//! player's perspective (standard USI convention) and α (`--outcome-weight`)
//! controls how strongly the actual game result overrides the per-ply eval
//! (Stockfish-style hybrid).

use clap::Parser;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{NNUEAccumulator, NNUEWeights};
use shogi_engine::evaluation::nnue_training::{
    extract_active_features, NNUETrainer, NNUETrainingConfig, TrainingPosition,
};
use shogi_engine::types::core::Player;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "nnue-offline-trainer")]
#[command(about = "Train NNUE against a JSONL teacher-eval corpus")]
struct Cli {
    /// Path to the JSONL corpus (output of `corpus-gen`).
    #[arg(long)]
    corpus: PathBuf,

    /// Path to initial weights. If omitted, a fresh random network is created.
    #[arg(long)]
    init_weights: Option<PathBuf>,

    /// Architecture if we are initialising fresh weights.
    #[arg(long, default_value_t = 256)]
    hidden_1: usize,
    #[arg(long, default_value_t = 32)]
    hidden_2: usize,

    /// Where to save the trained weights.
    #[arg(long, default_value = "nnue_weights_yaneura_trained.json")]
    output_weights: PathBuf,

    /// Also save a checkpoint every N epochs (pattern: <stem>_epoch_N.json).
    #[arg(long, default_value_t = 10)]
    checkpoint_every: u32,

    /// Number of passes over the corpus.
    #[arg(long, default_value_t = 20)]
    epochs: u32,

    /// Mini-batch size (positions per weight-update call).
    #[arg(long, default_value_t = 256)]
    batch_size: usize,

    /// SGD learning rate.
    #[arg(long, default_value_t = 0.005)]
    learning_rate: f32,

    /// Output-layer gradient scale (see NNUETrainingConfig). Default 1e4 is
    /// the teacher-target regime; the 1e7 PST default would saturate weights
    /// in a single batch here.
    #[arg(long, default_value_t = 1e4)]
    output_grad_scale: f32,

    /// Input-layer gradient scale. Default 1e3 for teacher-target regime.
    #[arg(long, default_value_t = 1e3)]
    input_grad_scale: f32,

    /// Blend weight for game outcome vs teacher eval (0 = eval only,
    /// 1 = outcome only).
    #[arg(long, default_value_t = 0.3)]
    outcome_weight: f32,

    /// Clamp |eval_cp| to this value before normalising, to keep mate scores
    /// from dominating the gradient.
    #[arg(long, default_value_t = 2000)]
    eval_clamp: i32,

    /// Shuffle seed (0 = system entropy).
    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Drop records with `eval_cp = null` (opening plies) — they have no
    /// teacher target. On by default.
    #[arg(long, default_value_t = true)]
    skip_null_eval: bool,

    /// Print extra per-batch diagnostics.
    #[arg(long)]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct CorpusRecord {
    sfen: String,
    player: Player,
    eval_cp: Option<i32>,
    outcome: i8, // +1 black win, 0 draw, -1 white win
}

/// Scan a JSON object for the given "key":... value, returning the raw token
/// after the colon. Caller must trim and unquote as needed.
fn find_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", key);
    let idx = line.find(&needle)?;
    Some(&line[idx + needle.len()..])
}

fn parse_jsonl_line(line: &str) -> Option<CorpusRecord> {
    let sfen_raw = find_field(line, "sfen")?;
    let sfen_start = sfen_raw.find('"')? + 1;
    let rest = &sfen_raw[sfen_start..];
    let sfen_end = rest.find('"')?;
    let sfen = rest[..sfen_end].to_string();

    let player_raw = find_field(line, "player")?;
    let p_start = player_raw.find('"')? + 1;
    let prest = &player_raw[p_start..];
    let p_end = prest.find('"')?;
    let player = match &prest[..p_end] {
        "black" => Player::Black,
        "white" => Player::White,
        _ => return None,
    };

    let eval_raw = find_field(line, "eval_cp")?.trim_start();
    let eval_end = eval_raw
        .find(|c: char| c == ',' || c == '}')
        .unwrap_or(eval_raw.len());
    let eval_tok = eval_raw[..eval_end].trim();
    let eval_cp = if eval_tok == "null" {
        None
    } else {
        eval_tok.parse::<i32>().ok()
    };

    let outcome_raw = find_field(line, "outcome")?.trim_start();
    let out_end = outcome_raw
        .find(|c: char| c == ',' || c == '}')
        .unwrap_or(outcome_raw.len());
    let outcome = outcome_raw[..out_end].trim().parse::<i8>().ok()?;

    Some(CorpusRecord { sfen, player, eval_cp, outcome })
}

fn load_corpus(path: &Path, skip_null_eval: bool) -> std::io::Result<Vec<CorpusRecord>> {
    let f = File::open(path)?;
    let mut out = Vec::new();
    let mut skipped_parse = 0_usize;
    let mut skipped_null = 0_usize;
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match parse_jsonl_line(&line) {
            Some(rec) => {
                if skip_null_eval && rec.eval_cp.is_none() {
                    skipped_null += 1;
                    continue;
                }
                out.push(rec);
            }
            None => skipped_parse += 1,
        }
    }
    if skipped_parse > 0 {
        eprintln!("warning: {} JSONL lines failed to parse", skipped_parse);
    }
    if skipped_null > 0 {
        eprintln!("info: skipped {} records with eval_cp = null", skipped_null);
    }
    Ok(out)
}

/// Parse the SFEN produced by `BitboardBoard::to_fen` (3 fields: board, side,
/// hand). Our `BitboardBoard::from_fen` already handles this.
fn board_from_sfen(sfen: &str) -> Option<BitboardBoard> {
    BitboardBoard::from_fen(sfen).ok().map(|(board, _p, _cap)| board)
}

/// Compute the supervised target for a record, returning target in [-1, 1] from
/// the to-move player's perspective.
fn target_for(rec: &CorpusRecord, outcome_weight: f32, eval_clamp: i32) -> Option<f32> {
    let cp = rec.eval_cp?;
    let clamped = cp.max(-eval_clamp).min(eval_clamp);
    let teacher = (clamped as f32 / 600.0).tanh();

    let player_mult = if rec.player == Player::Black { 1.0_f32 } else { -1.0 };
    let outcome_tomove = rec.outcome as f32 * player_mult;

    let t = (1.0 - outcome_weight) * teacher + outcome_weight * outcome_tomove;
    Some(t.clamp(-1.0, 1.0))
}

/// Build a TrainingPosition from a corpus record (assumes target can be computed).
fn build_position(
    rec: &CorpusRecord,
    weights: &NNUEWeights,
    outcome_weight: f32,
    eval_clamp: i32,
) -> Option<TrainingPosition> {
    let target = target_for(rec, outcome_weight, eval_clamp)?;
    let board = board_from_sfen(&rec.sfen)?;
    let active_features = extract_active_features(&board);
    let (h1, h2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(h1, h2);
    accumulator.refresh(&board, weights);
    let nnue_eval = accumulator.evaluate(weights);
    Some(TrainingPosition {
        active_features,
        accumulator,
        evaluation: nnue_eval,
        pst_evaluation: rec.eval_cp.unwrap_or(0),
        player: rec.player,
        move_made: None,
        outcome: None,
        td_target: Some(target),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    println!("=== NNUE Offline Trainer ===");
    println!("  Corpus:         {}", cli.corpus.display());
    println!("  Init weights:   {:?}", cli.init_weights);
    println!("  Output weights: {}", cli.output_weights.display());
    println!("  Arch:           {} -> {} -> 1", cli.hidden_1, cli.hidden_2);
    println!("  Epochs:         {}", cli.epochs);
    println!("  Batch size:     {}", cli.batch_size);
    println!("  Learning rate:  {}", cli.learning_rate);
    println!("  Out grad scale: {}", cli.output_grad_scale);
    println!("  In  grad scale: {}", cli.input_grad_scale);
    println!("  Outcome weight: {}", cli.outcome_weight);
    println!("  Eval clamp:     {}", cli.eval_clamp);
    println!("  Seed:           {}", cli.seed);
    println!();

    let mut records = load_corpus(&cli.corpus, cli.skip_null_eval)?;
    println!("Loaded {} training records", records.len());
    if records.is_empty() {
        return Err("no usable records in corpus".into());
    }

    let mut rng = if cli.seed == 0 {
        StdRng::from_entropy()
    } else {
        StdRng::seed_from_u64(cli.seed)
    };

    let initial_weights = match &cli.init_weights {
        Some(p) => {
            println!("Loading initial weights from {}", p.display());
            NNUEWeights::load(p)?
        }
        None => {
            println!("Initialising fresh random weights");
            NNUEWeights::new(cli.hidden_1, cli.hidden_2)
        }
    };

    let mut config = NNUETrainingConfig::default();
    config.learning_rate = cli.learning_rate;
    config.output_grad_scale = cli.output_grad_scale;
    config.input_grad_scale = cli.input_grad_scale;
    // min_batch_size is irrelevant here — we drive the batching ourselves
    // via train_batch, but set it so any stray add_training_game path
    // doesn't fire unexpectedly.
    config.min_batch_size = cli.batch_size.max(100_000);

    let mut trainer = NNUETrainer::new(initial_weights, config);

    let start = Instant::now();
    let total_epochs = cli.epochs;
    let total_positions = records.len();

    for epoch in 0..total_epochs {
        let epoch_start = Instant::now();
        records.shuffle(&mut rng);

        let mut batch = Vec::with_capacity(cli.batch_size);
        let mut batch_count = 0_usize;
        let mut epoch_td_sum = 0.0_f64;
        let mut epoch_batches = 0_usize;

        for rec in records.iter() {
            if let Some(pos) = build_position(
                rec,
                trainer.get_weights(),
                cli.outcome_weight,
                cli.eval_clamp,
            ) {
                batch.push(pos);
            }
            if batch.len() >= cli.batch_size {
                let stats = trainer.train_batch_accumulated(&batch);
                epoch_td_sum += stats.avg_td_error as f64;
                epoch_batches += 1;
                if cli.verbose && batch_count % 20 == 0 {
                    println!(
                        "  epoch {:3} batch {:4}: td_err={:.4} w_chg={:.3}",
                        epoch + 1,
                        batch_count,
                        stats.avg_td_error,
                        stats.avg_weight_change
                    );
                }
                batch.clear();
                batch_count += 1;
            }
        }
        // Tail batch
        if !batch.is_empty() {
            let stats = trainer.train_batch_accumulated(&batch);
            epoch_td_sum += stats.avg_td_error as f64;
            epoch_batches += 1;
            batch.clear();
        }

        let epoch_avg_td = if epoch_batches > 0 {
            epoch_td_sum / epoch_batches as f64
        } else {
            0.0
        };

        println!(
            "Epoch {:3}/{:3}: {:6} pos, {:4} batches, avg td_err={:.4}, t={:.1}s",
            epoch + 1,
            total_epochs,
            total_positions,
            epoch_batches,
            epoch_avg_td,
            epoch_start.elapsed().as_secs_f64()
        );

        if cli.checkpoint_every > 0 && (epoch + 1) % cli.checkpoint_every == 0 {
            let stem = cli.output_weights.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "nnue_weights".into());
            let parent = cli.output_weights.parent().unwrap_or(Path::new("."));
            let ckpt = parent.join(format!("{}_epoch_{}.json", stem, epoch + 1));
            trainer.get_weights().save(&ckpt)?;
            println!("  checkpoint -> {}", ckpt.display());
        }
    }

    trainer.get_weights().save(&cli.output_weights)?;
    println!();
    println!("=== TRAINING COMPLETE ===");
    println!("  Epochs:       {}", total_epochs);
    println!("  Positions/ep: {}", total_positions);
    println!("  Wall time:    {:.1}s", start.elapsed().as_secs_f64());
    println!("  Weights ->    {}", cli.output_weights.display());

    Ok(())
}
