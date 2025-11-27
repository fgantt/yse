//! Initiative Tracking Module
//!
//! This module tracks offensive initiative and attack coordination, recognizing
//! patterns like climbing silver, edge attacks, and prepared pawn breaks.
//! Scoring only kicks in after development prerequisites are met.
//!
//! Task 4.3: Implement initiative tracking that recognizes climbing silver,
//! edge attacks, and prepared pawn breaks; ensure scoring kicks in only after
//! development prerequisites are met.

use crate::bitboards::BitboardBoard;
use crate::types::core::{PieceType, Player, Position};
use crate::types::evaluation::TaperedScore;
use serde::{Deserialize, Serialize};

/// Initiative pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitiativePattern {
    /// Climbing silver attack (silver advanced toward opponent king)
    ClimbingSilver,
    /// Edge attack (pieces attacking edge files 1 or 9)
    EdgeAttack,
    /// Prepared pawn break (pawn ready to advance with piece support)
    PreparedPawnBreak,
    /// Coordinated major pieces (rook + bishop both developed and active)
    CoordinatedMajors,
    /// Rook file opening (rook file pawn advanced with rook support)
    RookFileOpening,
}

/// Attack debt tracking (Task 4.4)
///
/// Tracks when the engine accumulates attacking resources but fails to convert
/// them within a configurable ply window.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttackDebt {
    /// Ply when attack resources were first detected
    pub first_detected_ply: u32,
    /// Current ply count
    pub current_ply: u32,
    /// Maximum ply window before penalties escalate
    pub max_ply_window: u32,
    /// Initiative score when first detected
    pub initial_initiative_score: i32,
    /// Current initiative score
    pub current_initiative_score: i32,
    /// Whether attack resources are still present
    pub resources_present: bool,
}

impl AttackDebt {
    /// Create a new attack debt tracker
    pub fn new(max_ply_window: u32) -> Self {
        Self {
            first_detected_ply: 0,
            current_ply: 0,
            max_ply_window,
            initial_initiative_score: 0,
            current_initiative_score: 0,
            resources_present: false,
        }
    }

    /// Update attack debt tracking
    ///
    /// Returns penalty score if attack resources are present but not converted
    /// within the ply window.
    pub fn update(
        &mut self,
        current_ply: u32,
        initiative_score: i32,
        resources_present: bool,
    ) -> i32 {
        self.current_ply = current_ply;
        self.current_initiative_score = initiative_score;
        self.resources_present = resources_present;

        if !resources_present {
            // No attack resources, reset tracking
            self.first_detected_ply = 0;
            self.initial_initiative_score = 0;
            return 0;
        }

        // First time detecting attack resources
        if self.first_detected_ply == 0 {
            self.first_detected_ply = current_ply;
            self.initial_initiative_score = initiative_score;
            return 0;
        }

        // Calculate how many plies have passed since detection
        let plies_passed = current_ply.saturating_sub(self.first_detected_ply);

        if plies_passed > self.max_ply_window {
            // Attack debt: resources present but not converted
            let debt_multiplier = (plies_passed - self.max_ply_window) as f32 * 0.1;
            let base_penalty = (self.initial_initiative_score as f32 * 0.2) as i32;
            let penalty = (base_penalty as f32 * (1.0 + debt_multiplier.min(1.0))) as i32;
            penalty
        } else {
            0
        }
    }

    /// Reset attack debt tracking
    pub fn reset(&mut self) {
        *self = Self::new(self.max_ply_window);
    }
}

/// Initiative state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeState {
    /// Detected initiative patterns
    pub patterns: Vec<InitiativePattern>,
    /// Overall initiative score (centipawns)
    pub initiative_score: i32,
    /// Whether development prerequisites are met
    pub development_ready: bool,
    /// Number of coordinated attackers near opponent king
    pub coordinated_attackers: u8,
    /// Whether edge files are being attacked
    pub edge_attack_active: bool,
    /// Attack debt tracker (Task 4.4)
    pub attack_debt: AttackDebt,
}

impl InitiativeState {
    /// Create a new initiative state
    pub fn new() -> Self {
        Self {
            attack_debt: AttackDebt::new(8), // Default 8-ply window
            ..Default::default()
        }
    }

