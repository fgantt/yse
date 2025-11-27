//! Attack pattern generation for magic bitboards
//!
//! This module provides functionality to generate attack patterns for rook and
//! bishop pieces using ray-casting algorithms. These patterns are used to build
//! the magic bitboard lookup tables.

use crate::types::core::PieceType;
use crate::types::Bitboard;
use lazy_static::lazy_static;
use std::collections::{HashMap, VecDeque};

/// Attack pattern generator with optimization
pub struct AttackGenerator {
    /// Attack pattern cache with LRU eviction
    pattern_cache: AttackCache,
    /// Cache statistics
    cache_stats: CacheStatsInternal,
}

/// Internal cache statistics
#[derive(Debug, Clone, Default)]
struct CacheStatsInternal {
    hits: u64,
    misses: u64,
    evictions: u64,
}

/// Configuration for attack generator cache
#[derive(Debug, Clone, Copy)]
pub struct AttackGeneratorConfig {
    /// Maximum cache size (default: 10,000 entries)
    pub cache_size: usize,
}

impl Default for AttackGeneratorConfig {
    fn default() -> Self {
        Self { cache_size: 10_000 }
    }
}

// Precomputed direction vectors (optimized to const/lazy_static for zero-cost
// access)
lazy_static! {
    static ref ROOK_DIRECTIONS: Vec<Direction> = vec![
        Direction { row_delta: 1, col_delta: 0 },   // Up
        Direction { row_delta: -1, col_delta: 0 },  // Down
        Direction { row_delta: 0, col_delta: 1 },   // Right
        Direction { row_delta: 0, col_delta: -1 },  // Left
    ];

    static ref BISHOP_DIRECTIONS: Vec<Direction> = vec![
        Direction { row_delta: 1, col_delta: 1 },   // Up-Right
        Direction { row_delta: 1, col_delta: -1 },  // Up-Left
        Direction { row_delta: -1, col_delta: 1 },  // Down-Right
        Direction { row_delta: -1, col_delta: -1 }, // Down-Left
    ];

    static ref PROMOTED_ROOK_DIRECTIONS: Vec<Direction> = vec![
        Direction { row_delta: 1, col_delta: 0 },   // Up
        Direction { row_delta: -1, col_delta: 0 },  // Down
        Direction { row_delta: 0, col_delta: 1 },   // Right
        Direction { row_delta: 0, col_delta: -1 },  // Left
        Direction { row_delta: 1, col_delta: 1 },   // Up-Right
        Direction { row_delta: 1, col_delta: -1 },  // Up-Left
        Direction { row_delta: -1, col_delta: 1 },  // Down-Right
        Direction { row_delta: -1, col_delta: -1 }, // Down-Left
    ];

    static ref PROMOTED_BISHOP_DIRECTIONS: Vec<Direction> = vec![
        Direction { row_delta: 1, col_delta: 1 },   // Up-Right
        Direction { row_delta: 1, col_delta: -1 },  // Up-Left
        Direction { row_delta: -1, col_delta: 1 },  // Down-Right
        Direction { row_delta: -1, col_delta: -1 }, // Down-Left
        Direction { row_delta: 1, col_delta: 0 },   // Up
        Direction { row_delta: -1, col_delta: 0 },  // Down
        Direction { row_delta: 0, col_delta: 1 },   // Right
        Direction { row_delta: 0, col_delta: -1 },  // Left
    ];
}

/// Direction vector for piece movement
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Direction {
    pub row_delta: i8,
    pub col_delta: i8,
}

impl AttackGenerator {
    /// Create a new attack generator with default cache size
    pub fn new() -> Self {
        Self::with_config(AttackGeneratorConfig::default())
    }

    /// Create a new attack generator with custom configuration
    pub fn with_config(config: AttackGeneratorConfig) -> Self {
        Self {
            pattern_cache: AttackCache::new(config.cache_size.max(1)),
            cache_stats: CacheStatsInternal::default(),
        }
    }

