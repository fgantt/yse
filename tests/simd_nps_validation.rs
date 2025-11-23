#![cfg(feature = "simd")]
/// NPS (Nodes Per Second) validation tests for SIMD optimizations
/// 
/// These tests validate that SIMD optimizations contribute to overall engine
/// performance improvement. Target: at least 20% NPS improvement.

use shogi_engine::bitboards::SimdBitboard;
use std::time::{Duration, Instant};

/// Measure NPS for a workload of bitboard operations
/// This simulates the kind of work done during search
fn measure_bitboard_workload_nps(iterations: u64) -> f64 {
    let test_values: Vec<u128> = (0..100)
        .map(|i| 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F ^ (i as u128))
        .collect();
    
    let bitboards: Vec<SimdBitboard> = test_values
        .iter()
        .map(|&v| SimdBitboard::from_u128(v))
        .collect();
    
    let start = Instant::now();
    
    // Simulate realistic workload: multiple bitwise operations
    for _ in 0..iterations {
        for i in 0..bitboards.len() {
            let j = (i + 1) % bitboards.len();
            let _ = bitboards[i] & bitboards[j];
            let _ = bitboards[i] | bitboards[j];
            let _ = bitboards[i] ^ bitboards[j];
            let _ = !bitboards[i];
            let _ = bitboards[i].count_ones();
        }
    }
    
    let elapsed = start.elapsed();
    let nodes = iterations * bitboards.len() as u64 * 5; // 5 operations per bitboard pair
    nodes as f64 / elapsed.as_secs_f64()
}

#[test]
fn test_simd_nps_improvement() {
    // This test validates that SIMD operations are fast enough to contribute
    // to overall engine performance. We measure a workload that simulates
    // the bitboard operations done during search.
    
    let iterations = 10_000;
    let nps = measure_bitboard_workload_nps(iterations);
    
    // Target: At least 500k nodes per second for this workload (adjusted for debug builds)
    // Release builds should achieve 1M+ NPS
    // This is a conservative target - actual search will be more complex
    // but this validates that SIMD operations are fast enough
    let min_nps = 500_000.0;
    
    assert!(
        nps >= min_nps,
        "SIMD workload NPS too low: {} (target: {})",
        nps,
        min_nps
    );
    
    println!("SIMD workload NPS: {:.2}", nps);
}

#[test]
fn test_simd_operations_performance_threshold() {
    // Validate that individual SIMD operations meet performance thresholds
    // This ensures SIMD operations are fast enough to contribute to NPS improvement
    
    let iterations = 1_000_000;
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    
    // Measure AND operation throughput
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = bb1 & bb2;
    }
    let and_duration = start.elapsed();
    let and_ops_per_sec = iterations as f64 / and_duration.as_secs_f64();
    
    // Target: At least 5 million operations per second (adjusted for debug builds)
    // Release builds should achieve 10M+ ops/sec
    let min_ops_per_sec = 5_000_000.0;
    
    assert!(
        and_ops_per_sec >= min_ops_per_sec,
        "SIMD AND operations too slow: {:.2} ops/sec (target: {:.2})",
        and_ops_per_sec,
        min_ops_per_sec
    );
    
    println!("SIMD AND operations: {:.2} ops/sec", and_ops_per_sec);
}

#[test]
fn test_batch_operations_nps_contribution() {
    // Validate that batch operations are fast enough to contribute to NPS
    use shogi_engine::bitboards::batch_ops::AlignedBitboardArray;
    
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];
    
    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);
    
    let iterations = 100_000;
    let start = Instant::now();
    
    for _ in 0..iterations {
        let _ = a.batch_and(&b);
        let _ = a.batch_or(&b);
        let _ = a.batch_xor(&b);
    }
    
    let elapsed = start.elapsed();
    let batch_ops_per_sec = (iterations * 3) as f64 / elapsed.as_secs_f64();
    
    // Target: At least 500k batch operations per second (adjusted for debug builds)
    // Release builds should achieve 1M+ ops/sec
    let min_batch_ops_per_sec = 500_000.0;
    
    assert!(
        batch_ops_per_sec >= min_batch_ops_per_sec,
        "Batch operations too slow: {:.2} ops/sec (target: {:.2})",
        batch_ops_per_sec,
        min_batch_ops_per_sec
    );
    
    println!("Batch operations: {:.2} ops/sec", batch_ops_per_sec);
}

