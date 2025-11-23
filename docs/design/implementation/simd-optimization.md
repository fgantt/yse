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

1.  **Native `u128` Focus**:
    - We explicitly chose to use Rust's native `u128` type instead of manual SIMD intrinsics (like `__m128i` or `uint64x2_t`) for the baseline implementation.
    - **Reasoning**: `u128` is a first-class citizen in Rust. On 64-bit systems, operations on `u128` are compiled down to a pair of 64-bit register operations (e.g., `AND`, `OR`, `XOR` are done in two instructions). This is extremely fast and avoids the complexity and portability issues of raw intrinsics.
    - **Auto-Vectorization**: In many cases, the compiler's optimizer can merge these operations into SIMD instructions (SSE/AVX on x86, NEON on ARM) automatically if it sees fit.

2.  **Native-Only SIMD Support**:
    - **Decision**: WebAssembly SIMD support has been completely removed to simplify the codebase and focus on maximizing native performance.
    - **Supported Platforms**: x86_64 (AVX2/SSE) and ARM64 (NEON) only.
    - **Build Configuration**: The `simd` feature flag enables explicit SIMD intrinsics for native platforms. When enabled, the implementation uses platform-specific SIMD instructions via `std::arch` intrinsics.
    - **Fallback**: When the `simd` feature is disabled, operations fall back to scalar `u128` operations (which may still benefit from compiler auto-vectorization).

3.  **Platform Detection**:
    - **Location**: `src/bitboards/platform_detection.rs`
    - The engine includes logic to detect CPU features at runtime (e.g., `POPCNT`, `BMI1`, `BMI2` on x86_64).
    - Currently, this is primarily used for selecting the best bit-scanning algorithms (hardware vs. software fallback), but the `SimdBitboard` itself relies on the compiler for instruction selection.

## Performance Characteristics

- **Bitwise Operations (`&`, `|`, `^`, `!`)**:
    - Extremely fast. Typically compiles to 2 instructions on 64-bit CPUs (one for lower 64 bits, one for upper 64 bits).
- **Shifts (`<<`, `>>`)**:
    - Handled by the compiler. `u128` shifts are more complex than 64-bit shifts but are still efficient.
- **Population Count (`count_ones`)**:
    - Uses the native `u128::count_ones()`. On x86_64 with `POPCNT` support, this compiles to two `popcnt` instructions and an add.

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

## Future Optimization Opportunities

While the current implementation provides a solid foundation, further optimizations are planned:

1.  **Explicit AVX2/AVX-512 Intrinsics**:
    - Implement explicit `std::arch::x86_64` intrinsics for bitwise operations
    - Use 256-bit (AVX2) or 512-bit (AVX-512) registers to process multiple bitboards simultaneously

2.  **ARM NEON Intrinsics**:
    - Implement explicit `std::arch::aarch64` NEON intrinsics for ARM64 platforms
    - Optimize bitwise operations using NEON vector instructions

3.  **Batch Operations**:
    - Implement vectorized batch operations for processing arrays of bitboards
    - Use aligned memory layouts for optimal SIMD access patterns

4.  **Prefetching**:
    - Add explicit prefetching hints for large bitboard arrays (magic tables, attack maps)
    - Optimize cache usage for SIMD-friendly memory access patterns
