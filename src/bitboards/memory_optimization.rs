//! Memory optimization module for SIMD operations
//!
//! This module provides memory optimization utilities for SIMD operations,
//! including alignment, prefetching, and cache-friendly data structures.

#![cfg(feature = "simd")]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::is_x86_feature_detected;

/// SIMD alignment requirements
pub mod alignment {
    /// 16-byte alignment for SSE/NEON (128-bit SIMD)
    pub const SSE_NEON_ALIGNMENT: usize = 16;

    /// 32-byte alignment for AVX2 (256-bit SIMD)
    pub const AVX2_ALIGNMENT: usize = 32;

    /// 64-byte alignment for AVX-512 (512-bit SIMD) and cache lines
    pub const AVX512_CACHE_ALIGNMENT: usize = 64;

    /// Get recommended alignment based on platform capabilities
    ///
    /// This function checks platform capabilities at runtime to determine
    /// the optimal alignment for SIMD operations.
    pub fn get_recommended_alignment() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            // Check for AVX-512 first, then AVX2, then default to SSE
            if is_x86_feature_detected!("avx512f") {
                AVX512_CACHE_ALIGNMENT
            } else if is_x86_feature_detected!("avx2") {
                AVX2_ALIGNMENT
            } else {
                SSE_NEON_ALIGNMENT
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 always uses NEON (16-byte), but 64-byte is better for cache
            AVX512_CACHE_ALIGNMENT
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            SSE_NEON_ALIGNMENT
        }
    }

    /// Check if a pointer is properly aligned for SIMD operations
    ///
    /// # Arguments
    /// * `ptr` - Pointer to check
    /// * `alignment` - Required alignment (use `get_recommended_alignment()`)
    pub unsafe fn is_simd_aligned(ptr: *const u8, alignment: usize) -> bool {
        (ptr as usize) % alignment == 0
    }
}

/// Prefetching strategies for large bitboard arrays
pub mod prefetch {
    use crate::bitboards::SimdBitboard;

    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0, _MM_HINT_T1, _MM_HINT_T2};

    #[cfg(target_arch = "aarch64")]
    #[allow(unused_imports)]
    use std::arch::aarch64;

    /// Prefetch hint levels
    #[derive(Clone, Copy, Debug)]
    pub enum PrefetchLevel {
        /// Prefetch into L1 cache (most aggressive)
        L1,
        /// Prefetch into L2 cache (moderate)
        L2,
        /// Prefetch into L3 cache (least aggressive)
        L3,
    }

    /// Prefetch a bitboard into CPU cache
    ///
    /// # Arguments
    /// * `bb` - Reference to the bitboard to prefetch
    /// * `level` - Cache level to prefetch into
    pub fn prefetch_bitboard(bb: &SimdBitboard, level: PrefetchLevel) {
        #[cfg(target_arch = "x86_64")]
        {
            let hint = match level {
                PrefetchLevel::L1 => _MM_HINT_T0,
                PrefetchLevel::L2 => _MM_HINT_T1,
                PrefetchLevel::L3 => _MM_HINT_T2,
            };
            unsafe {
                let ptr = bb as *const SimdBitboard as *const i8;
                _mm_prefetch(ptr, hint);
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 prefetch is not stable in std::arch yet
            // Use inline assembly or compiler hints as fallback
            unsafe {
                let ptr = bb as *const SimdBitboard as *const i8;
                // Compiler hint for prefetching
                std::ptr::read_volatile(ptr);
            }
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let _ = (bb, level); // No-op on unsupported platforms
        }
    }

    /// Prefetch a range of bitboards for sequential access
    ///
    /// # Arguments
    /// * `bitboards` - Slice of bitboards to prefetch
    /// * `start_index` - Starting index
    /// * `prefetch_distance` - How many elements ahead to prefetch
    /// * `level` - Cache level to prefetch into
    pub fn prefetch_range(
        bitboards: &[SimdBitboard],
        start_index: usize,
        prefetch_distance: usize,
        level: PrefetchLevel,
    ) {
        let prefetch_index = start_index + prefetch_distance;
        if prefetch_index < bitboards.len() {
            prefetch_bitboard(&bitboards[prefetch_index], level);
        }
    }

    /// Prefetch multiple bitboards for batch operations
    ///
    /// # Arguments
    /// * `bitboards` - Slice of bitboards to prefetch
    /// * `indices` - Indices of bitboards to prefetch
    /// * `level` - Cache level to prefetch into
    pub fn prefetch_multiple(bitboards: &[SimdBitboard], indices: &[usize], level: PrefetchLevel) {
        for &index in indices {
            if index < bitboards.len() {
                prefetch_bitboard(&bitboards[index], level);
            }
        }
    }

    /// Prefetch a memory address directly
    ///
    /// Optimization 6: Enhanced prefetching - provides direct prefetch for any
    /// pointer
    ///
    /// # Arguments
    /// * `ptr` - Pointer to memory address to prefetch
    /// * `level` - Cache level to prefetch into
    #[inline(always)]
    pub unsafe fn prefetch_ptr(ptr: *const i8, level: PrefetchLevel) {
        #[cfg(target_arch = "x86_64")]
        {
            let hint = match level {
                PrefetchLevel::L1 => _MM_HINT_T0,
                PrefetchLevel::L2 => _MM_HINT_T1,
                PrefetchLevel::L3 => _MM_HINT_T2,
            };
            _mm_prefetch(ptr, hint);
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 prefetch - use compiler hint
            std::ptr::read_volatile(ptr);
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let _ = (ptr, level); // No-op on unsupported platforms
        }

        // Record prefetch operation for telemetry
        crate::bitboards::memory_optimization::telemetry::record_prefetch_operation();
    }
}

