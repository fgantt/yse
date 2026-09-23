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
    score_pov_scratch, NNUEAccumulator, NNUEFeatureSet, NNUEWeights, HAND_FEATURE_BASE_FLAT,
    HAND_FEATURE_BASE_HALFKP,
};
use shogi_engine::evaluation::nnue_training::{
    RankConfig,
    extract_active_features, extract_active_features_halfkp,
    extract_active_features_halfkp_with_hand, extract_active_features_halfkp_with_stm,
    extract_active_features_halfkp_with_stm_and_hand, extract_active_features_with_hand,
    extract_features_frame,
    extract_active_features_with_stm, extract_active_features_with_stm_and_hand, NNUETrainer,
    NNUETrainingConfig, RankingGroup, TrainingPosition,
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

    /// Normalise features to the side-to-move's perspective: when White is to
    /// move, rotate the board 180° and swap piece colours so that a position and
    /// its colour-mirror produce identical features. Without this the encoding is
    /// colour-absolute while the target is to-move-POV, so the network must learn
    /// each colour as an independent problem from half the corpus — measured as a
    /// held-out Pearson split of 0.525 (Black) vs 0.378 (White).
    ///
    /// Supersedes `--use-stm-feature` (the bit becomes constant once normalised,
    /// and a colour-conditional bit would undo the symmetry). Flat space only for
    /// now; rejected together with `--use-halfkp`.
    ///
    /// See `docs/nnue-phase2/FEATURE_SPEC_AND_AUDIT.md` Finding 2.
    #[arg(long)]
    use_pov_features: bool,

    /// Stop training after this many consecutive validation passes without a new
    /// best held-out score. 0 disables early stopping (the full `--epochs` run).
    ///
    /// Worth setting: on the current corpus held-out r peaks around epoch 10 and
    /// then drifts *down*, so a 30-epoch run is not merely slower, it ends on a
    /// worse network than it passed through.
    #[arg(long, default_value_t = 0)]
    early_stop_patience: u32,

    /// Give the network a PSQT path initialised to material values: a centipawn
    /// value per input feature, summed over the active features and added to the
    /// output. Requires `--use-pov-features`.
    ///
    /// With `--psqt-lr 0` (the default) the path stays frozen at material, so the
    /// network learns only `teacher - material`. That is the point: trained on
    /// the teacher score alone, the network learned almost no material (removing
    /// an opponent's pawn gained it +1 cp, a rook +191), because strong-engine
    /// games rarely leave material hanging, and it gave pieces away in play.
    /// Ignored when `--init-weights` already has a PSQT path.
    #[arg(long)]
    psqt_material: bool,

    /// Learning rate for the PSQT path, in centipawns per Adam step. 0 freezes it.
    #[arg(long, default_value_t = 0.0)]
    psqt_lr: f32,

    /// Train only on quiet positions: drop records where the side to move is in
    /// check or the teacher's move was a capture. Validation still uses every
    /// held-out record, so scores stay comparable with runs without the filter.
    #[arg(long)]
    skip_noisy: bool,

    /// Condition each perspective's features on its own king square (81 buckets
    /// of the POV feature space). A POV network passed as `--init-weights` is
    /// copied into every bucket, so training starts from the same function.
    /// Requires `--use-pov-features --use-hand-features`.
    #[arg(long)]
    king_features: bool,

    /// Layer 2 reads both perspectives' accumulators (side to move, then
    /// opponent). A single-perspective `--init-weights` network is extended with
    /// a zeroed opponent half, so training starts from the same function.
    /// Requires `--use-pov-features`.
    #[arg(long)]
    dual_perspective: bool,

    /// With `--king-features`: train each king-conditioned weight as a
    /// king-specific part plus a part shared by all 81 king squares (the
    /// "factoriser" of modern NNUE trainers). The shared part learns from every
    /// position, so rarely seen king squares still get good weights; the two are
    /// summed into the saved network, so the engine is unaffected.
    #[arg(long)]
    king_factorizer: bool,

    /// King buckets for `--king-features`: 81 (one per square) or 9 (3 file
    /// groups × home rank / second rank / further up).
    #[arg(long, default_value_t = 81)]
    king_buckets: usize,

    /// With `--king-factorizer`: per-batch decay of each touched king-specific
    /// correction towards zero (decoupled, AdamW-style), i.e. of each bucket
    /// towards the shared weights. 0 = off.
    #[arg(long, default_value_t = 0.0)]
    king_delta_decay: f32,

    /// Sibling-ranking corpus from `rank-gen`: one group of scored legal moves
    /// per line. Trains the network to put a position's moves in the teacher's
    /// order, which is the question a search asks — as opposed to the value
    /// corpus, which only asks what a position is worth.
    ///
    /// Session 28 measured the difference: r = 0.81 against the teacher on
    /// position values, while choosing moves at roughly twice the teacher's
    /// error. Used alongside `--corpus`, not instead of it.
    #[arg(long)]
    rank_corpus: Option<PathBuf>,

    /// Weight of the ranking term against the value term. 0 disables ranking.
    #[arg(long, default_value_t = 1.0)]
    rank_weight: f32,

    /// Softmax temperature in centipawns, for both the teacher's target
    /// distribution and the network's. Lower = the target concentrates harder
    /// on the single best move; it also scales the gradient by 1/T.
    #[arg(long, default_value_t = 100.0)]
    rank_temperature: f32,

    /// Clamp sibling scores to +/- this before the softmax. A move that loses
    /// and a move that loses to forced mate are not a distinction worth a large
    /// gradient, and unclamped mate scores would make the target one-hot.
    #[arg(long, default_value_t = 2000)]
    rank_clamp: i32,

    /// Ranking groups per batch, alongside `--batch-size` value positions.
    #[arg(long, default_value_t = 32)]
    rank_groups_per_batch: usize,

    /// Use a pairwise hinge instead of softmax cross-entropy.
    ///
    /// Softmax CE can be minimised by lowering confidence rather than by
    /// reordering: measured over 3 epochs it moved |p - t| 0.1259 -> 0.1208
    /// against a control where that rose, while top-1 agreement stayed flat at
    /// 0.29 and one-ply move loss did not improve. The hinge is zero once a
    /// pair is correctly ordered by the margin, so shrinking the spread makes
    /// it worse, not better.
    #[arg(long)]
    rank_pairwise: bool,

    /// Largest score gap the hinge will ask our network to reproduce, in
    /// centipawns. Uncapped, a pair the teacher separates by 2000 cp would
    /// demand our gap match it, which is value regression in disguise.
    #[arg(long, default_value_t = 400.0)]
    rank_margin_cap: f32,
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
    eval_clamp: i32,
    use_pov: bool,
    use_king: bool,
) -> Option<f32> {
    if sample_size == 0 {
        return None;
    }
    let (h1, h2) = weights.hidden_sizes();
    let mut acc = NNUEAccumulator::new(h1, h2);
    let hand_base = if use_halfkp { HAND_FEATURE_BASE_HALFKP } else { HAND_FEATURE_BASE_FLAT };

    // The network is trained against `clamp(eval_cp, +/-eval_clamp)`, so that is
    // what the headline metric scores it against. Scoring against the *raw*
    // teacher cp — as this diagnostic did through Session 20 — imposes a hard
    // ceiling, because the mate tail (|cp| > 20000, ~8% of the corpus) carries
    // ~97% of the variance in raw cp and is clamped away during training. On the
    // YaneuraOu d10 corpus at eval_clamp = 2000 that ceiling is r = 0.608, which
    // is exactly where Sessions 15-20 "plateaued". See
    // `docs/nnue-phase2/FEATURE_SPEC_AND_AUDIT.md`.
    //
    // Reported per validation pass:
    //   pearson[clamp] : vs the clamped target the network actually optimises.
    //                    Ceiling 1.0. This is the headline and the return value.
    //   spearman[raw]  : rank correlation vs raw teacher cp. Rank-based, so the
    //                    mate tail cannot dominate it; measures whether the
    //                    network *orders* positions like the teacher, which is
    //                    what matters for move selection.
    //   pearson[raw]   : the legacy Session 14-20 number, kept for continuity,
    //                    printed next to its own ceiling so it cannot mislead.
    //
    // POV is resolved once, from pearson[clamp], by taking whichever of the
    // board-absolute and to-move-POV aggregations has larger magnitude (a network
    // can converge to either fixed point — S19 Issue 6). Every other metric then
    // uses that same orientation, so the whole line is self-consistent.
    let mut preds_blk: Vec<f32> = Vec::new();
    let mut targets_blk: Vec<f32> = Vec::new();
    let mut targets_blk_clamped: Vec<f32> = Vec::new();
    let mut preds_wht_pov: Vec<f32> = Vec::new();
    let mut targets_wht: Vec<f32> = Vec::new();
    let mut targets_wht_clamped: Vec<f32> = Vec::new();
    let mut n_clamped = 0usize;
    let mut total = 0;
    // Sample across the whole validation set, not its first `sample_size`
    // records. A combined corpus is concatenated in blocks (one per generation
    // run), so the first records are all one block: Session 28 mixed self-play
    // games in ahead of the teacher games and every validation number was then
    // computed on self-play alone, which sent early stopping to the wrong epoch.
    let stride = (records.len() / sample_size.max(1)).max(1);
    for rec in records.iter().step_by(stride) {
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
        if use_pov {
            // Handled below by `score_pov_scratch`, the engine's own reference.
        } else if use_halfkp {
            acc.refresh_halfkp(&board, rec.player, weights, use_stm_feature);
        } else if use_stm_feature {
            acc.refresh_with_stm(&board, rec.player, weights);
        } else {
            acc.refresh(&board, weights);
        }
        if use_hand_features && !use_pov {
            acc.add_hand_contributions(&captured, weights, hand_base);
        }
        let net_cp = if use_pov {
            score_pov_scratch(weights, use_king, use_hand_features, &board, rec.player, &captured)
                as f32
        } else {
            acc.evaluate(weights) as f32
        };
        let clamped = teacher_cp.max(-eval_clamp).min(eval_clamp);
        if clamped != teacher_cp {
            n_clamped += 1;
        }
        match rec.player {
            Player::Black => {
                preds_blk.push(net_cp);
                targets_blk.push(teacher_cp as f32);
                targets_blk_clamped.push(clamped as f32);
            }
            Player::White => {
                preds_wht_pov.push(net_cp);
                targets_wht.push(teacher_cp as f32);
                targets_wht_clamped.push(clamped as f32);
            }
        }
        total += 1;
    }
    if total < 2 {
        return None;
    }
    let mut targets_raw: Vec<f32> = Vec::with_capacity(total);
    targets_raw.extend(targets_blk.iter().copied());
    targets_raw.extend(targets_wht.iter().copied());
    let mut targets_clamped: Vec<f32> = Vec::with_capacity(total);
    targets_clamped.extend(targets_blk_clamped.iter().copied());
    targets_clamped.extend(targets_wht_clamped.iter().copied());

    // Resolve POV once, against the clamped target.
    let mut preds_pov: Vec<f32> = Vec::with_capacity(total);
    preds_pov.extend(preds_blk.iter().copied());
    preds_pov.extend(preds_wht_pov.iter().copied());
    let mut preds_abs: Vec<f32> = Vec::with_capacity(total);
    preds_abs.extend(preds_blk.iter().copied());
    preds_abs.extend(preds_wht_pov.iter().map(|p| -p));

    let clamp_pov = pearson(&preds_pov, &targets_clamped);
    let clamp_abs = pearson(&preds_abs, &targets_clamped);
    let (preds, pov_label, is_pov) = if clamp_pov.abs() >= clamp_abs.abs() {
        (&preds_pov, "to-move-POV", true)
    } else {
        (&preds_abs, "board-absolute", false)
    };

    let r_clamped = pearson(preds, &targets_clamped);
    let rho_raw = spearman(preds, &targets_raw);
    let r_raw = pearson(preds, &targets_raw);
    // The best any network trained on the clamped target can score against raw cp.
    let r_raw_ceiling = pearson(&targets_clamped, &targets_raw);

    let r_blk = if preds_blk.len() >= 2 {
        pearson(&preds_blk, &targets_blk_clamped)
    } else {
        0.0
    };
    let r_wht = if preds_wht_pov.len() >= 2 {
        let p: Vec<f32> = if is_pov {
            preds_wht_pov.clone()
        } else {
            preds_wht_pov.iter().map(|p| -p).collect()
        };
        pearson(&p, &targets_wht_clamped)
    } else {
        0.0
    };

    println!(
        "    pearson[clamp] all={:+.3} ({})  blk={:+.3} (n={})  wht={:+.3} (n={})",
        r_clamped,
        pov_label,
        r_blk,
        preds_blk.len(),
        r_wht,
        preds_wht_pov.len()
    );
    println!(
        "    spearman[raw]  all={:+.3}   legacy pearson[raw]={:+.3} (ceiling {:.3})  clamped={:.1}%",
        rho_raw,
        r_raw,
        r_raw_ceiling,
        100.0 * n_clamped as f32 / total as f32
    );
    Some(r_clamped)
}