    /// Generate attack pattern with caching
    pub fn generate_attack_pattern(
        &mut self,
        square: u8,
        piece_type: PieceType,
        blockers: Bitboard,
    ) -> Bitboard {
        let key = CacheKey::new(square, piece_type, blockers);

        // Check cache first
        if let Some(cached) = self.pattern_cache.get(&key) {
            self.cache_stats.hits += 1;
            return cached;
        }

        // Cache miss
        self.cache_stats.misses += 1;

        // Generate pattern
        let pattern = self.generate_attack_pattern_internal(square, piece_type, blockers);

        // Cache the result (may evict LRU entry)
        if self.pattern_cache.insert(key, pattern) {
            self.cache_stats.evictions += 1;
        }

        pattern
    }

    /// Internal attack pattern generation
    fn generate_attack_pattern_internal(
        &self,
        square: u8,
        piece_type: PieceType,
        blockers: Bitboard,
    ) -> Bitboard {
        let mut attacks = Bitboard::default();

        let (sliding_dirs, stepping_dirs) = match piece_type {
            PieceType::Rook => (ROOK_DIRECTIONS.as_slice(), &[][..]),
            PieceType::Bishop => (BISHOP_DIRECTIONS.as_slice(), &[][..]),
            PieceType::PromotedRook => (ROOK_DIRECTIONS.as_slice(), BISHOP_DIRECTIONS.as_slice()),
            PieceType::PromotedBishop => (BISHOP_DIRECTIONS.as_slice(), ROOK_DIRECTIONS.as_slice()),
            _ => (&[][..], &[][..]),
        };

        // Sliding
        for direction in sliding_dirs {
            let mut current_square = square;

            while let Some(next_square) = self.get_next_square(current_square, *direction) {
                set_bit(&mut attacks, next_square);

                if is_bit_set(blockers, next_square) {
                    break;
                }

                current_square = next_square;
            }
        }

        // Stepping
        for direction in stepping_dirs {
            if let Some(next_square) = self.get_next_square(square, *direction) {
                set_bit(&mut attacks, next_square);
            }
        }

        attacks
    }

    /// Get directions for a piece type (using lazy_static for zero-cost access)
    #[allow(dead_code)]
    fn get_directions(&self, piece_type: PieceType) -> &[Direction] {
        match piece_type {
            PieceType::Rook => &ROOK_DIRECTIONS,
            PieceType::Bishop => &BISHOP_DIRECTIONS,
            PieceType::PromotedRook => &PROMOTED_ROOK_DIRECTIONS,
            PieceType::PromotedBishop => &PROMOTED_BISHOP_DIRECTIONS,
            _ => &[],
        }
    }

    /// Get next square in a direction
    fn get_next_square(&self, square: u8, direction: Direction) -> Option<u8> {
        let row = (square / 9) as i8;
        let col = (square % 9) as i8;

        let new_row = row + direction.row_delta;
        let new_col = col + direction.col_delta;

        if new_row >= 0 && new_row < 9 && new_col >= 0 && new_col < 9 {
            Some((new_row * 9 + new_col) as u8)
        } else {
            None
        }
    }

    /// Clear the pattern cache
    pub fn clear_cache(&mut self) {
        self.pattern_cache.clear();
        self.cache_stats = CacheStatsInternal::default();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let total_requests = self.cache_stats.hits + self.cache_stats.misses;
        let hit_rate = if total_requests > 0 {
            self.cache_stats.hits as f64 / total_requests as f64
        } else {
            0.0
        };

        CacheStats {
            cache_size: self.pattern_cache.len(),
            max_cache_size: self.pattern_cache.capacity(),
            hits: self.cache_stats.hits,
            misses: self.cache_stats.misses,
            evictions: self.cache_stats.evictions,
            hit_rate,
            direction_cache_size: 4, // Rook, Bishop, PromotedRook, PromotedBishop (now lazy_static)
        }
    }

    /// Get cache configuration
    pub fn cache_config(&self) -> AttackGeneratorConfig {
        AttackGeneratorConfig { cache_size: self.pattern_cache.capacity() }
    }

