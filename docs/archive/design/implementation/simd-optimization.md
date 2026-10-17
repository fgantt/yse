# SIMD Implementation & Optimization

## Overview
The Shogi Engine uses a 128-bit bitboard representation (`SimdBitboard`) to efficiently represent the 9x9 Shogi board (81 squares). The implementation is designed to be high-performance on **native 64-bit architectures only** (x86_64 with AVX2/SSE, ARM64 with NEON) by leveraging the `u128` primitive and explicit SIMD intrinsics.

**Note**: WebAssembly SIMD support has been removed. This implementation focuses exclusively on native platform performance.

## Current Architecture

### Core Type: `SimdBitboard`
- **Location**: `src/bitboards/simd.rs`
- **Representation**: A transparent wrapper around a single `u128` value.
- **Alignment**: `repr(transparent)` ensures it has the same ABI and memory layout as `u128`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SimdBitboard {
    data: u128,
}
```

### Design Decisions

1.  **Explicit SIMD Intrinsics vs Compiler Auto-Vectorization**:
    - **Decision**: Use explicit SIMD intrinsics via `std::arch` when the `simd` feature is enabled.
    - **Reasoning**: While compiler auto-vectorization can work for simple cases, explicit intrinsics provide:
      - Guaranteed SIMD instruction generation
      - Better control over which SIMD instructions are used
      - Predictable performance characteristics
      - Platform-specific optimizations (SSE, AVX2, AVX-512 for x86_64; NEON for ARM64)
    - **Implementation**: When `simd` feature is enabled, bitwise operations use `std::arch::x86_64` (SSE/AVX2) or `std::arch::aarch64` (NEON) intrinsics.
    - **Fallback**: When `simd` feature is disabled, operations use scalar `u128` operations (which may still benefit from compiler auto-vectorization, but without guarantees).

2.  **Native-Only SIMD Support**:
    - **Decision**: WebAssembly SIMD support has been completely removed to simplify the codebase and focus on maximizing native performance.
    - **Supported Platforms**: 
      - **x86_64**: SSE (baseline), AVX2 (when available), AVX-512 (when available)
      - **ARM64**: NEON (always available on aarch64)
    - **Build Configuration**: The `simd` feature flag enables explicit SIMD intrinsics for native platforms only.
    - **Platform Detection**: Runtime CPU feature detection (AVX2, AVX-512) is used to inform optimal implementation selection, though actual SIMD selection is compile-time.
    - **No WebAssembly**: WebAssembly SIMD (SIMD128) is not supported. The implementation focuses exclusively on native platform performance.

3.  **Platform Detection**:
    - **Location**: `src/bitboards/platform_detection.rs`
    - The engine includes logic to detect CPU features at runtime (e.g., `POPCNT`, `BMI1`, `BMI2` on x86_64).
    - Currently, this is primarily used for selecting the best bit-scanning algorithms (hardware vs. software fallback), but the `SimdBitboard` itself relies on the compiler for instruction selection.

## Performance Characteristics

### With SIMD Feature Enabled

- **Bitwise Operations (`&`, `|`, `^`, `!`)**:
    - Use explicit SIMD intrinsics (`_mm_and_si128`, `_mm_or_si128`, `_mm_xor_si128` on x86_64; `vandq_u8`, `vorrq_u8`, `veorq_u8` on ARM64)
    - Target: 2-4x speedup vs scalar implementation
    - Single SIMD instruction per operation (128-bit operations)
- **Shifts (`<<`, `>>`)**:
    - Currently use scalar `u128` shifts (SIMD shifts can be added if performance analysis shows benefit)
    - Handled efficiently by the compiler
- **Population Count (`count_ones`)**:
    - Uses hardware `POPCNT` instruction when available (via `u128::count_ones()`)
    - On x86_64 with `POPCNT` support, compiles to hardware instruction
    - On ARM64, uses efficient hardware popcount when available

### Batch Operations

- **Batch AND/OR/XOR**:
    - Use SIMD intrinsics to process multiple bitboards
    - Target: 4-8x speedup vs scalar loops
    - Includes prefetching hints for cache optimization
    - Memory-aligned arrays (32-byte alignment) for optimal performance

### Without SIMD Feature

- Operations fall back to scalar `u128` implementation
- May benefit from compiler auto-vectorization, but without guarantees
- Performance is still good, but not optimized for SIMD

## Performance Targets

When the `simd` feature is enabled with explicit SIMD intrinsics:

- **Bitwise Operations**: Target 2-4x speedup vs scalar implementation
- **Batch Operations**: Target 4-8x speedup for processing multiple bitboards simultaneously
- **Overall Engine Performance**: Target 20%+ improvement in nodes per second (NPS)

## Performance Regression Testing

Performance regression tests are located in `tests/simd_performance_regression_tests.rs`. These tests ensure that:

1. SIMD operations are at least as fast as scalar operations (no performance regression)
2. Operations meet minimum performance thresholds
3. Correctness is maintained alongside performance

Run performance regression tests with:
```bash
cargo test --features simd simd_performance_regression_tests
```

## CI Integration

To prevent performance regressions in CI, add a benchmark check step:

```bash
# Run performance regression tests
cargo test --features simd simd_performance_regression_tests --release

# If benchmarks are available, compare against baseline
cargo bench --features simd --bench simd_performance_benchmarks
```

## Implementation Status

### Completed Features

1.  **Explicit SIMD Intrinsics**:
    - ✅ Implemented explicit `std::arch::x86_64` intrinsics for bitwise operations (SSE)
    - ✅ Implemented explicit `std::arch::aarch64` NEON intrinsics for ARM64 platforms
    - ✅ Platform-specific conditional compilation for optimal instruction selection

2.  **Platform Detection**:
    - ✅ Runtime CPU feature detection (AVX2, AVX-512, NEON)
    - ✅ Platform capabilities reporting
    - ✅ SIMD level detection and reporting

3.  **Batch Operations**:
    - ✅ Implemented vectorized batch operations (`AlignedBitboardArray`)
    - ✅ Memory-aligned arrays (32-byte alignment) for optimal SIMD access
    - ✅ Prefetching hints for cache optimization
    - ✅ SIMD-optimized batch AND/OR/XOR operations

4.  **Performance Validation**:
    - ✅ Comprehensive benchmark suite
    - ✅ Performance regression tests
    - ✅ NPS validation tests
    - ✅ CI integration for performance checks

## Future Optimization Opportunities

While the current implementation provides a solid foundation, further optimizations are possible:

1.  **AVX2/AVX-512 Optimizations**:
    - Process 2 bitboards simultaneously with AVX2 (256-bit registers)
    - Process 4 bitboards simultaneously with AVX-512 (512-bit registers)
    - Runtime selection based on CPU feature detection

2.  **Additional Operations**:
    - SIMD-optimized shift operations (if performance analysis shows benefit)
    - SIMD-optimized `combine_all()` for batch operations

3.  **Integration**:
    - Integrate batch operations into actual move generation code paths
    - Use batch operations in attack pattern combination
    - Optimize critical paths with batch processing
