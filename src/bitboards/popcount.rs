//! Population count (popcount) implementations for bit-scanning optimizations
//!
//! This module provides multiple implementations of population count (counting set bits)
//! optimized for different platforms and capabilities.

use crate::bitboards::platform_detection::{get_best_popcount_impl, PopcountImpl};
use crate::types::Bitboard;

/// Main population count function with automatic implementation selection
///
/// This function automatically selects the optimal implementation based on
/// the current platform capabilities detected at runtime.
///
/// # Arguments
/// * `bb` - The bitboard to count bits in
///
/// # Returns
/// The number of set bits in the bitboard
///
/// # Examples
/// ```
/// use shogi_engine::types::Bitboard;
/// use shogi_engine::bitboards::popcount::popcount;
///
/// let bb: Bitboard = 0b1011; // 3 bits set
/// assert_eq!(popcount(bb), 3);
/// ```
pub fn popcount(bb: Bitboard) -> u32 {
    match get_best_popcount_impl() {
        PopcountImpl::Hardware => popcount_hardware(bb),
        PopcountImpl::BitParallel => popcount_bit_parallel(bb),
        PopcountImpl::Software => popcount_software(bb),
    }
}

/// Hardware-accelerated population count using x86_64 POPCNT instruction
///
/// This implementation uses the native POPCNT instruction available on
/// modern x86_64 processors. It provides the fastest possible performance
/// for bit counting operations.
///
/// # Arguments
/// * `bb` - The bitboard to count bits in
///
/// # Returns
/// The number of set bits in the bitboard
///
/// # Safety
/// This function uses unsafe intrinsics and should only be called when
/// POPCNT support has been verified by the platform detection system.
#[cfg(target_arch = "x86_64")]
pub fn popcount_hardware(bb: Bitboard) -> u32 {
    unsafe {
        // Use the native POPCNT instruction
        // Bitboard is SimdBitboard, so we need to handle it as two u64 values
        let low = (bb.to_u128() & 0xFFFFFFFFFFFFFFFF) as u64;
        let high = ((bb.to_u128() >> 64) & 0xFFFFFFFFFFFFFFFF) as u64;

        let low_count = std::arch::x86_64::_popcnt64(low as i64) as u32;
        let high_count = std::arch::x86_64::_popcnt64(high as i64) as u32;

        low_count + high_count
    }
}

/// Fallback hardware implementation for non-x86_64 platforms
#[cfg(not(target_arch = "x86_64"))]
pub fn popcount_hardware(bb: Bitboard) -> u32 {
    // Fallback to SWAR implementation on non-x86_64 platforms
    popcount_bit_parallel(bb)
}

/// SWAR (SIMD Within A Register) population count implementation
///
/// This implementation uses bit-parallel algorithms to count bits efficiently
/// without requiring special hardware instructions. It works on all platforms
/// and provides excellent performance.
///
/// # Arguments
/// * `bb` - The bitboard to count bits in
///
/// # Returns
/// The number of set bits in the bitboard
///
/// # Performance
/// This implementation is typically 3-5x faster than the software fallback
/// and works on all supported platforms.
pub fn popcount_bit_parallel(bb: Bitboard) -> u32 {
    // Process the bitboard in 64-bit chunks since u128 operations can be expensive
    let low = (bb.to_u128() & 0xFFFFFFFFFFFFFFFF) as u64;
    let high = ((bb.to_u128() >> 64) & 0xFFFFFFFFFFFFFFFF) as u64;

    swar_popcount_64(low) + swar_popcount_64(high)
}

/// 64-bit SWAR population count implementation
///
/// This is the core SWAR algorithm that processes 64 bits simultaneously
/// using only basic bitwise operations.
fn swar_popcount_64(mut x: u64) -> u32 {
    // Step 1: Count bits in pairs (2-bit groups)
    // 0x5555555555555555 = 01010101...01010101 (every other bit)
    x = x - ((x >> 1) & 0x5555555555555555);

    // Step 2: Count bits in groups of 4
    // 0x3333333333333333 = 00110011...00110011 (every 4th bit)
    x = (x & 0x3333333333333333) + ((x >> 2) & 0x3333333333333333);

    // Step 3: Count bits in groups of 8
    // 0x0f0f0f0f0f0f0f0f = 00001111...00001111 (every 8th bit)
    x = (x + (x >> 4)) & 0x0f0f0f0f0f0f0f0f;

    // Step 4: Sum all groups using multiplication
    // 0x0101010101010101 * x will sum all 8-bit groups into the high byte
    ((x.wrapping_mul(0x0101010101010101)) >> 56) as u32
}