    // Removed initialize_direction_cache - now using lazy_static for zero-cost
    // access

    /// Generate all possible blocker combinations for a mask
    pub fn generate_all_blocker_combinations(&self, mask: Bitboard) -> Vec<Bitboard> {
        let bits: Vec<u8> = (0..81).filter(|&i| is_bit_set(mask, i)).collect();

        (0..(1 << bits.len()))
            .map(|combination| {
                let mut result = Bitboard::default();
                for (i, &bit_pos) in bits.iter().enumerate() {
                    if (combination >> i) & 1 != 0 {
                        set_bit(&mut result, bit_pos);
                    }
                }
                result
            })
            .collect()
    }

    /// Generate attack pattern for a specific square and piece type without
    /// blockers
    pub fn generate_attack_pattern_no_blockers(
        &mut self,
        square: u8,
        piece_type: PieceType,
    ) -> Bitboard {
        self.generate_attack_pattern(square, piece_type, Bitboard::default())
    }

    /// Generate attack pattern for a specific square and piece type with all
    /// possible blockers
    pub fn generate_attack_pattern_all_blockers(
        &mut self,
        square: u8,
        piece_type: PieceType,
    ) -> Bitboard {
        let mask = self.get_relevant_mask(square, piece_type);
        self.generate_attack_pattern(square, piece_type, mask)
    }

    /// Get the relevant mask for a square and piece type
    pub fn get_relevant_mask(&self, square: u8, piece_type: PieceType) -> Bitboard {
        // Only sliding directions matter for the mask
        let sliding_dirs = match piece_type {
            PieceType::Rook | PieceType::PromotedRook => ROOK_DIRECTIONS.as_slice(),
            PieceType::Bishop | PieceType::PromotedBishop => BISHOP_DIRECTIONS.as_slice(),
            _ => &[][..],
        };

        let mut mask = Bitboard::default();

        for direction in sliding_dirs {
            let mut current_square = square;

            while let Some(next_square) = self.get_next_square(current_square, *direction) {
                // For magic bitboards, we technically don't need the edge squares in the mask
                // if we handle the edge cases correctly, but for simplicity and correctness
                // with the current magic number generation, we include them.
                // However, we must ensure we don't include stepping directions.
                set_bit(&mut mask, next_square);
                current_square = next_square;
            }
        }

        mask
    }

    /// Generate attack pattern for a specific direction
    pub fn generate_directional_attack(
        &self,
        square: u8,
        direction: Direction,
        blockers: Bitboard,
    ) -> Bitboard {
        let mut attacks = Bitboard::default();
        let mut current_square = square;

        while let Some(next_square) = self.get_next_square(current_square, direction) {
            set_bit(&mut attacks, next_square);

            if is_bit_set(blockers, next_square) {
                break;
            }

            current_square = next_square;
        }

        attacks
    }

    /// Generate attack pattern with custom directions
    pub fn generate_attack_with_directions(
        &mut self,
        square: u8,
        directions: &[Direction],
        blockers: Bitboard,
    ) -> Bitboard {
        let mut attacks = Bitboard::default();

        for direction in directions {
            let directional_attacks =
                self.generate_directional_attack(square, *direction, blockers);
            attacks |= directional_attacks;
        }

        attacks
    }

    /// Check if a square is attacked by a piece
    pub fn is_square_attacked(
        &mut self,
        from_square: u8,
        to_square: u8,
        piece_type: PieceType,
        blockers: Bitboard,
    ) -> bool {
        let attacks = self.generate_attack_pattern(from_square, piece_type, blockers);
        is_bit_set(attacks, to_square)
    }

    /// Get all attacked squares for a piece
    pub fn get_attacked_squares(
        &mut self,
        square: u8,
        piece_type: PieceType,
        blockers: Bitboard,
    ) -> Vec<u8> {
        let attacks = self.generate_attack_pattern(square, piece_type, blockers);
        (0..81).filter(|&i| is_bit_set(attacks, i)).collect()
    }

