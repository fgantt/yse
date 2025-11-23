# Tasks: Full SIMD Implementation

Based on `SIMD_IMPLEMENTATION_EVALUATION.md` - implementing proper SIMD instructions to replace the current scalar `u128` wrapper and achieve 2-4x performance improvements.

## Relevant Files

- `Cargo.toml` - Ensure SIMD feature is properly configured for native platforms (x86_64, ARM64)
- `src/bitboards/simd.rs` - Core SIMD implementation that needs actual SIMD instructions (currently just u128 wrapper)
- `src/bitboards/simd.rs` - Unit tests for SIMD operations with validation that SIMD instructions are used
- `src/bitboards/platform_detection.rs` - Extend platform detection to include AVX2, AVX-512, and ARM NEON
- `src/bitboards/batch_ops.rs` - New file for AlignedBitboardArray and batch operations (doesn't exist yet)
- `src/bitboards/batch_ops.rs` - Unit tests for batch operations
- `benches/simd_performance_benchmarks.rs` - Performance benchmarks comparing SIMD vs scalar (update existing)
- `benches/simd_instruction_validation.rs` - New benchmarks to verify SIMD instructions are actually used
- `tests/simd_tests.rs` - Update existing tests to validate SIMD instruction usage
- `docs/design/implementation/simd-optimization/SIMD_IMPLEMENTATION_EVALUATION.md` - Reference evaluation report
- `README.md` - Update with SIMD feature documentation
- `.cargo/config.toml` - Remove WebAssembly-specific SIMD flags, keep native platform optimizations

### Notes

- Unit tests should be placed alongside the code files they are testing (e.g., `simd.rs` and `simd_tests.rs` in the same directory).
- Use `cargo test` to run tests. Use `cargo bench` to run performance benchmarks.
- SIMD implementation must use explicit intrinsics (`std::arch` or `std::simd`) - compiler auto-vectorization is not sufficient.
- Focus on native platforms only: x86_64 (AVX2/SSE) and ARM64 (NEON). WebAssembly support is dropped.
- All SIMD operations must be benchmarked to ensure they outperform scalar operations.
- Platform detection should be used to select optimal SIMD implementation per architecture.
- Target: 2-4x speedup for bitwise operations, 4-8x for batch operations, 20%+ overall NPS improvement.

## Tasks

- [ ] 1.0 Setup and Configuration for Full SIMD Implementation
  - [ ] 1.1 Ensure `simd` feature is properly configured in `Cargo.toml` for native platforms
  - [ ] 1.2 Remove all WebAssembly-specific SIMD configuration from `.cargo/config.toml`
  - [ ] 1.3 Remove WebAssembly SIMD dependencies and conditional compilation from codebase
  - [ ] 1.4 Update build configuration to target native SIMD features (AVX2 for x86_64, NEON for ARM64)
  - [ ] 1.5 Add CI benchmark check to prevent performance regressions (fail if SIMD is slower than scalar)
  - [ ] 1.6 Create performance regression test that fails if SIMD operations are slower than scalar baseline
  - [ ] 1.7 Update documentation to reflect native-only SIMD support (no WebAssembly)

- [ ] 2.0 Implement Real SIMD Operations for Core Bitwise Operations
  - [ ] 2.1 Research and choose SIMD approach: `std::simd` (Rust 1.75+) vs `std::arch` platform-specific intrinsics
  - [ ] 2.2 Implement x86_64 SIMD operations using `std::arch::x86_64` intrinsics (AVX2/SSE) for BitAnd, BitOr, BitXor, Not
  - [ ] 2.3 Implement ARM64 SIMD operations using `std::arch::aarch64` NEON intrinsics for bitwise operations
  - [ ] 2.4 Add conditional compilation to select optimal SIMD implementation (AVX2 > SSE for x86_64, NEON for ARM64)
  - [ ] 2.5 Implement SIMD-optimized shift operations (Shl, Shr) using platform intrinsics
  - [ ] 2.6 Ensure all bitwise operations use actual SIMD instructions, not scalar fallbacks
  - [ ] 2.7 Update `count_ones()` to use existing hardware popcount (already works, verify integration)
  - [ ] 2.8 Add benchmarks to verify SIMD instructions are actually generated (use objdump/llvm-objdump)
  - [ ] 2.9 Validate performance: target 2-4x speedup for bitwise operations vs current scalar implementation

- [ ] 3.0 Extend Platform Detection for Advanced SIMD Features
  - [ ] 3.1 Add AVX2 detection to `src/bitboards/platform_detection.rs` using CPUID
  - [ ] 3.2 Add AVX-512 detection to platform detection (with proper feature flags)
  - [ ] 3.3 Add ARM NEON detection for aarch64 targets
  - [ ] 3.4 Add runtime feature selection logic to choose optimal SIMD implementation
  - [ ] 3.5 Update `PlatformCapabilities` struct to include AVX2, AVX-512, and NEON flags
  - [ ] 3.6 Add platform detection tests for all supported architectures
  - [ ] 3.7 Integrate platform detection into SIMD operation selection

- [ ] 4.0 Implement Batch Operations with Vectorization
  - [ ] 4.1 Create `src/bitboards/batch_ops.rs` with `AlignedBitboardArray<const N: usize>` struct
  - [ ] 4.2 Implement memory alignment (16-byte or 32-byte aligned) for SIMD-friendly access
  - [ ] 4.3 Implement `batch_and()` using SIMD to process multiple bitboards simultaneously (target: 4+ at once)
  - [ ] 4.4 Implement `batch_or()` with SIMD vectorization
  - [ ] 4.5 Implement `batch_xor()` with SIMD vectorization
  - [ ] 4.6 Add prefetching hints for large batch operations to improve cache performance
  - [ ] 4.7 Create comprehensive unit tests for batch operations with various array sizes
  - [ ] 4.8 Add benchmarks for batch operations targeting 4-8x speedup vs scalar loops
  - [ ] 4.9 Integrate batch operations into critical paths (move generation, attack calculation)

- [ ] 5.0 Performance Validation and Benchmarking
  - [ ] 5.1 Update `benches/simd_performance_benchmarks.rs` with comprehensive operation coverage
  - [ ] 5.2 Create `benches/simd_instruction_validation.rs` to verify SIMD instructions are generated
  - [ ] 5.3 Add benchmark comparison: SIMD vs scalar for all bitwise operations
  - [ ] 5.4 Add benchmark for batch operations comparing vectorized vs scalar loops
  - [ ] 5.5 Set up CI integration to run benchmarks and fail on performance regressions
  - [ ] 5.6 Document target performance metrics: 2-4x speedup for bitwise ops, 4-8x for batch ops
  - [ ] 5.7 Create performance regression test suite that must pass before merging SIMD changes
  - [ ] 5.8 Validate that SIMD implementation achieves at least 20% overall NPS improvement

- [ ] 6.0 Update Tests and Documentation
  - [ ] 6.1 Update `tests/simd_tests.rs` to validate SIMD instructions are used (not just correctness)
  - [ ] 6.2 Add tests that verify platform detection works correctly on x86_64 and ARM64 architectures
  - [ ] 6.3 Add integration tests for batch operations in real-world scenarios
  - [ ] 6.4 Update `docs/design/implementation/simd-optimization.md` to reflect actual implementation
  - [ ] 6.5 Remove or update aspirational documentation that describes non-existent features (especially WebAssembly)
  - [ ] 6.6 Document the decision to use explicit SIMD intrinsics vs compiler auto-vectorization
  - [ ] 6.7 Document native-only SIMD support (x86_64 and ARM64, no WebAssembly)
  - [ ] 6.8 Update API documentation with performance characteristics and platform requirements (native platforms only)

- [ ] 7.0 Algorithm Vectorization (Long-term Optimization)
  - [ ] 7.1 Analyze move generation code to identify vectorization opportunities
  - [ ] 7.2 Implement vectorized attack generation for sliding pieces (rooks, bishops, lances)
  - [ ] 7.3 Add parallel attack calculation for multiple pieces using batch operations
  - [ ] 7.4 Implement SIMD-based pattern matching for tactical patterns
  - [ ] 7.5 Vectorize evaluation functions where beneficial (material counting, piece-square tables)
  - [ ] 7.6 Benchmark algorithm vectorization improvements
  - [ ] 7.7 Integrate vectorized algorithms into search and evaluation paths
  - [ ] 7.8 Validate overall engine performance improvement (target: 20%+ NPS improvement)

- [ ] 8.0 Memory Optimization and Advanced Features
  - [ ] 8.1 Optimize memory layouts for SIMD access patterns (Structure of Arrays where beneficial)
  - [ ] 8.2 Add prefetching strategies for large bitboard arrays (magic tables, attack maps)
  - [ ] 8.3 Implement cache-friendly data structures for SIMD operations
  - [ ] 8.4 Optimize memory alignment throughout codebase for SIMD operations (16-byte for SSE/NEON, 32-byte for AVX2)
  - [ ] 8.5 Profile memory access patterns and optimize cache usage
  - [ ] 8.6 Add telemetry/monitoring for SIMD performance in production
  - [ ] 8.7 Create performance tuning guide based on platform-specific optimizations (x86_64 AVX2, ARM64 NEON)
  - [ ] 8.8 Document platform-specific optimization strategies and best practices
