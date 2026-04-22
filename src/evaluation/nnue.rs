//! NNUE (Efficiently Updatable Neural Networks) Evaluation for Shogi
//!
//! This module implements NNUE evaluation, a neural network architecture that
//! allows for efficient incremental updates when making/unmaking moves.
//!
//! # Architecture
//!
//! NNUE uses a sparse input layer where each feature represents a piece-square
//! combination. The network typically has:
//! - Input layer: Sparse features (piece-square combinations)
//! - Hidden layer 1: Typically 256-512 neurons with ReLU activation
//! - Hidden layer 2: Typically 32-256 neurons with ReLU activation (optional)
//! - Output layer: Single value (evaluation score)
//!
//! # Feature Representation
//!
//! Features are indexed as: [player][piece_type][square]
//! - 2 players (Black, White)
//! - 14 piece types (7 base + 7 promoted)
//! - 81 squares (9x9 board)
//! - Total: 2 * 14 * 81 = 2268 features

use crate::bitboards::BitboardBoard;
use crate::types::board::CapturedPieces;
use crate::types::core::{Piece, PieceType, Player, Position};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Number of squares on a Shogi board
const NUM_SQUARES: usize = 81;

/// Number of piece types (including promoted pieces)
const NUM_PIECE_TYPES: usize = 14;

/// Number of players
const NUM_PLAYERS: usize = 2;

/// Total number of NNUE features
pub const NUM_NNUE_FEATURES: usize = NUM_PLAYERS * NUM_PIECE_TYPES * NUM_SQUARES;

/// Default size of first hidden layer
pub const DEFAULT_HIDDEN_SIZE_1: usize = 256;

/// Default size of second hidden layer (0 means no second layer)
pub const DEFAULT_HIDDEN_SIZE_2: usize = 32;

// Stockfish-compatible quantization constants for output scaling.
// These map the integer-domain network output to centipawns.
/// Centipawn scale factor (maps network output range to evaluation range)
const SCALE_FACTOR: i32 = 400;
/// CReLU activation bound (practical maximum from hidden layer activations)
const QUANTIZER_A: i32 = 255;
/// Output layer scaling divisor (used in hidden-to-output computation)
const QUANTIZER_B: i32 = 64;
/// Combined divisor: QUANTIZER_A * QUANTIZER_B = 16320
const FINAL_DIVISOR: i32 = QUANTIZER_A * QUANTIZER_B;

/// Calculate feature index for a piece-square combination
#[inline]
pub fn feature_index(player: Player, piece_type: PieceType, square: u8) -> usize {
    let player_idx = match player {
        Player::Black => 0,
        Player::White => 1,
    };
    let piece_idx = piece_type.as_index();
    player_idx * NUM_PIECE_TYPES * NUM_SQUARES + piece_idx * NUM_SQUARES + square as usize
}

/// NNUE network weights
#[derive(Debug, Clone)]
pub struct NNUEWeights {
    /// Input-to-hidden-1 weights: [NUM_NNUE_FEATURES][hidden_size_1]
    pub input_weights_1: Vec<Vec<i16>>,
    /// Hidden-1 biases: [hidden_size_1]
    pub hidden_biases_1: Vec<i32>,
    /// Hidden-1-to-hidden-2 weights: [hidden_size_1][hidden_size_2] (if hidden_size_2 > 0)
    pub input_weights_2: Option<Vec<Vec<i16>>>,
    /// Hidden-2 biases: [hidden_size_2] (if hidden_size_2 > 0)
    pub hidden_biases_2: Option<Vec<i32>>,
    /// Hidden-2-to-output weights: [hidden_size_2] or [hidden_size_1] if no layer 2
    pub output_weights: Vec<i16>,
    /// Output bias
    pub output_bias: i32,
}