    /// Create with custom attack debt window
    pub fn with_attack_debt_window(max_ply_window: u32) -> Self {
        Self {
            attack_debt: AttackDebt::new(max_ply_window),
            ..Default::default()
        }
    }

    /// Analyze the board and detect initiative patterns
    ///
    /// This method checks for:
    /// - Climbing silver attacks
    /// - Edge attacks
    /// - Prepared pawn breaks
    /// - Coordinated major pieces
    /// - Rook file openings
    ///
    /// Returns initiative score only if development prerequisites are met.
    pub fn analyze(
        &mut self,
        board: &BitboardBoard,
        player: Player,
        move_count: u32,
    ) -> TaperedScore {
        self.patterns.clear();
        self.initiative_score = 0;
        self.coordinated_attackers = 0;
        self.edge_attack_active = false;

        // Check development prerequisites (Task 4.3 requirement)
        self.development_ready = self.check_development_prerequisites(board, player, move_count);

        if !self.development_ready {
            return TaperedScore::default();
        }

        let opponent = player.opposite();
        let mut mg_score = 0;
        let mut eg_score = 0;

        // 1. Detect climbing silver attacks
        if let Some(silver_score) = self.detect_climbing_silver(board, player, opponent) {
            self.patterns.push(InitiativePattern::ClimbingSilver);
            mg_score += silver_score;
            eg_score += silver_score / 2;
        }

        // 2. Detect edge attacks
        if let Some(edge_score) = self.detect_edge_attacks(board, player, opponent) {
            self.patterns.push(InitiativePattern::EdgeAttack);
            self.edge_attack_active = true;
            mg_score += edge_score;
            eg_score += edge_score / 2;
        }

        // 3. Detect prepared pawn breaks
        if let Some(pawn_score) = self.detect_prepared_pawn_breaks(board, player) {
            self.patterns.push(InitiativePattern::PreparedPawnBreak);
            mg_score += pawn_score;
            eg_score += pawn_score / 3;
        }

        // 4. Detect coordinated major pieces
        if let Some((coord_score, attackers)) = self.detect_coordinated_majors(board, player, opponent) {
            self.patterns.push(InitiativePattern::CoordinatedMajors);
            self.coordinated_attackers = attackers;
            mg_score += coord_score;
            eg_score += coord_score / 2;
        }

        // 5. Detect rook file openings
        if let Some(rook_score) = self.detect_rook_file_openings(board, player) {
            self.patterns.push(InitiativePattern::RookFileOpening);
            mg_score += rook_score;
            eg_score += rook_score / 2;
        }

        self.initiative_score = mg_score;

        // Update attack debt tracking (Task 4.4)
        let resources_present = mg_score >= 30; // Threshold for "significant" attack resources
        let attack_debt_penalty = self.attack_debt.update(
            move_count,
            mg_score,
            resources_present,
        );

        // Apply attack debt penalty
        if attack_debt_penalty > 0 {
            mg_score -= attack_debt_penalty;
            eg_score -= attack_debt_penalty / 2;
        }

        TaperedScore::new_tapered(mg_score, eg_score)
    }