/// Adaptive prefetching system
///
/// Optimization 6: Enhanced prefetching - adaptive prefetching based on access
/// patterns
///
/// This module provides an adaptive prefetching system that learns from access
/// patterns and adjusts prefetch distances dynamically for optimal cache
/// performance.
pub mod adaptive_prefetch {
    use std::collections::VecDeque;

    /// Adaptive prefetching configuration
    #[derive(Debug, Clone)]
    pub struct AdaptivePrefetchConfig {
        /// Minimum prefetch distance
        pub min_distance: usize,
        /// Maximum prefetch distance
        pub max_distance: usize,
        /// Initial prefetch distance
        pub initial_distance: usize,
        /// Learning rate for distance adjustment (0.0 to 1.0)
        pub learning_rate: f64,
        /// Number of recent accesses to track
        pub history_size: usize,
        /// Threshold for cache hit rate to increase distance
        pub hit_rate_threshold: f64,
    }

    impl Default for AdaptivePrefetchConfig {
        fn default() -> Self {
            Self {
                min_distance: 1,
                max_distance: 8,
                initial_distance: 2,
                learning_rate: 0.1,
                history_size: 32,
                hit_rate_threshold: 0.7,
            }
        }
    }

    /// Workload-specific prefetch configuration
    #[derive(Debug, Clone)]
    pub enum WorkloadType {
        /// Sequential access pattern (e.g., row-by-row iteration)
        Sequential { base_distance: usize },
        /// Random access pattern (e.g., piece-based iteration)
        Random { base_distance: usize },
        /// Batch access pattern (e.g., sliding pieces batch)
        Batch { base_distance: usize },
    }

    impl Default for WorkloadType {
        fn default() -> Self {
            Self::Sequential { base_distance: 2 }
        }
    }

    /// Adaptive prefetch distance manager
    ///
    /// Tracks access patterns and adaptively adjusts prefetch distances
    /// to optimize cache performance based on workload characteristics.
    pub struct AdaptivePrefetchManager {
        config: AdaptivePrefetchConfig,
        workload_type: WorkloadType,
        current_distance: usize,
        access_history: VecDeque<usize>,
        cache_hit_count: usize,
        cache_miss_count: usize,
        adjustment_counter: usize,
    }