impl NNUEWeights {
    /// Create new weights with random initialization.
    ///
    /// Uses small normal distribution (Stockfish-compatible) instead of uniform
    /// [-128, 128] to prevent accumulator saturation. With sparse inputs
    /// (~20/2268 active features), uniform [-128, 128] produces weight sums
    /// of ~2000 per neuron, immediately saturating ReLU and killing gradients.
    /// Normal(0, 0.01) with 100x quantization gives i16 weights in ~[-3, 3],
    /// keeping accumulator sums small enough for learning.
    pub fn new(hidden_size_1: usize, hidden_size_2: usize) -> Self {
        let mut rng = rand::thread_rng();
        let weight_dist = Normal::new(0.0, 0.01).unwrap();
        let bias_dist = Normal::new(0.0, 10.0).unwrap();

        // Initialize input-to-hidden-1 weights with small normal distribution
        let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
            .map(|_| {
                (0..hidden_size_1)
                    .map(|_| {
                        let w: f64 = weight_dist.sample(&mut rng);
                        (w * 100.0).clamp(-127.0, 127.0) as i16
                    })
                    .collect()
            })
            .collect();

        let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
            .map(|_| {
                let b: f64 = bias_dist.sample(&mut rng);
                b.clamp(-32768.0, 32767.0) as i32
            })
            .collect();

        let (input_weights_2, hidden_biases_2, output_weights) = if hidden_size_2 > 0 {
            let weights_2: Vec<Vec<i16>> = (0..hidden_size_1)
                .map(|_| {
                    (0..hidden_size_2)
                        .map(|_| {
                            let w: f64 = weight_dist.sample(&mut rng);
                            (w * 100.0).clamp(-127.0, 127.0) as i16
                        })
                        .collect()
                })
                .collect();

            let biases_2: Vec<i32> = (0..hidden_size_2)
                .map(|_| {
                    let b: f64 = bias_dist.sample(&mut rng);
                    b.clamp(-32768.0, 32767.0) as i32
                })
                .collect();

            let output: Vec<i16> = (0..hidden_size_2)
                .map(|_| {
                    let w: f64 = weight_dist.sample(&mut rng);
                    (w * 100.0).clamp(-127.0, 127.0) as i16
                })
                .collect();

            (Some(weights_2), Some(biases_2), output)
        } else {
            let output: Vec<i16> = (0..hidden_size_1)
                .map(|_| {
                    let w: f64 = weight_dist.sample(&mut rng);
                    (w * 100.0).clamp(-127.0, 127.0) as i16
                })
                .collect();
            (None, None, output)
        };

        let output_bias = {
            let b: f64 = bias_dist.sample(&mut rng);
            b.clamp(-32768.0, 32767.0) as i32
        };

        Self {
            input_weights_1,
            hidden_biases_1,
            input_weights_2,
            hidden_biases_2,
            output_weights,
            output_bias,
        }
    }

    /// Load weights from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, NNUEError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let file_data: NNUEWeightFile = serde_json::from_reader(reader)?;

        Ok(Self {
            input_weights_1: file_data.input_weights_1,
            hidden_biases_1: file_data.hidden_biases_1,
            input_weights_2: file_data.input_weights_2,
            hidden_biases_2: file_data.hidden_biases_2,
            output_weights: file_data.output_weights,
            output_bias: file_data.output_bias,
        })
    }

    /// Save weights to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), NNUEError> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);

        let file_data = NNUEWeightFile {
            input_weights_1: self.input_weights_1.clone(),
            hidden_biases_1: self.hidden_biases_1.clone(),
            input_weights_2: self.input_weights_2.clone(),
            hidden_biases_2: self.hidden_biases_2.clone(),
            output_weights: self.output_weights.clone(),
            output_bias: self.output_bias,
        };

        serde_json::to_writer_pretty(writer, &file_data)?;
        Ok(())
    }

    /// Get hidden layer sizes
    pub fn hidden_sizes(&self) -> (usize, usize) {
        let size_1 = self.hidden_biases_1.len();
        let size_2 = self.hidden_biases_2.as_ref().map(|b| b.len()).unwrap_or(0);
        (size_1, size_2)
    }
}

/// Serializable weight file format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NNUEWeightFile {
    input_weights_1: Vec<Vec<i16>>,
    hidden_biases_1: Vec<i32>,
    input_weights_2: Option<Vec<Vec<i16>>>,
    hidden_biases_2: Option<Vec<i32>>,
    output_weights: Vec<i16>,
    output_bias: i32,
}

/// NNUE accumulator for efficient incremental updates
/// Stores the accumulated hidden layer values
#[derive(Debug, Clone)]
pub struct NNUEAccumulator {
    /// Accumulated values for hidden layer 1: [hidden_size_1]
    pub hidden_1: Vec<i32>,
    /// Accumulated values for hidden layer 2 (if present): [hidden_size_2]
    pub hidden_2: Option<Vec<i32>>,
    /// Size of hidden layer 1 (used internally)
    #[allow(dead_code)]
    hidden_size_1: usize,
    /// Size of hidden layer 2 (0 if not present, used internally)
    #[allow(dead_code)]
    hidden_size_2: usize,
}

