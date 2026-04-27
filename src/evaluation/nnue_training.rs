//! NNUE Training Module
//!
//! This module implements self-play training for NNUE weights using TD(λ) learning.

use crate::bitboards::BitboardBoard;
use crate::evaluation::nnue::{
    NNUEAccumulator, NNUEWeights, OUTPUT_DIVISOR, STM_FEATURE_INDEX, feature_index,
};

/// f32 mirror of `OUTPUT_DIVISOR` from `nnue.rs` so the training-time
/// `prediction = tanh(raw_output / OUTPUT_DIVISOR)` mapping stays in
/// lock-step with inference. Session 11 lowered this from 16320 to 4080.
const OUTPUT_DIVISOR_F32: f32 = OUTPUT_DIVISOR as f32;
/// Centipawn scale factor (mirrors `SCALE_FACTOR` in nnue.rs).
const SCALE_FACTOR_F32: f32 = 400.0;
use crate::types::core::{Move, Player, Position};
// GameResult is in lib.rs, we'll use a local enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Win,   // From Black's perspective
    Loss,  // From Black's perspective  
    Draw,
}
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Configuration for NNUE training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NNUETrainingConfig {
    /// Learning rate (alpha) for weight updates
    pub learning_rate: f32,
    /// Lambda parameter for TD(λ) (0.0 = TD(0), 1.0 = Monte Carlo)
    pub lambda: f32,
    /// Number of self-play games to generate per training iteration
    pub games_per_iteration: usize,
    /// Maximum moves per game (to prevent infinite games)
    pub max_moves_per_game: usize,
    /// Search depth for self-play games
    pub search_depth: u8,
    /// Time per move in milliseconds
    pub time_per_move_ms: u32,
    /// Number of training iterations
    pub iterations: usize,
    /// Whether to add noise to positions (exploration)
    pub add_noise: bool,
    /// Noise strength (0.0 = no noise, 1.0 = full noise)
    pub noise_strength: f32,
    /// Minimum number of positions to accumulate before updating weights
    pub min_batch_size: usize,
    /// Gradient scaling for output-layer weights/bias. The raw gradients
    /// through tanh+quantization are tiny (~1e-5); the per-position update is
    /// `grad * learning_rate * output_grad_scale`. Bigger = faster learning
    /// but prone to i16 saturation. Default (1e7) is tuned for PST-imitation
    /// training where error ≈ 0.001; teacher-target training wants ~1e4.
    #[serde(default = "default_output_grad_scale")]
    pub output_grad_scale: f32,
    /// Gradient scaling for input-layer weights/bias. Default (1e6) is tuned
    /// for PST; teacher-target training wants ~1e3.
    #[serde(default = "default_input_grad_scale")]
    pub input_grad_scale: f32,
    /// If true, the training-time forward pass recomputes `h1_pre` from
    /// `shadow.input_weights_1` (f32) over the position's active features
    /// instead of reading `position.accumulator.hidden_1` (which was computed
    /// from the i16 `weights.input_weights_1`). This eliminates the per-batch
    /// quantize-then-readback round-trip on the input layer and is Session
    /// 12's Experiment B for unblocking input-layer learning. Output and
    /// hidden-2 layers continue to use i16 weights in the forward pass.
    #[serde(default)]
    pub f32_input_forward: bool,
    /// Per-position loss multiplier applied when |teacher_eval_cp| exceeds
    /// `decisive_threshold_cp`. The trainer's `pst_evaluation` field carries
    /// the teacher's centipawn eval (the offline trainer wires it that way),
    /// so this scales gradient magnitude on decisive positions. 1.0 = off
    /// (default). 3.0 means decisive positions contribute 3× as much
    /// gradient signal as non-decisive ones. Session 12's Experiment C.
    #[serde(default = "default_decisive_weight")]
    pub decisive_weight: f32,
    /// |teacher_eval_cp| threshold above which a position is "decisive" for
    /// the `decisive_weight` multiplier.
    #[serde(default = "default_decisive_threshold_cp")]
    pub decisive_threshold_cp: i32,
    /// Session 13: switch the trainer's loss from `tanh(eval/scale) + L2` (over
    /// targets in [-1,1]) to `sigmoid(eval/scale) + L2` (over targets in
    /// [0,1]). The sigmoid derivative `p(1-p)` peaks at `p=0.5` rather than
    /// vanishing at `p=±1`, removing the tanh-saturation bottleneck Session 12
    /// localised. The offline trainer's `target_for` must be in matching
    /// units; the CLI flag `--use-sigmoid-loss` toggles both.
    #[serde(default)]
    pub use_sigmoid_loss: bool,
    /// cp scale used inside the sigmoid both for the teacher target and the
    /// network prediction: `sigmoid(eval_cp / sigmoid_eval_scale)`. Stockfish
    /// uses 410. Only meaningful when `use_sigmoid_loss = true`.
    #[serde(default = "default_sigmoid_eval_scale")]
    pub sigmoid_eval_scale: f32,
    /// Session 15: replace per-position SGD updates on the f32 shadow with
    /// Adam-style first/second moment estimates. The grad-scale knobs become
    /// largely irrelevant under Adam (the sqrt(v) normalisation removes the
    /// per-layer-magnitude bias the SGD knobs were compensating for), so under
    /// `use_adam = true` the recommended config uses a single `learning_rate`
    /// in the i16-shadow scale (≈ 0.05–0.5) and `output_grad_scale = input_grad_scale = 1.0`.
    /// Sparse update: only feature rows that appeared in this batch get an
    /// `m / v / w` update — feature rows with no gradient this step retain
    /// their previous `m, v`. Bias correction uses the global step counter.
    #[serde(default)]
    pub use_adam: bool,
    /// Adam first-moment decay (β1). Standard default 0.9.
    #[serde(default = "default_adam_beta1")]
    pub adam_beta1: f32,
    /// Adam second-moment decay (β2). Standard default 0.999.
    #[serde(default = "default_adam_beta2")]
    pub adam_beta2: f32,
    /// Adam denominator stabiliser (ε). Standard default 1e-8.
    #[serde(default = "default_adam_epsilon")]
    pub adam_epsilon: f32,
}

fn default_output_grad_scale() -> f32 { 1e7 }
fn default_input_grad_scale() -> f32 { 1e6 }
fn default_decisive_weight() -> f32 { 1.0 }
fn default_decisive_threshold_cp() -> i32 { 500 }
fn default_sigmoid_eval_scale() -> f32 { 410.0 }
fn default_adam_beta1() -> f32 { 0.9 }
fn default_adam_beta2() -> f32 { 0.999 }
fn default_adam_epsilon() -> f32 { 1e-8 }