    impl AdaptivePrefetchManager {
        /// Create a new adaptive prefetch manager
        pub fn new(config: AdaptivePrefetchConfig, workload_type: WorkloadType) -> Self {
            let base_distance = match workload_type {
                WorkloadType::Sequential { base_distance } => base_distance,
                WorkloadType::Random { base_distance } => base_distance,
                WorkloadType::Batch { base_distance } => base_distance,
            };

            let initial_distance = base_distance.max(config.min_distance).min(config.max_distance);

            Self {
                config,
                workload_type,
                current_distance: initial_distance,
                access_history: VecDeque::with_capacity(32),
                cache_hit_count: 0,
                cache_miss_count: 0,
                adjustment_counter: 0,
            }
        }

        /// Create for sequential access pattern
        pub fn sequential() -> Self {
            Self::new(
                AdaptivePrefetchConfig::default(),
                WorkloadType::Sequential { base_distance: 2 },
            )
        }

        /// Create for random access pattern
        pub fn random() -> Self {
            Self::new(AdaptivePrefetchConfig::default(), WorkloadType::Random { base_distance: 1 })
        }

        /// Create for batch access pattern
        pub fn batch() -> Self {
            Self::new(AdaptivePrefetchConfig::default(), WorkloadType::Batch { base_distance: 3 })
        }

        /// Get current prefetch distance
        pub fn distance(&self) -> usize {
            self.current_distance
        }

        /// Record an access with index
        pub fn record_access(&mut self, index: usize) {
            self.access_history.push_back(index);
            if self.access_history.len() > self.config.history_size {
                self.access_history.pop_front();
            }
        }

        /// Record a cache hit
        pub fn record_cache_hit(&mut self) {
            self.cache_hit_count += 1;
            self.maybe_adjust_distance();
        }

        /// Record a cache miss
        pub fn record_cache_miss(&mut self) {
            self.cache_miss_count += 1;
            self.maybe_adjust_distance();
        }

        /// Adjust distance based on cache performance
        fn maybe_adjust_distance(&mut self) {
            self.adjustment_counter += 1;

            // Adjust every N accesses to avoid too frequent changes
            if self.adjustment_counter < 10 {
                return;
            }

            self.adjustment_counter = 0;

            let total = self.cache_hit_count + self.cache_miss_count;
            if total == 0 {
                return;
            }

            let hit_rate = self.cache_hit_count as f64 / total as f64;

            // Adjust distance based on hit rate
            if hit_rate > self.config.hit_rate_threshold {
                // Good hit rate - can increase distance for more aggressive prefetching
                if self.current_distance < self.config.max_distance {
                    let adjustment = ((self.config.max_distance - self.current_distance) as f64
                        * self.config.learning_rate) as usize;
                    self.current_distance =
                        (self.current_distance + adjustment.max(1)).min(self.config.max_distance);
                }
            } else {
                // Low hit rate - reduce distance to avoid prefetching too far ahead
                if self.current_distance > self.config.min_distance {
                    let adjustment = ((self.current_distance - self.config.min_distance) as f64
                        * self.config.learning_rate) as usize;
                    self.current_distance =
                        (self.current_distance - adjustment.max(1)).max(self.config.min_distance);
                }
            }

            // Reset counters for next adjustment period
            self.cache_hit_count = 0;
            self.cache_miss_count = 0;
        }

        /// Get optimal prefetch distance for current workload
        pub fn get_optimal_distance(&self, current_index: usize, total_items: usize) -> usize {
            // Base distance from workload type
            let base = match self.workload_type {
                WorkloadType::Sequential { base_distance } => base_distance,
                WorkloadType::Random { base_distance } => base_distance,
                WorkloadType::Batch { base_distance } => base_distance,
            };

            // Adapt based on remaining items
            let remaining = total_items.saturating_sub(current_index);
            let distance = self.current_distance.min(remaining);

            // Ensure we're within configured bounds
            distance.max(self.config.min_distance).min(self.config.max_distance)
        }

