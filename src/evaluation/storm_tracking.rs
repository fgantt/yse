//! Pawn Storm Tracking Module
//!
//! This module provides comprehensive storm-state tracking for pawn storms,
//! including consecutive pushes, file ownership, and time since response.
//! Used by both evaluation and search layers to detect and respond to
//! threatening pawn advances.
//!
//! Task 4.1: Define storm-state tracking structs accessible to evaluation and search layers.

use crate::bitboards::BitboardBoard;
use crate::types::core::{PieceType, Player, Position};
use serde::{Deserialize, Serialize};

/// Comprehensive storm state tracking for a single file
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FileStormState {
    /// Number of consecutive pawn pushes on this file by the opponent
    pub consecutive_pushes: u8,
    /// Deepest penetration (how far the pawn has advanced)
    pub deepest_penetration: u8,
    /// File ownership: true if opponent controls this file
    pub opponent_owns_file: bool,
    /// Number of plies since last defensive response on this file
    pub plies_since_response: u8,
    /// Whether this file is a critical file (edge files 1, 9 or central files 4, 5, 6)
    pub is_critical_file: bool,
}

impl FileStormState {
    /// Calculate storm severity for this file (0.0 to 1.0+)
    pub fn severity(&self) -> f32 {
        let base_severity = self.consecutive_pushes as f32 * 0.3
            + self.deepest_penetration as f32 * 0.4
            + self.plies_since_response as f32 * 0.1;

        let critical_multiplier = if self.is_critical_file { 1.5 } else { 1.0 };
        let ownership_multiplier = if self.opponent_owns_file { 1.3 } else { 1.0 };

        base_severity * critical_multiplier * ownership_multiplier
    }

    /// Check if this file has an active storm
    pub fn is_active(&self) -> bool {
        self.consecutive_pushes > 0 && self.deepest_penetration >= 1
    }
}

/// Comprehensive storm state tracking across all files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StormState {
    /// Storm state for each file (0-8, indexed by column)
    pub file_states: [FileStormState; 9],
    /// Overall storm severity (sum of all file severities)
    pub total_severity: f32,
    /// Number of files with active storms
    pub active_file_count: u8,
    /// Whether center files (4, 5) are under pressure
    pub center_pressure: bool,
    /// Whether edge files (1, 9) are under pressure
    pub edge_pressure: bool,
    /// Most critical file (highest severity)
    pub most_critical_file: Option<u8>,
}

impl StormState {
    /// Create a new empty storm state
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze the board and update storm state for the given player
    ///
    /// This method detects opponent pawn advances and tracks storm metrics.
    pub fn analyze(
        &mut self,
        board: &BitboardBoard,
        player: Player,
        _move_count: u32,
        last_storm_state: Option<&StormState>,
    ) {
        let opponent = player.opposite();
        let mut total_severity = 0.0;
        let mut active_count = 0;
        let mut center_pressure = false;
        let mut edge_pressure = false;
        let mut most_critical_file: Option<u8> = None;
        let mut max_severity = 0.0;

        // Reset file states
        self.file_states = [FileStormState::default(); 9];

        // Analyze each file
        for col in 0..9 {
            let file_state = &mut self.file_states[col as usize];
            file_state.is_critical_file = col == 0 || col == 8 || (col >= 3 && col <= 5);

            // Find opponent pawns on this file
            let mut deepest_pawn_row: Option<u8> = None;
            let mut has_opponent_pawn = false;
            let mut has_friendly_blocker = false;

            for row in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.piece_type == PieceType::Pawn {
                        if piece.player == opponent {
                            has_opponent_pawn = true;
                            // Calculate penetration
                            let penetration = match player {
                                Player::Black => {
                                    if row < 5 {
                                        Some(5 - row)
                                    } else {
                                        None
                                    }
                                }
                                Player::White => {
                                    if row > 3 {
                                        Some(row - 3)
                                    } else {
                                        None
                                    }
                                }
                            };

                            if let Some(pen) = penetration {
                                if deepest_pawn_row.is_none() || pen > file_state.deepest_penetration {
                                    deepest_pawn_row = Some(row);
                                    file_state.deepest_penetration = pen;
                                }
                            }
                        } else if piece.player == player {
                            // Friendly pawn or gold/silver blocking
                            has_friendly_blocker = true;
                        }
                    } else if piece.player == player
                        && matches!(piece.piece_type, PieceType::Gold | PieceType::Silver)
                    {
                        // Friendly gold/silver can block
                        has_friendly_blocker = true;
                    }
                }
            }

            // Determine file ownership (opponent has more pawns/control)
            file_state.opponent_owns_file = has_opponent_pawn && !has_friendly_blocker;