    /// Count the number of attacked squares
    pub fn count_attacked_squares(
        &mut self,
        square: u8,
        piece_type: PieceType,
        blockers: Bitboard,
    ) -> u32 {
        let attacks = self.generate_attack_pattern(square, piece_type, blockers);
        attacks.count_ones()
    }

    /// Generate attack pattern for multiple pieces
    pub fn generate_combined_attacks(
        &mut self,
        squares: &[u8],
        piece_types: &[PieceType],
        blockers: Bitboard,
    ) -> Bitboard {
        let mut combined_attacks = Bitboard::default();

        for (square, piece_type) in squares.iter().zip(piece_types.iter()) {
            let attacks = self.generate_attack_pattern(*square, *piece_type, blockers);
            combined_attacks |= attacks;
        }

        combined_attacks
    }

    /// Pre-generate all attack patterns for a piece type
    pub fn pregenerate_attack_patterns(&mut self, piece_type: PieceType) {
        for square in 0..81 {
            let mask = self.get_relevant_mask(square, piece_type);
            let combinations = self.generate_all_blocker_combinations(mask);

            for blockers in combinations {
                self.generate_attack_pattern(square, piece_type, blockers);
            }
        }
    }

    /// Get attack pattern statistics
    pub fn get_attack_stats(&mut self, square: u8, piece_type: PieceType) -> AttackStats {
        let mask = self.get_relevant_mask(square, piece_type);
        let max_attacks = self.count_attacked_squares(square, piece_type, Bitboard::default());
        let min_attacks = self.count_attacked_squares(square, piece_type, mask);

        AttackStats {
            square,
            piece_type,
            relevant_squares: mask.count_ones(),
            max_attacks,
            min_attacks,
            average_attacks: (max_attacks + min_attacks) / 2,
        }
    }
}

/// Cache statistics for attack generator
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cache_size: usize,
    pub max_cache_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub hit_rate: f64,
    pub direction_cache_size: usize,
}

/// Attack pattern statistics
#[derive(Debug, Clone)]
pub struct AttackStats {
    pub square: u8,
    pub piece_type: PieceType,
    pub relevant_squares: u32,
    pub max_attacks: u32,
    pub min_attacks: u32,
    pub average_attacks: u32,
}

impl Default for AttackGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct CacheKey {
    square: u8,
    piece_type: PieceType,
    blockers: Bitboard,
}

impl CacheKey {
    #[inline]
    fn new(square: u8, piece_type: PieceType, blockers: Bitboard) -> Self {
        Self { square, piece_type, blockers }
    }
}

struct AttackCache {
    capacity: usize,
    map: HashMap<CacheKey, Bitboard>,
    order: VecDeque<CacheKey>,
}

impl AttackCache {
    fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            capacity: cap,
            map: HashMap::with_capacity(cap.min(1024)),
            order: VecDeque::with_capacity(cap.min(1024)),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity
    }

    fn get(&mut self, key: &CacheKey) -> Option<Bitboard> {
        if let Some(&value) = self.map.get(key) {
            self.order.push_back(*key);
            Some(value)
        } else {
            None
        }
    }

    fn insert(&mut self, key: CacheKey, value: Bitboard) -> bool {
        if let Some(entry) = self.map.get_mut(&key) {
            *entry = value;
            self.order.push_back(key);
            return false;
        }

        let mut evicted = false;
        if self.map.len() >= self.capacity {
            evicted = self.evict_one();
        }

        self.map.insert(key, value);
        self.order.push_back(key);
        evicted
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    fn evict_one(&mut self) -> bool {
        while let Some(old_key) = self.order.pop_front() {
            if self.map.remove(&old_key).is_some() {
                return true;
            }
        }
        false
    }
}

// Helper functions for bitboard operations
fn set_bit(bitboard: &mut Bitboard, square: u8) {
    *bitboard |= Bitboard::from_u128(1u128 << square);
}