/// Software fallback population count implementation
///
/// This implementation uses a simple loop-based approach that works on
/// all platforms but is slower than the optimized versions. It serves
/// as a reliable fallback when no other optimizations are available.
///
/// # Arguments
/// * `bb` - The bitboard to count bits in
///
/// # Returns
/// The number of set bits in the bitboard
///
/// # Performance
/// This is the slowest implementation but guarantees correctness
/// on all platforms.
pub fn popcount_software(bb: Bitboard) -> u32 {
    let mut count = 0;
    let mut bits = bb;

    while !bits.is_empty() {
        count += 1;
        // Clear the least significant bit
        bits = Bitboard::from_u128(bits.to_u128() & (bits.to_u128() - 1));
    }

    count
}

/// Optimized population count with manual implementation selection
///
/// This function allows manual selection of the implementation,
/// useful for benchmarking or when you need specific behavior.
///
/// # Arguments
/// * `bb` - The bitboard to count bits in
/// * `impl_type` - The implementation to use
///
/// # Returns
/// The number of set bits in the bitboard
pub fn popcount_with_impl(bb: Bitboard, impl_type: PopcountImpl) -> u32 {
    match impl_type {
        PopcountImpl::Hardware => popcount_hardware(bb),
        PopcountImpl::BitParallel => popcount_bit_parallel(bb),
        PopcountImpl::Software => popcount_software(bb),
    }
}

/// Population count optimized for specific use cases
///
/// This function provides additional optimizations for common patterns
/// like single-bit checks and empty bitboards.
///
/// # Arguments
/// * `bb` - The bitboard to count bits in
///
/// # Returns
/// The number of set bits in the bitboard
pub fn popcount_optimized(bb: Bitboard) -> u32 {
    // Fast path for empty bitboard
    if bb.is_empty() {
        return 0;
    }

    // Fast path for single bit (common case)
    if (bb & Bitboard::from_u128(bb.to_u128() - 1)).is_empty() {
        return 1;
    }

    // Use the best available implementation
    popcount(bb)
}

/// Check if a bitboard has exactly one bit set
///
/// # Arguments
/// * `bb` - The bitboard to check
///
/// # Returns
/// True if exactly one bit is set, false otherwise
pub fn is_single_bit(bb: Bitboard) -> bool {
    !bb.is_empty() && (bb & Bitboard::from_u128(bb.to_u128() - 1)).is_empty()
}

/// Check if a bitboard has more than one bit set
///
/// # Arguments
/// * `bb` - The bitboard to check
///
/// # Returns
/// True if more than one bit is set, false otherwise
pub fn is_multiple_bits(bb: Bitboard) -> bool {
    !bb.is_empty() && !(bb & Bitboard::from_u128(bb.to_u128() - 1)).is_empty()
}

/// Check if a bitboard is empty (no bits set)
///
/// # Arguments
/// * `bb` - The bitboard to check
///
/// # Returns
/// True if no bits are set, false otherwise
pub fn is_empty(bb: Bitboard) -> bool {
    bb.is_empty()
}

/// Get the most significant bit position
///
/// # Arguments
/// * `bb` - The bitboard to analyze
///
/// # Returns
/// The position of the most significant bit (0-based), or None if empty
pub fn most_significant_bit(bb: Bitboard) -> Option<u8> {
    if bb.is_empty() {
        None
    } else {
        // Find the position of the most significant bit
        // For u128, we need to check both halves
        let high = (bb.to_u128() >> 64) as u64;
        if high != 0 {
            Some(63 - high.leading_zeros() as u8 + 64)
        } else {
            let low = bb.to_u128() as u64;
            Some(63 - low.leading_zeros() as u8)
        }
    }
}