            // Track consecutive pushes
            if let Some(prev_state) = last_storm_state {
                let prev_file_state = &prev_state.file_states[col as usize];
                if file_state.deepest_penetration > prev_file_state.deepest_penetration {
                    // Pawn advanced further
                    file_state.consecutive_pushes = prev_file_state.consecutive_pushes + 1;
                    file_state.plies_since_response = 0;
                } else if file_state.deepest_penetration == prev_file_state.deepest_penetration
                    && file_state.deepest_penetration > 0
                {
                    // Storm still active but no new advance
                    file_state.consecutive_pushes = prev_file_state.consecutive_pushes;
                    file_state.plies_since_response = prev_file_state.plies_since_response + 1;
                } else {
                    // Storm has receded or defensive response occurred
                    file_state.consecutive_pushes = 0;
                    file_state.plies_since_response = 0;
                }
            } else {
                // First analysis - initialize based on current state
                if file_state.deepest_penetration > 0 {
                    file_state.consecutive_pushes = 1;
                    file_state.plies_since_response = 0;
                }
            }

            // Calculate severity for this file
            let severity = file_state.severity();
            if file_state.is_active() {
                total_severity += severity;
                active_count += 1;

                // Track center and edge pressure
                if col >= 3 && col <= 5 {
                    center_pressure = true;
                }
                if col == 0 || col == 8 {
                    edge_pressure = true;
                }

                // Track most critical file
                if severity > max_severity {
                    max_severity = severity;
                    most_critical_file = Some(col);
                }
            }
        }

        self.total_severity = total_severity;
        self.active_file_count = active_count;
        self.center_pressure = center_pressure;
        self.edge_pressure = edge_pressure;
        self.most_critical_file = most_critical_file;
    }

    /// Get storm state for a specific file
    pub fn get_file_state(&self, file: u8) -> &FileStormState {
        &self.file_states[file as usize]
    }

    /// Check if there's an active storm
    pub fn has_active_storm(&self) -> bool {
        self.active_file_count > 0
    }

    /// Get the most critical file (highest severity)
    pub fn get_most_critical_file(&self) -> Option<u8> {
        self.most_critical_file
    }

    /// Check if a specific file has an active storm
    pub fn file_has_storm(&self, file: u8) -> bool {
        self.file_states[file as usize].is_active()
    }
}

/// Storm response recommendations
#[derive(Debug, Clone)]
pub struct StormResponse {
    /// Recommended drop type (if any)
    pub recommended_drop: Option<PieceType>,
    /// Recommended drop position (if any)
    pub recommended_position: Option<Position>,
    /// Priority score (higher = more urgent)
    pub priority: f32,
    /// Whether a blocking move is recommended
    pub needs_blocking: bool,
}

impl StormResponse {
    /// Create a new storm response
    pub fn new() -> Self {
        Self {
            recommended_drop: None,
            recommended_position: None,
            priority: 0.0,
            needs_blocking: false,
        }
    }

    /// Calculate response priority based on storm state
    pub fn calculate_priority(&mut self, storm_state: &StormState, file: u8) {
        let file_state = storm_state.get_file_state(file);
        self.priority = file_state.severity();

        // Increase priority if no response has been made
        if file_state.plies_since_response > 2 {
            self.priority *= 1.5;
        }

        // Increase priority for critical files
        if file_state.is_critical_file {
            self.priority *= 1.3;
        }

        self.needs_blocking = file_state.is_active() && file_state.deepest_penetration >= 2;
    }
}

impl Default for StormResponse {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_storm_state_severity() {
        let mut state = FileStormState::default();
        state.consecutive_pushes = 3;
        state.deepest_penetration = 2;
        state.is_critical_file = true;

        let severity = state.severity();
        assert!(severity > 0.0);
    }

    #[test]
    fn test_storm_state_analysis() {
        let board = BitboardBoard::new();
        let mut storm_state = StormState::new();
        storm_state.analyze(&board, Player::Black, 10, None);

        // Starting position should have minimal storm activity
        assert!(!storm_state.has_active_storm() || storm_state.total_severity < 1.0);
    }

    #[test]
    fn test_storm_response_priority() {
        let mut storm_state = StormState::new();
        let mut file_state = FileStormState::default();
        file_state.consecutive_pushes = 2;
        file_state.deepest_penetration = 2;
        file_state.plies_since_response = 3;
        file_state.is_critical_file = true;
        storm_state.file_states[4] = file_state;
        storm_state.active_file_count = 1;
        storm_state.total_severity = file_state.severity();

        let mut response = StormResponse::new();
        response.calculate_priority(&storm_state, 4);

        assert!(response.priority > 0.0);
        assert!(response.needs_blocking);
    }
}

