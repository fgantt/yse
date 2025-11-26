#![cfg(feature = "simd")]
/// Tests for memory optimization utilities
use shogi_engine::bitboards::memory_optimization;
use shogi_engine::bitboards::SimdBitboard;

#[test]
fn test_alignment_constants() {
    assert_eq!(memory_optimization::alignment::SSE_NEON_ALIGNMENT, 16);
    assert_eq!(memory_optimization::alignment::AVX2_ALIGNMENT, 32);
    assert_eq!(memory_optimization::alignment::AVX512_CACHE_ALIGNMENT, 64);
}

#[test]
fn test_get_recommended_alignment() {
    let alignment = memory_optimization::alignment::get_recommended_alignment();
    assert!(alignment == 16 || alignment == 32 || alignment == 64);
}

#[test]
fn test_is_simd_aligned() {
    let bb = SimdBitboard::from_u128(0x1234);
    unsafe {
        let ptr = &bb as *const SimdBitboard as *const u8;
        let alignment = memory_optimization::alignment::get_recommended_alignment();
        let is_aligned = memory_optimization::alignment::is_simd_aligned(ptr, alignment);
        // Alignment may vary, but function should not panic
        let _ = is_aligned;
    }
}

#[test]
fn test_prefetch_bitboard() {
    let bb = SimdBitboard::from_u128(0x1234);

    // Should not panic
    memory_optimization::prefetch::prefetch_bitboard(
        &bb,
        memory_optimization::prefetch::PrefetchLevel::L1,
    );
    memory_optimization::prefetch::prefetch_bitboard(
        &bb,
        memory_optimization::prefetch::PrefetchLevel::L2,
    );
    memory_optimization::prefetch::prefetch_bitboard(
        &bb,
        memory_optimization::prefetch::PrefetchLevel::L3,
    );
}

#[test]
fn test_prefetch_range() {
    let bitboards = vec![
        SimdBitboard::from_u128(0x1234),
        SimdBitboard::from_u128(0x5678),
        SimdBitboard::from_u128(0x9ABC),
    ];

    memory_optimization::prefetch::prefetch_range(
        &bitboards,
        0,
        1,
        memory_optimization::prefetch::PrefetchLevel::L1,
    );
}

#[test]
fn test_cache_friendly_soa() {
    use memory_optimization::cache_friendly::BitboardSoA;

    let mut soa = BitboardSoA::<4>::new();
    let bb1 = SimdBitboard::from_u128(0x1234);
    let bb2 = SimdBitboard::from_u128(0x5678);

    soa.set(0, bb1);
    soa.set(1, bb2);

    assert_eq!(soa.get(0).to_u128(), bb1.to_u128());
    assert_eq!(soa.get(1).to_u128(), bb2.to_u128());
}

#[test]
fn test_cache_aligned_array() {
    use memory_optimization::cache_friendly::CacheAlignedBitboardArray;

    let mut array = CacheAlignedBitboardArray::<8>::new();
    let bb = SimdBitboard::from_u128(0x1234);

    array.set(0, bb);
    assert_eq!(array.get(0).to_u128(), bb.to_u128());

    assert_eq!(array.get(10).to_u128(), SimdBitboard::empty().to_u128());
}

#[test]
fn test_telemetry() {
    use memory_optimization::telemetry;

    telemetry::reset_stats();

    telemetry::record_simd_operation();
    telemetry::record_simd_batch_operation();
    telemetry::record_prefetch_operation();

    let stats = telemetry::get_stats();
    assert_eq!(stats.simd_operations, 1);
    assert_eq!(stats.simd_batch_operations, 1);
    assert_eq!(stats.prefetch_operations, 1);

    telemetry::reset_stats();
    let stats_after_reset = telemetry::get_stats();
    assert_eq!(stats_after_reset.simd_operations, 0);
    assert_eq!(stats_after_reset.simd_batch_operations, 0);
    assert_eq!(stats_after_reset.prefetch_operations, 0);
}
