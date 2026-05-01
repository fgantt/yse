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
//!     target = (1 - α) * tanh(eval_cp / target_eval_scale) + α * outcome_to_move_pov
//!
//! where `eval_cp` is already the teacher's evaluation from the to-move
//! player's perspective (standard USI convention) and α (`--outcome-weight`)
//! controls how strongly the actual game result overrides the per-ply eval
//! (Stockfish-style hybrid). `target_eval_scale` (default 600) controls how
//! aggressively the target saturates; raising it (e.g. to 2400) keeps targets
//! in the output range the network can reach when output weights are small.
//! See docs/nnue-phase2/SESSION_LOG_008.md for background.

use clap::Parser;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{
    HAND_FEATURE_BASE_FLAT, HAND_FEATURE_BASE_HALFKP, NNUEAccumulator, NNUEWeights,
};
use shogi_engine::evaluation::nnue_training::{
    extract_active_features, extract_active_features_halfkp,
    extract_active_features_halfkp_with_hand, extract_active_features_halfkp_with_stm,
    extract_active_features_halfkp_with_stm_and_hand, extract_active_features_with_hand,
    extract_active_features_with_stm, extract_active_features_with_stm_and_hand, NNUETrainer,
    NNUETrainingConfig, TrainingPosition,
};
use shogi_engine::types::board::CapturedPieces;
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

    /// Where to save the trained weights. Session 19: default extension is
    /// `.bin` (bincode, ~5× smaller and ~10× faster to load than `.json`);
    /// pass an explicit `.json` path if a JSON file is needed for inspection.
    /// `NNUEWeights::save` selects the format from the path's extension.
    #[arg(long, default_value = "nnue_weights_yaneura_trained.bin")]
    output_weights: PathBuf,

    /// Also save a checkpoint every N epochs. The checkpoint path is
    /// `<output stem>_epoch_N.<output ext>` (i.e. `.bin` by default,
    /// matching `--output-weights`).
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

    /// Scale used in `tanh(eval_cp / scale)` when computing the supervised
    /// target. Historic default 600. Session 8 showed this saturates targets
    /// well outside the range the current PST-imitation output layer can
    /// reach; 2400 keeps targets roughly in the achievable `prediction`
    /// range. `0` selects a linear target (`eval/eval_clamp` clamped to
    /// [-1, 1]) — useful for diagnostic runs where we want no saturation.
    #[arg(long, default_value_t = 600.0)]
    target_eval_scale: f32,

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

    /// Number of records to use for the per-epoch Pearson-r diagnostic that
    /// measures how well the network's cp output linearly tracks the
    /// teacher's `eval_cp`. Independent of `target_eval_scale` — directly
    /// answers Session 10's "td_err is unreliable" issue. Set to 0 to skip.
    #[arg(long, default_value_t = 5000)]
    validate_sample: usize,

    /// Session 12 Experiment B: training-time forward pass uses the f32
    /// shadow `input_weights_1` instead of `position.accumulator.hidden_1`
    /// (which is computed from the i16 weights). This bypasses the
    /// once-per-batch i16 quantization on the input layer so the forward
    /// pass sees the un-rounded weights that gradient updates are
    /// accumulating into. Output and hidden-2 layers continue to use i16
    /// in the forward pass.
    #[arg(long)]
    f32_input_forward: bool,

    /// Session 12 Experiment C: per-position gradient multiplier applied
    /// when |teacher_eval_cp| exceeds `--decisive-threshold-cp`. 1.0 = off
    /// (default). 3.0 means decisive positions contribute 3× as much
    /// gradient signal as non-decisive ones, compensating for the tanh
    /// saturation that suppresses gradient on |target| close to 1.
    #[arg(long, default_value_t = 1.0)]
    decisive_weight: f32,

    /// |teacher_eval_cp| threshold above which a position is "decisive" for
    /// the `--decisive-weight` multiplier.
    #[arg(long, default_value_t = 500)]
    decisive_threshold_cp: i32,

    /// Session 13: switch the trainer's target/loss from
    /// `tanh(eval/target_eval_scale) + L2` (over targets in [-1,1]) to
    /// `sigmoid(eval/sigmoid_eval_scale) + L2` (over targets in [0,1]).
    /// Sigmoid's `p(1-p)` derivative peaks at `p=0.5` rather than vanishing at
    /// the extremes, removing the tanh-saturation bottleneck Session 12
    /// localised. When this flag is on, `--target-eval-scale` controls the cp
    /// scale of the teacher sigmoid only via `--sigmoid-eval-scale`; the
    /// outcome-weight blend uses {0, 0.5, 1} instead of {-1, 0, +1}.
    #[arg(long)]
    use_sigmoid_loss: bool,

    /// cp scale used inside the sigmoid (both teacher target and network
    /// prediction): `sigmoid(eval_cp / sigmoid_eval_scale)`. 410 is
    /// Stockfish's default. Only meaningful when `--use-sigmoid-loss` is set.
    #[arg(long, default_value_t = 410.0)]
    sigmoid_eval_scale: f32,

    /// Session 14: enable the side-to-move feature. When set, the trainer
    /// activates a single binary input feature at `STM_FEATURE_INDEX`
    /// (= `NUM_NNUE_FEATURES`) for Black-to-move positions and leaves it
    /// inactive for White-to-move positions. This gives the network a colour-
    /// asymmetric signal that the per-square piece features alone cannot
    /// produce, testing the Session 13 hand-off hypothesis that the bottleneck
    /// is structural (feature representation) rather than gradient flow.
    /// Pre-Session-14 weight files (rows = `NUM_NNUE_FEATURES`) are padded on
    /// load with a zero stm row, so loading them and then enabling this flag
    /// is a clean A/B against the Session 13 baseline.
    #[arg(long)]
    use_stm_feature: bool,

    /// Session 15: replace the per-batch SGD update with Adam (sparse over
    /// active input-feature rows). When set, the per-step update on each
    /// shadow weight is `lr * m_hat / (sqrt(v_hat) + epsilon)`, which removes
    /// the per-layer magnitude bias that the `--output-grad-scale` /
    /// `--input-grad-scale` knobs were compensating for under SGD. Recommended
    /// config under Adam: `--learning-rate 0.05 --output-grad-scale 1.0
    /// --input-grad-scale 1.0` (Adam absorbs the scale; the SGD bumped 5×
    /// recommendation from Session 14 is unnecessary here).
    #[arg(long)]
    use_adam: bool,

    /// Adam first-moment decay (β1). Standard 0.9. Only meaningful when
    /// `--use-adam` is set.
    #[arg(long, default_value_t = 0.9)]
    adam_beta1: f32,

    /// Adam second-moment decay (β2). Standard 0.999.
    #[arg(long, default_value_t = 0.999)]
    adam_beta2: f32,

    /// Adam denominator stabiliser (ε). Standard 1e-8.
    #[arg(long, default_value_t = 1e-8)]
    adam_epsilon: f32,

    /// Session 16: switch the input encoding from the flat
    /// `(side × piece_type × square) = 2 × 14 × 81 = 2268`-feature space to
    /// the HalfKP-style `(own_king_sq × side × piece_type × square)` space
    /// (≈ 81× larger, 183_708 + 1 stm row = 183_709 input rows). The
    /// per-position active-feature count stays in the ~38 range — only the
    /// row indexing changes — so training time per epoch is only modestly
    /// higher than the flat path. Fresh init only: pre-Session-16 weight
    /// files have the wrong feature space and cannot be loaded under this
    /// flag. Recommended pairing: `--use-stm-feature --use-sigmoid-loss
    /// --use-adam --learning-rate 0.05` (the empirically-supported
    /// fresh-init Adam recipe from Session 15).
    #[arg(long)]
    use_halfkp: bool,

    /// Session 19: enable pieces-in-hand thermometer features. Adds 76 binary
    /// inputs (38 per side: P=18, L=4, N=4, S=4, G=4, B=2, R=2 levels) where
    /// `(player, piece_type, k)` is active iff player has `>= k` of that
    /// type in hand. Features are appended after the stm feature in either
    /// flat (rows = 2_345) or HalfKP (rows = 183_785) feature spaces. Fresh
    /// init only when row count needs to grow — pre-Session-19 weight files
    /// load with their existing rows and the trainer pads with zeros.
    #[arg(long)]
    use_hand_features: bool,
}

