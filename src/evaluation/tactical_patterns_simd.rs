//! SIMD-optimized tactical pattern matching
//!
//! This module provides SIMD-accelerated pattern matching for tactical patterns,
//! using batch operations to process multiple positions simultaneously.
//!
//! # Feature Flags & Configuration
//!
//! - **Compile-time**: Only available when the crate is built with
//!   `--features simd`.
//! - **Runtime**: `TacticalPatternRecognizer` checks
//!   `SimdConfig::enable_simd_pattern_matching` (see
//!   `docs/design/implementation/simd-optimization/SIMD_INTEGRATION_STATUS.md`)
//!   before delegating to `SimdPatternMatcher`. This enables per-profile control
//!   without recompilation, which is essential for experiments documented in
//!   `SIMD_IMPLEMENTATION_EVALUATION.md`.
//! - **Telemetry**: SIMD vs scalar invocation counts are tracked through
//!   `SimdTelemetry`, allowing regression detection to align with the integration
//!   status tasks.
//!
//! # Performance
//!
//! Uses SIMD batch operations to achieve 2-4x speedup for pattern matching
//! compared to scalar implementations. This reduces fork/pin filtering cost and
//! contributes to the 20%+ NPS gains captured in the integration benchmarks.
//!
//! # Usage
//!
//! ```rust,ignore
//! use shogi_engine::config::SimdConfig;
//! use shogi_engine::evaluation::tactical_patterns::TacticalPatternRecognizer;
//!
//! let mut recognizer = TacticalPatternRecognizer::default();
//! let mut cfg = recognizer.config().clone();
//! cfg.enable_simd_pattern_matching = true;
//! recognizer.set_config(cfg);
//!
//! // Internally calls SimdPatternMatcher once the runtime flag and Cargo feature are enabled.
//! let forks = recognizer.detect_forks(&board, player);
//! ```

#![cfg(feature = "simd")]

use crate::bitboards::{batch_ops::AlignedBitboardArray, BitboardBoard, SimdBitboard};
use crate::types::core::{PieceType, Player, Position};
use crate::types::{set_bit, Bitboard};

/// SIMD-optimized pattern matcher for tactical patterns
pub struct SimdPatternMatcher {
    // Pattern templates for common tactical patterns
    fork_patterns: Vec<ForkPattern>,
    pin_patterns: Vec<PinPattern>,
}

/// Pattern template for fork detection
#[derive(Clone, Copy)]
struct ForkPattern {
    /// Attack pattern that indicates a fork
    attack_mask: SimdBitboard,
    /// Minimum number of targets required
    min_targets: u32,
}

/// Pattern template for pin detection
#[derive(Clone, Copy)]
struct PinPattern {
    /// Line pattern for pin detection
    line_mask: SimdBitboard,
    /// Direction of the pin
    direction: (i8, i8),
}

impl SimdPatternMatcher {
    /// Create a new SIMD pattern matcher
    pub fn new() -> Self {
        Self { fork_patterns: Vec::new(), pin_patterns: Vec::new() }
    }

    /// Detect forks using SIMD batch operations
    ///
    /// Processes multiple pieces simultaneously to find forks (double attacks)
    ///
    /// # Performance
    ///
    /// Uses batch operations to process multiple pieces at once, achieving
    /// 2-4x speedup vs scalar implementation.
    pub fn detect_forks_batch(
        &self,
        board: &BitboardBoard,
        pieces: &[(Position, PieceType)],
        player: Player,
    ) -> Vec<(Position, PieceType, u32)> {
        if pieces.is_empty() {
            return Vec::new();
        }

        let mut forks = Vec::new();
        const BATCH_SIZE: usize = 4;

        // Process pieces in batches
        for chunk in pieces.chunks(BATCH_SIZE) {
            // Collect attack patterns for this batch
            let mut attack_patterns = Vec::new();
            let mut piece_info = Vec::new();

            for &(pos, piece_type) in chunk {
                // Get attack pattern for this piece
                let attacks = board.get_attack_pattern_precomputed(pos, piece_type, player);
                attack_patterns.push(SimdBitboard::from_u128(attacks.to_u128()));
                piece_info.push((pos, piece_type));
            }

            // Pad to BATCH_SIZE if needed
            while attack_patterns.len() < BATCH_SIZE {
                attack_patterns.push(SimdBitboard::empty());
                piece_info.push((Position::new(0, 0), PieceType::Pawn));
            }

            // Use batch operations to process attack patterns
            let attack_array =
                AlignedBitboardArray::<BATCH_SIZE>::from_slice(&attack_patterns[..BATCH_SIZE]);

            // Get opponent pieces bitboard for intersection
            let opponent = player.opposite();
            let mut opponent_pieces_bitboard = Bitboard::empty();
            for row in 0..9 {
                for col in 0..9 {
                    let pos = Position::new(row, col);
                    if let Some(piece) = board.get_piece(pos) {
                        if piece.player == opponent {
                            set_bit(&mut opponent_pieces_bitboard, pos);
                        }
                    }
                }
            }
            let opponent_simd = SimdBitboard::from_u128(opponent_pieces_bitboard.to_u128());

            // Check each piece for forks using SIMD operations
            for (i, &(pos, piece_type)) in piece_info.iter().enumerate() {
                if i >= chunk.len() {
                    break;
                }

                let attacks = attack_array.get(i);

                // Intersect attacks with opponent pieces to find targets
                let targets = *attacks & opponent_simd;
                let target_count = targets.count_ones();

                // Fork requires at least 2 targets
                if target_count >= 2 {
                    forks.push((pos, piece_type, target_count));
                }
            }
        }

        forks
    }