impl Default for NNUETrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            lambda: 0.7,
            games_per_iteration: 100,
            max_moves_per_game: 200,
            search_depth: 3,
            time_per_move_ms: 100,
            iterations: 1000,
            add_noise: false,
            noise_strength: 0.1,
            min_batch_size: 1000,
            output_grad_scale: default_output_grad_scale(),
            input_grad_scale: default_input_grad_scale(),
            f32_input_forward: false,
            decisive_weight: default_decisive_weight(),
            decisive_threshold_cp: default_decisive_threshold_cp(),
            use_sigmoid_loss: false,
            sigmoid_eval_scale: default_sigmoid_eval_scale(),
            use_adam: false,
            adam_beta1: default_adam_beta1(),
            adam_beta2: default_adam_beta2(),
            adam_epsilon: default_adam_epsilon(),
        }
    }
}

/// Training statistics
#[derive(Debug, Clone, Default)]
pub struct TrainingStats {
    /// Total positions processed
    pub positions_processed: usize,
    /// Total weight updates
    pub weight_updates: usize,
    /// Average TD error magnitude
    pub avg_td_error: f32,
    /// Average weight change magnitude
    pub avg_weight_change: f32,
    /// Maximum weight change
    pub max_weight_change: f32,
    /// Number of games processed
    pub games_processed: usize,
    /// Average game length
    pub avg_game_length: f32,
    /// Average positions per game
    pub avg_positions_per_game: f32,
}

/// A game position with its evaluation and outcome
#[derive(Debug, Clone)]
pub struct TrainingPosition {
    /// Board state (as feature indices)
    pub active_features: Vec<usize>,
    /// NNUE accumulator state (for efficient incremental updates)
    pub accumulator: NNUEAccumulator,
    /// NNUE evaluation (student/current network prediction)
    pub evaluation: i32,
    /// PST evaluation (teacher/oracle target) - external signal for supervised learning
    pub pst_evaluation: i32,
    /// Player to move
    pub player: Player,
    /// Move made from this position
    pub move_made: Option<Move>,
    /// Final game outcome (if terminal position)
    pub outcome: Option<GameResult>,
    /// TD target value (computed during training)
    pub td_target: Option<f32>,
}

/// Self-play training data from a single game
#[derive(Debug, Clone)]
pub struct TrainingGame {
    pub positions: Vec<TrainingPosition>,
    pub result: GameResult,
}

/// f32 shadow weights for gradient accumulation during training.
///
/// The canonical `NNUEWeights` are i16 (for cheap inference via
/// `act * weight` on small ints). Training in the i16 domain hits two problems:
///
///   * per-position gradient updates of magnitude < 0.5 round to 0 — the input
///     layer, where gradients are spread across many features, barely moves;
///   * scaling the gradient up to cross the rounding threshold per position
///     causes the same weight to saturate i16 within a single batch.
///
/// The fix, standard in nnue-pytorch / YaneuraOu trainers, is to maintain a
/// full-precision f32 copy of every weight, apply gradient updates to the
/// shadow (no rounding, no clamping), and re-quantize the shadow into the
/// i16 `NNUEWeights` once per batch. Forward passes still read i16, so
/// training numerics match inference.
///
/// Structure mirrors `NNUEWeights` exactly.
#[derive(Debug, Clone)]
pub struct ShadowWeights {
    pub input_weights_1: Vec<Vec<f32>>,
    pub hidden_biases_1: Vec<f32>,
    pub input_weights_2: Option<Vec<Vec<f32>>>,
    pub hidden_biases_2: Option<Vec<f32>>,
    pub output_weights: Vec<f32>,
    pub output_bias: f32,
}

impl ShadowWeights {
    /// Build a shadow by casting each i16 weight to f32. This is the identity
    /// map — the shadow initially represents exactly the same values as the
    /// i16 weights, then drifts as training adds sub-unit f32 updates.
    pub fn from_weights(weights: &NNUEWeights) -> Self {
        Self {
            input_weights_1: weights
                .input_weights_1
                .iter()
                .map(|row| row.iter().map(|&w| w as f32).collect())
                .collect(),
            hidden_biases_1: weights
                .hidden_biases_1
                .iter()
                .map(|&b| b as f32)
                .collect(),
            input_weights_2: weights.input_weights_2.as_ref().map(|w2| {
                w2.iter()
                    .map(|row| row.iter().map(|&w| w as f32).collect())
                    .collect()
            }),
            hidden_biases_2: weights
                .hidden_biases_2
                .as_ref()
                .map(|b2| b2.iter().map(|&b| b as f32).collect()),
            output_weights: weights.output_weights.iter().map(|&w| w as f32).collect(),
            output_bias: weights.output_bias as f32,
        }
    }

    /// Render the shadow into canonical i16 weights via clamp + round-to-nearest.
    ///
    /// Input-layer weights use the conservative ±127 magnitude budget (fits in
    /// the i8 sub-range of i16 — matches how `NNUEWeights::new` initialises);
    /// hidden-2 and output weights use the full i16 range. Biases use the
    /// full i32 range with a generous soft bound. Returns (total_abs_change,
    /// max_abs_change, num_changed) so callers can populate `TrainingStats`.
    pub fn quantize_into(&self, weights: &mut NNUEWeights) -> (f32, f32, usize) {
        let mut total = 0.0_f32;
        let mut max: f32 = 0.0;
        let mut count = 0_usize;

        for (feat_i, row) in self.input_weights_1.iter().enumerate() {
            for (j, &w) in row.iter().enumerate() {
                let old = weights.input_weights_1[feat_i][j];
                let new = w.clamp(-127.0, 127.0).round() as i16;
                if new != old {
                    let ch = (new - old).abs() as f32;
                    total += ch;
                    if ch > max {
                        max = ch;
                    }
                    count += 1;
                    weights.input_weights_1[feat_i][j] = new;
                }
            }
        }
        for (i, &b) in self.hidden_biases_1.iter().enumerate() {
            let old = weights.hidden_biases_1[i];
            let new = b.clamp(-2.0e9, 2.0e9) as i32;
            if new != old {
                let ch = (new - old).abs() as f32;
                total += ch;
                if ch > max {
                    max = ch;
                }
                count += 1;
                weights.hidden_biases_1[i] = new;
            }
        }
        if let (Some(shadow_w2), Some(ref mut w2_mut)) =
            (self.input_weights_2.as_ref(), weights.input_weights_2.as_mut())
        {
            for (i, row) in shadow_w2.iter().enumerate() {
                for (j, &w) in row.iter().enumerate() {
                    let old = w2_mut[i][j];
                    let new = w.clamp(-32767.0, 32767.0).round() as i16;
                    if new != old {
                        let ch = (new - old).abs() as f32;
                        total += ch;
                        if ch > max {
                            max = ch;
                        }
                        count += 1;
                        w2_mut[i][j] = new;
                    }
                }
            }
        }
        if let (Some(shadow_b2), Some(ref mut b2_mut)) =
            (self.hidden_biases_2.as_ref(), weights.hidden_biases_2.as_mut())
        {
            for (i, &b) in shadow_b2.iter().enumerate() {
                let old = b2_mut[i];
                let new = b.clamp(-2.0e9, 2.0e9) as i32;
                if new != old {
                    let ch = (new - old).abs() as f32;
                    total += ch;
                    if ch > max {
                        max = ch;
                    }
                    count += 1;
                    b2_mut[i] = new;
                }
            }
        }
        for (i, &w) in self.output_weights.iter().enumerate() {
            let old = weights.output_weights[i];
            let new = w.clamp(-32767.0, 32767.0).round() as i16;
            if new != old {
                let ch = (new - old).abs() as f32;
                total += ch;
                if ch > max {
                    max = ch;
                }
                count += 1;
                weights.output_weights[i] = new;
            }
        }
        {
            let old = weights.output_bias;
            let new = self.output_bias.clamp(-2.0e9, 2.0e9) as i32;
            if new != old {
                let ch = (new - old).abs() as f32;
                total += ch;
                if ch > max {
                    max = ch;
                }
                count += 1;
                weights.output_bias = new;
            }
        }

        (total, max, count)
    }
}

