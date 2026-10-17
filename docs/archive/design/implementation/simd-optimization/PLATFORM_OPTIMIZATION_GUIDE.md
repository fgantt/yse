# Platform-Specific SIMD Optimization Guide

This guide documents platform-specific optimization strategies for SIMD operations in the Shogi engine.

## Overview

Different platforms have different SIMD capabilities and optimization strategies. This guide covers:
- x86_64 (AVX2/SSE) optimizations
- ARM64 (NEON) optimizations
- Platform detection and feature selection
- Best practices for each platform

## x86_64 Optimizations

### SIMD Instruction Sets

x86_64 supports multiple SIMD instruction sets:

1. **SSE (128-bit)**: Available on all x86_64 processors
2. **AVX2 (256-bit)**: Available on Haswell and later processors
3. **AVX-512 (512-bit)**: Available on Skylake-X and later processors

### Alignment Requirements

- **SSE**: 16-byte alignment
- **AVX2**: 32-byte alignment
- **AVX-512**: 64-byte alignment

### Prefetching

x86_64 provides prefetch instructions with different hint levels:

```rust
use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0, _MM_HINT_T1, _MM_HINT_T2};

// L1 cache prefetch (most aggressive)
_mm_prefetch(ptr, _MM_HINT_T0);

// L2 cache prefetch (moderate)
_mm_prefetch(ptr, _MM_HINT_T1);

// L3 cache prefetch (least aggressive)
_mm_prefetch(ptr, _MM_HINT_T2);
```

### Best Practices for x86_64

1. **Use AVX2 when available**: 256-bit operations are faster than 128-bit
2. **Align to 32 bytes**: Optimal for AVX2, also works for SSE
3. **Use prefetching**: Prefetch 4-8 elements ahead for sequential access
4. **Avoid AVX-512 frequency scaling**: AVX-512 can reduce CPU frequency on some processors

## ARM64 (NEON) Optimizations

### SIMD Instruction Set

ARM64 provides NEON (128-bit) SIMD instructions:

- Available on all ARM64 processors
- 128-bit vector registers
- Supports integer and floating-point operations

### Alignment Requirements

- **NEON**: 16-byte alignment

### Prefetching

ARM64 provides prefetch instructions with locality hints:

```rust
use std::arch::aarch64::{_prefetch, _PREFETCH_READ, _PREFETCH_LOCALITY0, _PREFETCH_LOCALITY1};

// L1 cache prefetch (high locality)
_prefetch(ptr, _PREFETCH_READ, _PREFETCH_LOCALITY0);

// L2 cache prefetch (low locality)
_prefetch(ptr, _PREFETCH_READ, _PREFETCH_LOCALITY1);
```

### Best Practices for ARM64

1. **Use 16-byte alignment**: Optimal for NEON operations
2. **Leverage NEON intrinsics**: Use platform-specific intrinsics for best performance
3. **Consider cache line size**: 64-byte cache lines on most ARM64 processors
4. **Use prefetching strategically**: ARM64 processors benefit from prefetching

## Platform Detection

### Runtime Detection

Use `PlatformCapabilities` to detect platform features:

```rust
use shogi_engine::bitboards::platform_detection::get_platform_capabilities;

let caps = get_platform_capabilities();
if caps.has_avx2 {
    // Use AVX2 optimizations
} else if caps.has_sse {
    // Use SSE optimizations
}
```

### Feature Selection

Select optimal implementation based on detected features:

```rust
let simd_level = caps.get_simd_level();
match simd_level {
    SimdLevel::AVX512 => {
        // Use AVX-512 optimizations
    }
    SimdLevel::AVX2 => {
        // Use AVX2 optimizations
    }
    SimdLevel::SSE => {
        // Use SSE optimizations
    }
    SimdLevel::NEON => {
        // Use NEON optimizations
    }
    SimdLevel::Scalar => {
        // Fall back to scalar
    }
}
```

## Performance Tuning

### x86_64 Tuning

1. **Enable AVX2**: Use `-C target-feature=+avx2` for AVX2 support
2. **Optimize for target CPU**: Use `-C target-cpu=native` for best performance
3. **Monitor frequency scaling**: AVX-512 can reduce CPU frequency

### ARM64 Tuning

1. **Enable NEON**: NEON is always available on ARM64
2. **Optimize for target CPU**: Use `-C target-cpu=native` for best performance
3. **Consider big.LITTLE**: Some ARM processors have different performance cores

## Memory Optimization

### x86_64 Memory

- **Cache line size**: 64 bytes
- **L1 cache**: 32KB (data), 32KB (instruction)
- **L2 cache**: 256KB - 1MB
- **L3 cache**: 8MB - 32MB

### ARM64 Memory

- **Cache line size**: 64 bytes (varies by processor)
- **L1 cache**: 32KB - 64KB (data), 32KB - 64KB (instruction)
- **L2 cache**: 256KB - 1MB
- **L3 cache**: 2MB - 16MB (if present)

## Benchmarking

### x86_64 Benchmarks

Run benchmarks on x86_64:

```bash
# With AVX2
RUSTFLAGS="-C target-feature=+avx2" cargo bench --features simd

# With SSE only
RUSTFLAGS="-C target-feature=+sse4.2" cargo bench --features simd
```

### ARM64 Benchmarks

Run benchmarks on ARM64:

```bash
# With NEON (always available)
cargo bench --features simd
```

## Troubleshooting

### x86_64 Issues

1. **Unaligned access**: Ensure 32-byte alignment for AVX2
2. **Frequency scaling**: AVX-512 can reduce CPU frequency
3. **Cache misses**: Use prefetching for sequential access

### ARM64 Issues

1. **Unaligned access**: Ensure 16-byte alignment for NEON
2. **Cache misses**: Use prefetching with appropriate locality hints
3. **Big.LITTLE scheduling**: Ensure critical code runs on performance cores

## Best Practices Summary

### General

1. **Detect platform capabilities**: Use runtime detection
2. **Select optimal implementation**: Choose best SIMD level available
3. **Align memory properly**: Match alignment to SIMD width
4. **Use prefetching**: Prefetch data before it's needed
5. **Profile and measure**: Always benchmark on target hardware

### x86_64 Specific

1. **Prefer AVX2 over SSE**: 256-bit operations are faster
2. **Use 32-byte alignment**: Optimal for AVX2
3. **Avoid AVX-512 frequency scaling**: Use AVX-512 carefully
4. **Optimize for target CPU**: Use `target-cpu=native`

### ARM64 Specific

1. **Use 16-byte alignment**: Optimal for NEON
2. **Leverage NEON intrinsics**: Use platform-specific optimizations
3. **Consider cache line size**: 64-byte alignment for cache optimization
4. **Use prefetching strategically**: ARM64 benefits from prefetching

## References

- [Intel AVX2 Programming Reference](https://www.intel.com/content/www/us/en/docs/intrinsics-guide/)
- [ARM NEON Intrinsics Reference](https://developer.arm.com/architectures/instruction-sets/simd-isas/neon)
- [x86-64 Optimization Guide](https://www.agner.org/optimize/)