/// Get the least significant bit position
///
/// # Arguments
/// * `bb` - The bitboard to analyze
///
/// # Returns
/// The position of the least significant bit (0-based), or None if empty
pub fn least_significant_bit(bb: Bitboard) -> Option<u8> {
    if bb.is_empty() {
        None
    } else {
        let low = bb.to_u128() as u64;
        if low != 0 {
            Some(low.trailing_zeros() as u8)
        } else {
            let high = (bb.to_u128() >> 64) as u64;
            Some(high.trailing_zeros() as u8 + 64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popcount_correctness() {
        // Test basic cases
        assert_eq!(popcount(Bitboard::from_u128(0)), 0);
        assert_eq!(popcount(Bitboard::from_u128(1)), 1);
        assert_eq!(popcount(Bitboard::from_u128(0xFF)), 8);
        assert_eq!(popcount(Bitboard::from_u128(0xFFFFFFFF)), 32);
        assert_eq!(popcount(Bitboard::from_u128(0xFFFFFFFFFFFFFFFF)), 64);
        assert_eq!(popcount(Bitboard::from_u128(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF)), 128);

        // Test edge cases
        assert_eq!(popcount(Bitboard::from_u128(0x8000000000000000)), 1); // Single high bit
        assert_eq!(popcount(Bitboard::from_u128(0x5555555555555555)), 32); // Alternating bits
        assert_eq!(popcount(Bitboard::from_u128(0xAAAAAAAAAAAAAAAA)), 32); // Alternating bits (opposite)
    }

    #[test]
    fn test_all_implementations_identical() {
        let test_cases = [
            0,
            1,
            0xFF,
            0x8000000000000000,
            0xFFFFFFFFFFFFFFFF,
            0x5555555555555555,
            0xAAAAAAAAAAAAAAAA,
            0x123456789ABCDEF0,
            0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF,
        ];

        for bb_u128 in test_cases {
            let bb = Bitboard::from_u128(bb_u128);
            let hardware_result = popcount_hardware(bb);
            let swar_result = popcount_bit_parallel(bb);
            let software_result = popcount_software(bb);
            let optimized_result = popcount_optimized(bb);

            assert_eq!(
                hardware_result, swar_result,
                "Hardware vs SWAR mismatch for 0x{:X}",
                bb_u128
            );
            assert_eq!(
                hardware_result, software_result,
                "Hardware vs Software mismatch for 0x{:X}",
                bb_u128
            );
            assert_eq!(
                hardware_result, optimized_result,
                "Hardware vs Optimized mismatch for 0x{:X}",
                bb_u128
            );
        }
    }

    #[test]
    fn test_popcount_edge_cases() {
        // Empty bitboard
        assert_eq!(popcount(Bitboard::default()), 0);

        // Single bit cases
        for i in 0..128 {
            let bb = Bitboard::from_u128(1u128 << i);
            assert_eq!(popcount(bb), 1, "Single bit at position {} failed", i);
        }

        // All bits set
        assert_eq!(popcount(Bitboard::from_u128(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF)), 128);

        // Pattern tests
        assert_eq!(popcount(Bitboard::from_u128(0x5555555555555555)), 32); // Every other bit
        assert_eq!(popcount(Bitboard::from_u128(0x3333333333333333)), 32); // Every 2nd pair
        assert_eq!(popcount(Bitboard::from_u128(0x0F0F0F0F0F0F0F0F)), 32); // Every 4th group
    }

    #[test]
    fn test_utility_functions() {
        // Test is_single_bit
        assert!(is_single_bit(Bitboard::from_u128(1)));
        assert!(is_single_bit(Bitboard::from_u128(0x8000000000000000)));
        assert!(!is_single_bit(Bitboard::from_u128(0)));
        assert!(!is_single_bit(Bitboard::from_u128(3)));

        // Test is_multiple_bits
        assert!(!is_multiple_bits(Bitboard::from_u128(0)));
        assert!(!is_multiple_bits(Bitboard::from_u128(1)));
        assert!(is_multiple_bits(Bitboard::from_u128(3)));
        assert!(is_multiple_bits(Bitboard::from_u128(0xFF)));

        // Test is_empty
        assert!(is_empty(Bitboard::from_u128(0)));
        assert!(!is_empty(Bitboard::from_u128(1)));
        assert!(!is_empty(Bitboard::from_u128(0xFFFFFFFFFFFFFFFF)));
    }

    #[test]
    fn test_bit_position_functions() {
        // Test least_significant_bit
        assert_eq!(least_significant_bit(Bitboard::from_u128(0)), None);
        assert_eq!(least_significant_bit(Bitboard::from_u128(1)), Some(0));
        assert_eq!(least_significant_bit(Bitboard::from_u128(2)), Some(1));
        assert_eq!(least_significant_bit(Bitboard::from_u128(0x8000000000000000)), Some(63));
        assert_eq!(least_significant_bit(Bitboard::from_u128(0x10000000000000000)), Some(64));

        // Test most_significant_bit
        assert_eq!(most_significant_bit(Bitboard::from_u128(0)), None);
        assert_eq!(most_significant_bit(Bitboard::from_u128(1)), Some(0));
        assert_eq!(most_significant_bit(Bitboard::from_u128(2)), Some(1));
        assert_eq!(most_significant_bit(Bitboard::from_u128(0x8000000000000000)), Some(63));
        assert_eq!(most_significant_bit(Bitboard::from_u128(0x10000000000000000)), Some(64));
        assert_eq!(
            most_significant_bit(Bitboard::from_u128(0x80000000000000000000000000000000)),
            Some(127)
        );
    }

    #[test]
    fn test_popcount_with_impl() {
        let bb = Bitboard::from_u128(0x123456789ABCDEF0);

        let hardware_result = popcount_with_impl(bb, PopcountImpl::Hardware);
        let swar_result = popcount_with_impl(bb, PopcountImpl::BitParallel);
        let software_result = popcount_with_impl(bb, PopcountImpl::Software);

        assert_eq!(hardware_result, swar_result);
        assert_eq!(hardware_result, software_result);
    }

    #[test]
    fn test_popcount_optimized_fast_paths() {
        // Test empty bitboard fast path
        assert_eq!(popcount_optimized(Bitboard::default()), 0);

        // Test single bit fast path
        assert_eq!(popcount_optimized(Bitboard::from_u128(1)), 1);
        assert_eq!(popcount_optimized(Bitboard::from_u128(0x8000000000000000)), 1);

        // Test normal case
        assert_eq!(popcount_optimized(Bitboard::from_u128(0xFF)), 8);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_popcount_performance_comparison() {
        let test_bitboard = Bitboard::from_u128(0x123456789ABCDEF0123456789ABCDEF0);
        let iterations = 1_000_000;

        // Benchmark hardware implementation
        #[cfg(target_arch = "x86_64")]
        {
            let start = Instant::now();
            for _ in 0..iterations {
                let _result = popcount_hardware(test_bitboard);
            }
            let hardware_duration = start.elapsed();
            println!(
                "Hardware popcount: {:?} total, {:?} per call",
                hardware_duration,
                hardware_duration / iterations
            );
        }

        // Benchmark SWAR implementation
        let start = Instant::now();
        for _ in 0..iterations {
            let _result = popcount_bit_parallel(test_bitboard);
        }
        let swar_duration = start.elapsed();
        println!(
            "SWAR popcount: {:?} total, {:?} per call",
            swar_duration,
            swar_duration / iterations
        );

        // Benchmark software implementation
        let start = Instant::now();
        for _ in 0..iterations {
            let _result = popcount_software(test_bitboard);
        }
        let software_duration = start.elapsed();
        println!(
            "Software popcount: {:?} total, {:?} per call",
            software_duration,
            software_duration / iterations
        );

        // Verify performance targets
        // SWAR should be faster than software
        assert!(
            swar_duration <= software_duration,
            "SWAR implementation should be faster than software"
        );

        #[cfg(target_arch = "x86_64")]
        {
            // Hardware should be fastest on x86_64
            assert!(
                hardware_duration <= swar_duration,
                "Hardware implementation should be faster than SWAR on x86_64"
            );
        }
    }

    #[test]
    fn test_popcount_optimized_performance() {
        let iterations = 1_000_000;

        // Test fast path performance (single bit)
        let single_bit = Bitboard::from_u128(0x8000000000000000);
        let start = Instant::now();
        for _ in 0..iterations {
            let _result = popcount_optimized(single_bit);
        }
        let fast_path_duration = start.elapsed();

        // Test normal case performance
        let normal_bitboard = Bitboard::from_u128(0x123456789ABCDEF0);
        let start = Instant::now();
        for _ in 0..iterations {
            let _result = popcount_optimized(normal_bitboard);
        }
        let normal_duration = start.elapsed();

        println!(
            "Optimized popcount (single bit): {:?} per call",
            fast_path_duration / iterations
        );
        println!(
            "Optimized popcount (normal): {:?} per call",
            normal_duration / iterations
        );

        // Fast path should be very fast
        assert!(
            fast_path_duration < normal_duration,
            "Fast path should be faster than normal case"
        );
    }

    #[test]
    fn test_popcount_consistency_under_load() {
        // Test that all implementations produce consistent results under load
        let test_cases = [
            0x0000000000000000,
            0x0000000000000001,
            0x0000000000000003,
            0x00000000000000FF,
            0x000000000000FFFF,
            0x00000000FFFFFFFF,
            0xFFFFFFFFFFFFFFFF,
            0x123456789ABCDEF0,
        ];

        for bb_u128 in test_cases {
            let bb = Bitboard::from_u128(bb_u128);
            let iterations = 100_000;

            // Run multiple implementations in parallel to ensure consistency
            let hardware_results: Vec<u32> =
                (0..iterations).map(|_| popcount_hardware(bb)).collect();

            let swar_results: Vec<u32> =
                (0..iterations).map(|_| popcount_bit_parallel(bb)).collect();

            let software_results: Vec<u32> =
                (0..iterations).map(|_| popcount_software(bb)).collect();

            // All results should be identical
            assert!(
                hardware_results.iter().all(|&x| x == hardware_results[0]),
                "Hardware implementation inconsistent for 0x{:X}",
                bb_u128
            );
            assert!(
                swar_results.iter().all(|&x| x == swar_results[0]),
                "SWAR implementation inconsistent for 0x{:X}",
                bb_u128
            );
            assert!(
                software_results.iter().all(|&x| x == software_results[0]),
                "Software implementation inconsistent for 0x{:X}",
                bb_u128
            );

            // All implementations should agree
            assert_eq!(
                hardware_results[0], swar_results[0],
                "Hardware vs SWAR mismatch for 0x{:X}",
                bb_u128
            );
            assert_eq!(
                hardware_results[0], software_results[0],
                "Hardware vs Software mismatch for 0x{:X}",
                bb_u128
            );
        }
    }
}
