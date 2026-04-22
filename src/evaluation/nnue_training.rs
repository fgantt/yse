//! NNUE Training Module
//!
//! This module implements self-play training for NNUE weights using TD(λ) learning.

use crate::bitboards::BitboardBoard;
use crate::evaluation::nnue::{NNUEAccumulator, NNUEWeights, feature_index};
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
}

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

/// NNUE Trainer
pub struct NNUETrainer {
    /// Current NNUE weights
    weights: NNUEWeights,
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
        Self {
            weights,
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

        // Scale to centipawns: output_cp = raw_output * 400 / 16320
        let output_cp = raw_output * 400.0 / 16320.0;

        // Map to [-1, 1] using sigmoid-like scaling: prediction = tanh(output_cp / 400)
        // Using /400 instead of /600 gives a wider active gradient region for small evals.
        let prediction = (output_cp / 400.0).tanh();

        // === Backward pass ===

        // Loss = 0.5 * (target - prediction)^2
        // d(loss)/d(prediction) = -(target - prediction) = prediction - target
        let error = target_value - prediction;

        // d(prediction)/d(output_cp) = (1 - prediction^2) / 400
        let tanh_deriv = (1.0 - prediction * prediction) / 400.0;

        // d(output_cp)/d(raw_output) = 400 / 16320
        let scale_deriv = 400.0 / 16320.0;

        // d(loss)/d(raw_output) = -error * tanh_deriv * scale_deriv
        // We want to MINIMIZE loss, so update = -d(loss)/d(w) = error * chain
        let d_raw = error * tanh_deriv * scale_deriv;

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
                let update_f = gradient * learning_rate * 1e7;
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
        let bias_update_f = d_raw * learning_rate * 1e7;
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
                            let update_f = grad * learning_rate * 1e7;
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
                        let update_f = bias_grad * learning_rate * 1e7;
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
                        let update_f = grad * learning_rate * 1e6;
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
                let update_f = grad * learning_rate * 1e6;
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

    /// Get current weights
    pub fn get_weights(&self) -> &NNUEWeights {
        &self.weights
    }

    /// Get mutable reference to weights
    pub fn get_weights_mut(&mut self) -> &mut NNUEWeights {
        &mut self.weights
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

/// Extract active features from a board position
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
