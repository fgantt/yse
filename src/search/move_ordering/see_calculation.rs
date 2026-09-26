//! Static Exchange Evaluation (SEE) calculation
//!
//! This module contains the SEE calculation implementation.
//! SEE evaluates the material gain/loss from a sequence of captures
//! starting with the given move.

use crate::bitboards::BitboardBoard;
use crate::types::core::{Move, Piece, PieceType, Player, Position};
use std::collections::HashMap;

/// SEE calculation result
pub type SEEResult<T> = Result<T, String>;

/// Find all attackers and defenders of a given square
///
/// This function identifies all pieces that can attack the target square.
/// For SEE calculation, we need to know which pieces can capture on this
/// square.
///
/// Returns a vector of all pieces that can attack the square, with their
/// positions. The caller will separate them by player.
/// Task 3.0.3.1: Rewritten to use bitboard iteration instead of nested 9×9
/// loops Task 3.0.3.4: Uses iter_pieces for efficient iteration over board
/// pieces
pub fn find_attackers_defenders(square: Position, board: &BitboardBoard) -> Vec<(Position, Piece)> {
    // Built on the board's own attack tables -- the ones `is_square_attacked_by`
    // uses and the board oracle tests cover. The hand-written version this
    // replaces had pawns, lances and knights moving the wrong way for both
    // colours and gave golds, silvers and promoted minors a king's eight
    // squares, so SEE saw attackers that were not there and missed ones that
    // were.
    use crate::bitboards::integration::GlobalOptimizer;
    let pieces = board.get_pieces();
    let mut found: Vec<(Position, Piece)> = Vec::new();
    let push_all = |mut bb: crate::types::Bitboard, player: Player, pt: PieceType, found: &mut Vec<(Position, Piece)>| {
        while let Some(sq) = GlobalOptimizer::bit_scan_forward(bb) {
            bb &= crate::types::Bitboard::from_u128(bb.to_u128() - 1);
            let pos = Position::from_index(sq);
            if pos != square && !found.iter().any(|(p, _)| *p == pos) {
                found.push((pos, Piece::new(pt, player)));
            }
        }
    };
    for player in [Player::Black, Player::White] {
        let own = &pieces[player as usize];
        // Step attacks, looked up from the target: pieces are point-symmetric
        // between the colours, so `player`'s pieces that attack `square` sit on
        // the squares a same-type piece of the other colour attacks from it.
        for pt in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Silver,
            PieceType::Gold,
            PieceType::PromotedPawn,
            PieceType::PromotedLance,
            PieceType::PromotedKnight,
            PieceType::PromotedSilver,
        ] {
            let bb = board.get_attack_pattern_precomputed(square, pt, player.opposite()) & own[pt.as_index()];
            push_all(bb, player, pt, &mut found);
        }
        // The king, and the one-step moves of dragon and horse.
        let adjacent = board.get_attack_pattern_precomputed(square, PieceType::King, player);
        for pt in [PieceType::King, PieceType::PromotedRook, PieceType::PromotedBishop] {
            push_all(adjacent & own[pt.as_index()], player, pt, &mut found);
        }
    }
    // Sliders (rook, bishop, dragon, horse rays, lance) under the current occupancy.
    for (pos, piece) in sliding_attackers(board, square, board.get_occupied_bitboard()) {
        if pos != square && !found.iter().any(|(p, _)| *p == pos) {
            found.push((pos, piece));
        }
    }

    // Sort by piece value (ascending) - least valuable first for SEE
    found.sort_by_key(|(_, p)| p.piece_type.base_value());
    found
}

