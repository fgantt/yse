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
    /// Position evaluation from NNUE
    pub evaluation: i32,
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

    /// Compute TD(λ) targets for positions
    fn compute_td_targets(
        &self,
        positions: &mut [TrainingPosition],
        final_result: GameResult,
    ) {
        if positions.is_empty() {
            return;
        }

        // Convert final result to score from Black's perspective
        let final_score = match final_result {
            GameResult::Win => 1.0,      // Black wins
            GameResult::Loss => -1.0,    // White wins
            GameResult::Draw => 0.0,
        };

        let n = positions.len();
        
        // TD(λ) computation with eligibility traces
        // For simplicity, we use TD(0) approach with λ-weighted future values
        let mut next_value = final_score;

        for i in (0..n).rev() {
            let player_score_multiplier = if positions[i].player == Player::Black { 1.0 } else { -1.0 };
            
            // Convert evaluation to [-1, 1] range (approximate, assuming max eval around 20000 centipawns)
            let current_value = (positions[i].evaluation as f32 / 20000.0).tanh() * player_score_multiplier;
            
            // TD error: δ = r + γ*V(s') - V(s)
            // For terminal: δ = final_outcome - V(s)
            let td_error = if i == n - 1 {
                // Terminal position
                final_score * player_score_multiplier - current_value
            } else {
                // Non-terminal: use next position's value
                next_value - current_value
            };

            // TD(λ) update: V(s) += α * δ * e(s)
            // For simplicity, we store the TD target directly
            positions[i].td_target = Some(current_value + self.config.learning_rate * td_error);

            // Update next_value using λ-weighting
            next_value = self.config.lambda * next_value + (1.0 - self.config.lambda) * current_value;
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

    /// Update weights for a single position using TD error
    /// Returns (td_error, avg_weight_change, max_weight_change)
    fn update_weights_for_position(&mut self, position: &TrainingPosition, target_value: f32) -> (f32, f32, f32) {
        // Get current prediction
        let current_prediction = (position.evaluation as f32 / 20000.0).tanh();
        let player_multiplier = if position.player == Player::Black { 1.0 } else { -1.0 };
        let predicted_value = current_prediction * player_multiplier;
        
        // TD error
        let error = target_value - predicted_value;
        let learning_rate = self.config.learning_rate;

        // Track weight changes
        let mut total_change = 0.0;
        let mut max_change: f32 = 0.0;
        let mut change_count = 0;

        // Update output layer weights (simplified gradient descent)
        let (hidden_size_1, _hidden_size_2) = self.weights.hidden_sizes();

        // Compute activated hidden layer values
        let mut hidden_1_activated = vec![0i32; hidden_size_1];
        for (i, (&h, &bias)) in position.accumulator.hidden_1.iter()
            .zip(self.weights.hidden_biases_1.iter())
            .enumerate()
        {
            hidden_1_activated[i] = (h + bias).max(0);
        }

        let final_values: Vec<i32> = if let Some(ref h2) = position.accumulator.hidden_2 {
            // Second layer present
            let h2_len = h2.len();
            let mut h2_values = vec![0i32; h2_len];
            if let (Some(ref w2), Some(ref b2)) = (self.weights.input_weights_2.as_ref(), self.weights.hidden_biases_2.as_ref()) {
                for (i, &act_1) in hidden_1_activated.iter().enumerate() {
                    for (j, &weight) in w2[i].iter().enumerate() {
                        h2_values[j] += (act_1 * weight as i32) / 64;
                    }
                }
                // Apply ReLU and bias
                for (i, &bias) in b2.iter().enumerate() {
                    h2_values[i] = (h2_values[i] + bias).max(0);
                }
            }
            h2_values
        } else {
            hidden_1_activated.clone()
        };

        // Update output weights (gradient descent on output layer)
        for (i, &value) in final_values.iter().enumerate() {
            if i < self.weights.output_weights.len() {
                let gradient = error * (value as f32 / 64.0);
                // Use learning rate directly (removed 10x multiplier to prevent weight explosion)
                let weight_update = (gradient * learning_rate) as i16;
                let old_weight = self.weights.output_weights[i];
                self.weights.output_weights[i] = self.weights.output_weights[i]
                    .saturating_add(weight_update)
                    .max(-32768)
                    .min(32767);
                let change = (self.weights.output_weights[i] - old_weight).abs() as f32;
                total_change += change;
                max_change = max_change.max(change);
                change_count += 1;
            }
        }

        // Update output bias
        let bias_gradient = error;
        // Reduced multiplier from 160x to 10x to prevent bias explosion
        let bias_update = (bias_gradient * learning_rate * 10.0) as i32;
        let old_bias = self.weights.output_bias;
        self.weights.output_bias += bias_update;
        let change = (self.weights.output_bias - old_bias).abs() as f32;
        total_change += change;
        max_change = max_change.max(change);
        change_count += 1;

        // Update input-to-hidden weights (simplified - only for active features)
        // This is a simplified update - full backpropagation would update all layers
        for &feature_idx in &position.active_features {
            if feature_idx < self.weights.input_weights_1.len() {
                for (i, &hidden_val) in hidden_1_activated.iter().enumerate() {
                    if hidden_val > 0 && i < self.weights.input_weights_1[feature_idx].len() {
                        // Simplified gradient update - scale up for meaningful changes
                        let gradient = error * (hidden_val as f32 / 64.0) * learning_rate * 0.01;
                        let weight_update = gradient as i16;
                        let old_weight = self.weights.input_weights_1[feature_idx][i];
                        self.weights.input_weights_1[feature_idx][i] = self.weights.input_weights_1[feature_idx][i]
                            .saturating_add(weight_update)
                            .max(-32768)
                            .min(32767);
                        let change = (self.weights.input_weights_1[feature_idx][i] - old_weight).abs() as f32;
                        total_change += change;
                        max_change = max_change.max(change);
                        change_count += 1;
                    }
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