/// Session 15: Adam optimiser state. Mirrors `ShadowWeights` shape with two
/// f32 tensors per parameter (first-moment `m`, second-moment `v`), plus a
/// global step counter for bias correction. Sparse-update semantics: input
/// rows that have no gradient in a given batch keep their existing `m, v`
/// rather than decaying — this is the "lazy" / sparse-Adam variant standard
/// in nnue-pytorch / YaneuraOu trainers.
#[derive(Debug, Clone)]
pub struct AdamState {
    pub input_weights_1_m: Vec<Vec<f32>>,
    pub input_weights_1_v: Vec<Vec<f32>>,
    pub hidden_biases_1_m: Vec<f32>,
    pub hidden_biases_1_v: Vec<f32>,
    pub input_weights_2_m: Option<Vec<Vec<f32>>>,
    pub input_weights_2_v: Option<Vec<Vec<f32>>>,
    pub hidden_biases_2_m: Option<Vec<f32>>,
    pub hidden_biases_2_v: Option<Vec<f32>>,
    pub output_weights_m: Vec<f32>,
    pub output_weights_v: Vec<f32>,
    pub output_bias_m: f32,
    pub output_bias_v: f32,
    pub step: u64,
}

impl AdamState {
    /// Allocate zero-initialised m/v buffers shaped like the shadow.
    pub fn from_shadow(shadow: &ShadowWeights) -> Self {
        let zeros_2d = |w: &Vec<Vec<f32>>| -> Vec<Vec<f32>> {
            w.iter().map(|r| vec![0.0_f32; r.len()]).collect()
        };
        Self {
            input_weights_1_m: zeros_2d(&shadow.input_weights_1),
            input_weights_1_v: zeros_2d(&shadow.input_weights_1),
            hidden_biases_1_m: vec![0.0; shadow.hidden_biases_1.len()],
            hidden_biases_1_v: vec![0.0; shadow.hidden_biases_1.len()],
            input_weights_2_m: shadow.input_weights_2.as_ref().map(zeros_2d),
            input_weights_2_v: shadow.input_weights_2.as_ref().map(zeros_2d),
            hidden_biases_2_m: shadow.hidden_biases_2.as_ref().map(|b| vec![0.0; b.len()]),
            hidden_biases_2_v: shadow.hidden_biases_2.as_ref().map(|b| vec![0.0; b.len()]),
            output_weights_m: vec![0.0; shadow.output_weights.len()],
            output_weights_v: vec![0.0; shadow.output_weights.len()],
            output_bias_m: 0.0,
            output_bias_v: 0.0,
            step: 0,
        }
    }
}

/// One Adam update step on a single scalar shadow weight. Updates `m`, `v`,
/// then applies the bias-corrected `m_hat / (sqrt(v_hat) + eps)` step scaled
/// by `lr`. Caller is responsible for incrementing the global step counter
/// before invoking this for all weights in a batch.
#[inline]
fn adam_apply(
    shadow_w: &mut f32,
    m: &mut f32,
    v: &mut f32,
    grad: f32,
    lr: f32,
    beta1: f32,
    beta2: f32,
    eps: f32,
    t: u64,
) {
    *m = beta1 * *m + (1.0 - beta1) * grad;
    *v = beta2 * *v + (1.0 - beta2) * grad * grad;
    let bc1 = 1.0 - beta1.powi(t as i32);
    let bc2 = 1.0 - beta2.powi(t as i32);
    let m_hat = *m / bc1;
    let v_hat = *v / bc2;
    *shadow_w += lr * m_hat / (v_hat.sqrt() + eps);
}

/// NNUE Trainer
pub struct NNUETrainer {
    /// Current NNUE weights (i16, what inference reads)
    weights: NNUEWeights,
    /// f32 shadow weights for lossless gradient accumulation during training.
    /// Re-quantized into `weights` once per batch in `train_batch_accumulated`.
    shadow: ShadowWeights,
    /// Session 15: Adam optimiser state (None when SGD path is in use).
    adam: Option<AdamState>,
    /// Training configuration
    config: NNUETrainingConfig,
    /// Accumulated training positions
    training_positions: VecDeque<TrainingPosition>,
    /// Training statistics
    stats: TrainingStats,
}

impl NNUETrainer {
    /// Create a new trainer with given weights and configuration
    pub fn new(weights: NNUEWeights, config: NNUETrainingConfig) -> Self {
        let shadow = ShadowWeights::from_weights(&weights);
        let adam = if config.use_adam {
            Some(AdamState::from_shadow(&shadow))
        } else {
            None
        };
        Self {
            weights,
            shadow,
            adam,
            config,
            training_positions: VecDeque::new(),
            stats: TrainingStats::default(),
        }
    }

    /// Extract positions from a self-play game
    pub fn extract_game_positions(
        &mut self,
        game: &TrainingGame,
    ) -> Vec<TrainingPosition> {
        let mut positions = game.positions.clone();
        
        // Compute TD targets using TD(λ)
        self.compute_td_targets(&mut positions, game.result);
        
        positions
    }