/// Whether `piece` standing on `from_pos` attacks `target_pos`, from the
/// board's attack tables (sliders along rays under the current occupancy).
pub fn piece_attacks_square(
    piece: &Piece,
    from_pos: Position,
    target_pos: Position,
    board: &BitboardBoard,
) -> bool {
    let player = piece.player;
    if from_pos.row >= 9 || from_pos.col >= 9 || target_pos.row >= 9 || target_pos.col >= 9 {
        return false;
    }
    let target_bit = crate::types::Bitboard::from_u128(1u128 << target_pos.to_index());
    let step = |pt: PieceType| {
        !(board.get_attack_pattern_precomputed(from_pos, pt, player) & target_bit).is_empty()
    };
    match piece.piece_type {
        PieceType::Lance => {
            // Towards the opponent: decreasing rows for Black.
            let dir: i8 = if player == Player::Black { -1 } else { 1 };
            check_ray_attack(from_pos, target_pos, (dir, 0), board)
        }
        PieceType::Rook | PieceType::PromotedRook => {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&d| check_ray_attack(from_pos, target_pos, d, board))
                || (piece.piece_type == PieceType::PromotedRook && step(PieceType::King))
        }
        PieceType::Bishop | PieceType::PromotedBishop => {
            [(1, 1), (1, -1), (-1, 1), (-1, -1)]
                .iter()
                .any(|&d| check_ray_attack(from_pos, target_pos, d, board))
                || (piece.piece_type == PieceType::PromotedBishop && step(PieceType::King))
        }
        pt => step(pt),
    }
}

/// Check if a ray from from_pos in direction (dr, dc) hits target_pos
fn check_ray_attack(
    from_pos: Position,
    target_pos: Position,
    direction: (i8, i8),
    board: &BitboardBoard,
) -> bool {
    let (dr, dc) = direction;
    let mut current_pos = from_pos;

    loop {
        let new_row = current_pos.row as i8 + dr;
        let new_col = current_pos.col as i8 + dc;

        // Out of bounds
        if new_row < 0 || new_row >= 9 || new_col < 0 || new_col >= 9 {
            break;
        }

        current_pos = Position::new(new_row as u8, new_col as u8);

        // Found target
        if current_pos == target_pos {
            return true;
        }

        // Blocked by a piece
        if board.is_square_occupied(current_pos) {
            break;
        }
    }

    false
}

/// Calculate Static Exchange Evaluation (SEE) for a move
///
/// This function simulates the sequence of captures that would follow
/// the given move and returns the net material gain/loss.
///
/// # Arguments
/// * `move_` - The move to evaluate
/// * `board` - The current board position
///
/// # Returns
/// The net material gain/loss from the exchange sequence

/// Sliding attackers of `to` under a hypothetical `occupied`, as (position, piece).
///
/// The exchange removes one piece from the board at a time, and each removal can
/// open a line: a rook behind the bishop that just captured now bears on the
/// square. Recomputing from the magic table under the reduced occupancy is what
/// makes those x-ray attackers visible. Without it an exchange defended "twice"
/// through a single file looks defended once, and SEE calls a losing capture
/// winning.
fn sliding_attackers(
    board: &BitboardBoard,
    to: Position,
    occupied: crate::types::Bitboard,
) -> Vec<(Position, Piece)> {
    let Some(magic) = board.get_magic_table() else {
        return Vec::new(); // No table: callers fall back to the static list.
    };
    let to_idx = to.to_index();
    let rook_rays = magic.get_attacks(to_idx, PieceType::Rook, occupied);
    let bishop_rays = magic.get_attacks(to_idx, PieceType::Bishop, occupied);

    let mut found = Vec::new();
    let pieces = board.get_pieces();
    for player in [Player::Black, Player::White] {
        for pt in [
            PieceType::Rook,
            PieceType::PromotedRook,
            PieceType::Bishop,
            PieceType::PromotedBishop,
            PieceType::Lance,
        ] {
            // A lance only ever moves up its own file, so the rook rays bound
            // it; the direction is checked below.
            let rays = match pt {
                PieceType::Bishop | PieceType::PromotedBishop => bishop_rays,
                _ => rook_rays,
            };
            let mut bb = pieces[player as usize][pt.as_index()] & rays & occupied;
            while let Some(sq) = crate::bitboards::integration::GlobalOptimizer::bit_scan_forward(bb)
            {
                bb &= crate::types::Bitboard::from_u128(bb.to_u128() - 1);
                let pos = Position::from_index(sq);
                if pt == PieceType::Lance {
                    // Same file, and in front of the target from its own side.
                    if pos.col != to.col {
                        continue;
                    }
                    let forward = if player == Player::Black { pos.row > to.row } else { pos.row < to.row };
                    if !forward {
                        continue;
                    }
                }
                found.push((pos, Piece::new(pt, player)));
            }
        }
    }
    found
}