/// Average-rank transform, ties sharing their mean rank. Used for Spearman.
fn ranks(xs: &[f32]) -> Vec<f32> {
    let n = xs.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![0.0f32; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && xs[idx[j + 1]] == xs[idx[i]] {
            j += 1;
        }
        let mean_rank = (i + j) as f32 / 2.0 + 1.0;
        for k in i..=j {
            out[idx[k]] = mean_rank;
        }
        i = j + 1;
    }
    out
}

/// Spearman rank correlation: Pearson over average ranks. Unlike Pearson on raw
/// cp, this cannot be dominated by the corpus's mate tail.
fn spearman(xs: &[f32], ys: &[f32]) -> f32 {
    pearson(&ranks(xs), &ranks(ys))
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
    /// Source self-play game. Used to split train/validation at game granularity:
    /// adjacent plies of one game are near-identical positions, so a record-level
    /// split would leak training positions into validation and inflate the score.
    game_id: u32,
    ply: u32,
    /// The move played from this position was random, not the teacher's
    /// (`corpus-gen --random-move-prob`), so the next record says nothing about
    /// the teacher's best move here.
    random_move: bool,
    /// Set by [`noisy_flags`] under `--skip-noisy`.
    noisy: bool,
}

/// Hand-piece count of `player` in an SFEN's hand field (`-` for none).
fn hand_count(sfen: &str, player: Player) -> u32 {
    let hand = sfen.split_whitespace().nth(2).unwrap_or("-");
    let (mut total, mut n) = (0u32, 0u32);
    for c in hand.chars() {
        if let Some(d) = c.to_digit(10) {
            n = n * 10 + d;
            continue;
        }
        let owner = if c.is_ascii_uppercase() { Player::Black } else { Player::White };
        if c != '-' && owner == player {
            total += n.max(1);
        }
        n = 0;
    }
    total
}