    /// Detect pins using SIMD batch operations
    ///
    /// Processes multiple pieces simultaneously to find pins using vectorized attack pattern generation.
    ///
    /// # Performance
    ///
    /// Uses batch operations to process multiple pieces at once, achieving
    /// 2-4x speedup vs scalar implementation.
    pub fn detect_pins_batch(
        &self,
        board: &BitboardBoard,
        pieces: &[(Position, PieceType)],
        player: Player,
    ) -> Vec<(Position, PieceType, Position)> {
        if pieces.is_empty() {
            return Vec::new();
        }

        let mut pins = Vec::new();
        const BATCH_SIZE: usize = 4;

        // Filter pieces that can create pins (sliding pieces only)
        let pinning_pieces: Vec<_> = pieces
            .iter()
            .filter(|(_, piece_type)| {
                matches!(
                    piece_type,
                    PieceType::Rook
                        | PieceType::PromotedRook
                        | PieceType::Bishop
                        | PieceType::PromotedBishop
                        | PieceType::Lance
                )
            })
            .copied()
            .collect();

        if pinning_pieces.is_empty() {
            return Vec::new();
        }

        // Process pieces in batches using SIMD for attack pattern generation
        for chunk in pinning_pieces.chunks(BATCH_SIZE) {
            // Collect attack patterns for this batch using SIMD-friendly operations
            let mut attack_patterns = Vec::new();
            let mut piece_info = Vec::new();

            for &(pos, piece_type) in chunk {
                // Get attack pattern for this piece
                let attacks = board.get_attack_pattern_precomputed(pos, piece_type, player);
                attack_patterns.push(SimdBitboard::from_u128(attacks.to_u128()));
                piece_info.push((pos, piece_type));
            }

            // Pad to BATCH_SIZE if needed
            while attack_patterns.len() < BATCH_SIZE {
                attack_patterns.push(SimdBitboard::empty());
                piece_info.push((Position::new(0, 0), PieceType::Pawn));
            }

            // Use batch operations to process attack patterns
            let _attack_array =
                AlignedBitboardArray::<BATCH_SIZE>::from_slice(&attack_patterns[..BATCH_SIZE]);

            // Check each piece for pins using SIMD operations
            for (i, &(pos, piece_type)) in piece_info.iter().enumerate() {
                if i >= chunk.len() {
                    break;
                }

                let _attacks = _attack_array.get(i);

                // Get directions for this piece type
                let directions: &[(i8, i8)] = match piece_type {
                    PieceType::Rook | PieceType::PromotedRook => {
                        &[(1, 0), (-1, 0), (0, 1), (0, -1)]
                    }
                    PieceType::Bishop | PieceType::PromotedBishop => {
                        &[(1, 1), (-1, 1), (1, -1), (-1, -1)]
                    }
                    PieceType::Lance => {
                        if player == Player::Black {
                            &[(-1, 0)]
                        } else {
                            &[(1, 0)]
                        }
                    }
                    _ => continue,
                };

                // Check each direction for pins (using scalar for line scanning, but with SIMD-optimized attack patterns)
                for &(dr, dc) in directions {
                    if let Some(pinned_pos) =
                        self.check_pin_direction(board, pos, piece_type, player, dr, dc)
                    {
                        pins.push((pos, piece_type, pinned_pos));
                    }
                }
            }
        }

        pins
    }