impl NNUEAccumulator {
    /// Create a new accumulator with given hidden layer sizes
    pub fn new(hidden_size_1: usize, hidden_size_2: usize) -> Self {
        Self {
            hidden_1: vec![0; hidden_size_1],
            hidden_2: if hidden_size_2 > 0 { Some(vec![0; hidden_size_2]) } else { None },
            hidden_size_1,
            hidden_size_2,
        }
    }

    /// Refresh accumulator from scratch for a position
    pub fn refresh(&mut self, board: &BitboardBoard, weights: &NNUEWeights) {
        // Reset accumulator
        self.hidden_1.fill(0);
        if let Some(ref mut h2) = self.hidden_2 {
            h2.fill(0);
        }

        // Add contributions from all pieces on the board
        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    self.add_piece(piece, pos, weights);
                }
            }
        }
    }

    /// Add a piece's contribution to the accumulator
    pub fn add_piece(&mut self, piece: Piece, square: Position, weights: &NNUEWeights) {
        let square_idx = square.to_u8();
        let feature_idx = feature_index(piece.player, piece.piece_type, square_idx);

        // Accumulate into hidden layer 1
        for (i, &weight) in weights.input_weights_1[feature_idx].iter().enumerate() {
            self.hidden_1[i] += weight as i32;
        }
    }

    /// Remove a piece's contribution from the accumulator
    pub fn remove_piece(&mut self, piece: Piece, square: Position, weights: &NNUEWeights) {
        let square_idx = square.to_u8();
        let feature_idx = feature_index(piece.player, piece.piece_type, square_idx);

        // Remove from hidden layer 1
        for (i, &weight) in weights.input_weights_1[feature_idx].iter().enumerate() {
            self.hidden_1[i] -= weight as i32;
        }
    }

    /// Update accumulator for a move
    pub fn update_move(
        &mut self,
        from: Option<Position>,
        to: Position,
        moved_piece: Piece,
        captured_piece: Option<Piece>,
        weights: &NNUEWeights,
    ) {
        // Remove piece from source square (if not a drop)
        if let Some(from_sq) = from {
            self.remove_piece(moved_piece, from_sq, weights);
        }

        // Remove captured piece if any
        if let Some(captured) = captured_piece {
            self.remove_piece(captured, to, weights);
        }

        // Add moved piece to destination square
        self.add_piece(moved_piece, to, weights);
    }

    /// Evaluate the position using the accumulator
    pub fn evaluate(&self, weights: &NNUEWeights) -> i32 {
        // Apply ReLU to hidden layer 1
        let activated_1: Vec<i32> = self
            .hidden_1
            .iter()
            .zip(weights.hidden_biases_1.iter())
            .map(|(&h, &bias)| (h + bias).max(0))
            .collect();

        // Apply second hidden layer if present
        let final_values = if let (Some(ref weights_2), Some(ref biases_2)) =
            (weights.input_weights_2.as_ref(), weights.hidden_biases_2.as_ref())
        {
            // Compute hidden layer 2 from activated layer 1
            let hidden_size_2 = biases_2.len();
            let mut h2_values = vec![0i32; hidden_size_2];
            for (i, &act_1) in activated_1.iter().enumerate() {
                for (j, &weight) in weights_2[i].iter().enumerate() {
                    h2_values[j] += (act_1 * weight as i32) / 64; // Scale down to prevent overflow
                }
            }

            // Apply ReLU and bias to hidden layer 2
            h2_values
                .iter()
                .zip(biases_2.iter())
                .map(|(&h, &bias)| (h + bias).max(0))
                .collect()
        } else {
            activated_1
        };

        // Compute output
        let mut output = weights.output_bias;
        for (i, &value) in final_values.iter().enumerate() {
            output += (value * weights.output_weights[i] as i32) / 64;
        }

        // Stockfish-compatible quantization to centipawns:
        //   output_cp = (raw_output * 400) / (255 * 64)
        //
        // This ensures network output naturally maps to a bounded centipawn
        // range (typically [-300, 300]) compatible with search heuristics
        // (aspiration windows, move ordering, time management).
        (output * SCALE_FACTOR) / FINAL_DIVISOR
    }
}