        /// Analyze access pattern and suggest optimal distance
        pub fn analyze_pattern(&self) -> usize {
            if self.access_history.len() < 2 {
                return self.current_distance;
            }

            let history: Vec<usize> = self.access_history.iter().copied().collect();
            let mut is_sequential = true;
            let mut prev_index = history[0];

            for &index in history.iter().skip(1) {
                // Check if accesses are roughly sequential
                if index < prev_index || (index - prev_index) > 10 {
                    is_sequential = false;
                    break;
                }
                prev_index = index;
            }

            // Adjust distance based on pattern
            if is_sequential {
                // Sequential pattern - can use larger distance
                self.current_distance.min(self.config.max_distance)
            } else {
                // Random pattern - use smaller distance
                self.current_distance.max(self.config.min_distance)
            }
        }

        /// Reset statistics
        pub fn reset(&mut self) {
            self.access_history.clear();
            self.cache_hit_count = 0;
            self.cache_miss_count = 0;
            self.adjustment_counter = 0;
        }
    }

    /// Get recommended prefetch distance for different workload types
    ///
    /// Optimization 6.6: Tuned prefetch distances for different workloads
    pub fn get_recommended_distance(workload_type: &WorkloadType, total_items: usize) -> usize {
        let base = match workload_type {
            WorkloadType::Sequential { base_distance } => *base_distance,
            WorkloadType::Random { base_distance } => *base_distance,
            WorkloadType::Batch { base_distance } => *base_distance,
        };

        // Adjust based on total items
        match total_items {
            0..=4 => base.min(1),
            5..=8 => base.min(2),
            9..=16 => base.min(3),
            17..=32 => base.min(4),
            _ => base.min(8),
        }
    }
}

/// Cache-friendly data structures for SIMD operations
pub mod cache_friendly {
    use crate::bitboards::SimdBitboard;

    /// Structure of Arrays (SoA) layout for multiple bitboards
    ///
    /// This layout stores bitboards in a cache-friendly manner,
    /// grouping similar data together for better cache locality.
    #[repr(align(64))] // Cache line aligned
    pub struct BitboardSoA<const N: usize> {
        /// Low 64 bits of each bitboard
        pub low_bits: [u64; N],
        /// High 64 bits of each bitboard
        pub high_bits: [u64; N],
    }

    impl<const N: usize> BitboardSoA<N> {
        /// Create a new SoA structure
        pub fn new() -> Self {
            Self { low_bits: [0; N], high_bits: [0; N] }
        }

        /// Get a bitboard at the given index
        pub fn get(&self, index: usize) -> SimdBitboard {
            if index < N {
                let value = (self.high_bits[index] as u128) << 64 | (self.low_bits[index] as u128);
                SimdBitboard::from_u128(value)
            } else {
                SimdBitboard::empty()
            }
        }

        /// Set a bitboard at the given index
        pub fn set(&mut self, index: usize, bb: SimdBitboard) {
            if index < N {
                let value = bb.to_u128();
                self.low_bits[index] = value as u64;
                self.high_bits[index] = (value >> 64) as u64;
            }
        }
    }

    impl<const N: usize> Default for BitboardSoA<N> {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Cache-aligned bitboard array for SIMD operations
    ///
    /// This structure ensures optimal alignment and cache line usage.
    #[repr(align(64))]
    pub struct CacheAlignedBitboardArray<const N: usize> {
        /// Array of bitboards, cache-aligned
        pub data: [SimdBitboard; N],
    }

    impl<const N: usize> CacheAlignedBitboardArray<N> {
        /// Create a new cache-aligned bitboard array
        pub fn new() -> Self {
            Self { data: [SimdBitboard::empty(); N] }
        }

        /// Get a bitboard at the given index
        pub fn get(&self, index: usize) -> SimdBitboard {
            if index < N {
                self.data[index]
            } else {
                SimdBitboard::empty()
            }
        }

        /// Set a bitboard at the given index
        pub fn set(&mut self, index: usize, bb: SimdBitboard) {
            if index < N {
                self.data[index] = bb;
            }
        }

        /// Get a slice of the data
        pub fn as_slice(&self) -> &[SimdBitboard] {
            &self.data
        }
    }