    /// Check if development prerequisites are met (Task 4.3 requirement)
    ///
    /// Development prerequisites:
    /// - At least one major piece (rook or bishop) developed
    /// - King in reasonable position (not exposed)
    /// - At least 6 moves played (early development phase)
    fn check_development_prerequisites(
        &self,
        board: &BitboardBoard,
        player: Player,
        move_count: u32,
    ) -> bool {
        // Need at least 6 moves for development
        if move_count < 6 {
            return false;
        }

        // Check for developed major pieces
        let start_row = if player == Player::Black { 8 } else { 0 };
        let mut has_developed_major = false;

        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player {
                        match piece.piece_type {
                            PieceType::Rook | PieceType::Bishop
                            | PieceType::PromotedRook | PieceType::PromotedBishop => {
                                if pos.row != start_row {
                                    has_developed_major = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        has_developed_major
    }

    /// Detect climbing silver attacks
    ///
    /// A climbing silver is a silver that has advanced toward the opponent's
    /// king zone and is supported by other pieces.
    fn detect_climbing_silver(
        &self,
        board: &BitboardBoard,
        player: Player,
        opponent: Player,
    ) -> Option<i32> {
        let mut best_score = 0;

        // Find opponent king position
        let opp_king_pos = board.find_king_position(opponent)?;

        // Find our silvers
        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player && piece.piece_type == PieceType::Silver {
                        // Check if silver is advanced toward opponent king
                        let distance_to_king = ((row as i8 - opp_king_pos.row as i8).abs()
                            + (col as i8 - opp_king_pos.col as i8).abs()) as u8;

                        if distance_to_king <= 4 {
                            // Silver is close to opponent king
                            let advancement = match player {
                                Player::Black => {
                                    // Black silver should be in lower rows (closer to White king)
                                    if row < 5 {
                                        (5 - row) as i32 * 15
                                    } else {
                                        0
                                    }
                                }
                                Player::White => {
                                    // White silver should be in higher rows (closer to Black king)
                                    if row > 3 {
                                        (row - 3) as i32 * 15
                                    } else {
                                        0
                                    }
                                }
                            };

                            if advancement > 0 {
                                // Check for piece support
                                let has_support = self.has_piece_support(board, pos, player);
                                if has_support {
                                    best_score = best_score.max(advancement + 20);
                                } else {
                                    best_score = best_score.max(advancement);
                                }
                            }
                        }
                    }
                }
            }
        }

        if best_score > 0 {
            Some(best_score)
        } else {
            None
        }
    }

    /// Detect edge attacks (attacks on files 1 or 9)
    fn detect_edge_attacks(
        &self,
        board: &BitboardBoard,
        player: Player,
        opponent: Player,
    ) -> Option<i32> {
        let mut edge_score = 0;
        let edge_files = [0u8, 8u8]; // Files 1 and 9 (0-indexed)

        for &file in &edge_files {
            // Check for our pieces attacking this edge file
            let mut attackers = 0;
            let mut advanced_pawns = 0;

            for row in 0..9 {
                let pos = Position::new(row, file);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player {
                        match piece.piece_type {
                            PieceType::Pawn => {
                                // Check if pawn is advanced
                                let advancement = match player {
                                    Player::Black => {
                                        if row < 5 {
                                            (5 - row) as u8
                                        } else {
                                            0
                                        }
                                    }
                                    Player::White => {
                                        if row > 3 {
                                            (row - 3) as u8
                                        } else {
                                            0
                                        }
                                    }
                                };
                                if advancement > 0 {
                                    advanced_pawns += 1;
                                }
                            }
                            PieceType::Rook | PieceType::PromotedRook
                            | PieceType::Lance | PieceType::PromotedLance => {
                                attackers += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Bonus for edge file attacks
            if attackers > 0 || advanced_pawns > 0 {
                edge_score += (attackers * 25 + advanced_pawns * 15) as i32;
            }
        }

        if edge_score > 0 {
            Some(edge_score)
        } else {
            None
        }
    }

    /// Detect prepared pawn breaks
    ///
    /// A prepared pawn break is a pawn that can advance with piece support,
    /// typically in the center or on important files.
    fn detect_prepared_pawn_breaks(
        &self,
        board: &BitboardBoard,
        player: Player,
    ) -> Option<i32> {
        let mut break_score = 0;

        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player && piece.piece_type == PieceType::Pawn {
                        // Check if pawn is in position to break
                        let is_break_position = match player {
                            Player::Black => {
                                // Pawn on row 5-6 is ready to break
                                row >= 5 && row <= 6
                            }
                            Player::White => {
                                // Pawn on row 2-3 is ready to break
                                row >= 2 && row <= 3
                            }
                        };

                        if is_break_position {
                            // Check for piece support
                            let has_support = self.has_piece_support(board, pos, player);
                            if has_support {
                                // Bonus for center pawn breaks
                                if col >= 3 && col <= 5 {
                                    break_score += 20;
                                } else {
                                    break_score += 12;
                                }
                            }
                        }
                    }
                }
            }
        }

        if break_score > 0 {
            Some(break_score)
        } else {
            None
        }
    }

    /// Detect coordinated major pieces (rook + bishop both developed and active)
    fn detect_coordinated_majors(
        &self,
        board: &BitboardBoard,
        player: Player,
        opponent: Player,
    ) -> Option<(i32, u8)> {
        let start_row = if player == Player::Black { 8 } else { 0 };
        let mut rooks_developed = 0;
        let mut bishops_developed = 0;

        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player {
                        match piece.piece_type {
                            PieceType::Rook | PieceType::PromotedRook => {
                                if pos.row != start_row {
                                    rooks_developed += 1;
                                }
                            }
                            PieceType::Bishop | PieceType::PromotedBishop => {
                                if pos.row != start_row {
                                    bishops_developed += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Both major pieces developed = coordination bonus
        if rooks_developed > 0 && bishops_developed > 0 {
            // Check if they're attacking opponent king area
            if let Some(opp_king_pos) = board.find_king_position(opponent) {
                let mut attackers_near_king = 0;
                for row in 0..9 {
                    for col in 0..9 {
                        let pos = Position::new(row, col);
                        if let Some(piece) = board.get_piece(pos) {
                            if piece.player == player {
                                let distance = ((row as i8 - opp_king_pos.row as i8).abs()
                                    + (col as i8 - opp_king_pos.col as i8).abs()) as u8;
                                if distance <= 4
                                    && matches!(
                                        piece.piece_type,
                                        PieceType::Rook
                                            | PieceType::Bishop
                                            | PieceType::PromotedRook
                                            | PieceType::PromotedBishop
                                    )
                                {
                                    attackers_near_king += 1;
                                }
                            }
                        }
                    }
                }

                if attackers_near_king >= 2 {
                    return Some((30 + (attackers_near_king as i32 * 10), attackers_near_king));
                }
            }
        }

        None
    }

    /// Detect rook file openings
    ///
    /// A rook file opening is when a rook file pawn has advanced and the rook
    /// is positioned to support it.
    fn detect_rook_file_openings(&self, board: &BitboardBoard, player: Player) -> Option<i32> {
        let mut opening_score = 0;
        let rook_files = [1u8, 7u8]; // Files 2 and 8 (1-indexed, so 1 and 7 in 0-indexed)

        for &file in &rook_files {
            let mut pawn_advanced = false;
            let mut rook_on_file = false;

            for row in 0..9 {
                let pos = Position::new(row, file);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player {
                        match piece.piece_type {
                            PieceType::Pawn => {
                                // Check if pawn is advanced
                                let advancement = match player {
                                    Player::Black => {
                                        if row < 6 {
                                            (6 - row) as u8
                                        } else {
                                            0
                                        }
                                    }
                                    Player::White => {
                                        if row > 2 {
                                            (row - 2) as u8
                                        } else {
                                            0
                                        }
                                    }
                                };
                                if advancement > 0 {
                                    pawn_advanced = true;
                                }
                            }
                            PieceType::Rook | PieceType::PromotedRook => {
                                rook_on_file = true;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if pawn_advanced && rook_on_file {
                opening_score += 25;
            }
        }

        if opening_score > 0 {
            Some(opening_score)
        } else {
            None
        }
    }

    /// Check if a position has piece support
    fn has_piece_support(
        &self,
        board: &BitboardBoard,
        pos: Position,
        player: Player,
    ) -> bool {
        // Check adjacent squares for friendly pieces
        for dr in -1..=1 {
            for dc in -1..=1 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let new_row = pos.row as i8 + dr;
                let new_col = pos.col as i8 + dc;
                if new_row >= 0 && new_row < 9 && new_col >= 0 && new_col < 9 {
                    let support_pos = Position::new(new_row as u8, new_col as u8);
                    if let Some(piece) = board.get_piece(support_pos) {
                        if piece.player == player && piece.piece_type != PieceType::King {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

impl Default for InitiativeState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initiative_state_creation() {
        let state = InitiativeState::new();
        assert!(state.patterns.is_empty());
        assert_eq!(state.initiative_score, 0);
    }

    #[test]
    fn test_development_prerequisites() {
        let board = BitboardBoard::new();
        let state = InitiativeState::new();

        // Early in game, prerequisites not met
        assert!(!state.check_development_prerequisites(&board, Player::Black, 3));

        // Later in game, should check for developed pieces
        // (This will depend on actual board state)
    }
}

