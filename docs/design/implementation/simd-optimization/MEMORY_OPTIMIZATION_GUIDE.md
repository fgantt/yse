# Memory Optimization Guide for SIMD Operations

This guide documents memory optimization strategies and best practices for SIMD operations in the Shogi engine.

## Overview

Memory optimization is crucial for achieving maximum SIMD performance. This guide covers:
- Memory alignment strategies
- Prefetching techniques
- Cache-friendly data structures
- Platform-specific optimizations

## Memory Alignment

### Alignment Requirements

Different SIMD instruction sets have different alignment requirements:

- **SSE/NEON (128-bit)**: 16-byte alignment
- **AVX2 (256-bit)**: 32-byte alignment
- **AVX-512 (512-bit)**: 64-byte alignment (also cache line size)

### Implementation

The `memory_optimization::alignment` module provides utilities for alignment:

```rust
use shogi_engine::bitboards::memory_optimization::alignment;

// Get recommended alignment based on platform
let alignment = alignment::get_recommended_alignment();

// Check if a pointer is aligned
unsafe {
    let is_aligned = alignment::is_simd_aligned(ptr);
}
```

### Best Practices

1. **Use `#[repr(align(N))]` for structs**: Ensures proper alignment at compile time
2. **Use aligned allocators**: For dynamic allocations, use aligned allocators
3. **Verify alignment at runtime**: Check alignment before using SIMD operations

## Prefetching Strategies

### Prefetch Levels

Prefetching can target different cache levels:

- **L1 Cache**: Most aggressive, for data needed immediately
- **L2 Cache**: Moderate, for data needed soon
- **L3 Cache**: Least aggressive, for data needed later

### Implementation

The `memory_optimization::prefetch` module provides prefetching utilities:

```rust
use shogi_engine::bitboards::memory_optimization::prefetch::{prefetch_bitboard, PrefetchLevel};

// Prefetch a single bitboard
prefetch_bitboard(&bitboard, PrefetchLevel::L1);

// Prefetch a range of bitboards
prefetch::prefetch_range(&bitboards, current_index, 8, PrefetchLevel::L2);
```

### Best Practices

1. **Prefetch distance**: Prefetch 4-8 elements ahead for sequential access
2. **Prefetch level**: Use L1 for immediate access, L2/L3 for future access
3. **Avoid over-prefetching**: Don't prefetch data that won't be used

## Cache-Friendly Data Structures

### Structure of Arrays (SoA) Layout

For SIMD operations, Structure of Arrays (SoA) can be more efficient than Array of Structures (AoS):

```rust
use shogi_engine::bitboards::memory_optimization::cache_friendly::BitboardSoA;

// SoA layout groups similar data together
let mut soa = BitboardSoA::<16>::new();
soa.set(0, bitboard1);
soa.set(1, bitboard2);
```

### Cache-Aligned Arrays

Use cache-aligned arrays for better cache line utilization:

```rust
use shogi_engine::bitboards::memory_optimization::cache_friendly::CacheAlignedBitboardArray;

let mut array = CacheAlignedBitboardArray::<32>::new();
array.set(0, bitboard);
```

### Best Practices

1. **Use SoA for SIMD operations**: Better cache locality for vectorized operations
2. **Align to cache lines**: 64-byte alignment for cache line optimization
3. **Group related data**: Keep data that's accessed together close in memory

## Platform-Specific Optimizations

### x86_64 (AVX2/SSE)

- **Alignment**: 32-byte for AVX2, 16-byte for SSE
- **Prefetching**: Use `_mm_prefetch` with appropriate hint levels
- **Cache line size**: 64 bytes

### ARM64 (NEON)

- **Alignment**: 16-byte for NEON
- **Prefetching**: Use `_prefetch` with locality hints
- **Cache line size**: 64 bytes (varies by processor)

### Best Practices

1. **Detect platform capabilities**: Use `PlatformCapabilities` to select optimal implementation
2. **Use platform-specific intrinsics**: Leverage platform-specific optimizations
3. **Test on target platforms**: Validate performance on actual hardware

## Memory Access Patterns

### Sequential Access

For sequential access patterns:
- Use prefetching with appropriate distance
- Ensure data is aligned
- Use streaming stores for write-only data

### Random Access

For random access patterns:
- Use cache-friendly data structures
- Implement hash-based organization
- Prefetch likely access patterns

## Performance Monitoring

### Telemetry

The `memory_optimization::telemetry` module provides performance monitoring:

```rust
use shogi_engine::bitboards::memory_optimization::telemetry;

// Record operations
telemetry::record_simd_operation();
telemetry::record_simd_batch_operation();
telemetry::record_prefetch_operation();

// Get statistics
let stats = telemetry::get_stats();
println!("SIMD operations: {}", stats.simd_operations);
```

### Best Practices

1. **Monitor SIMD usage**: Track how often SIMD operations are used
2. **Measure prefetch effectiveness**: Monitor cache hit rates
3. **Profile memory access**: Use profiling tools to identify bottlenecks

## Integration Examples

### Batch Operations with Prefetching

```rust
use shogi_engine::bitboards::memory_optimization::prefetch::{prefetch_range, PrefetchLevel};
use shogi_engine::bitboards::batch_ops::AlignedBitboardArray;

fn process_batch_with_prefetch<const N: usize>(
    arrays: &[AlignedBitboardArray<N>],
) {
    for i in 0..arrays.len() {
        // Prefetch next batch
        if i + 1 < arrays.len() {
            prefetch_range(arrays[i].as_slice(), 0, 4, PrefetchLevel::L2);
        }
        
        // Process current batch
        // ... SIMD operations ...
    }
}
```

### Cache-Friendly Attack Pattern Storage

```rust
use shogi_engine::bitboards::memory_optimization::cache_friendly::CacheAlignedBitboardArray;

// Store attack patterns in cache-aligned arrays
let mut attack_patterns = CacheAlignedBitboardArray::<81>::new();
for square in 0..81 {
    attack_patterns.set(square, generate_attack_pattern(square));
}
```

## Performance Targets

- **Memory alignment**: 100% of SIMD operations use properly aligned data
- **Prefetch effectiveness**: 80%+ cache hit rate for prefetched data
- **Cache utilization**: 90%+ cache line utilization for aligned structures

## Troubleshooting

### Common Issues

1. **Unaligned access**: Use alignment checks and aligned allocators
2. **Cache misses**: Increase prefetch distance or adjust prefetch level
3. **Memory bandwidth**: Use SoA layout for better cache locality

### Debugging Tools

- Use `perf` on Linux to analyze cache performance
- Use `Instruments` on macOS to profile memory access
- Use alignment verification functions to check alignment

## References

- [Intel Optimization Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
- [ARM NEON Programming Guide](https://developer.arm.com/documentation/102467/0100/)
- [Cache-Friendly Data Structures](https://en.wikipedia.org/wiki/AoS_and_SoA)

