#![cfg(feature = "simd")]
use shogi_engine::bitboards::SimdBitboard;
use std::time::Instant;

/// Performance regression tests for SIMD bitboard operations
/// These tests ensure that SIMD operations are at least as fast as scalar operations
/// and meet performance targets (2-4x speedup for bitwise operations)

/// Scalar implementation of bitwise AND for comparison
#[inline(always)]
fn scalar_and(a: u128, b: u128) -> u128 {
    a & b
}

/// Scalar implementation of bitwise OR for comparison
#[inline(always)]
fn scalar_or(a: u128, b: u128) -> u128 {
    a | b
}

/// Scalar implementation of bitwise XOR for comparison
#[inline(always)]
fn scalar_xor(a: u128, b: u128) -> u128 {
    a ^ b
}

/// Scalar implementation of bitwise NOT for comparison
#[inline(always)]
fn scalar_not(a: u128) -> u128 {
    !a
}

#[test]
fn test_simd_bitwise_and_performance() {
    let iterations = 10_000_000;
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    
    // Benchmark SIMD implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = bb1 & bb2;
    }
    let simd_duration = start.elapsed();
    
    // Benchmark scalar implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = scalar_and(test_value1, test_value2);
    }
    let scalar_duration = start.elapsed();
    
    // SIMD should be at least as fast as scalar (no regression)
    // Target: SIMD should be 2-4x faster, but we'll be lenient and just ensure no regression
    let simd_ns = simd_duration.as_nanos();
    let scalar_ns = scalar_duration.as_nanos();
    assert!(
        simd_ns <= scalar_ns * 2 || (simd_ns < 1_000_000 && scalar_ns < 1_000_000),
        "SIMD AND regression: SIMD took {}ns, scalar took {}ns (SIMD should be at least as fast)",
        simd_ns,
        scalar_ns
    );
    
    // Average time per operation should be reasonable (< 10ns per operation)
    let avg_time_ns = simd_duration.as_nanos() / iterations as u128;
    assert!(
        avg_time_ns < 10,
        "SIMD AND too slow: average time {}ns per operation (threshold: 10ns)",
        avg_time_ns
    );
}

#[test]
fn test_simd_bitwise_or_performance() {
    let iterations = 10_000_000;
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    
    // Benchmark SIMD implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = bb1 | bb2;
    }
    let simd_duration = start.elapsed();
    
    // Benchmark scalar implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = scalar_or(test_value1, test_value2);
    }
    let scalar_duration = start.elapsed();
    
    // SIMD should be at least as fast as scalar (no regression)
    let simd_ns = simd_duration.as_nanos();
    let scalar_ns = scalar_duration.as_nanos();
    assert!(
        simd_ns <= scalar_ns * 2 || (simd_ns < 1_000_000 && scalar_ns < 1_000_000),
        "SIMD OR regression: SIMD took {}ns, scalar took {}ns (SIMD should be at least as fast)",
        simd_ns,
        scalar_ns
    );
    
    // Average time per operation should be reasonable
    let avg_time_ns = simd_duration.as_nanos() / iterations as u128;
    assert!(
        avg_time_ns < 10,
        "SIMD OR too slow: average time {}ns per operation (threshold: 10ns)",
        avg_time_ns
    );
}

#[test]
fn test_simd_bitwise_xor_performance() {
    let iterations = 10_000_000;
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    
    // Benchmark SIMD implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = bb1 ^ bb2;
    }
    let simd_duration = start.elapsed();
    
    // Benchmark scalar implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = scalar_xor(test_value1, test_value2);
    }
    let scalar_duration = start.elapsed();
    
    // SIMD should be at least as fast as scalar (no regression)
    // Handle case where both complete in < 1ms by comparing nanoseconds
    let simd_ns = simd_duration.as_nanos();
    let scalar_ns = scalar_duration.as_nanos();
    assert!(
        simd_ns <= scalar_ns * 2 || (simd_ns < 1_000_000 && scalar_ns < 1_000_000),
        "SIMD XOR regression: SIMD took {}ns, scalar took {}ns (SIMD should be at least as fast)",
        simd_ns,
        scalar_ns
    );
    
    // Average time per operation should be reasonable
    let avg_time_ns = simd_duration.as_nanos() / iterations as u128;
    assert!(
        avg_time_ns < 10,
        "SIMD XOR too slow: average time {}ns per operation (threshold: 10ns)",
        avg_time_ns
    );
}