/// Compute Pearson correlation r between the network's cp evaluation and the
/// teacher's `eval_cp` over the first `sample_size` records that have a
/// non-null teacher eval. Pearson is invariant to linear scaling, so this is
/// directly comparable across `target_eval_scale` and `OUTPUT_DIVISOR`
/// settings — what it actually measures is whether the network *differentiates*
/// positions in a way that lines up with the teacher.
fn validation_pearson(
    weights: &NNUEWeights,
    records: &[CorpusRecord],
    sample_size: usize,
    use_stm_feature: bool,
    use_halfkp: bool,
    use_hand_features: bool,
) -> Option<f32> {
    if sample_size == 0 {
        return None;
    }
    let (h1, h2) = weights.hidden_sizes();
    let mut acc = NNUEAccumulator::new(h1, h2);
    let hand_base = if use_halfkp { HAND_FEATURE_BASE_HALFKP } else { HAND_FEATURE_BASE_FLAT };

    // Pearson is computed three ways to disentangle "the network can't learn"
    // from "the network learned but in the wrong POV":
    //   r_all  : all records, network output as-is vs to-move-POV teacher target
    //   r_blk  : Black-to-move records only (POV agreement is automatic here)
    //   r_wht  : White-to-move records, network output negated (assumes the
    //            network produces a Black-POV evaluation)
    // If r_blk or r_wht is materially > r_all, the network has learned a
    // board-absolute mapping but the trainer's to-move target is mixing the
    // signal across both colors. See Session 11 hand-off discussion.
    let mut preds_blk: Vec<f32> = Vec::new();
    let mut targets_blk: Vec<f32> = Vec::new();
    let mut preds_wht: Vec<f32> = Vec::new();
    let mut targets_wht: Vec<f32> = Vec::new();
    let mut total = 0;
    for rec in records.iter() {
        if total >= sample_size {
            break;
        }
        let teacher_cp = match rec.eval_cp {
            Some(cp) => cp,
            None => continue,
        };
        let (board, captured) = match board_from_sfen(&rec.sfen) {
            Some(b) => b,
            None => continue,
        };
        if use_halfkp {
            acc.refresh_halfkp(&board, rec.player, weights, use_stm_feature);
        } else if use_stm_feature {
            acc.refresh_with_stm(&board, rec.player, weights);
        } else {
            acc.refresh(&board, weights);
        }
        if use_hand_features {
            acc.add_hand_contributions(&captured, weights, hand_base);
        }
        let net_cp = acc.evaluate(weights) as f32;
        match rec.player {
            Player::Black => {
                preds_blk.push(net_cp);
                targets_blk.push(teacher_cp as f32);
            }
            Player::White => {
                preds_wht.push(-net_cp);
                targets_wht.push(teacher_cp as f32);
            }
        }
        total += 1;
    }
    if total < 2 {
        return None;
    }
    let mut preds_all: Vec<f32> = Vec::with_capacity(total);
    let mut targets_all: Vec<f32> = Vec::with_capacity(total);
    preds_all.extend(preds_blk.iter().copied());
    preds_all.extend(preds_wht.iter().copied());
    targets_all.extend(targets_blk.iter().copied());
    targets_all.extend(targets_wht.iter().copied());
    let r_all = pearson(&preds_all, &targets_all);
    let r_blk = if preds_blk.len() >= 2 { pearson(&preds_blk, &targets_blk) } else { 0.0 };
    let r_wht = if preds_wht.len() >= 2 { pearson(&preds_wht, &targets_wht) } else { 0.0 };
    println!(
        "    pearson  all={:+.3} (n={})  black-stm={:+.3} (n={})  white-stm-neg={:+.3} (n={})",
        r_all, total, r_blk, preds_blk.len(), r_wht, preds_wht.len()
    );
    Some(r_all)
}

