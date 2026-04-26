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
#[allow(dead_code)]
const QUANTIZER_A: i32 = 255;
/// Output layer scaling divisor (used in hidden-to-output computation)
#[allow(dead_code)]
const QUANTIZER_B: i32 = 64;
/// Output mapping divisor: `cp = output * SCALE_FACTOR / OUTPUT_DIVISOR`,
/// equivalently `prediction = tanh(raw_output / OUTPUT_DIVISOR)`.
///
/// Session 11 lowered this from `QUANTIZER_A * QUANTIZER_B = 16320` to 4080.
/// At the old divisor the per-position contribution `Σ (h2_act · output_w) >> 6`
/// (~±6350 in practice) only reached `tanh(6350/16320) ≈ 0.37` in prediction
/// space — well below the supervised targets of ±0.69+ for decisive teacher
/// evals at `target_eval_scale ∈ {600, 2400}`. Lowering the divisor 4× lets
/// per-position variance reach `tanh(6350/4080) ≈ 0.85`, restoring the
/// network's ability to learn position-discriminating cp values.
/// See `docs/nnue-phase2/SESSION_LOG_011.md`.
pub const OUTPUT_DIVISOR: i32 = 4080;

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

    /// Evaluate the position using the accumulator.
    ///
    /// Forward pass through the network: hidden_1 -> (ReLU) -> hidden_2 -> (ReLU) -> output.
    /// Optimized to avoid heap allocations by using a stack-allocated array for the
    /// second hidden layer (max 32 neurons in the default architecture).
    pub fn evaluate(&self, weights: &NNUEWeights) -> i32 {
        // Apply ReLU to hidden layer 1: activated[i] = max(0, hidden_1[i] + bias[i])
        let activated_1: Vec<i32> = self
            .hidden_1
            .iter()
            .zip(weights.hidden_biases_1.iter())
            .map(|(&h, &bias)| (h + bias).max(0))
            .collect();

        // Apply second hidden layer if present
        if let (Some(ref weights_2), Some(ref biases_2)) =
            (weights.input_weights_2.as_ref(), weights.hidden_biases_2.as_ref())
        {
            // Compute hidden layer 2 from activated layer 1, then output
            // in a single pass to avoid a second allocation.
            let hidden_size_2 = biases_2.len();
            debug_assert!(hidden_size_2 <= 32, "Hidden layer 2 size exceeds stack buffer");
            let mut h2_values = [0i32; 32];

            for (i, &act_1) in activated_1.iter().enumerate() {
                if act_1 == 0 {
                    continue; // Skip zero activations (ReLU killed)
                }
                for (j, &weight) in weights_2[i].iter().enumerate() {
                    h2_values[j] += (act_1 * weight as i32) >> 6;
                }
            }

            // Fused: ReLU(h2 + bias) dot output_weights, avoiding a second allocation
            let mut output = weights.output_bias;
            for j in 0..hidden_size_2 {
                let h2_activated = (h2_values[j] + biases_2[j]).max(0);
                output += (h2_activated * weights.output_weights[j] as i32) >> 6;
            }

            (output * SCALE_FACTOR) / OUTPUT_DIVISOR
        } else {
            // No second hidden layer: dot activated_1 with output_weights
            let mut output = weights.output_bias;
            for (&act, &weight) in activated_1.iter().zip(weights.output_weights.iter()) {
                output += (act * weight as i32) >> 6;
            }

            (output * SCALE_FACTOR) / OUTPUT_DIVISOR
        }
    }
}

/// NNUE evaluator with stack-based accumulator for incremental search updates.
///
/// During alpha-beta search, the engine makes and unmakes moves. Instead of
/// recomputing the accumulator from scratch on every evaluation (O(81) piece scan),
/// we maintain a stack of accumulator states. On make_move, we push the current
/// state and incrementally update; on unmake_move, we pop to restore.
///
/// This reduces per-evaluation cost from ~2.0µs (full refresh + forward pass)
/// to ~1.4µs (incremental update + forward pass), a ~30% speedup.
pub struct NNUEEvaluator {
    /// Network weights
    weights: NNUEWeights,
    /// Accumulator for the current position
    accumulator: NNUEAccumulator,
    /// Stack of accumulator hidden_1 states for make/unmake during search.
    /// Each entry is a snapshot of hidden_1 before a make_move.
    accumulator_stack: Vec<Vec<i32>>,
    /// Whether NNUE is enabled
    enabled: bool,
    /// Whether the accumulator needs a full refresh (out of sync with board).
    /// Set to true initially and after any operation that invalidates the state.
    needs_refresh: bool,
}

