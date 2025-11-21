# SIMD Implementation & Optimization

## Overview
The Shogi Engine uses a 128-bit bitboard representation (`SimdBitboard`) to efficiently represent the 9x9 Shogi board (81 squares). The implementation is designed to be high-performance on native 64-bit architectures (x86_64, AArch64) by leveraging the `u128` primitive, which modern compilers (LLVM) can often auto-vectorize or map to efficient register operations.

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

2.  **Removal of WASM Support**:
    - Previously, the implementation contained conditional compilation for `wasm32` using `v128` intrinsics.
    - **Decision**: WASM support was dropped to simplify the codebase and focus on maximizing native performance. The `SimdBitboard` is now uniform across all supported targets.

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

## Future Optimization Opportunities

While the current `u128` implementation is efficient, further optimizations are possible:

1.  **Explicit Intrinsics (AVX2/AVX-512)**:
    - For operations that don't auto-vectorize well, we could introduce explicit `std::arch` intrinsics.
    - Example: Parallel bitboard updates or complex pattern matching might benefit from 256-bit (AVX2) registers processing two bitboards at once.

2.  **ARM NEON**:
    - Explicit usage of NEON intrinsics could potentially speed up specific operations if the compiler's auto-vectorization is suboptimal.

3.  **Prefetching**:
    - The `SimdBitboard` is small (16 bytes), fitting easily into cache lines. Explicit prefetching (using `core::arch::x86_64::_mm_prefetch`) is likely unnecessary for individual bitboards but could be beneficial for arrays of bitboards (e.g., magic tables).