pub fn calculate_see_internal(move_: &Move, board: &BitboardBoard) -> i32 {
    let Some(captured) = move_.captured_piece.as_ref() else {
        return 0; // No capture, no SEE value
    };
    swap_value(move_, board, captured.piece_type.base_value())
}

/// Exchange value of a quiet move or drop: 0 if the piece is safe on its new
/// square, otherwise minus the material the opponent nets by capturing it
/// (and whatever exchange follows). A drop onto a square the opponent attacks
/// and we do not defend scores minus the dropped piece.
///
/// Capturing moves use [`calculate_see_internal`].
pub fn see_quiet(move_: &Move, board: &BitboardBoard) -> i32 {
    swap_value(move_, board, 0)
}

/// The swap-list exchange on `move_.to`, starting from `first_gain` -- the
/// value of what the move itself captures, 0 for a quiet move.
fn swap_value(move_: &Move, board: &BitboardBoard, first_gain: i32) -> i32 {
    let to = move_.to;
    let mover = move_.player;

    // Value of whatever now stands on the target square: it is what the
    // opponent wins if they recapture.
    let mut on_square = match move_.from.and_then(|f| board.get_piece(f)) {
        Some(p) => p.piece_type.base_value(),
        None => move_.piece_type.base_value(), // drop
    };

    // Everything else bearing on the square. The piece that just moved is no
    // longer among them, and its square is no longer occupied — which may
    // already have opened a line onto the target.
    let mut occupied = board.get_occupied_bitboard();
    if let Some(from) = move_.from {
        occupied &= !crate::types::Bitboard::from_u128(1u128 << from.to_index());
    }
    let mut pieces: Vec<(Position, Piece)> = find_attackers_defenders(to, board)
        .into_iter()
        .filter(|(pos, _)| Some(*pos) != move_.from)
        .collect();
    // Sliders revealed by the mover vacating `from`.
    for (pos, piece) in sliding_attackers(board, to, occupied) {
        if pos != to && !pieces.iter().any(|(p, _)| *p == pos) {
            pieces.push((pos, piece));
        }
    }

    // The standard swap list. `gain[d]` is the material balance for the side
    // to move at depth d if the exchange runs that far; folding it back with
    // the negamax maximum lets either side stand pat when continuing would
    // lose, which is what makes SEE an exchange evaluation rather than a sum.
    //
    // What this replaces did two things wrong: it subtracted the moving
    // piece's value up front, though that piece is only lost if it is
    // recaptured, and on each recapture it subtracted the value of the piece
    // *doing* the capturing rather than the piece being taken. An even
    // pawn-for-pawn trade defended by a rook came out at -990.
    let mut gain = [0i32; 40];
    gain[0] = first_gain;
    let mut d = 0usize;
    let mut side = mover.opposite();

    while d + 1 < gain.len() {
        d += 1;
        gain[d] = on_square - gain[d - 1];
        // Neither side will enter an exchange that loses by every measure.
        if (-gain[d - 1]).max(gain[d]) < 0 {
            break;
        }
        let Some(idx) = pieces
            .iter()
            .enumerate()
            .filter(|(_, (_, p))| p.player == side)
            .min_by_key(|(_, (_, p))| p.piece_type.base_value())
            .map(|(i, _)| i)
        else {
            break;
        };
        let (used_pos, used_piece) = pieces.remove(idx);
        on_square = used_piece.piece_type.base_value();
        // That piece has left its square, which may reveal a slider behind it.
        occupied &= !crate::types::Bitboard::from_u128(1u128 << used_pos.to_index());
        for (pos, piece) in sliding_attackers(board, to, occupied) {
            if pos != to && !pieces.iter().any(|(p, _)| *p == pos) {
                pieces.push((pos, piece));
            }
        }
        side = side.opposite();
    }

    while d > 1 {
        d -= 1;
        gain[d - 1] = -((-gain[d - 1]).max(gain[d]));
    }
    gain[0]
}