fn is_bit_set(bitboard: Bitboard, square: u8) -> bool {
    !(bitboard & Bitboard::from_u128(1u128 << square)).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attack_generator_creation() {
        let generator = AttackGenerator::new();
        let stats = generator.cache_stats();
        assert_eq!(stats.cache_size, 0);
        assert_eq!(stats.direction_cache_size, 4); // Rook, Bishop,
                                                   // PromotedRook,
                                                   // PromotedBishop
    }

    #[test]
    fn test_direction_cache() {
        let generator = AttackGenerator::new();

        let rook_directions = generator.get_directions(PieceType::Rook);
        assert_eq!(rook_directions.len(), 4);

        let bishop_directions = generator.get_directions(PieceType::Bishop);
        assert_eq!(bishop_directions.len(), 4);
    }

    #[test]
    fn test_get_next_square() {
        let generator = AttackGenerator::new();

        // Test moving right from square 0 (top-left corner)
        let direction = Direction { row_delta: 0, col_delta: 1 };
        assert_eq!(generator.get_next_square(0, direction), Some(1));

        // Test moving down from square 0
        let direction = Direction { row_delta: 1, col_delta: 0 };
        assert_eq!(generator.get_next_square(0, direction), Some(9));

        // Test moving out of bounds
        let direction = Direction { row_delta: -1, col_delta: 0 };
        assert_eq!(generator.get_next_square(0, direction), None);
    }

    #[test]
    fn test_generate_all_blocker_combinations() {
        let generator = AttackGenerator::new();
        let mask = 0b111; // First 3 squares
        let combinations = generator.generate_all_blocker_combinations(Bitboard::from_u128(mask));

        assert_eq!(combinations.len(), 8); // 2^3 = 8 combinations
    }

    #[test]
    fn test_attack_pattern_generation() {
        let mut generator = AttackGenerator::new();

        // Test rook attack from center square (40)
        let attacks = generator.generate_attack_pattern_no_blockers(40, PieceType::Rook);
        assert_ne!(attacks, Bitboard::default());

        // Test bishop attack from center square (40)
        let attacks = generator.generate_attack_pattern_no_blockers(40, PieceType::Bishop);
        assert_ne!(attacks, Bitboard::default());
    }

    #[test]
    fn test_relevant_mask_generation() {
        let generator = AttackGenerator::new();

        // Test rook mask from center square
        let mask = generator.get_relevant_mask(40, PieceType::Rook);
        assert_ne!(mask, Bitboard::default());

        // Test bishop mask from center square
        let mask = generator.get_relevant_mask(40, PieceType::Bishop);
        assert_ne!(mask, Bitboard::default());
    }

    #[test]
    fn test_directional_attack() {
        let generator = AttackGenerator::new();

        // Test moving right from square 0
        let direction = Direction { row_delta: 0, col_delta: 1 };
        let attacks = generator.generate_directional_attack(0, direction, Bitboard::default());
        assert_ne!(attacks, Bitboard::default());

        // Test with blockers
        let blockers = Bitboard::from_u128(1u128 << 2); // Block square 2
        let attacks = generator.generate_directional_attack(0, direction, blockers);
        assert_ne!(attacks, Bitboard::default());
    }

    #[test]
    fn test_square_attack_check() {
        let mut generator = AttackGenerator::new();

        // Test if square 1 is attacked by rook from square 0
        let is_attacked = generator.is_square_attacked(0, 1, PieceType::Rook, Bitboard::default());
        assert!(is_attacked);

        // Test if square 9 is attacked by rook from square 0
        let is_attacked = generator.is_square_attacked(0, 9, PieceType::Rook, Bitboard::default());
        assert!(is_attacked);
    }

    #[test]
    fn test_attacked_squares() {
        let mut generator = AttackGenerator::new();

        // Get all squares attacked by rook from square 0
        let attacked_squares =
            generator.get_attacked_squares(0, PieceType::Rook, Bitboard::default());
        assert!(!attacked_squares.is_empty());
        assert!(attacked_squares.contains(&1)); // Right
        assert!(attacked_squares.contains(&9)); // Down
    }

    #[test]
    fn test_attack_count() {
        let mut generator = AttackGenerator::new();

        // Count attacks from corner square (should be fewer)
        let count = generator.count_attacked_squares(0, PieceType::Rook, Bitboard::default());
        assert!(count > 0);

        // Bishops have strictly more mobility from the center than the corner
        let bishop_corner =
            generator.count_attacked_squares(0, PieceType::Bishop, Bitboard::default());
        let bishop_center =
            generator.count_attacked_squares(40, PieceType::Bishop, Bitboard::default());
        assert!(bishop_center > bishop_corner);
    }

    #[test]
    fn test_combined_attacks() {
        let mut generator = AttackGenerator::new();

        let squares = vec![0, 1];
        let piece_types = vec![PieceType::Rook, PieceType::Bishop];
        let combined =
            generator.generate_combined_attacks(&squares, &piece_types, Bitboard::default());

        assert_ne!(combined, Bitboard::default());
    }

    #[test]
    fn test_attack_with_directions() {
        let mut generator = AttackGenerator::new();

        let directions = vec![
            Direction { row_delta: 0, col_delta: 1 },
            Direction { row_delta: 1, col_delta: 0 },
        ];

        let attacks =
            generator.generate_attack_with_directions(0, &directions, Bitboard::default());
        assert_ne!(attacks, Bitboard::default());
    }

    #[test]
    fn test_attack_stats() {
        let mut generator = AttackGenerator::new();

        let stats = generator.get_attack_stats(40, PieceType::Rook);
        assert_eq!(stats.square, 40);
        assert_eq!(stats.piece_type, PieceType::Rook);
        assert!(stats.max_attacks > 0);
        assert!(stats.min_attacks >= 0);
        assert!(stats.relevant_squares > 0);
    }

    #[test]
    fn test_promoted_pieces() {
        let mut generator = AttackGenerator::new();

        // Test promoted rook (should have more directions)
        let promoted_rook_directions = generator.get_directions(PieceType::PromotedRook);
        assert_eq!(promoted_rook_directions.len(), 8);

        // Test promoted bishop (should have more directions)
        let promoted_bishop_directions = generator.get_directions(PieceType::PromotedBishop);
        assert_eq!(promoted_bishop_directions.len(), 8);
    }

    #[test]
    fn test_edge_cases() {
        let mut generator = AttackGenerator::new();

        // Test corner squares
        let corner_attacks = generator.generate_attack_pattern_no_blockers(0, PieceType::Rook);
        assert_ne!(corner_attacks, Bitboard::default());

        // Test edge squares
        let edge_attacks = generator.generate_attack_pattern_no_blockers(4, PieceType::Bishop);
        assert_ne!(edge_attacks, Bitboard::default());

        // Test center square
        let center_attacks = generator.generate_attack_pattern_no_blockers(40, PieceType::Rook);
        assert_ne!(center_attacks, Bitboard::default());
    }

    #[test]
    fn test_blocker_combinations() {
        let generator = AttackGenerator::new();

        // Test with 2-bit mask
        let mask = Bitboard::from_u128(0b11u128);
        let combinations = generator.generate_all_blocker_combinations(mask);
        assert_eq!(combinations.len(), 4); // 2^2 = 4

        // Test with 3-bit mask
        let mask = Bitboard::from_u128(0b111u128);
        let combinations = generator.generate_all_blocker_combinations(mask);
        assert_eq!(combinations.len(), 8); // 2^3 = 8
    }

    #[test]
    fn test_cache_functionality() {
        let mut generator = AttackGenerator::new();

        // Generate some attack patterns
        generator.generate_attack_pattern(0, PieceType::Rook, Bitboard::default());
        generator.generate_attack_pattern(1, PieceType::Bishop, Bitboard::default());

        let stats = generator.cache_stats();
        assert!(stats.cache_size > 0);

        // Clear cache
        generator.clear_cache();
        let stats = generator.cache_stats();
        assert_eq!(stats.cache_size, 0);
    }
}