/// Records whose position is not quiet: the side to move is in check, or the
/// teacher's move from it was a capture (the mover's hand grew by the next ply).
///
/// A static evaluator is only asked about quiet positions (quiescence resolves
/// captures first), and in a non-quiet position the teacher's score already
/// includes the capture about to happen, which the material on the board does
/// not show. Training on such positions teaches the network to discount
/// material. Standard NNUE trainers skip them for the same reason.
fn noisy_flags(records: &[CorpusRecord]) -> Vec<bool> {
    let mut noisy = vec![false; records.len()];
    for i in 0..records.len() {
        let r = &records[i];
        if let Some((board, captured)) = board_from_sfen(&r.sfen) {
            if board.is_king_in_check(r.player, &captured) {
                noisy[i] = true;
                continue;
            }
        }
        if let Some(next) = records.get(i + 1) {
            if !r.random_move
                && next.game_id == r.game_id
                && next.ply == r.ply + 1
                && hand_count(&next.sfen, r.player) > hand_count(&r.sfen, r.player)
            {
                noisy[i] = true;
            }
        }
    }
    noisy
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

    let gid_raw = find_field(line, "game_id")?.trim_start();
    let gid_end = gid_raw
        .find(|c: char| c == ',' || c == '}')
        .unwrap_or(gid_raw.len());
    let game_id = gid_raw[..gid_end].trim().parse::<u32>().ok()?;

    let ply = find_field(line, "ply")
        .and_then(|v| v.trim().split(|c| c == ',' || c == '}').next()?.trim().parse().ok())
        .unwrap_or(0);
    let random_move = find_field(line, "random_move").map_or(false, |v| v.trim_start().starts_with("true"));

    Some(CorpusRecord { sfen, player, eval_cp, outcome, game_id, ply, random_move, noisy: false })
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
/// One position's legal moves with the teacher's score for each, as written by
/// `rank-gen`. `children[i]` is the position after move i, so its side to move
/// is the opponent; `scores[i]` is from the *moving* side's point of view.
struct RankRecord {
    children: Vec<String>,
    scores: Vec<f32>,
    /// Side to move in the parent. Each child's perspective is its opposite.
    player: Player,
}

/// Load a ranking corpus, clamping sibling scores.
///
/// Scores are clamped because `rank-gen` maps mate to +/-30000: left alone, a
/// single mating sibling makes the softmax target exactly one-hot and swamps
/// every other move in the group.
fn load_rank_corpus(path: &Path, clamp: i32) -> std::io::Result<Vec<RankRecord>> {
    let reader = BufReader::new(File::open(path)?);
    let clamp = clamp.max(1) as f32;
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let player = match v.get("player").and_then(|p| p.as_str()) {
            Some("black") => Player::Black,
            Some("white") => Player::White,
            _ => continue,
        };
        let (Some(children), Some(scores)) = (
            v.get("children").and_then(|c| c.as_array()),
            v.get("scores").and_then(|s| s.as_array()),
        ) else {
            continue;
        };
        if children.len() != scores.len() || children.len() < 2 {
            continue;
        }
        let children: Vec<String> =
            children.iter().filter_map(|c| c.as_str().map(str::to_string)).collect();
        let scores: Vec<f32> = scores
            .iter()
            .filter_map(|s| s.as_i64())
            .map(|s| (s as f32).clamp(-clamp, clamp))
            .collect();
        if children.len() != scores.len() {
            continue;
        }
        out.push(RankRecord { children, scores, player });
    }
    Ok(out)
}