/// Score a move using Static Exchange Evaluation (SEE)
///
/// SEE evaluates the material gain/loss from a sequence of captures
/// starting with the given move. This provides a more accurate assessment
/// of capture moves than simple piece values.
///
/// # Arguments
/// * `move_` - The move to score
/// * `board` - The current board position
/// * `see_weight` - Weight for SEE scores
///
/// # Returns
/// The SEE score for the move, scaled by the weight
pub fn score_see_move(move_: &Move, board: &BitboardBoard, see_weight: i32) -> SEEResult<i32> {
    if !move_.is_capture {
        return Ok(0);
    }

    let see_value = calculate_see_internal(move_, board);
    let see_score = (see_value * see_weight) / 1000;

    Ok(see_score)
}

/// SEE cache entry with metadata for eviction policies
/// Task 7.0: Enhanced with LRU tracking
#[derive(Debug, Clone)]
pub struct SEECacheEntry {
    /// Cached SEE value
    pub value: i32,
    /// Last access timestamp (for LRU tracking)
    pub last_access: u64,
    /// Number of times this entry was accessed
    pub access_count: u64,
    /// Absolute value of SEE (for value-based eviction)
    pub see_abs_value: i32,
}

/// SEE cache manager
///
/// Manages caching of SEE calculation results for performance optimization.
/// Task 7.0: Enhanced with advanced eviction policies (FIFO, LRU, Value-Based)
#[derive(Debug, Clone)]
pub struct SEECache {
    /// SEE cache: maps (from_square, to_square) -> cache entry
    cache: HashMap<(Position, Position), SEECacheEntry>,
    /// Maximum cache size
    max_size: usize,
    /// LRU access counter (incremented on each access)
    lru_access_counter: u64,
}

impl SEECache {
    /// Create a new SEE cache
    pub fn new(max_size: usize) -> Self {
        Self { cache: HashMap::new(), max_size, lru_access_counter: 0 }
    }

    /// Get a cached SEE value
    /// Task 7.0: Updates LRU tracking on access
    pub fn get(&mut self, from: Position, to: Position) -> Option<i32> {
        if let Some(entry) = self.cache.get_mut(&(from, to)) {
            self.lru_access_counter += 1;
            entry.last_access = self.lru_access_counter;
            entry.access_count += 1;
            Some(entry.value)
        } else {
            None
        }
    }

    /// Cache a SEE value
    /// Task 7.0: Evicts least valuable entry when cache is full
    pub fn insert(&mut self, from: Position, to: Position, value: i32) -> bool {
        let key = (from, to);

        // If entry already exists, update it
        if let Some(entry) = self.cache.get_mut(&key) {
            self.lru_access_counter += 1;
            entry.value = value;
            entry.last_access = self.lru_access_counter;
            entry.access_count += 1;
            entry.see_abs_value = value.abs();
            return false; // No eviction
        }

        // If cache has room, just insert
        if self.cache.len() < self.max_size {
            self.lru_access_counter += 1;
            let entry = SEECacheEntry {
                value,
                last_access: self.lru_access_counter,
                access_count: 1,
                see_abs_value: value.abs(),
            };
            self.cache.insert(key, entry);
            return false; // No eviction
        }

        // Cache is full - use LRU eviction with value-preference
        // Prefer evicting entries with low absolute SEE values and low access counts
        if let Some(evict_key) = self.select_eviction_candidate() {
            self.cache.remove(&evict_key);

            self.lru_access_counter += 1;
            let entry = SEECacheEntry {
                value,
                last_access: self.lru_access_counter,
                access_count: 1,
                see_abs_value: value.abs(),
            };
            self.cache.insert(key, entry);
            return true; // Eviction occurred
        }

        false
    }