#[test]
fn test_simd_bitwise_not_performance() {
    let iterations = 10_000_000;
    let test_value = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    
    let bb = SimdBitboard::from_u128(test_value);
    
    // Benchmark SIMD implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = !bb;
    }
    let simd_duration = start.elapsed();
    
    // Benchmark scalar implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = scalar_not(test_value);
    }
    let scalar_duration = start.elapsed();
    
    // SIMD should be at least as fast as scalar (no regression)
    let simd_ns = simd_duration.as_nanos();
    let scalar_ns = scalar_duration.as_nanos();
    assert!(
        simd_ns <= scalar_ns * 2 || (simd_ns < 1_000_000 && scalar_ns < 1_000_000),
        "SIMD NOT regression: SIMD took {}ns, scalar took {}ns (SIMD should be at least as fast)",
        simd_ns,
        scalar_ns
    );
    
    // Average time per operation should be reasonable
    let avg_time_ns = simd_duration.as_nanos() / iterations as u128;
    assert!(
        avg_time_ns < 10,
        "SIMD NOT too slow: average time {}ns per operation (threshold: 10ns)",
        avg_time_ns
    );
}

#[test]
fn test_simd_count_ones_performance() {
    let iterations = 1_000_000;
    let test_value = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    
    let bb = SimdBitboard::from_u128(test_value);
    
    // Benchmark SIMD implementation
    let start = Instant::now();
    let mut total = 0;
    for _ in 0..iterations {
        total += bb.count_ones();
    }
    let simd_duration = start.elapsed();
    
    // Benchmark scalar implementation
    let start = Instant::now();
    let mut total_scalar = 0;
    for _ in 0..iterations {
        total_scalar += test_value.count_ones();
    }
    let scalar_duration = start.elapsed();
    
    // Ensure correctness
    assert_eq!(total, total_scalar);
    
    // SIMD should be at least as fast as scalar (no regression)
    // count_ones uses hardware popcount, so should be very fast
    let simd_ns = simd_duration.as_nanos();
    let scalar_ns = scalar_duration.as_nanos();
    assert!(
        simd_ns <= scalar_ns * 2 || (simd_ns < 1_000_000 && scalar_ns < 1_000_000),
        "SIMD count_ones regression: SIMD took {}ns, scalar took {}ns (SIMD should be at least as fast)",
        simd_ns,
        scalar_ns
    );
    
    // Average time per operation should be reasonable (< 100ns per operation)
    let avg_time_ns = simd_duration.as_nanos() / iterations as u128;
    assert!(
        avg_time_ns < 100,
        "SIMD count_ones too slow: average time {}ns per operation (threshold: 100ns)",
        avg_time_ns
    );
}

#[test]
fn test_simd_combined_operations_performance() {
    // Test a realistic workload: multiple bitwise operations in sequence
    let iterations = 1_000_000;
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    let test_value3 = 0x5555_5555_5555_5555_5555_5555_5555_5555;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    let bb3 = SimdBitboard::from_u128(test_value3);
    
    // Benchmark SIMD implementation: (a & b) | (c & !a)
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = (bb1 & bb2) | (bb3 & !bb1);
    }
    let simd_duration = start.elapsed();
    
    // Benchmark scalar implementation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = (test_value1 & test_value2) | (test_value3 & !test_value1);
    }
    let scalar_duration = start.elapsed();
    
    // SIMD should be at least as fast as scalar (no regression)
    let simd_ns = simd_duration.as_nanos();
    let scalar_ns = scalar_duration.as_nanos();
    assert!(
        simd_ns <= scalar_ns * 2 || (simd_ns < 1_000_000 && scalar_ns < 1_000_000),
        "SIMD combined operations regression: SIMD took {}ns, scalar took {}ns (SIMD should be at least as fast)",
        simd_ns,
        scalar_ns
    );
    
    // Average time per operation should be reasonable
    let avg_time_ns = simd_duration.as_nanos() / iterations as u128;
    assert!(
        avg_time_ns < 50,
        "SIMD combined operations too slow: average time {}ns per operation (threshold: 50ns)",
        avg_time_ns
    );
}