/// NNUE evaluator
pub struct NNUEEvaluator {
    /// Network weights
    weights: NNUEWeights,
    /// Accumulator for the current position
    accumulator: NNUEAccumulator,
    /// Whether NNUE is enabled
    enabled: bool,
}

impl NNUEEvaluator {
    /// Create a new NNUE evaluator with random weights
    pub fn new(hidden_size_1: usize, hidden_size_2: usize) -> Self {
        let weights = NNUEWeights::new(hidden_size_1, hidden_size_2);
        let accumulator = NNUEAccumulator::new(hidden_size_1, hidden_size_2);
        Self {
            weights,
            accumulator,
            enabled: true,
        }
    }

    /// Create NNUE evaluator from loaded weights
    pub fn from_weights(weights: NNUEWeights) -> Self {
        let (size_1, size_2) = weights.hidden_sizes();
        let accumulator = NNUEAccumulator::new(size_1, size_2);
        Self {
            weights,
            accumulator,
            enabled: true,
        }
    }

    /// Load NNUE evaluator from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, NNUEError> {
        let weights = NNUEWeights::load(path)?;
        Ok(Self::from_weights(weights))
    }

    /// Evaluate a position
    pub fn evaluate(
        &mut self,
        board: &BitboardBoard,
        _player: Player,
        _captured_pieces: &CapturedPieces,
    ) -> i32 {
        if !self.enabled {
            return 0;
        }

        // Refresh accumulator from current board position
        self.accumulator.refresh(board, &self.weights);

        // Evaluate using accumulator
        self.accumulator.evaluate(&self.weights)
    }

    /// Update accumulator for a move (for incremental evaluation)
    /// Note: This requires the piece type at the source square, which should be passed separately
    pub fn update_move_incremental(
        &mut self,
        from: Option<Position>,
        to: Position,
        moved_piece: Piece,
        captured_piece: Option<Piece>,
    ) {
        if !self.enabled {
            return;
        }

        self.accumulator.update_move(from, to, moved_piece, captured_piece, &self.weights);
    }

    /// Refresh accumulator for a new position (full refresh)
    pub fn refresh_position(&mut self, board: &BitboardBoard) {
        if !self.enabled {
            return;
        }
        self.accumulator.refresh(board, &self.weights);
    }

    /// Enable or disable NNUE evaluation
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if NNUE is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Save weights to file
    pub fn save_weights<P: AsRef<Path>>(&self, path: P) -> Result<(), NNUEError> {
        self.weights.save(path)
    }

    /// Get reference to weights (for training)
    pub fn get_weights(&self) -> &NNUEWeights {
        &self.weights
    }

    /// Get mutable reference to weights (for training)
    pub fn get_weights_mut(&mut self) -> &mut NNUEWeights {
        &mut self.weights
    }
}

/// NNUE-related errors
#[derive(Debug, thiserror::Error)]
pub enum NNUEError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid weight file format")]
    InvalidFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_index() {
        // Test feature indexing
        let idx1 = feature_index(Player::Black, PieceType::King, 0);
        let idx2 = feature_index(Player::Black, PieceType::King, 1);
        assert_ne!(idx1, idx2);

        let idx3 = feature_index(Player::White, PieceType::King, 0);
        assert_ne!(idx1, idx3);

        let idx4 = feature_index(Player::Black, PieceType::Pawn, 0);
        assert_ne!(idx1, idx4);
    }

    #[test]
    fn test_nnue_weights() {
        let weights = NNUEWeights::new(256, 32);
        assert_eq!(weights.input_weights_1.len(), NUM_NNUE_FEATURES);
        assert_eq!(weights.hidden_biases_1.len(), 256);
        assert_eq!(weights.hidden_biases_2.as_ref().unwrap().len(), 32);
        assert_eq!(weights.output_weights.len(), 32);
    }

    #[test]
    fn test_nnue_accumulator() {
        let weights = NNUEWeights::new(256, 32);
        let mut acc = NNUEAccumulator::new(256, 32);
        assert_eq!(acc.hidden_1.len(), 256);
        assert_eq!(acc.hidden_2.as_ref().unwrap().len(), 32);
    }
}

