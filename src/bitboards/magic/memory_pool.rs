//! Memory pool for efficient allocation of attack tables in magic bitboards
//!
//! This module provides a memory pool implementation optimized for allocating
//! attack pattern tables for magic bitboards. It uses pre-allocated blocks
//! to reduce memory fragmentation and improve cache locality.

use crate::types::{Bitboard, MagicError, MemoryPool};

impl MemoryPool {
    /// Create a new memory pool with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new memory pool with custom block size
    pub fn with_block_size(block_size: usize) -> Self {
        Self { blocks: Vec::new(), current_block: 0, current_offset: 0, block_size }
    }

    /// Create a new memory pool with adaptive block sizing
    ///
    /// The block size is calculated based on the estimated total size:
    /// - Small tables (< 1MB): 1024 entries per block
    /// - Medium tables (1-10MB): 4096 entries per block
    /// - Large tables (> 10MB): 16384 entries per block
    pub fn with_adaptive_block_size(estimated_total_size: usize) -> Self {
        let block_size = if estimated_total_size < 1_000_000 {
            1024
        } else if estimated_total_size < 10_000_000 {
            4096
        } else {
            16384
        };

        Self { blocks: Vec::new(), current_block: 0, current_offset: 0, block_size }
    }

    /// Allocate memory for attack table
    ///
    /// Returns the base index where the allocated memory starts
    pub fn allocate(&mut self, size: usize) -> Result<usize, MagicError> {
        // The caller indexes one flat `attack_storage` vector with what this
        // returns, so bases have to be globally unique and monotonic. The
        // previous version returned an offset *within the current block* and
        // reset to 0 whenever it started a new one: every table allocated after
        // the first block landed on top of an earlier one. Square 0's rook
        // table was overwritten by a later square's, which is what made
        // `validate()` fail at square 0 with no blockers.
        if self.blocks.is_empty() {
            self.allocate_new_block()?;
        }
        let base = self.current_block * self.block_size + self.current_offset;
        let mut end = self.current_offset + size;
        // A request may span several blocks; `allocate_new_block` advances
        // `current_block` by one each time.
        while end > self.block_size {
            self.allocate_new_block()?;
            end -= self.block_size;
        }
        self.current_offset = end;
        Ok(base)
    }

    /// Allocate new memory block
    fn allocate_new_block(&mut self) -> Result<(), MagicError> {
        let block = vec![Bitboard::default(); self.block_size];
        self.blocks.push(block);
        self.current_block = self.blocks.len() - 1;
        self.current_offset = 0;
        Ok(())
    }

    /// Get the total number of allocated blocks
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Get the total allocated memory in bytes
    pub fn total_allocated_bytes(&self) -> usize {
        self.blocks.len() * self.block_size * std::mem::size_of::<Bitboard>()
    }

    /// Get the current block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get the current offset within the current block
    pub fn current_offset(&self) -> usize {
        self.current_offset
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Clear all allocated memory
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.current_block = 0;
        self.current_offset = 0;
    }

    /// Reserve space for a specific number of attack patterns
    ///
    /// This is useful for pre-allocating space when the total size is known
    pub fn reserve(&mut self, total_patterns: usize) -> Result<(), MagicError> {
        let blocks_needed = (total_patterns + self.block_size - 1) / self.block_size;

        for _ in 0..blocks_needed {
            self.allocate_new_block()?;
        }

        Ok(())
    }

    /// Get memory usage statistics
    pub fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            total_blocks: self.blocks.len(),
            current_block: self.current_block,
            current_offset: self.current_offset,
            block_size: self.block_size,
            total_allocated_bytes: self.total_allocated_bytes(),
            utilization_percentage: if self.blocks.is_empty() {
                0.0
            } else {
                (self.current_offset as f64 / self.block_size as f64) * 100.0
            },
        }
    }
}

/// Memory usage statistics for the memory pool
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_blocks: usize,
    pub current_block: usize,
    pub current_offset: usize,
    pub block_size: usize,
    pub total_allocated_bytes: usize,
    pub utilization_percentage: f64,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            total_blocks: 0,
            current_block: 0,
            current_offset: 0,
            block_size: 0,
            total_allocated_bytes: 0,
            utilization_percentage: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool_creation() {
        let pool = MemoryPool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.block_count(), 0);
        assert_eq!(pool.total_allocated_bytes(), 0);
    }

    #[test]
    fn test_memory_pool_custom_block_size() {
        let pool = MemoryPool::with_block_size(1024);
        assert_eq!(pool.block_size(), 1024);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_memory_allocation() {
        let mut pool = MemoryPool::with_block_size(100);

        // Allocate within first block
        let offset1 = pool.allocate(50).unwrap();
        assert_eq!(offset1, 0);
        assert_eq!(pool.current_offset(), 50);
        assert_eq!(pool.block_count(), 1);

        // Allocate more within first block
        let offset2 = pool.allocate(30).unwrap();
        assert_eq!(offset2, 50);
        assert_eq!(pool.current_offset(), 80);
        assert_eq!(pool.block_count(), 1);

        // Allocate beyond the first block. The base must keep climbing: the
        // caller indexes one flat vector with it, so restarting at 0 for each
        // new block (as this asserted, and as the pool used to do) hands out a
        // region that overlaps the very first allocation.
        let offset3 = pool.allocate(50).unwrap();
        assert_eq!(offset3, 80, "bases are global, not block-relative");
        assert_eq!(pool.block_count(), 2);

        // No two allocations may overlap, across any number of blocks.
        let mut pool = MemoryPool::with_block_size(100);
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for size in [30usize, 50, 40, 120, 10, 90] {
            let base = pool.allocate(size).unwrap();
            for (prev_base, prev_size) in &spans {
                let overlaps = base < prev_base + prev_size && *prev_base < base + size;
                assert!(
                    !overlaps,
                    "allocation {base}..{} overlaps {prev_base}..{}",
                    base + size,
                    prev_base + prev_size
                );
            }
            spans.push((base, size));
        }
    }

    #[test]
    fn test_memory_pool_clear() {
        let mut pool = MemoryPool::with_block_size(100);
        pool.allocate(50).unwrap();
        assert_eq!(pool.block_count(), 1);

        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.block_count(), 0);
    }

    #[test]
    fn test_memory_stats() {
        let mut pool = MemoryPool::with_block_size(100);
        pool.allocate(75).unwrap();

        let stats = pool.memory_stats();
        assert_eq!(stats.total_blocks, 1);
        assert_eq!(stats.current_offset, 75);
        assert_eq!(stats.block_size, 100);
        assert_eq!(stats.utilization_percentage, 75.0);
    }

    #[test]
    fn test_reserve() {
        let mut pool = MemoryPool::with_block_size(100);
        pool.reserve(250).unwrap();

        assert_eq!(pool.block_count(), 3); // 250 / 100 = 3 blocks
        assert_eq!(pool.current_offset(), 0); // No allocation yet
    }
}