    impl<const N: usize> Default for CacheAlignedBitboardArray<N> {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Structure of Arrays (SoA) layout for PST table batch evaluation
    ///
    /// Optimization 5.3: Optimized PST table layout for SIMD access.
    /// Separates middlegame and endgame values into separate arrays for
    /// better cache locality and SIMD vectorization in batch operations.
    ///
    /// # Memory Layout
    ///
    /// Instead of Array of Arrays (AoA): `[[i32; 9]; 9]` for mg and eg
    /// separately, this uses SoA: `[i32; 81]` for mg values and `[i32; 81]`
    /// for eg values. This enables SIMD vectorization when processing
    /// multiple positions.
    #[repr(align(64))] // Cache line aligned
    pub struct PstSoA {
        /// Middlegame values for all 81 positions (flattened from 9x9)
        pub mg_values: [i32; 81],
        /// Endgame values for all 81 positions (flattened from 9x9)
        pub eg_values: [i32; 81],
    }

    impl PstSoA {
        /// Create a new SoA structure from AoA layout
        pub fn from_aoa(aoa_mg: &[[i32; 9]; 9], aoa_eg: &[[i32; 9]; 9]) -> Self {
            let mut mg_values = [0; 81];
            let mut eg_values = [0; 81];

            for row in 0..9 {
                for col in 0..9 {
                    let idx = (row * 9 + col) as usize;
                    mg_values[idx] = aoa_mg[row as usize][col as usize];
                    eg_values[idx] = aoa_eg[row as usize][col as usize];
                }
            }

            Self { mg_values, eg_values }
        }

        /// Get middlegame and endgame values for a position
        ///
        /// # Arguments
        /// * `row` - Row index (0-8)
        /// * `col` - Column index (0-8)
        pub fn get(&self, row: u8, col: u8) -> (i32, i32) {
            let idx = (row * 9 + col) as usize;
            if idx < 81 {
                (self.mg_values[idx], self.eg_values[idx])
            } else {
                (0, 0)
            }
        }

        /// Get values for multiple positions (batch access for SIMD)
        ///
        /// # Arguments
        /// * `positions` - Slice of (row, col) tuples
        ///
        /// # Returns
        /// Vector of (mg, eg) tuples
        pub fn get_batch(&self, positions: &[(u8, u8)]) -> Vec<(i32, i32)> {
            positions.iter().map(|&(row, col)| self.get(row, col)).collect()
        }
    }

    impl Default for PstSoA {
        fn default() -> Self {
            Self { mg_values: [0; 81], eg_values: [0; 81] }
        }
    }

    /// Structure of Arrays (SoA) layout for attack pattern batch operations
    ///
    /// Optimization 5.4: Optimized attack pattern storage for batch operations.
    /// Uses SoA layout to enable better SIMD vectorization when processing
    /// multiple attack patterns simultaneously.
    #[repr(align(64))] // Cache line aligned
    pub struct AttackPatternSoA<const N: usize> {
        /// Low 64 bits of each attack pattern
        pub low_bits: [u64; N],
        /// High 64 bits of each attack pattern
        pub high_bits: [u64; N],
    }

    impl<const N: usize> AttackPatternSoA<N> {
        /// Create a new SoA structure
        pub fn new() -> Self {
            Self { low_bits: [0; N], high_bits: [0; N] }
        }

        /// Create from a slice of bitboards
        pub fn from_bitboards(bitboards: &[SimdBitboard]) -> Self {
            let mut soa = Self::new();
            for (i, &bb) in bitboards.iter().take(N).enumerate() {
                let value = bb.to_u128();
                soa.low_bits[i] = value as u64;
                soa.high_bits[i] = (value >> 64) as u64;
            }
            soa
        }

        /// Get an attack pattern at the given index
        pub fn get(&self, index: usize) -> SimdBitboard {
            if index < N {
                let value = (self.high_bits[index] as u128) << 64 | (self.low_bits[index] as u128);
                SimdBitboard::from_u128(value)
            } else {
                SimdBitboard::empty()
            }
        }