    /// Compute training targets for positions using PST evaluation as oracle.
    ///
    /// Uses a hybrid target: blend of PST evaluation (immediate position quality)
    /// and game outcome (long-term correctness). This breaks the bootstrap problem
    /// by using external signals rather than NNUE's own weak evaluation.
    ///
    /// Target formula per position:
    ///   target = (1 - outcome_weight) * pst_normalized + outcome_weight * outcome_normalized
    ///
    /// Where outcome_weight increases for positions closer to the game end
    /// (later positions are more strongly influenced by the known outcome).
    fn compute_td_targets(
        &self,
        positions: &mut [TrainingPosition],
        final_result: GameResult,
    ) {
        if positions.is_empty() {
            return;
        }

        // Convert final result to value in [-1, 1] from Black's perspective
        let outcome_value = match final_result {
            GameResult::Win => 1.0_f32,   // Black wins
            GameResult::Loss => -1.0,      // White wins (Black loses)
            GameResult::Draw => 0.0,
        };

        let n = positions.len();

        for i in 0..n {
            // Normalize PST evaluation to [-1, 1] range.
            // PST evaluations are typically in [-2000, 2000] centipawns.
            // tanh(eval / 600) maps this range smoothly to [-1, 1].
            let pst_normalized = (positions[i].pst_evaluation as f32 / 600.0).tanh();

            // Outcome weight increases linearly from 0.1 at game start to 0.9 at game end.
            // Earlier positions rely more on PST (immediate quality), later positions
            // rely more on outcome (we know who actually won).
            let progress = i as f32 / n.max(1) as f32;
            let outcome_weight = 0.1 + 0.8 * progress;

            // Blend PST evaluation with game outcome.
            // PST eval is already from the perspective of the player to move,
            // but outcome is from Black's perspective. Adjust outcome for player.
            let player_mult = if positions[i].player == Player::Black { 1.0 } else { -1.0 };
            let outcome_adjusted = outcome_value * player_mult;

            let target = (1.0 - outcome_weight) * pst_normalized + outcome_weight * outcome_adjusted;
            positions[i].td_target = Some(target);
        }
    }

    /// Update weights using accumulated training data
    pub fn update_weights(&mut self) -> TrainingStats {
        if self.training_positions.len() < self.config.min_batch_size {
            return self.stats.clone();
        }

        let batch_size = self.training_positions.len().min(self.config.min_batch_size * 10);
        let positions: Vec<TrainingPosition> = self.training_positions
            .drain(..batch_size)
            .collect();

        let mut total_td_error = 0.0;
        let mut total_weight_change = 0.0;
        let mut max_weight_change: f32 = 0.0;
        let mut weight_update_count = 0;

        // Gradient-based weight update
        // For each position, compute gradient and update weights
        for position in &positions {
            if let Some(td_target) = position.td_target {
                let (td_error, weight_change, max_change) = self.update_weights_for_position(position, td_target);
                total_td_error += td_error.abs();
                total_weight_change += weight_change;
                max_weight_change = max_weight_change.max(max_change);
                weight_update_count += 1;
            }
        }

        // Update statistics (positions_processed already updated in add_training_game)
        self.stats.weight_updates += weight_update_count;
        if weight_update_count > 0 {
            self.stats.avg_td_error = total_td_error / weight_update_count as f32;
            self.stats.avg_weight_change = total_weight_change / weight_update_count as f32;
            self.stats.max_weight_change = max_weight_change;
        }

        self.stats.clone()
    }