/// Take the next `n` ranking groups round-robin and turn each child SFEN into a
/// `TrainingPosition`.
///
/// Children are built fresh against the current weights every time a group is
/// used, because their accumulators must reflect the network as it is now — the
/// value path does the same for its positions. A child whose SFEN will not parse
/// is dropped, and a group left with fewer than two usable children is skipped:
/// one move on its own carries no ordering information.
fn next_rank_groups(
    records: &[RankRecord],
    cursor: &mut usize,
    n: usize,
    trainer: &NNUETrainer,
    cli: &Cli,
) -> Vec<RankingGroup> {
    if records.is_empty() || n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let rec = &records[*cursor % records.len()];
        *cursor += 1;
        let child_player = rec.player.opposite();
        let mut children = Vec::with_capacity(rec.children.len());
        let mut scores = Vec::with_capacity(rec.scores.len());
        for (sfen, &score) in rec.children.iter().zip(rec.scores.iter()) {
            if let Some(pos) = build_position_at(
                sfen,
                child_player,
                None,
                0,
                trainer.get_weights(),
                cli.use_stm_feature,
                cli.use_halfkp,
                cli.use_hand_features,
                cli.use_pov_features,
                cli.king_features,
            ) {
                children.push(pos);
                scores.push(score);
            }
        }
        if children.len() >= 2 {
            out.push(RankingGroup { children, scores });
        }
    }
    out
}

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