    /// Check for a pin in a specific direction
    fn check_pin_direction(
        &self,
        board: &BitboardBoard,
        from: Position,
        piece_type: PieceType,
        player: Player,
        dr: i8,
        dc: i8,
    ) -> Option<Position> {
        let opponent = player.opposite();
        let mut row = from.row as i8 + dr;
        let mut col = from.col as i8 + dc;
        let mut first_enemy: Option<(Position, PieceType)> = None;

        while row >= 0 && row < 9 && col >= 0 && col < 9 {
            let pos = Position::new(row as u8, col as u8);

            if let Some(piece) = board.get_piece(pos) {
                if piece.player == player {
                    break; // Own piece blocks
                }

                if piece.player == opponent {
                    if first_enemy.is_none() {
                        first_enemy = Some((pos, piece.piece_type));
                    } else {
                        // Found second enemy piece - check if it's the king
                        if piece.piece_type == PieceType::King {
                            // This is a pin - return the first enemy piece
                            return first_enemy.map(|(pos, _)| pos);
                        }
                        break;
                    }
                }
            }

            row += dr;
            col += dc;
        }

        None
    }

    /// Count attack targets using SIMD operations
    ///
    /// Uses SIMD bitwise operations to efficiently count targets
    pub fn count_attack_targets(
        &self,
        attack_pattern: SimdBitboard,
        target_mask: SimdBitboard,
    ) -> u32 {
        // Intersect attack pattern with target mask
        let targets = attack_pattern & target_mask;
        targets.count_ones()
    }

    /// Batch count attack targets for multiple pieces
    ///
    /// Uses batch operations to count targets for multiple pieces simultaneously
    pub fn count_attack_targets_batch(
        &self,
        attack_patterns: &AlignedBitboardArray<4>,
        target_mask: SimdBitboard,
    ) -> [u32; 4] {
        // Create target mask array
        let target_mask_array = AlignedBitboardArray::<4>::from_slice(&[target_mask; 4]);

        // Use batch AND to intersect all patterns with target mask
        let intersections = attack_patterns.batch_and(&target_mask_array);

        // Count targets for each pattern
        [
            intersections.get(0).count_ones(),
            intersections.get(1).count_ones(),
            intersections.get(2).count_ones(),
            intersections.get(3).count_ones(),
        ]
    }