    /// Update weights for a single position using supervised learning from PST oracle.
    ///
    /// Works in floating-point domain to avoid gradient truncation, then quantizes
    /// accumulated updates back to integer weights. The target is in [-1, 1]
    /// (blended PST eval + game outcome). NNUE prediction is mapped to the same
    /// range via sigmoid(eval_cp / 400) where eval_cp is the centipawn output.
    ///
    /// Returns (error, avg_weight_change, max_weight_change).
    fn update_weights_for_position(&mut self, position: &TrainingPosition, target_value: f32) -> (f32, f32, f32) {
        let learning_rate = self.config.learning_rate;
        let output_scale = self.config.output_grad_scale;
        let input_scale = self.config.input_grad_scale;

        let (hidden_size_1, _hidden_size_2) = self.weights.hidden_sizes();

        // === Forward pass (floating point) ===

        // Hidden layer 1: ReLU(accumulator + bias)
        let hidden_1_activated: Vec<f32> = position.accumulator.hidden_1.iter()
            .zip(self.weights.hidden_biases_1.iter())
            .map(|(&h, &bias)| (h + bias).max(0) as f32)
            .collect();

        // Hidden layer 2 (if present): ReLU(W2 * h1 + b2)
        let final_values: Vec<f32> = if let Some(ref _h2) = position.accumulator.hidden_2 {
            let biases_2 = self.weights.hidden_biases_2.as_ref().unwrap();
            let h2_len = biases_2.len();
            let mut h2_values = vec![0.0_f32; h2_len];
            if let Some(ref w2) = self.weights.input_weights_2 {
                for (i, &act_1) in hidden_1_activated.iter().enumerate() {
                    for (j, &weight) in w2[i].iter().enumerate() {
                        h2_values[j] += act_1 * (weight as f32) / 64.0;
                    }
                }
                for (j, &bias) in biases_2.iter().enumerate() {
                    h2_values[j] = (h2_values[j] + bias as f32).max(0.0);
                }
            }
            h2_values
        } else {
            hidden_1_activated.clone()
        };

        // Output: dot(final_values, output_weights) / 64 + output_bias
        let mut raw_output: f32 = self.weights.output_bias as f32;
        for (i, &value) in final_values.iter().enumerate() {
            if i < self.weights.output_weights.len() {
                raw_output += value * (self.weights.output_weights[i] as f32) / 64.0;
            }
        }

        // Scale to centipawns: output_cp = raw_output * SCALE_FACTOR / OUTPUT_DIVISOR
        let output_cp = raw_output * SCALE_FACTOR_F32 / OUTPUT_DIVISOR_F32;

        // Session 13: prediction is either tanh over [-1,1] (legacy) or
        // sigmoid over [0,1] (Session 13). Both share the rest of the pipeline.
        let use_sigmoid_loss = self.config.use_sigmoid_loss;
        let sigmoid_eval_scale = self.config.sigmoid_eval_scale;
        let prediction = if use_sigmoid_loss {
            1.0 / (1.0 + (-output_cp / sigmoid_eval_scale).exp())
        } else {
            (output_cp / SCALE_FACTOR_F32).tanh()
        };

        // === Backward pass ===

        // Loss = 0.5 * (target - prediction)^2
        // d(loss)/d(prediction) = -(target - prediction) = prediction - target
        let error = target_value - prediction;

        // d(prediction)/d(output_cp) is either tanh or sigmoid derivative.
        let pred_deriv = if use_sigmoid_loss {
            prediction * (1.0 - prediction) / sigmoid_eval_scale
        } else {
            (1.0 - prediction * prediction) / SCALE_FACTOR_F32
        };

        // d(output_cp)/d(raw_output) = SCALE_FACTOR / OUTPUT_DIVISOR
        let scale_deriv = SCALE_FACTOR_F32 / OUTPUT_DIVISOR_F32;

        // d(loss)/d(raw_output) = -error * pred_deriv * scale_deriv
        // We want to MINIMIZE loss, so update = -d(loss)/d(w) = error * chain
        let d_raw = error * pred_deriv * scale_deriv;

        // Track weight changes
        let mut total_change = 0.0_f32;
        let mut max_change = 0.0_f32;
        let mut change_count = 0_usize;

        // === Update output weights ===
        // d(raw_output)/d(output_weight[i]) = final_values[i] / 64
        for (i, &value) in final_values.iter().enumerate() {
            if i < self.weights.output_weights.len() {
                let gradient = d_raw * (value / 64.0);
                // Scale up to make meaningful i16 updates.
                // With typical gradient ~1e-5 and value ~10, gradient*lr ~5e-8.
                // We need ~1e7 scaling to get integer-magnitude updates.
                let update_f = gradient * learning_rate * output_scale;
                let weight_update = update_f.clamp(-32000.0, 32000.0) as i16;
                if weight_update != 0 {
                    let old_weight = self.weights.output_weights[i];
                    self.weights.output_weights[i] = old_weight.saturating_add(weight_update);
                    let change = (self.weights.output_weights[i] - old_weight).abs() as f32;
                    total_change += change;
                    max_change = max_change.max(change);
                    change_count += 1;
                }
            }
        }

        // === Update output bias ===
        // d(raw_output)/d(output_bias) = 1
        let bias_update_f = d_raw * learning_rate * output_scale;
        let bias_update = bias_update_f.clamp(-2e9, 2e9) as i32;
        if bias_update != 0 {
            let old_bias = self.weights.output_bias;
            self.weights.output_bias = old_bias.saturating_add(bias_update);
            let change = (self.weights.output_bias - old_bias).abs() as f32;
            total_change += change;
            max_change = max_change.max(change);
            change_count += 1;
        }

        // === Backprop to hidden layer 2 (if present) and then to input weights ===
        // For the 2-layer network (256->32->1):
        //   d(raw_output)/d(h2_activated[j]) = output_weights[j] / 64
        //   d(h2_activated)/d(h2_pre) = 1 if h2_pre > 0 (ReLU)
        //   d(h2_pre[j])/d(h1_activated[i]) = w2[i][j] / 64
        //   d(h1_activated)/d(h1_pre) = 1 if h1_pre > 0 (ReLU)
        //   d(h1_pre[i])/d(input_weight[f][i]) = 1 (for active feature f)

        // Compute gradient at hidden layer 1
        let mut d_hidden_1 = vec![0.0_f32; hidden_size_1];

        if let Some(ref w2) = self.weights.input_weights_2 {
            // Gradient flows through layer 2
            let biases_2 = self.weights.hidden_biases_2.as_ref().unwrap();
            let h2_len = biases_2.len();

            // d(loss)/d(h2_activated[j]) = d_raw * output_weights[j] / 64
            let d_h2: Vec<f32> = (0..h2_len)
                .map(|j| {
                    if j < self.weights.output_weights.len() {
                        d_raw * (self.weights.output_weights[j] as f32 / 64.0)
                    } else {
                        0.0
                    }
                })
                .collect();

            // Recompute h2 pre-activation to check ReLU gate
            let mut h2_pre = vec![0.0_f32; h2_len];
            for (i, &act_1) in hidden_1_activated.iter().enumerate() {
                for (j, &weight) in w2[i].iter().enumerate() {
                    h2_pre[j] += act_1 * (weight as f32) / 64.0;
                }
            }
            for (j, &bias) in biases_2.iter().enumerate() {
                h2_pre[j] += bias as f32;
            }

            // d(loss)/d(h1_activated[i]) = sum_j(d_h2[j] * relu_gate[j] * w2[i][j] / 64)
            for i in 0..hidden_size_1 {
                for j in 0..h2_len {
                    if h2_pre[j] > 0.0 {
                        d_hidden_1[i] += d_h2[j] * (w2[i][j] as f32 / 64.0);
                    }
                }
            }

            // Also update layer 2 weights: w2[i][j]
            if let Some(ref mut w2_mut) = self.weights.input_weights_2 {
                for i in 0..hidden_size_1 {
                    for j in 0..h2_len {
                        if h2_pre[j] > 0.0 {
                            let grad = d_h2[j] * (hidden_1_activated[i] / 64.0);
                            let update_f = grad * learning_rate * output_scale;
                            let weight_update = update_f.clamp(-32000.0, 32000.0) as i16;
                            if weight_update != 0 {
                                let old_w = w2_mut[i][j];
                                w2_mut[i][j] = old_w.saturating_add(weight_update);
                                let change = (w2_mut[i][j] - old_w).abs() as f32;
                                total_change += change;
                                max_change = max_change.max(change);
                                change_count += 1;
                            }
                        }
                    }
                }
            }

            // Update layer 2 biases
            if let Some(ref mut b2_mut) = self.weights.hidden_biases_2 {
                for j in 0..h2_len {
                    if h2_pre[j] > 0.0 {
                        let bias_grad = d_h2[j];
                        let update_f = bias_grad * learning_rate * output_scale;
                        let update = update_f.clamp(-2e9, 2e9) as i32;
                        if update != 0 {
                            let old_b = b2_mut[j];
                            b2_mut[j] = old_b.saturating_add(update);
                            let change = (b2_mut[j] - old_b).abs() as f32;
                            total_change += change;
                            max_change = max_change.max(change);
                            change_count += 1;
                        }
                    }
                }
            }
        } else {
            // No layer 2: gradient goes directly from output to hidden 1
            for i in 0..hidden_size_1 {
                if i < self.weights.output_weights.len() {
                    d_hidden_1[i] = d_raw * (self.weights.output_weights[i] as f32 / 64.0);
                }
            }
        }

        // === Update input-to-hidden-1 weights (sparse: only active features) ===
        // d(h1_pre[i])/d(input_weight[f][i]) = 1 for active feature f
        // ReLU gate: only update if h1_pre > 0
        for &feature_idx in &position.active_features {
            if feature_idx < self.weights.input_weights_1.len() {
                for i in 0..hidden_size_1 {
                    let h1_pre = position.accumulator.hidden_1[i] + self.weights.hidden_biases_1[i];
                    if h1_pre > 0 && i < self.weights.input_weights_1[feature_idx].len() {
                        let grad = d_hidden_1[i];
                        // Smaller scaling for input weights (many features, don't want explosion)
                        let update_f = grad * learning_rate * input_scale;
                        let weight_update = update_f.clamp(-127.0, 127.0) as i16;
                        if weight_update != 0 {
                            let old_weight = self.weights.input_weights_1[feature_idx][i];
                            self.weights.input_weights_1[feature_idx][i] = old_weight.saturating_add(weight_update);
                            let change = (self.weights.input_weights_1[feature_idx][i] - old_weight).abs() as f32;
                            total_change += change;
                            max_change = max_change.max(change);
                            change_count += 1;
                        }
                    }
                }
            }
        }

        // === Update hidden-1 biases ===
        for i in 0..hidden_size_1 {
            let h1_pre = position.accumulator.hidden_1[i] + self.weights.hidden_biases_1[i];
            if h1_pre > 0 {
                let grad = d_hidden_1[i];
                let update_f = grad * learning_rate * input_scale;
                let update = update_f.clamp(-2e9, 2e9) as i32;
                if update != 0 {
                    let old_b = self.weights.hidden_biases_1[i];
                    self.weights.hidden_biases_1[i] = old_b.saturating_add(update);
                    let change = (self.weights.hidden_biases_1[i] - old_b).abs() as f32;
                    total_change += change;
                    max_change = max_change.max(change);
                    change_count += 1;
                }
            }
        }

        let avg_change = if change_count > 0 { total_change / change_count as f32 } else { 0.0 };
        (error, avg_change, max_change)
    }