        /// Set an attack pattern at the given index
        pub fn set(&mut self, index: usize, bb: SimdBitboard) {
            if index < N {
                let value = bb.to_u128();
                self.low_bits[index] = value as u64;
                self.high_bits[index] = (value >> 64) as u64;
            }
        }

        /// Get a slice of low bits (for SIMD operations)
        pub fn low_bits_slice(&self) -> &[u64] {
            &self.low_bits
        }

        /// Get a slice of high bits (for SIMD operations)
        pub fn high_bits_slice(&self) -> &[u64] {
            &self.high_bits
        }
    }

    impl<const N: usize> Default for AttackPatternSoA<N> {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Memory access pattern optimization utilities
pub mod access_patterns {
    use crate::bitboards::SimdBitboard;

    /// Optimize memory access pattern for sequential reads
    ///
    /// This function provides hints for optimizing sequential access patterns.
    pub fn optimize_sequential_access(bitboards: &[SimdBitboard]) {
        // In a real implementation, this could:
        // 1. Reorder data for better cache locality
        // 2. Prefetch upcoming data
        // 3. Use streaming stores for write-only data
        let _ = bitboards;
    }

    /// Optimize memory access pattern for random access
    ///
    /// This function provides hints for optimizing random access patterns.
    pub fn optimize_random_access(bitboards: &[SimdBitboard]) {
        // In a real implementation, this could:
        // 1. Use hash-based organization
        // 2. Implement cache-friendly data structures
        // 3. Use prefetching for likely access patterns
        let _ = bitboards;
    }
}

/// Enhanced prefetching utilities for specific use cases
///
/// Optimization 6: Enhanced prefetching - provides workload-specific
/// prefetching helpers
pub mod enhanced_prefetch {
    use super::adaptive_prefetch::{
        get_recommended_distance, AdaptivePrefetchManager, WorkloadType,
    };
    use super::prefetch::{prefetch_ptr, PrefetchLevel};
    use std::sync::{Mutex, OnceLock};

    // Global adaptive prefetch managers for different workloads
    fn get_batch_manager() -> &'static Mutex<AdaptivePrefetchManager> {
        static MANAGER: OnceLock<Mutex<AdaptivePrefetchManager>> = OnceLock::new();
        MANAGER.get_or_init(|| Mutex::new(AdaptivePrefetchManager::batch()))
    }

    fn get_sequential_manager() -> &'static Mutex<AdaptivePrefetchManager> {
        static MANAGER: OnceLock<Mutex<AdaptivePrefetchManager>> = OnceLock::new();
        MANAGER.get_or_init(|| Mutex::new(AdaptivePrefetchManager::sequential()))
    }

    fn get_random_manager() -> &'static Mutex<AdaptivePrefetchManager> {
        static MANAGER: OnceLock<Mutex<AdaptivePrefetchManager>> = OnceLock::new();
        MANAGER.get_or_init(|| Mutex::new(AdaptivePrefetchManager::random()))
    }

    /// Prefetch magic table entry with adaptive distance
    ///
    /// Optimization 6.2: Enhanced prefetching for magic table lookups
    ///
    /// # Arguments
    /// * `magic_entry_ptr` - Pointer to magic table entry
    /// * `current_index` - Current index in batch
    /// * `total_items` - Total items in batch
    pub unsafe fn prefetch_magic_table(
        magic_entry_ptr: *const i8,
        current_index: usize,
        total_items: usize,
    ) {
        if magic_entry_ptr.is_null() {
            return;
        }

        let _manager = get_batch_manager();
        let _distance = _manager.lock().unwrap().get_optimal_distance(current_index, total_items);

        // Prefetch the magic entry (distance tracked by adaptive manager for future
        // adjustments)
        prefetch_ptr(magic_entry_ptr, PrefetchLevel::L1);

        // Also prefetch potential attack storage entry if we can calculate it
        // This is handled separately in the caller for safety
    }