fn pearson(xs: &[f32], ys: &[f32]) -> f32 {
    let n = xs.len() as f32;
    let mean_x = xs.iter().sum::<f32>() / n;
    let mean_y = ys.iter().sum::<f32>() / n;
    let mut num = 0.0_f32;
    let mut sx = 0.0_f32;
    let mut sy = 0.0_f32;
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        let dx = x - mean_x;
        let dy = y - mean_y;
        num += dx * dy;
        sx += dx * dx;
        sy += dy * dy;
    }
    if sx <= 0.0 || sy <= 0.0 {
        return 0.0;
    }
    num / (sx.sqrt() * sy.sqrt())
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
/// hand). Our `BitboardBoard::from_fen` already handles this. Session 19:
/// returns the parsed `CapturedPieces` alongside the board so the trainer's
/// hand-feature path has the data it needs.
fn board_from_sfen(sfen: &str) -> Option<(BitboardBoard, CapturedPieces)> {
    BitboardBoard::from_fen(sfen).ok().map(|(board, _p, cap)| (board, cap))
}

/// Compute the supervised target for a record, returning a target in either
/// [-1, 1] (legacy tanh path) or [0, 1] (Session 13 sigmoid path), from the
/// to-move player's perspective.
///
/// `target_eval_scale` controls the shape of the legacy teacher term:
///   * `scale > 0`: `teacher = tanh(clamped_eval / scale)`.
///   * `scale <= 0`: `teacher = clamped_eval / eval_clamp` (linear, no saturation).
///
/// When `use_sigmoid` is true, the target is `sigmoid(clamped_eval / sigmoid_eval_scale)`
/// blended with `outcome_to_move ∈ {-1, 0, 1}` re-mapped to `{0, 0.5, 1}` and
/// clamped to [0, 1]. The trainer's prediction map must be in matching units.
fn target_for(
    rec: &CorpusRecord,
    outcome_weight: f32,
    eval_clamp: i32,
    target_eval_scale: f32,
    use_sigmoid: bool,
    sigmoid_eval_scale: f32,
) -> Option<f32> {
    let cp = rec.eval_cp?;
    let clamped = cp.max(-eval_clamp).min(eval_clamp);
    let player_mult = if rec.player == Player::Black { 1.0_f32 } else { -1.0 };
    let outcome_tomove = rec.outcome as f32 * player_mult;

    if use_sigmoid {
        let teacher = 1.0 / (1.0 + (-clamped as f32 / sigmoid_eval_scale).exp());
        // {-1, 0, 1} → {0, 0.5, 1} so a draw maps to the sigmoid centre.
        let outcome_01 = (outcome_tomove + 1.0) * 0.5;
        let t = (1.0 - outcome_weight) * teacher + outcome_weight * outcome_01;
        Some(t.clamp(0.0, 1.0))
    } else {
        let teacher = if target_eval_scale > 0.0 {
            (clamped as f32 / target_eval_scale).tanh()
        } else {
            (clamped as f32 / eval_clamp as f32).clamp(-1.0, 1.0)
        };
        let t = (1.0 - outcome_weight) * teacher + outcome_weight * outcome_tomove;
        Some(t.clamp(-1.0, 1.0))
    }
}