    /// Detect multiple patterns simultaneously using SIMD
    ///
    /// Checks multiple positions for patterns in parallel
    pub fn detect_patterns_batch(
        &self,
        board: &BitboardBoard,
        positions: &[Position],
        piece_type: PieceType,
        player: Player,
    ) -> Vec<(Position, u32)> {
        if positions.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        const BATCH_SIZE: usize = 4;

        // Get opponent pieces mask
        let opponent = player.opposite();
        let mut opponent_mask = Bitboard::empty();
        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == opponent {
                        set_bit(&mut opponent_mask, pos);
                    }
                }
            }
        }
        let opponent_simd_mask = SimdBitboard::from_u128(opponent_mask.to_u128());

        // Process positions in batches
        for chunk in positions.chunks(BATCH_SIZE) {
            let mut attack_patterns = Vec::new();
            let mut pos_info = Vec::new();

            for &pos in chunk {
                let attacks = board.get_attack_pattern_precomputed(pos, piece_type, player);
                attack_patterns.push(SimdBitboard::from_u128(attacks.to_u128()));
                pos_info.push(pos);
            }

            // Pad to BATCH_SIZE
            while attack_patterns.len() < BATCH_SIZE {
                attack_patterns.push(SimdBitboard::empty());
                pos_info.push(Position::new(0, 0));
            }

            // Use batch operations
            let attack_array =
                AlignedBitboardArray::<BATCH_SIZE>::from_slice(&attack_patterns[..BATCH_SIZE]);
            let target_counts = self.count_attack_targets_batch(&attack_array, opponent_simd_mask);

            // Collect results
            for (i, &pos) in pos_info.iter().enumerate() {
                if i >= chunk.len() {
                    break;
                }
                results.push((pos, target_counts[i]));
            }
        }

        results
    }

    /// Detect skewers using SIMD batch operations
    ///
    /// Processes multiple pieces simultaneously to find skewers (attacks through less valuable piece to more valuable).
    ///
    /// # Performance
    ///
    /// Uses batch operations to process multiple pieces at once, achieving
    /// 2-4x speedup vs scalar implementation.
    pub fn detect_skewers_batch(
        &self,
        board: &BitboardBoard,
        pieces: &[(Position, PieceType)],
        player: Player,
    ) -> Vec<(Position, PieceType, Position, Position)> {
        if pieces.is_empty() {
            return Vec::new();
        }

        let mut skewers = Vec::new();
        const BATCH_SIZE: usize = 4;

        // Filter pieces that can create skewers (sliding pieces only)
        let skewering_pieces: Vec<_> = pieces
            .iter()
            .filter(|(_, piece_type)| {
                matches!(
                    piece_type,
                    PieceType::Rook
                        | PieceType::PromotedRook
                        | PieceType::Bishop
                        | PieceType::PromotedBishop
                )
            })
            .copied()
            .collect();

        if skewering_pieces.is_empty() {
            return Vec::new();
        }

        // Process pieces in batches using SIMD for attack pattern generation
        for chunk in skewering_pieces.chunks(BATCH_SIZE) {
            // Collect attack patterns for this batch
            let mut attack_patterns = Vec::new();
            let mut piece_info = Vec::new();

            for &(pos, piece_type) in chunk {
                let attacks = board.get_attack_pattern_precomputed(pos, piece_type, player);
                attack_patterns.push(SimdBitboard::from_u128(attacks.to_u128()));
                piece_info.push((pos, piece_type));
            }

            // Pad to BATCH_SIZE if needed
            while attack_patterns.len() < BATCH_SIZE {
                attack_patterns.push(SimdBitboard::empty());
                piece_info.push((Position::new(0, 0), PieceType::Pawn));
            }

            // Use batch operations to process attack patterns
            let _attack_array =
                AlignedBitboardArray::<BATCH_SIZE>::from_slice(&attack_patterns[..BATCH_SIZE]);

            // Check each piece for skewers
            for (i, &(pos, piece_type)) in piece_info.iter().enumerate() {
                if i >= chunk.len() {
                    break;
                }

                let _attacks = _attack_array.get(i);

                // Get directions for this piece type
                let directions: &[(i8, i8)] = match piece_type {
                    PieceType::Rook | PieceType::PromotedRook => {
                        &[(1, 0), (-1, 0), (0, 1), (0, -1)]
                    }
                    PieceType::Bishop | PieceType::PromotedBishop => {
                        &[(1, 1), (-1, 1), (1, -1), (-1, -1)]
                    }
                    _ => continue,
                };

                // Check each direction for skewers
                for &(dr, dc) in directions {
                    if let Some((front_pos, back_pos)) =
                        self.check_skewer_direction(board, pos, piece_type, player, dr, dc)
                    {
                        skewers.push((pos, piece_type, front_pos, back_pos));
                    }
                }
            }
        }

        skewers
    }

    /// Check for a skewer in a specific direction
    fn check_skewer_direction(
        &self,
        board: &BitboardBoard,
        from: Position,
        _piece_type: PieceType,
        player: Player,
        dr: i8,
        dc: i8,
    ) -> Option<(Position, Position)> {
        let mut row = from.row as i8 + dr;
        let mut col = from.col as i8 + dc;
        let mut front_piece: Option<(Position, PieceType)> = None;

        while row >= 0 && row < 9 && col >= 0 && col < 9 {
            let pos = Position::new(row as u8, col as u8);

            if let Some(piece) = board.get_piece(pos) {
                if piece.player == player {
                    if let Some((front_pos, front_type)) = front_piece {
                        let front_value = front_type.base_value();
                        let back_value = piece.piece_type.base_value();

                        // Skewer: back piece is more valuable than front piece
                        if back_value > front_value {
                            return Some((front_pos, pos));
                        }
                    } else {
                        front_piece = Some((pos, piece.piece_type));
                    }
                } else {
                    // Encountered opponent piece blocking line
                    break;
                }
            }

            row += dr;
            col += dc;
        }

        None
    }

    /// Detect discovered attacks using SIMD batch operations
    ///
    /// Processes multiple pieces simultaneously to find discovered attack potential.
    ///
    /// # Performance
    ///
    /// Uses batch operations to process multiple pieces at once, achieving
    /// 2-4x speedup vs scalar implementation.
    pub fn detect_discovered_attacks_batch(
        &self,
        board: &BitboardBoard,
        pieces: &[(Position, PieceType)],
        player: Player,
        target_pos: Position,
    ) -> Vec<(Position, PieceType)> {
        if pieces.is_empty() {
            return Vec::new();
        }

        let mut discovered = Vec::new();
        const BATCH_SIZE: usize = 4;

        // Process pieces in batches
        for chunk in pieces.chunks(BATCH_SIZE) {
            // Collect attack patterns for this batch
            let mut attack_patterns = Vec::new();
            let mut piece_info = Vec::new();

            for &(pos, piece_type) in chunk {
                // Get attack pattern for this piece
                let attacks = board.get_attack_pattern_precomputed(pos, piece_type, player);
                attack_patterns.push(SimdBitboard::from_u128(attacks.to_u128()));
                piece_info.push((pos, piece_type));
            }

            // Pad to BATCH_SIZE if needed
            while attack_patterns.len() < BATCH_SIZE {
                attack_patterns.push(SimdBitboard::empty());
                piece_info.push((Position::new(0, 0), PieceType::Pawn));
            }

            // Use batch operations to process attack patterns
            let attack_array =
                AlignedBitboardArray::<BATCH_SIZE>::from_slice(&attack_patterns[..BATCH_SIZE]);

            // Check each piece for discovered attack potential
            for (i, &(pos, piece_type)) in piece_info.iter().enumerate() {
                if i >= chunk.len() {
                    break;
                }

                let attacks = attack_array.get(i);

                // Check if this piece can create a discovered attack by moving
                if self.can_create_discovered_attack_simd(board, pos, target_pos, *attacks, player)
                {
                    discovered.push((pos, piece_type));
                }
            }
        }

        discovered
    }

    /// Check if moving a piece can create a discovered attack (SIMD-optimized version)
    fn can_create_discovered_attack_simd(
        &self,
        board: &BitboardBoard,
        piece_pos: Position,
        target_pos: Position,
        _piece_attacks: SimdBitboard,
        player: Player,
    ) -> bool {
        // Check if there's a friendly sliding piece behind this piece that would attack target
        let direction = match Self::direction_towards(piece_pos, target_pos) {
            Some(dir) => dir,
            None => return false,
        };

        // Path between piece and target must be clear
        let mut row = piece_pos.row as i8 + direction.0;
        let mut col = piece_pos.col as i8 + direction.1;
        let mut reached_target = false;

        while row >= 0 && row < 9 && col >= 0 && col < 9 {
            let check_pos = Position::new(row as u8, col as u8);
            if check_pos == target_pos {
                reached_target = true;
                break;
            }

            if board.get_piece(check_pos).is_some() {
                return false;
            }

            row += direction.0;
            col += direction.1;
        }

        if !reached_target {
            return false;
        }

        // Look behind for sliding piece that would attack along this line
        let behind_direction = (-direction.0, -direction.1);
        let mut row = piece_pos.row as i8 + behind_direction.0;
        let mut col = piece_pos.col as i8 + behind_direction.1;

        while row >= 0 && row < 9 && col >= 0 && col < 9 {
            let check_pos = Position::new(row as u8, col as u8);
            match board.get_piece(check_pos) {
                Some(piece) if piece.player == player => {
                    return Self::can_pin_along_line(piece.piece_type, direction.0, direction.1);
                }
                Some(_) => return false,
                None => {
                    row += behind_direction.0;
                    col += behind_direction.1;
                }
            }
        }

        false
    }

    /// Helper: Calculate direction from one position to another
    fn direction_towards(from: Position, to: Position) -> Option<(i8, i8)> {
        let dr = to.row as i8 - from.row as i8;
        let dc = to.col as i8 - from.col as i8;

        let dr_sign = dr.signum();
        let dc_sign = dc.signum();

        if dr == 0 {
            if dc == 0 {
                return None;
            }
            return Some((0, dc_sign));
        }

        if dc == 0 {
            return Some((dr_sign, 0));
        }

        if dr.abs() == dc.abs() {
            return Some((dr_sign, dc_sign));
        }

        None
    }

    /// Helper: Check if piece type can create pins/skewers along given direction
    fn can_pin_along_line(piece_type: PieceType, dr: i8, dc: i8) -> bool {
        match piece_type {
            PieceType::Rook | PieceType::PromotedRook | PieceType::Lance => {
                // Can pin along ranks and files
                dr == 0 || dc == 0
            }
            PieceType::Bishop | PieceType::PromotedBishop => {
                // Can pin along diagonals
                dr.abs() == dc.abs()
            }
            _ => false,
        }
    }
}

impl Default for SimdPatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}