    /// Select a cache entry for eviction
    /// Task 7.0: Hybrid eviction policy combining LRU and value-based eviction
    /// Prefers evicting entries with low absolute SEE values and low recent
    /// access
    fn select_eviction_candidate(&self) -> Option<(Position, Position)> {
        if self.cache.is_empty() {
            return None;
        }

        // Score each entry: lower score = more likely to evict
        // Score = (access_age_weight * normalized_age) + (value_weight *
        // normalized_inverse_value)
        let access_age_weight = 0.6;
        let value_weight = 0.4;

        // Find min/max for normalization
        let max_access = self.cache.values().map(|e| e.last_access).max().unwrap_or(1);
        let min_access = self.cache.values().map(|e| e.last_access).min().unwrap_or(1);
        let access_range = (max_access - min_access).max(1) as f32;

        let max_value = self.cache.values().map(|e| e.see_abs_value).max().unwrap_or(1);
        let min_value = self.cache.values().map(|e| e.see_abs_value).min().unwrap_or(0);
        let value_range = (max_value - min_value).max(1) as f32;

        let mut evict_key = None;
        let mut evict_score = f32::MAX;

        for (key, entry) in &self.cache {
            // Age score: normalized to 0.0 (oldest) .. 1.0 (newest)
            // Invert so older = higher score (more likely to evict)
            let age_score = if access_range > 0.0 {
                1.0 - ((entry.last_access - min_access) as f32 / access_range)
            } else {
                0.5
            };

            // Value score: normalized to 0.0 (lowest value) .. 1.0 (highest value)
            // Invert so lower value = higher score (more likely to evict)
            let value_score = if value_range > 0.0 {
                1.0 - ((entry.see_abs_value - min_value) as f32 / value_range)
            } else {
                0.5
            };

            // Combined score: higher = more likely to evict
            let combined_score = access_age_weight * age_score + value_weight * value_score;

            if combined_score > evict_score {
                evict_score = combined_score;
                evict_key = Some(*key);
            }
        }

        evict_key
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_access_counter = 0;
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Check if cache is full
    pub fn is_full(&self) -> bool {
        self.cache.len() >= self.max_size
    }

    /// Get cache utilization as a percentage
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.cache.len() as f64 / self.max_size as f64) * 100.0
        }
    }

    /// Get memory usage estimate for cache
    pub fn memory_bytes(&self) -> usize {
        self.cache.len()
            * (std::mem::size_of::<(Position, Position)>() + std::mem::size_of::<SEECacheEntry>())
    }

    /// Get maximum cache size
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Set maximum cache size
    /// If new size is smaller than current cache size, evicts entries
    pub fn set_max_size(&mut self, new_max_size: usize) {
        self.max_size = new_max_size;

        // Evict entries if cache exceeds new max size
        while self.cache.len() > self.max_size {
            if let Some(evict_key) = self.select_eviction_candidate() {
                self.cache.remove(&evict_key);
            } else {
                break;
            }
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> SEECacheStats {
        let total_access_count: u64 = self.cache.values().map(|e| e.access_count).sum();
        let avg_access_count = if !self.cache.is_empty() {
            total_access_count as f64 / self.cache.len() as f64
        } else {
            0.0
        };

        SEECacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
            utilization: self.utilization(),
            total_accesses: total_access_count,
            avg_accesses_per_entry: avg_access_count,
            memory_bytes: self.memory_bytes(),
        }
    }
}

/// SEE cache statistics
#[derive(Debug, Clone)]
pub struct SEECacheStats {
    /// Current cache size
    pub size: usize,
    /// Maximum cache size
    pub max_size: usize,
    /// Cache utilization percentage
    pub utilization: f64,
    /// Total accesses across all entries
    pub total_accesses: u64,
    /// Average accesses per cache entry
    pub avg_accesses_per_entry: f64,
    /// Memory usage in bytes
    pub memory_bytes: usize,
}

impl Default for SEECache {
    fn default() -> Self {
        Self::new(5000) // Default max size (increased from 1000)
    }
}