/// The network's king-bucket count (0 when not king-conditioned).
fn king_buckets(weights: &NNUEWeights, use_king: bool) -> usize {
    if use_king {
        weights.input_weights_1.len() / shogi_engine::evaluation::nnue::KING_FEATURE_STRIDE
    } else {
        0
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
    use_pov: bool,
    use_king: bool,
) -> Option<TrainingPosition> {
    let target = target_for(
        rec,
        outcome_weight,
        eval_clamp,
        target_eval_scale,
        use_sigmoid,
        sigmoid_eval_scale,
    )?;
    build_position_at(
        &rec.sfen,
        rec.player,
        Some(target),
        rec.eval_cp.unwrap_or(0),
        weights,
        use_stm_feature,
        use_halfkp,
        use_hand_features,
        use_pov,
        use_king,
    )
}

/// Build a `TrainingPosition` for one board, with the supervised target already
/// decided (or absent).
///
/// Split out of `build_position` because ranking groups need the same feature
/// extraction and accumulator setup for each child position, but have no value
/// target of their own — their signal is entirely in how they compare to their
/// siblings.
#[allow(clippy::too_many_arguments)]
fn build_position_at(
    sfen: &str,
    player: Player,
    target: Option<f32>,
    pst_evaluation: i32,
    weights: &NNUEWeights,
    use_stm_feature: bool,
    use_halfkp: bool,
    use_hand_features: bool,
    use_pov: bool,
    use_king: bool,
) -> Option<TrainingPosition> {
    let (board, captured) = board_from_sfen(sfen)?;
    let active_features = if use_pov {
        // Perspective normalisation supersedes the stm bit: the mover is always
        // "Black" in the normalised frame, so the bit carries no information.
        extract_features_frame(&board, player, &captured, use_hand_features, king_buckets(weights, use_king))
    } else if use_halfkp {
        if use_hand_features && use_stm_feature {
            extract_active_features_halfkp_with_stm_and_hand(&board, player, &captured)
        } else if use_hand_features {
            extract_active_features_halfkp_with_hand(&board, player, &captured)
        } else if use_stm_feature {
            extract_active_features_halfkp_with_stm(&board, player)
        } else {
            extract_active_features_halfkp(&board, player)
        }
    } else if use_hand_features && use_stm_feature {
        extract_active_features_with_stm_and_hand(&board, player, &captured)
    } else if use_hand_features {
        extract_active_features_with_hand(&board, &captured)
    } else if use_stm_feature {
        extract_active_features_with_stm(&board, player)
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
    accumulator.king_mode = use_king;
    let hand_base = if use_halfkp { HAND_FEATURE_BASE_HALFKP } else { HAND_FEATURE_BASE_FLAT };
    if use_pov {
        accumulator.refresh_pov(&board, player, weights);
    } else if use_halfkp {
        accumulator.refresh_halfkp(&board, player, weights, use_stm_feature);
    } else if use_stm_feature {
        accumulator.refresh_with_stm(&board, player, weights);
    } else {
        accumulator.refresh(&board, weights);
    }
    if use_hand_features {
        if use_pov {
            accumulator.add_hand_contributions_pov(&captured, player, weights, hand_base);
        } else {
            accumulator.add_hand_contributions(&captured, weights, hand_base);
        }
    }
    // Two-perspective networks also need the opponent's view of the position.
    let (active_features_nstm, accumulator_nstm) = if use_pov && weights.is_dual() {
        let other = player.opposite();
        let mut acc = NNUEAccumulator::new(h1, h2);
        acc.king_mode = use_king;
        acc.refresh_pov(&board, other, weights);
        if use_hand_features {
            acc.add_hand_contributions_pov(&captured, other, weights, hand_base);
        }
        (
            extract_features_frame(
                &board,
                other,
                &captured,
                use_hand_features,
                king_buckets(weights, use_king),
            ),
            Some(acc),
        )
    } else {
        (Vec::new(), None)
    };
    let nnue_eval = match &accumulator_nstm {
        Some(n) => NNUEAccumulator::evaluate_dual(&accumulator, n, weights),
        None => accumulator.evaluate(weights),
    };
    Some(TrainingPosition {
        active_features,
        accumulator,
        evaluation: nnue_eval,
        pst_evaluation,
        player: player,
        move_made: None,
        outcome: None,
        td_target: target,
        active_features_nstm,
        accumulator_nstm,
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

    if cli.use_pov_features && cli.use_halfkp {
        return Err(
            "--use-pov-features is flat-space only for now; HalfKP would also need its \
             own_king_sq mirrored. Run them separately."
                .into(),
        );
    }
    if cli.use_pov_features && cli.use_stm_feature {
        println!(
            "note: --use-stm-feature is ignored under --use-pov-features (the mover is always \
             \"Black\" in the normalised frame, so the bit is constant)"
        );
    }

    let mut all_records = load_corpus(&cli.corpus, cli.skip_null_eval)?;
    if cli.skip_noisy {
        let flags = noisy_flags(&all_records);
        for (r, f) in all_records.iter_mut().zip(flags) {
            r.noisy = f;
        }
    }
    println!("Loaded {} records", all_records.len());
    if all_records.is_empty() {
        return Err("no usable records in corpus".into());
    }

    let mut rank_records: Vec<RankRecord> = match cli.rank_corpus.as_ref() {
        Some(p) => {
            let r = load_rank_corpus(p, cli.rank_clamp)?;
            let siblings: usize = r.iter().map(|g| g.children.len()).sum();
            println!(
                "Loaded {} ranking groups ({:.2} siblings each) from {}",
                r.len(),
                siblings as f64 / r.len().max(1) as f64,
                p.display()
            );
            println!(
                "  rank {} weight {} temperature {} cp, clamp +/-{} cp, {} groups/batch",
                if cli.rank_pairwise { "pairwise-hinge" } else { "softmax-ce" },
                cli.rank_weight, cli.rank_temperature, cli.rank_clamp, cli.rank_groups_per_batch
            );
            r
        }
        None => Vec::new(),
    };
    let rank_cfg = RankConfig {
        weight: cli.rank_weight,
        temperature: cli.rank_temperature,
        pairwise: cli.rank_pairwise,
        margin_cap: cli.rank_margin_cap,
    };

    // Hold out a slice of ranking groups, never trained on. Held-out ranking
    // error is the only per-epoch number measured directly on the quantity this
    // objective exists to improve -- Session 28's lesson was that held-out
    // Pearson r on position values misranks checkpoints, so it cannot be the
    // gate here.
    let rank_val: Vec<RankRecord> = if rank_records.len() > 2000 {
        let keep = rank_records.len() - (rank_records.len() / 20).max(500);
        let v = rank_records.split_off(keep);
        println!("  held out {} ranking groups for validation", v.len());
        v
    } else {
        Vec::new()
    };

    // Hold out every `VALIDATION_GAME_STRIDE`-th game for validation. Splitting by
    // game (not by record) keeps adjacent plies of the same game on one side of
    // the split — a record-level split leaks near-identical positions into
    // validation and reports a training-fit score as if it were generalisation.
    const VALIDATION_GAME_STRIDE: u32 = 20;
    let (mut records, val_records): (Vec<_>, Vec<_>) = std::mem::take(&mut all_records)
        .into_iter()
        .partition(|r| r.game_id % VALIDATION_GAME_STRIDE != 0);
    if cli.skip_noisy {
        let before = records.len();
        records.retain(|r| !r.noisy);
        println!(
            "  Quiet filter: dropped {} of {} training records (in check, or teacher captured)",
            before - records.len(),
            before
        );
    }
    if val_records.is_empty() {
        return Err("validation split is empty — corpus has too few distinct games".into());
    }
    println!(
        "  Train:        {} records   Held-out validation: {} records ({} of every {} games)",
        records.len(),
        val_records.len(),
        1,
        VALIDATION_GAME_STRIDE
    );

    let mut rng = if cli.seed == 0 {
        StdRng::from_entropy()
    } else {
        StdRng::seed_from_u64(cli.seed)
    };
    // Fresh weight init previously always drew from the thread RNG, so `--seed`
    // controlled only shuffling and no fresh-init run was actually reproducible.
    // Offset the init stream so init and shuffling do not share a sequence.
    let init_seed = cli.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);

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
                let mut w = NNUEWeights::new_with_features_seeded(
                    shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_HALFKP_TOTAL,
                    cli.hidden_1,
                    cli.hidden_2,
                    init_seed,
                );
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
                let mut w = NNUEWeights::new_with_features_seeded(
                    shogi_engine::evaluation::nnue::NUM_NNUE_FEATURES_TOTAL,
                    cli.hidden_1,
                    cli.hidden_2,
                    init_seed,
                );
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

    let mut initial_weights = initial_weights;
    if (cli.king_features || cli.dual_perspective)
        && !(cli.use_pov_features && cli.use_hand_features)
    {
        return Err("--king-features / --dual-perspective need --use-pov-features \
                    --use-hand-features"
            .into());
    }
    if cli.psqt_material && initial_weights.psqt.is_empty() {
        if !cli.use_pov_features {
            return Err("--psqt-material needs --use-pov-features (material is scored from the \
                        mover's side, which only the POV encoding expresses)"
                .into());
        }
        initial_weights.psqt = NNUEWeights::material_psqt(initial_weights.input_weights_1.len());
    }
    if cli.king_features {
        initial_weights.tile_for_king(cli.king_buckets);
        if initial_weights.input_weights_1.len()
            != cli.king_buckets * shogi_engine::evaluation::nnue::KING_FEATURE_STRIDE
        {
            return Err(format!(
                "--king-features: weights have {} input rows, expected {} (or a {}-row POV \
                 network to expand)",
                initial_weights.input_weights_1.len(),
                cli.king_buckets * shogi_engine::evaluation::nnue::KING_FEATURE_STRIDE,
                shogi_engine::evaluation::nnue::KING_FEATURE_STRIDE
            )
            .into());
        }
    }
    if cli.dual_perspective {
        initial_weights.make_dual();
    }
    println!(
        "  Architecture: {} input rows, {}{} -> {} -> 1",
        initial_weights.input_weights_1.len(),
        initial_weights.hidden_sizes().0,
        if initial_weights.is_dual() { "x2 (both perspectives)" } else { "" },
        initial_weights.hidden_sizes().1
    );
    println!(
        "  PSQT path:    {}",
        match (initial_weights.psqt.is_empty(), cli.psqt_lr > 0.0) {
            (true, _) => "none".to_string(),
            (false, false) => "frozen".to_string(),
            (false, true) => format!("trained, lr {} cp/step", cli.psqt_lr),
        }
    );

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
    config.psqt_learning_rate = cli.psqt_lr;
    config.king_delta_decay = cli.king_delta_decay;
    // min_batch_size is irrelevant here — we drive the batching ourselves
    // via train_batch, but set it so any stray add_training_game path
    // doesn't fire unexpectedly.
    config.min_batch_size = cli.batch_size.max(100_000);

    let mut trainer = NNUETrainer::new(initial_weights, config);
    if cli.king_factorizer {
        if !cli.king_features {
            return Err("--king-factorizer needs --king-features".into());
        }
        trainer.enable_king_factorizer();
        println!("  Factoriser:   on (shared rows summed into every king bucket)");
    }

    // Recorded in every saved file so the engine can configure itself to match.
    let feature_set = NNUEFeatureSet {
        pov: cli.use_pov_features,
        stm: cli.use_stm_feature && !cli.use_pov_features,
        halfkp: cli.use_halfkp,
        hand: cli.use_hand_features,
        king: cli.king_features,
        dual: cli.dual_perspective,
    };
    println!("  Feature set:  {} (written into every saved weight file)", feature_set);

    let start = Instant::now();
    let total_epochs = cli.epochs;
    let total_positions = records.len();

    // Save the *best* network by held-out score, not the last one trained. These
    // diverge: held-out r peaks early and then declines, so the final epoch's
    // weights are routinely worse than the peak the run already reached.
    let mut best_r: Option<f32> = None;
    let mut best_epoch: u32 = 0;
    let mut epochs_since_best: u32 = 0;

    for epoch in 0..total_epochs {
        let epoch_start = Instant::now();
        records.shuffle(&mut rng);
        rank_records.shuffle(&mut rng);

        let mut batch = Vec::with_capacity(cli.batch_size);
        let mut batch_count = 0_usize;
        let mut epoch_td_sum = 0.0_f64;
        let mut epoch_batches = 0_usize;
        // Ranking groups are consumed round-robin alongside the value batches.
        // There are far fewer of them than value positions, so the cursor wraps
        // several times per epoch rather than running out.
        let mut rank_cursor = 0_usize;
        let mut epoch_rank_sum = 0.0_f64;
        let mut epoch_rank_batches = 0_usize;

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
                cli.use_pov_features,
                cli.king_features,
            ) {
                batch.push(pos);
            }
            if batch.len() >= cli.batch_size {
                let groups = next_rank_groups(
                    &rank_records,
                    &mut rank_cursor,
                    cli.rank_groups_per_batch,
                    &trainer,
                    &cli,
                );
                let stats = trainer.train_batch_mixed(&batch, &groups, &rank_cfg);
                epoch_td_sum += stats.avg_td_error as f64;
                epoch_batches += 1;
                if stats.rank_groups > 0 {
                    epoch_rank_sum += stats.avg_rank_error as f64;
                    epoch_rank_batches += 1;
                }
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
            let groups = next_rank_groups(
                &rank_records,
                &mut rank_cursor,
                cli.rank_groups_per_batch,
                &trainer,
                &cli,
            );
            let stats = trainer.train_batch_mixed(&batch, &groups, &rank_cfg);
            epoch_td_sum += stats.avg_td_error as f64;
            epoch_batches += 1;
            if stats.rank_groups > 0 {
                epoch_rank_sum += stats.avg_rank_error as f64;
                epoch_rank_batches += 1;
            }
            batch.clear();
        }

        let epoch_avg_td = if epoch_batches > 0 {
            epoch_td_sum / epoch_batches as f64
        } else {
            0.0
        };

        let pearson = validation_pearson(
            trainer.get_weights(),
            &val_records,
            cli.validate_sample,
            cli.use_stm_feature,
            cli.use_halfkp,
            cli.use_hand_features,
            cli.eval_clamp,
            cli.use_pov_features,
            cli.king_features,
        );
        let pearson_str = match pearson {
            Some(r) => format!(" r_clamp={:+.3}", r),
            None => String::new(),
        };
        // Held-out ranking error, measured without touching the weights.
        let val_rank_str = if rank_val.is_empty() {
            String::new()
        } else {
            let mut cursor = 0_usize;
            let groups = next_rank_groups(&rank_val, &mut cursor, rank_val.len(), &trainer, &cli);
            let m = trainer.measure_rank(&groups, cli.rank_temperature);
            // move_loss is the one to watch: |p - t| can fall through
            // confidence alone, without reordering a single move.
            format!(
                " val[diff={:.4} top1={:.3} loss={:.0}cp]",
                m.mean_abs_diff, m.top1_agreement, m.move_loss_cp
            )
        };

        // Mean |p - t| over sibling moves: how far the network's move preference
        // sits from the teacher's. Unlike td_err and r, this one is measured on
        // the quantity we are actually trying to fix.
        let rank_str = if epoch_rank_batches > 0 {
            format!(" rank_err={:.4}", epoch_rank_sum / epoch_rank_batches as f64)
        } else {
            String::new()
        };
        println!(
            "Epoch {:3}/{:3}: {:6} pos, {:4} batches, avg td_err={:.4}{}{}{}, t={:.1}s",
            epoch + 1,
            total_epochs,
            total_positions,
            epoch_batches,
            epoch_avg_td,
            pearson_str,
            rank_str,
            val_rank_str,
            epoch_start.elapsed().as_secs_f64()
        );

        if let Some(r) = pearson {
            let improved = best_r.map_or(true, |b| r > b);
            if improved {
                best_r = Some(r);
                best_epoch = epoch + 1;
                epochs_since_best = 0;
                trainer.get_weights().save_with_features(&cli.output_weights, feature_set)?;
            } else {
                epochs_since_best += 1;
                if cli.early_stop_patience > 0 && epochs_since_best >= cli.early_stop_patience {
                    println!(
                        "  early stop: {} validation passes without improving on r_clamp={:+.3} \
                         (epoch {})",
                        epochs_since_best,
                        best_r.unwrap_or(0.0),
                        best_epoch
                    );
                    break;
                }
            }
        }

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
            trainer.get_weights().save_with_features(&ckpt, feature_set)?;
            println!("  checkpoint -> {}", ckpt.display());
        }
    }

    // When validation ran, `--output-weights` already holds the best-scoring
    // network; overwriting it here with the final epoch would undo that.
    if best_r.is_none() {
        trainer.get_weights().save_with_features(&cli.output_weights, feature_set)?;
    }
    println!();
    println!("=== TRAINING COMPLETE ===");
    println!("  Epochs:       {}", total_epochs);
    println!("  Positions/ep: {}", total_positions);
    println!("  Wall time:    {:.1}s", start.elapsed().as_secs_f64());
    match best_r {
        Some(r) => println!(
            "  Weights ->    {}  (best: r_clamp={:+.3} at epoch {})",
            cli.output_weights.display(),
            r,
            best_epoch
        ),
        None => println!("  Weights ->    {}  (final epoch)", cli.output_weights.display()),
    }

    Ok(())
}