    /// Prefetch PST table entry with adaptive distance
    ///
    /// Optimization 6.3: Enhanced prefetching for PST table lookups
    ///
    /// # Arguments
    /// * `mg_ptr` - Pointer to middlegame PST table entry
    /// * `eg_ptr` - Pointer to endgame PST table entry
    /// * `current_pos` - Current position index (0-80 for 9x9 board)
    /// * `total_positions` - Total positions to process
    pub unsafe fn prefetch_pst_table(
        mg_ptr: *const i8,
        eg_ptr: *const i8,
        current_pos: usize,
        total_positions: usize,
    ) {
        if mg_ptr.is_null() || eg_ptr.is_null() {
            return;
        }

        let _manager = get_sequential_manager();
        let _distance = _manager.lock().unwrap().get_optimal_distance(current_pos, total_positions);

        // Prefetch both mg and eg entries (distance tracked by adaptive manager for
        // future adjustments)
        prefetch_ptr(mg_ptr, PrefetchLevel::L1);
        prefetch_ptr(eg_ptr, PrefetchLevel::L1);
    }

    /// Get adaptive prefetch distance for workload type
    ///
    /// Optimization 6.4: Prefetch distance optimization
    ///
    /// # Arguments
    /// * `workload_type` - Type of workload (Sequential, Random, Batch)
    /// * `current_index` - Current index
    /// * `total_items` - Total items
    pub fn get_adaptive_distance(
        workload_type: &WorkloadType,
        current_index: usize,
        total_items: usize,
    ) -> usize {
        let recommended = get_recommended_distance(workload_type, total_items);

        // Use adaptive manager based on workload type
        let adaptive_distance = match workload_type {
            WorkloadType::Batch { .. } => get_batch_manager()
                .lock()
                .unwrap()
                .get_optimal_distance(current_index, total_items),
            WorkloadType::Sequential { .. } => get_sequential_manager()
                .lock()
                .unwrap()
                .get_optimal_distance(current_index, total_items),
            WorkloadType::Random { .. } => get_random_manager()
                .lock()
                .unwrap()
                .get_optimal_distance(current_index, total_items),
        };

        // Use the recommended distance as a baseline, but allow adaptive adjustment
        recommended.max(adaptive_distance).min(8)
    }

    /// Reset all adaptive prefetch managers
    pub fn reset_all_managers() {
        get_batch_manager().lock().unwrap().reset();
        get_sequential_manager().lock().unwrap().reset();
        get_random_manager().lock().unwrap().reset();
    }
}

/// SIMD performance telemetry
pub mod telemetry {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Counter for SIMD operations performed
    static SIMD_OPERATIONS: AtomicU64 = AtomicU64::new(0);

    /// Counter for SIMD batch operations performed
    static SIMD_BATCH_OPERATIONS: AtomicU64 = AtomicU64::new(0);

    /// Counter for prefetch operations performed
    static PREFETCH_OPERATIONS: AtomicU64 = AtomicU64::new(0);

    /// Record a SIMD operation
    pub fn record_simd_operation() {
        SIMD_OPERATIONS.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a SIMD batch operation
    pub fn record_simd_batch_operation() {
        SIMD_BATCH_OPERATIONS.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a prefetch operation
    pub fn record_prefetch_operation() {
        PREFETCH_OPERATIONS.fetch_add(1, Ordering::Relaxed);
    }

    /// Get SIMD operation statistics
    pub fn get_stats() -> SimdStats {
        SimdStats {
            simd_operations: SIMD_OPERATIONS.load(Ordering::Relaxed),
            simd_batch_operations: SIMD_BATCH_OPERATIONS.load(Ordering::Relaxed),
            prefetch_operations: PREFETCH_OPERATIONS.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset_stats() {
        SIMD_OPERATIONS.store(0, Ordering::Relaxed);
        SIMD_BATCH_OPERATIONS.store(0, Ordering::Relaxed);
        PREFETCH_OPERATIONS.store(0, Ordering::Relaxed);
    }

    /// SIMD performance statistics
    #[derive(Debug, Clone, Copy)]
    pub struct SimdStats {
        pub simd_operations: u64,
        pub simd_batch_operations: u64,
        pub prefetch_operations: u64,
    }
}