impl NNUEEvaluator {
    /// Create a new NNUE evaluator with random weights
    pub fn new(hidden_size_1: usize, hidden_size_2: usize) -> Self {
        let weights = NNUEWeights::new(hidden_size_1, hidden_size_2);
        let accumulator = NNUEAccumulator::new(hidden_size_1, hidden_size_2);
        Self {
            weights,
            accumulator,
            accumulator_stack: Vec::with_capacity(128), // Typical max search depth
            enabled: true,
            needs_refresh: true,
        }
    }

    /// Create NNUE evaluator from loaded weights
    pub fn from_weights(weights: NNUEWeights) -> Self {
        let (size_1, size_2) = weights.hidden_sizes();
        let accumulator = NNUEAccumulator::new(size_1, size_2);
        Self {
            weights,
            accumulator,
            accumulator_stack: Vec::with_capacity(128),
            enabled: true,
            needs_refresh: true,
        }
    }

    /// Load NNUE evaluator from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, NNUEError> {
        let weights = NNUEWeights::load(path)?;
        Ok(Self::from_weights(weights))
    }

    /// Evaluate a position (full refresh - legacy path).
    ///
    /// This performs a full O(81) piece scan to rebuild the accumulator.
    /// Prefer `evaluate_incremental` when the accumulator is already in sync
    /// via make_move/unmake_move calls.
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
        self.needs_refresh = false;

        // Evaluate using accumulator
        self.accumulator.evaluate(&self.weights)
    }

    /// Evaluate using the current accumulator state (no refresh).
    ///
    /// This is the fast path used during search when the accumulator is kept
    /// in sync via `nnue_make_move` / `nnue_unmake_move`. If the accumulator
    /// needs a refresh (e.g., after a new position is set), falls back to
    /// full refresh.
    pub fn evaluate_incremental(
        &mut self,
        board: &BitboardBoard,
    ) -> i32 {
        if !self.enabled {
            return 0;
        }

        if self.needs_refresh {
            self.accumulator.refresh(board, &self.weights);
            self.needs_refresh = false;
        }

        self.accumulator.evaluate(&self.weights)
    }

    /// Push accumulator state and apply a move incrementally.
    ///
    /// Called by the search engine after `board.make_move_with_info()`.
    /// The `MoveInfo`-equivalent fields describe what changed on the board.
    ///
    /// - `from`: Source square (None for drops)
    /// - `to`: Destination square
    /// - `moved_piece`: The piece that moved (after promotion if applicable)
    /// - `original_piece`: The piece before promotion (same as moved_piece if no promotion)
    /// - `captured_piece`: Piece captured at destination (if any)
    /// - `was_promotion`: Whether the move involved a promotion
    pub fn nnue_make_move(
        &mut self,
        from: Option<Position>,
        to: Position,
        moved_piece: Piece,
        original_piece: Piece,
        captured_piece: Option<Piece>,
        was_promotion: bool,
    ) {
        if !self.enabled || self.needs_refresh {
            return;
        }

        // Save current hidden_1 state to stack
        self.accumulator_stack.push(self.accumulator.hidden_1.clone());

        // Remove piece from source square (if board move, not drop)
        if let Some(from_sq) = from {
            // Remove the original piece (before promotion) from the source
            self.accumulator.remove_piece(original_piece, from_sq, &self.weights);
        }

        // Remove captured piece if any
        if let Some(captured) = captured_piece {
            self.accumulator.remove_piece(captured, to, &self.weights);
        }

        // Add the moved piece at the destination (after promotion)
        if was_promotion {
            self.accumulator.add_piece(moved_piece, to, &self.weights);
        } else {
            self.accumulator.add_piece(original_piece, to, &self.weights);
        }
    }

    /// Pop accumulator state after unmaking a move.
    ///
    /// Called by the search engine after `board.unmake_move()`.
    /// Restores the accumulator to the state before the corresponding make_move.
    pub fn nnue_unmake_move(&mut self) {
        if !self.enabled {
            return;
        }

        if let Some(prev_hidden_1) = self.accumulator_stack.pop() {
            self.accumulator.hidden_1 = prev_hidden_1;
        } else {
            // Stack underflow — mark as needing refresh
            self.needs_refresh = true;
        }
    }

    /// Full refresh of the accumulator for a new position.
    ///
    /// Called when the board state changes outside of the make/unmake cycle
    /// (e.g., new game, setting position from FEN, at search root).
    pub fn refresh_accumulator(&mut self, board: &BitboardBoard) {
        if !self.enabled {
            return;
        }
        self.accumulator.refresh(board, &self.weights);
        self.accumulator_stack.clear();
        self.needs_refresh = false;
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
        self.needs_refresh = false;
    }

    /// Mark the accumulator as needing a full refresh.
    /// Use this when the board state changes in a way that can't be tracked incrementally.
    pub fn invalidate(&mut self) {
        self.needs_refresh = true;
        self.accumulator_stack.clear();
    }

    /// Check if the accumulator needs a full refresh
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh
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
        let acc = NNUEAccumulator::new(256, 32);
        assert_eq!(acc.hidden_1.len(), 256);
        assert_eq!(acc.hidden_2.as_ref().unwrap().len(), 32);
    }

    #[test]
    fn test_nnue_incremental_matches_full_refresh() {
        use crate::bitboards::BitboardBoard;
        use crate::moves::MoveGenerator;
        use crate::types::board::CapturedPieces;

        let weights = NNUEWeights::new(256, 32);
        let mut board = BitboardBoard::new();
        let mut captured_pieces = CapturedPieces::new();

        // Evaluate the starting position with a full refresh
        let mut evaluator = NNUEEvaluator::from_weights(weights.clone());
        evaluator.refresh_accumulator(&board);
        let score_before = evaluator.evaluate_incremental(&board);

        // Make a move incrementally
        let move_gen = MoveGenerator::new();
        let legal_moves = move_gen.generate_legal_moves(&board, Player::Black, &captured_pieces);
        assert!(!legal_moves.is_empty(), "Should have legal moves from start position");

        let mv = &legal_moves[0];
        let move_info = board.make_move_with_info(mv);

        // Update NNUE incrementally
        let original_piece = Piece::new(move_info.original_piece_type, move_info.player);
        let moved_piece = if move_info.was_promotion {
            if let Some(pt) = move_info.original_piece_type.promoted_version() {
                Piece::new(pt, move_info.player)
            } else {
                original_piece
            }
        } else {
            original_piece
        };
        evaluator.nnue_make_move(
            move_info.from,
            move_info.to,
            moved_piece,
            original_piece,
            move_info.captured_piece,
            move_info.was_promotion,
        );
        let score_incremental = evaluator.evaluate_incremental(&board);

        // Now do a full refresh on the same position for comparison
        let mut fresh_evaluator = NNUEEvaluator::from_weights(weights.clone());
        fresh_evaluator.refresh_accumulator(&board);
        let score_full_refresh = fresh_evaluator.evaluate_incremental(&board);

        assert_eq!(
            score_incremental, score_full_refresh,
            "Incremental evaluation ({}) should match full refresh ({})",
            score_incremental, score_full_refresh
        );

        // Unmake and verify we get back the original score
        evaluator.nnue_unmake_move();
        board.unmake_move(&move_info);
        let score_after_unmake = evaluator.evaluate_incremental(&board);

        assert_eq!(
            score_before, score_after_unmake,
            "Score after unmake ({}) should match original ({})",
            score_after_unmake, score_before
        );
    }

    /// Session 13 Issue-4 verification: with the actual trained weights file,
    /// `acc.evaluate()` after a fresh refresh should produce the same cp value
    /// as `NNUEEvaluator::evaluate_incremental()` on the same position.
    /// Skipped if the weights file isn't present in the worktree.
    #[test]
    fn test_eval_paths_agree_on_trained_weights() {
        use crate::bitboards::BitboardBoard;
        use std::path::Path;
        let weights_path = "nnue_weights_trained.json";
        if !Path::new(weights_path).exists() {
            eprintln!("skipping: {} not found", weights_path);
            return;
        }
        let weights = NNUEWeights::load(weights_path).expect("load weights");
        let fens = [
            ("startpos", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
            (
                "mid-game",
                "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
            ),
            ("black-winning", "4k4/9/4S4/9/9/9/9/9/4K4 b GGBB 1"),
        ];
        for (name, fen) in fens.iter() {
            let (board, _p, _cap) = BitboardBoard::from_fen(fen).expect("from_fen");

            // Path A: diagnostic — fresh accumulator, refresh, evaluate
            let (h1, h2) = weights.hidden_sizes();
            let mut acc = NNUEAccumulator::new(h1, h2);
            acc.refresh(&board, &weights);
            let path_a = acc.evaluate(&weights);

            // Path B: search-time — NNUEEvaluator + evaluate_incremental
            let mut evaluator = NNUEEvaluator::from_weights(weights.clone());
            let path_b = evaluator.evaluate_incremental(&board);

            println!("  {:<14} path_a={:>+6}  path_b={:>+6}", name, path_a, path_b);
            assert_eq!(
                path_a, path_b,
                "{}: diagnostic acc.evaluate ({}) != search-time evaluate_incremental ({})",
                name, path_a, path_b
            );
        }
    }
}