    /// Batched SGD with f32 shadow weights.
    ///
    /// Forward pass reads the i16 `self.weights` (matching inference); gradient
    /// accumulation is done in f32; the accumulated updates are added to the
    /// f32 `self.shadow`; then `self.shadow` is re-quantized into `self.weights`
    /// once per batch. This fixes the Session-7 blocker where sub-unit gradient
    /// updates at the input layer rounded to zero in i16 and the network never
    /// learned beyond the layer-2/output weights.
    ///
    /// Equivalent to: i16 weight = round(init_i16 + Σ_batches f32_grad_update),
    /// so any gradient direction that's consistent across many batches
    /// eventually crosses the quantization threshold and bumps the i16 weight.
    pub fn train_batch_accumulated(&mut self, positions: &[TrainingPosition]) -> TrainingStats {
        let (h1_size, _) = self.weights.hidden_sizes();
        let num_features = self.weights.input_weights_1.len();
        let h2_size = self
            .weights
            .hidden_biases_2
            .as_ref()
            .map(|b| b.len())
            .unwrap_or(0);

        let mut acc_output_w = vec![0.0_f32; self.weights.output_weights.len()];
        let mut acc_output_b: f32 = 0.0;
        let mut acc_w2: Vec<Vec<f32>> = self
            .weights
            .input_weights_2
            .as_ref()
            .map(|w2| w2.iter().map(|row| vec![0.0_f32; row.len()]).collect())
            .unwrap_or_default();
        let mut acc_b2: Vec<f32> = vec![0.0_f32; h2_size];
        let mut acc_input_w: std::collections::HashMap<usize, Vec<f32>> =
            std::collections::HashMap::new();
        let mut acc_b1: Vec<f32> = vec![0.0_f32; h1_size];

        let mut total_error = 0.0_f32;
        let mut count = 0_usize;

        let f32_input_forward = self.config.f32_input_forward;
        let decisive_weight = self.config.decisive_weight;
        let decisive_threshold_cp = self.config.decisive_threshold_cp;
        let use_sigmoid_loss = self.config.use_sigmoid_loss;
        let sigmoid_eval_scale = self.config.sigmoid_eval_scale;

        for position in positions {
            let target = match position.td_target {
                Some(t) => t,
                None => continue,
            };

            // Forward pass in f32.
            //
            // Default path: read `position.accumulator.hidden_1` (i32, computed
            // from the i16 input weights at refresh time) and add the i32 bias.
            //
            // f32-input-forward path (Session 12 Experiment B): recompute
            // `h1_pre` from scratch using the f32 shadow `input_weights_1`
            // and `hidden_biases_1`. This bypasses the i16 quantization that
            // is applied to the input layer once per batch in
            // `quantize_into`, so the forward pass during training sees the
            // un-rounded weight values that subsequent gradient updates are
            // accumulating into. Active features come straight from the
            // `position.active_features` list the offline trainer populates.
            let h1_pre: Vec<f32> = if f32_input_forward {
                let mut v = self.shadow.hidden_biases_1.clone();
                for &feat in &position.active_features {
                    if feat < self.shadow.input_weights_1.len() {
                        let row = &self.shadow.input_weights_1[feat];
                        for (i, &w) in row.iter().enumerate() {
                            v[i] += w;
                        }
                    }
                }
                v
            } else {
                position
                    .accumulator
                    .hidden_1
                    .iter()
                    .zip(self.weights.hidden_biases_1.iter())
                    .map(|(&h, &b)| (h + b) as f32)
                    .collect()
            };
            let h1_act: Vec<f32> = h1_pre.iter().map(|&x| x.max(0.0)).collect();

            let (h2_pre, final_values) = if let Some(ref w2) = self.weights.input_weights_2 {
                let biases_2 = self.weights.hidden_biases_2.as_ref().unwrap();
                let mut h2_pre = vec![0.0_f32; h2_size];
                for (i, &a1) in h1_act.iter().enumerate() {
                    for (j, &w) in w2[i].iter().enumerate() {
                        h2_pre[j] += a1 * (w as f32) / 64.0;
                    }
                }
                for (j, &b) in biases_2.iter().enumerate() {
                    h2_pre[j] += b as f32;
                }
                let h2_act: Vec<f32> = h2_pre.iter().map(|&x| x.max(0.0)).collect();
                (h2_pre, h2_act)
            } else {
                (Vec::new(), h1_act.clone())
            };

            let mut raw_output = self.weights.output_bias as f32;
            for (i, &v) in final_values.iter().enumerate() {
                if i < self.weights.output_weights.len() {
                    raw_output += v * (self.weights.output_weights[i] as f32) / 64.0;
                }
            }
            let output_cp = raw_output * SCALE_FACTOR_F32 / OUTPUT_DIVISOR_F32;
            // Session 13: prediction either tanh (legacy) over [-1,1] or
            // sigmoid (new) over [0,1]. Both forms accept output_cp; tanh uses
            // SCALE_FACTOR (=400) as its cp scale, sigmoid uses
            // sigmoid_eval_scale (=410 by default).
            let prediction = if use_sigmoid_loss {
                1.0 / (1.0 + (-output_cp / sigmoid_eval_scale).exp())
            } else {
                (output_cp / SCALE_FACTOR_F32).tanh()
            };

            let error = target - prediction;
            total_error += error.abs();
            count += 1;

            // Session 12 Experiment C: scale gradient on decisive positions.
            // The trainer sees `pst_evaluation` populated with the teacher's
            // centipawn evaluation (the offline trainer wires it that way),
            // so positions with |teacher cp| above `decisive_threshold_cp`
            // contribute `decisive_weight` × the usual gradient. Multiplying
            // `error` cascades through `d_raw` and all downstream gradient
            // accumulators — equivalent to a per-position learning rate.
            // total_error above is NOT reweighted, so the reported td_err
            // remains directly comparable to runs with weight=1.0.
            let decisive_mult = if decisive_weight != 1.0
                && position.pst_evaluation.abs() > decisive_threshold_cp
            {
                decisive_weight
            } else {
                1.0
            };
            let weighted_error = error * decisive_mult;

            // Session 13: chain rule for prediction → output_cp → raw_output.
            //   tanh:    d(p)/d(output_cp) = (1 - p^2) / SCALE_FACTOR
            //   sigmoid: d(p)/d(output_cp) = p(1 - p) / sigmoid_eval_scale
            // d(output_cp)/d(raw_output) = SCALE_FACTOR / OUTPUT_DIVISOR.
            let pred_deriv = if use_sigmoid_loss {
                prediction * (1.0 - prediction) / sigmoid_eval_scale
            } else {
                (1.0 - prediction * prediction) / SCALE_FACTOR_F32
            };
            let scale_deriv = SCALE_FACTOR_F32 / OUTPUT_DIVISOR_F32;
            let d_raw = weighted_error * pred_deriv * scale_deriv;

            for (i, &v) in final_values.iter().enumerate() {
                if i < acc_output_w.len() {
                    acc_output_w[i] += d_raw * (v / 64.0);
                }
            }
            acc_output_b += d_raw;

            let d_h2: Vec<f32> = if h2_size > 0 {
                (0..h2_size)
                    .map(|j| {
                        if j < self.weights.output_weights.len() {
                            d_raw * (self.weights.output_weights[j] as f32 / 64.0)
                        } else {
                            0.0
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let mut d_h1 = vec![0.0_f32; h1_size];
            if let Some(ref w2) = self.weights.input_weights_2 {
                for i in 0..h1_size {
                    for j in 0..h2_size {
                        if h2_pre[j] > 0.0 {
                            d_h1[i] += d_h2[j] * (w2[i][j] as f32 / 64.0);
                        }
                    }
                }
                for i in 0..h1_size {
                    for j in 0..h2_size {
                        if h2_pre[j] > 0.0 {
                            acc_w2[i][j] += d_h2[j] * (h1_act[i] / 64.0);
                        }
                    }
                }
                for j in 0..h2_size {
                    if h2_pre[j] > 0.0 {
                        acc_b2[j] += d_h2[j];
                    }
                }
            } else {
                for i in 0..h1_size {
                    if i < self.weights.output_weights.len() {
                        d_h1[i] = d_raw * (self.weights.output_weights[i] as f32 / 64.0);
                    }
                }
            }

            for &feature_idx in &position.active_features {
                if feature_idx >= num_features {
                    continue;
                }
                let entry = acc_input_w
                    .entry(feature_idx)
                    .or_insert_with(|| vec![0.0_f32; h1_size]);
                for i in 0..h1_size {
                    if h1_pre[i] > 0.0 {
                        entry[i] += d_h1[i];
                    }
                }
            }
            for i in 0..h1_size {
                if h1_pre[i] > 0.0 {
                    acc_b1[i] += d_h1[i];
                }
            }
        }

        // Apply accumulated gradients to the f32 shadow (no clamping, no
        // rounding). Sub-i16-unit updates that would have rounded to zero in
        // the old i16-direct path instead accumulate across batches here, and
        // only cross into i16 when the shadow crosses a rounding threshold.
        let lr = self.config.learning_rate;
        let out_scale = self.config.output_grad_scale;
        let in_scale = self.config.input_grad_scale;
        let use_adam = self.config.use_adam;

        if use_adam {
            // Session 15: Adam path. The grad-scale multipliers from the SGD
            // path are still applied to the gradient before m/v accumulation
            // — this lets the bumped-grad recommendation Session 14 found
            // optimal for sigmoid+stm carry over for fair A/B comparison
            // (1e5/1e5 grad scales bumped 5× to surface meaningful learning
            // signal under SGD). Adam's sqrt(v) normalisation will mostly
            // cancel the per-layer-magnitude difference, so the effective
            // step under Adam is `≈ lr * sign(g)` regardless of the grad
            // scale. Recommended config when use_adam=true:
            //   --learning-rate 0.05  --output-grad-scale 1.0  --input-grad-scale 1.0
            // (or any equivalent — Adam absorbs the scale).
            let beta1 = self.config.adam_beta1;
            let beta2 = self.config.adam_beta2;
            let eps = self.config.adam_epsilon;
            let adam = self
                .adam
                .as_mut()
                .expect("AdamState should be allocated when use_adam=true");
            adam.step += 1;
            let t = adam.step;

            for i in 0..self.shadow.output_weights.len() {
                adam_apply(
                    &mut self.shadow.output_weights[i],
                    &mut adam.output_weights_m[i],
                    &mut adam.output_weights_v[i],
                    acc_output_w[i] * out_scale,
                    lr,
                    beta1,
                    beta2,
                    eps,
                    t,
                );
            }
            adam_apply(
                &mut self.shadow.output_bias,
                &mut adam.output_bias_m,
                &mut adam.output_bias_v,
                acc_output_b * out_scale,
                lr,
                beta1,
                beta2,
                eps,
                t,
            );

            if let (Some(ref mut shadow_w2), Some(ref mut adam_w2_m), Some(ref mut adam_w2_v)) = (
                self.shadow.input_weights_2.as_mut(),
                adam.input_weights_2_m.as_mut(),
                adam.input_weights_2_v.as_mut(),
            ) {
                for i in 0..h1_size {
                    for j in 0..h2_size {
                        adam_apply(
                            &mut shadow_w2[i][j],
                            &mut adam_w2_m[i][j],
                            &mut adam_w2_v[i][j],
                            acc_w2[i][j] * out_scale,
                            lr,
                            beta1,
                            beta2,
                            eps,
                            t,
                        );
                    }
                }
            }
            if let (Some(ref mut shadow_b2), Some(ref mut adam_b2_m), Some(ref mut adam_b2_v)) = (
                self.shadow.hidden_biases_2.as_mut(),
                adam.hidden_biases_2_m.as_mut(),
                adam.hidden_biases_2_v.as_mut(),
            ) {
                for j in 0..h2_size {
                    adam_apply(
                        &mut shadow_b2[j],
                        &mut adam_b2_m[j],
                        &mut adam_b2_v[j],
                        acc_b2[j] * out_scale,
                        lr,
                        beta1,
                        beta2,
                        eps,
                        t,
                    );
                }
            }

            for (&feature_idx, grads) in acc_input_w.iter() {
                for i in 0..h1_size {
                    adam_apply(
                        &mut self.shadow.input_weights_1[feature_idx][i],
                        &mut adam.input_weights_1_m[feature_idx][i],
                        &mut adam.input_weights_1_v[feature_idx][i],
                        grads[i] * in_scale,
                        lr,
                        beta1,
                        beta2,
                        eps,
                        t,
                    );
                }
            }
            for i in 0..h1_size {
                adam_apply(
                    &mut self.shadow.hidden_biases_1[i],
                    &mut adam.hidden_biases_1_m[i],
                    &mut adam.hidden_biases_1_v[i],
                    acc_b1[i] * in_scale,
                    lr,
                    beta1,
                    beta2,
                    eps,
                    t,
                );
            }
        } else {
            for i in 0..self.shadow.output_weights.len() {
                self.shadow.output_weights[i] += acc_output_w[i] * lr * out_scale;
            }
            self.shadow.output_bias += acc_output_b * lr * out_scale;

            if let Some(ref mut shadow_w2) = self.shadow.input_weights_2 {
                for i in 0..h1_size {
                    for j in 0..h2_size {
                        shadow_w2[i][j] += acc_w2[i][j] * lr * out_scale;
                    }
                }
            }
            if let Some(ref mut shadow_b2) = self.shadow.hidden_biases_2 {
                for j in 0..h2_size {
                    shadow_b2[j] += acc_b2[j] * lr * out_scale;
                }
            }

            for (&feature_idx, grads) in acc_input_w.iter() {
                for i in 0..h1_size {
                    self.shadow.input_weights_1[feature_idx][i] += grads[i] * lr * in_scale;
                }
            }
            for i in 0..h1_size {
                self.shadow.hidden_biases_1[i] += acc_b1[i] * lr * in_scale;
            }
        }

        // Render shadow → i16 once per batch. Stats reflect actual i16 delta.
        let (total_change, max_change, change_count) =
            self.shadow.quantize_into(&mut self.weights);

        let avg_change = if change_count > 0 {
            total_change / change_count as f32
        } else {
            0.0
        };

        self.stats.positions_processed += positions.len();
        self.stats.weight_updates += count;
        if count > 0 {
            self.stats.avg_td_error = total_error / count as f32;
            self.stats.avg_weight_change = avg_change;
            self.stats.max_weight_change = max_change;
        }
        self.stats.clone()
    }

    /// Run a single supervised-learning pass over a batch of positions whose
    /// `td_target` is already set from an external source (e.g. a teacher-engine
    /// corpus). Unlike `update_weights`, this does not queue positions or run
    /// `compute_td_targets` — the caller is responsible for the target math.
    pub fn train_batch(&mut self, positions: &[TrainingPosition]) -> TrainingStats {
        let mut total_td_error = 0.0_f32;
        let mut total_weight_change = 0.0_f32;
        let mut max_weight_change: f32 = 0.0;
        let mut weight_update_count = 0_usize;

        for position in positions {
            if let Some(td_target) = position.td_target {
                let (td_error, weight_change, max_change) =
                    self.update_weights_for_position(position, td_target);
                total_td_error += td_error.abs();
                total_weight_change += weight_change;
                max_weight_change = max_weight_change.max(max_change);
                weight_update_count += 1;
            }
        }

        self.stats.positions_processed += positions.len();
        self.stats.weight_updates += weight_update_count;
        if weight_update_count > 0 {
            self.stats.avg_td_error = total_td_error / weight_update_count as f32;
            self.stats.avg_weight_change = total_weight_change / weight_update_count as f32;
            self.stats.max_weight_change = max_weight_change;
        }

        self.stats.clone()
    }

    /// Get current weights
    pub fn get_weights(&self) -> &NNUEWeights {
        &self.weights
    }

    /// Get mutable reference to weights.
    ///
    /// Note: external mutation leaves the f32 shadow out of sync. If callers
    /// modify weights and then call `train_batch_accumulated`, the shadow-
    /// driven update will be computed from stale values. Use
    /// `resync_shadow_from_weights` afterwards to rebind.
    pub fn get_weights_mut(&mut self) -> &mut NNUEWeights {
        &mut self.weights
    }

    /// Rebuild the f32 shadow from the current i16 weights. Call after any
    /// external mutation of weights (e.g. loading a checkpoint mid-training).
    pub fn resync_shadow_from_weights(&mut self) {
        self.shadow = ShadowWeights::from_weights(&self.weights);
    }

    /// Read-only access to the f32 shadow (for debugging / diagnostics).
    pub fn get_shadow(&self) -> &ShadowWeights {
        &self.shadow
    }

    /// Add training positions from a game
    pub fn add_training_game(&mut self, game: TrainingGame) {
        let mut positions = self.extract_game_positions(&game);
        let game_length = positions.len();
        self.stats.games_processed += 1;
        self.stats.positions_processed += positions.len(); // Track positions immediately
        self.stats.avg_game_length = (self.stats.avg_game_length * (self.stats.games_processed - 1) as f32 + game_length as f32) / self.stats.games_processed as f32;
        self.stats.avg_positions_per_game = (self.stats.avg_positions_per_game * (self.stats.games_processed - 1) as f32 + game_length as f32) / self.stats.games_processed as f32;
        self.training_positions.extend(positions.drain(..));
        
        // Update weights periodically
        if self.training_positions.len() >= self.config.min_batch_size {
            self.update_weights();
        }
    }

    /// Get training statistics
    pub fn get_stats(&self) -> &TrainingStats {
        &self.stats
    }

    /// Clear accumulated training positions
    pub fn clear_training_data(&mut self) {
        self.training_positions.clear();
    }
}

/// Extract active features from a board position (no side-to-move feature).
pub fn extract_active_features(board: &BitboardBoard) -> Vec<usize> {
    let mut features = Vec::new();

    for row in 0..9 {
        for col in 0..9 {
            let pos = Position::new(row, col);
            if let Some(piece) = board.get_piece(pos) {
                let square_idx = pos.to_u8();
                let feature_idx = feature_index(piece.player, piece.piece_type, square_idx);
                features.push(feature_idx);
            }
        }
    }

    features
}

/// Session 14: extract active features and additionally activate the
/// side-to-move feature when `stm == Black`. The side-to-move feature
/// occupies index `STM_FEATURE_INDEX` (= `NUM_NNUE_FEATURES`).
pub fn extract_active_features_with_stm(
    board: &BitboardBoard,
    stm: Player,
) -> Vec<usize> {
    let mut features = extract_active_features(board);
    if stm == Player::Black {
        features.push(STM_FEATURE_INDEX);
    }
    features
}