/// Build a TrainingPosition from a corpus record (assumes target can be computed).
#[allow(clippy::too_many_arguments)]
fn build_position(
    rec: &CorpusRecord,
    weights: &NNUEWeights,
    outcome_weight: f32,
    eval_clamp: i32,
    target_eval_scale: f32,
    use_sigmoid: bool,
    sigmoid_eval_scale: f32,
    use_stm_feature: bool,
    use_halfkp: bool,
    use_hand_features: bool,
) -> Option<TrainingPosition> {
    let target = target_for(
        rec,
        outcome_weight,
        eval_clamp,
        target_eval_scale,
        use_sigmoid,
        sigmoid_eval_scale,
    )?;
    let (board, captured) = board_from_sfen(&rec.sfen)?;
    let active_features = if use_halfkp {
        if use_hand_features && use_stm_feature {
            extract_active_features_halfkp_with_stm_and_hand(&board, rec.player, &captured)
        } else if use_hand_features {
            extract_active_features_halfkp_with_hand(&board, rec.player, &captured)
        } else if use_stm_feature {
            extract_active_features_halfkp_with_stm(&board, rec.player)
        } else {
            extract_active_features_halfkp(&board, rec.player)
        }
    } else if use_hand_features && use_stm_feature {
        extract_active_features_with_stm_and_hand(&board, rec.player, &captured)
    } else if use_hand_features {
        extract_active_features_with_hand(&board, &captured)
    } else if use_stm_feature {
        extract_active_features_with_stm(&board, rec.player)
    } else {
        extract_active_features(&board)
    };
    // Skip records whose own king is missing (HalfKP returns empty in that
    // case; non-halfkp paths don't depend on a king and produce features
    // for any board). The trainer's per-batch path silently skips empty
    // active_features but the network still learns nothing from it, so
    // we drop the record outright.
    if use_halfkp && active_features.is_empty() {
        return None;
    }
    let (h1, h2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(h1, h2);
    let hand_base = if use_halfkp { HAND_FEATURE_BASE_HALFKP } else { HAND_FEATURE_BASE_FLAT };
    if use_halfkp {
        accumulator.refresh_halfkp(&board, rec.player, weights, use_stm_feature);
    } else if use_stm_feature {
        accumulator.refresh_with_stm(&board, rec.player, weights);
    } else {
        accumulator.refresh(&board, weights);
    }
    if use_hand_features {
        accumulator.add_hand_contributions(&captured, weights, hand_base);
    }
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
    println!(
        "  Target scale:   {} ({})",
        cli.target_eval_scale,
        if cli.target_eval_scale > 0.0 {
            "tanh(eval/scale)"
        } else {
            "linear clamped"
        }
    );
    println!("  Seed:           {}", cli.seed);
    println!("  f32 input fwd:  {}", cli.f32_input_forward);
    println!(
        "  Decisive wt:    {}× when |teacher cp| > {}",
        cli.decisive_weight, cli.decisive_threshold_cp
    );
    if cli.use_sigmoid_loss {
        println!(
            "  Loss:           sigmoid(eval/{}) + L2 on [0,1] target (Session 13)",
            cli.sigmoid_eval_scale
        );
    } else {
        println!("  Loss:           tanh + L2 on [-1,1] target (legacy)");
    }
    println!(
        "  STM feature:    {} (Session 14)",
        if cli.use_stm_feature { "ON (active for Black-to-move)" } else { "off" }
    );
    println!(
        "  HalfKP:         {} (Session 16)",
        if cli.use_halfkp { "ON (own_king_sq × side × piece_type × sq)" } else { "off" }
    );
    println!(
        "  Hand features:  {} (Session 19)",
        if cli.use_hand_features {
            "ON (thermometer 38 levels × 2 sides = 76)"
        } else {
            "off"
        }
    );
    if cli.use_adam {
        println!(
            "  Optimiser:      Adam (β1={}, β2={}, ε={}) (Session 15)",
            cli.adam_beta1, cli.adam_beta2, cli.adam_epsilon
        );
    } else {
        println!("  Optimiser:      SGD (per-layer grad-scale)");
    }
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

    let target_rows = match (cli.use_halfkp, cli.use_hand_features) {
        (true, true) => {
            shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_HALFKP_WITH_HAND_TOTAL
        }
        (true, false) => shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_HALFKP_TOTAL,
        (false, true) => {
            shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_WITH_HAND_TOTAL
        }
        (false, false) => shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_TOTAL,
    };
    let initial_weights = match &cli.init_weights {
        Some(p) => {
            println!("Loading initial weights from {}", p.display());
            let mut w = NNUEWeights::load(p)?;
            if cli.use_halfkp
                && w.input_weights_1.len()
                    < shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_HALFKP_TOTAL
            {
                return Err(format!(
                    "--use-halfkp requires HalfKP-sized weights (rows >= {}); loaded file has {} rows",
                    shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_HALFKP_TOTAL,
                    w.input_weights_1.len()
                )
                .into());
            }
            // Session 19: pad to the hand-feature target with zero rows so
            // the appended hand-feature region is addressable. Pre-Session-19
            // files keep their existing weights bit-exact for the original
            // feature region.
            let current = w.input_weights_1.len();
            if cli.use_hand_features && current < target_rows {
                let row_len = w
                    .input_weights_1
                    .first()
                    .map(|r| r.len())
                    .unwrap_or(cli.hidden_1);
                println!(
                    "  Padding loaded weights from {} to {} rows for hand features (76 zero rows)",
                    current, target_rows
                );
                for _ in current..target_rows {
                    w.input_weights_1.push(vec![0i16; row_len]);
                }
            }
            w
        }
        None => {
            if cli.use_halfkp {
                println!(
                    "Initialising fresh random HalfKP weights ({} input rows{})",
                    target_rows,
                    if cli.use_hand_features { " incl. hand features" } else { "" }
                );
                let mut w = NNUEWeights::new_halfkp(cli.hidden_1, cli.hidden_2);
                if cli.use_hand_features {
                    let row_len = cli.hidden_1;
                    while w.input_weights_1.len() < target_rows {
                        w.input_weights_1.push(vec![0i16; row_len]);
                    }
                }
                w
            } else {
                println!(
                    "Initialising fresh random flat weights ({} input rows{})",
                    target_rows,
                    if cli.use_hand_features { " incl. hand features" } else { "" }
                );
                let mut w = NNUEWeights::new(cli.hidden_1, cli.hidden_2);
                if cli.use_hand_features {
                    let row_len = cli.hidden_1;
                    while w.input_weights_1.len() < target_rows {
                        w.input_weights_1.push(vec![0i16; row_len]);
                    }
                }
                w
            }
        }
    };

    let mut config = NNUETrainingConfig::default();
    config.learning_rate = cli.learning_rate;
    config.output_grad_scale = cli.output_grad_scale;
    config.input_grad_scale = cli.input_grad_scale;
    config.f32_input_forward = cli.f32_input_forward;
    config.decisive_weight = cli.decisive_weight;
    config.decisive_threshold_cp = cli.decisive_threshold_cp;
    config.use_sigmoid_loss = cli.use_sigmoid_loss;
    config.sigmoid_eval_scale = cli.sigmoid_eval_scale;
    config.use_adam = cli.use_adam;
    config.adam_beta1 = cli.adam_beta1;
    config.adam_beta2 = cli.adam_beta2;
    config.adam_epsilon = cli.adam_epsilon;
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
                cli.target_eval_scale,
                cli.use_sigmoid_loss,
                cli.sigmoid_eval_scale,
                cli.use_stm_feature,
                cli.use_halfkp,
                cli.use_hand_features,
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

        let pearson = validation_pearson(
            trainer.get_weights(),
            &records,
            cli.validate_sample,
            cli.use_stm_feature,
            cli.use_halfkp,
            cli.use_hand_features,
        );
        let pearson_str = match pearson {
            Some(r) => format!(" pearson_r={:+.3}", r),
            None => String::new(),
        };
        println!(
            "Epoch {:3}/{:3}: {:6} pos, {:4} batches, avg td_err={:.4}{}, t={:.1}s",
            epoch + 1,
            total_epochs,
            total_positions,
            epoch_batches,
            epoch_avg_td,
            pearson_str,
            epoch_start.elapsed().as_secs_f64()
        );

        if cli.checkpoint_every > 0 && (epoch + 1) % cli.checkpoint_every == 0 {
            let stem = cli.output_weights.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "nnue_weights".into());
            let parent = cli.output_weights.parent().unwrap_or(Path::new("."));
            // Session 19: preserve the output extension (default `.bin`) for
            // checkpoints — previously hard-coded to `.json`, which silently
            // wrote JSON checkpoints even when the final output was bincode.
            let ext = cli
                .output_weights
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_else(|| "bin".into());
            let ckpt = parent.join(format!("{}_epoch_{}.{}", stem, epoch + 1, ext));
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
