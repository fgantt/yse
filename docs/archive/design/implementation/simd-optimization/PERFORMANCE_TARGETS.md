# SIMD Performance Targets and Metrics

## Overview

This document defines the performance targets and validation criteria for SIMD optimizations in the Shogi engine.

## Performance Targets

### Bitwise Operations (2-4x speedup target)

**Target**: SIMD bitwise operations should achieve 2-4x speedup over scalar implementations.

**Operations**:
- `BitAnd` (AND): Target 2-4x speedup
- `BitOr` (OR): Target 2-4x speedup
- `BitXor` (XOR): Target 2-4x speedup
- `Not`: Target 2-4x speedup

**Measurement**: Compare `SimdBitboard` operations vs scalar `u128` operations using Criterion benchmarks.

**Validation**: Run `cargo bench --bench simd_performance_benchmarks --features simd` and verify speedup ratios.

### Batch Operations (4-8x speedup target)

**Target**: Batch operations should achieve 4-8x speedup over scalar loops.

**Operations**:
- `batch_and`: Target 4-8x speedup
- `batch_or`: Target 4-8x speedup
- `batch_xor`: Target 4-8x speedup

**Measurement**: Compare `AlignedBitboardArray` batch operations vs scalar loops processing the same number of bitboards.

**Validation**: Run `cargo bench --bench batch_ops_benchmarks --features simd` and verify speedup ratios.

### Population Count (Hardware Acceleration)

**Target**: Use hardware POPCNT instruction when available.

**Measurement**: Compare `SimdBitboard::count_ones()` vs scalar `u128::count_ones()`.

**Expected**: Hardware popcount should be at least as fast as scalar, often faster on modern CPUs.

### Overall Engine Performance (20% NPS improvement target)

**Target**: SIMD optimizations should contribute to at least 20% improvement in nodes per second (NPS) during search.

**Measurement**: 
- Run search benchmarks with SIMD enabled vs disabled
- Measure NPS (nodes per second) for various positions
- Calculate improvement percentage

**Validation**: 
- Run `cargo test --features simd simd_nps_validation --release` for validation tests
- Run `cargo bench --bench simd_nps_benchmarks --features simd` for detailed benchmarks
- Tests require 20%+ improvement in release builds
- Debug builds allow up to 50% regression (expected due to function call overhead)

**NPS Validation Tests**: `tests/simd_nps_validation.rs`
- `test_simd_nps_improvement_end_to_end`: End-to-end search comparison requiring 20%+ improvement
- `test_simd_nps_regression_detection`: Ensures no significant performance regression
- `test_simd_realistic_workload_simulation`: Realistic workload with multiple positions

**NPS Benchmarks**: `benches/simd_nps_benchmarks.rs`
- `bench_simd_vs_scalar_nps_starting_position`: Starting position NPS comparison
- `bench_simd_vs_scalar_nps_different_depths`: NPS at different search depths
- `bench_simd_vs_scalar_nps_realistic_workload`: Realistic multi-position workload

**Methodology** (Task 5.12.5):
1. Create SearchEngine with SIMD enabled/disabled via `EngineConfig`
2. Run searches at consistent depths (typically 3-4) for fair comparison
3. Measure total nodes searched and elapsed time
4. Calculate NPS = nodes / time_seconds
5. Calculate improvement = ((NPS_simd - NPS_scalar) / NPS_scalar) * 100%
6. Validate improvement >= 20% in release builds

**Results Interpretation**:
- **Release builds**: Require 20%+ improvement (strict requirement)
- **Debug builds**: Allow up to 50% regression (acceptable due to function call overhead)
- **Regression threshold**: Maximum 5% regression allowed in release builds

## Performance Regression Thresholds

### Regression Detection

Performance regressions are detected when:
- Any SIMD operation is slower than scalar implementation
- Batch operations show less than 2x speedup (below minimum target)
- Overall NPS decreases by more than 5% compared to baseline (Task 5.12.4)
- NPS improvement is less than 20% in release builds (Task 5.12.2)

### Regression Test Suite

Location: `tests/simd_performance_regression_tests.rs`

**Tests**:
- `test_simd_bitwise_and_performance`: Ensures SIMD AND is at least as fast as scalar
- `test_simd_bitwise_or_performance`: Ensures SIMD OR is at least as fast as scalar
- `test_simd_bitwise_xor_performance`: Ensures SIMD XOR is at least as fast as scalar
- `test_simd_bitwise_not_performance`: Ensures SIMD NOT is at least as fast as scalar
- `test_simd_count_ones_performance`: Ensures count_ones meets performance threshold
- `test_simd_combined_operations_performance`: Ensures combined operations are efficient

**Running**: `cargo test --features simd simd_performance_regression_tests --release`

**Note**: Performance regression tests must be run in release mode (`--release` flag) for accurate results. Debug builds are slower and may fail performance thresholds.

## Benchmark Suite

### Bitwise Operations Benchmarks

Location: `benches/simd_performance_benchmarks.rs`

**Benchmark Groups**:
- `bitwise_operations`: AND, OR, XOR, NOT (SIMD vs scalar)
- `count_ones`: Population count comparison
- `combined_operations`: Complex bitwise expression evaluation
- `trailing_zeros`: Trailing zero count
- `leading_zeros`: Leading zero count
- `is_empty`: Empty check performance
- `batch_operations_size_N`: Batch operations for various sizes (4, 8, 16)

**Running**: `cargo bench --bench simd_performance_benchmarks --features simd`

### Batch Operations Benchmarks

Location: `benches/batch_ops_benchmarks.rs`

**Benchmark Groups**:
- `batch_and`: Batch AND operations (SIMD vs scalar)
- `batch_or`: Batch OR operations (SIMD vs scalar)
- `batch_xor`: Batch XOR operations (SIMD vs scalar)
- `batch_various_sizes`: Performance across different array sizes

**Running**: `cargo bench --bench batch_ops_benchmarks --features simd`

### NPS (Nodes Per Second) Benchmarks

Location: `benches/simd_nps_benchmarks.rs` (Task 5.12.1)

**Benchmark Groups**:
- `NPS Starting Position`: SIMD vs scalar NPS for starting position
- `NPS Different Depths`: NPS comparison at depths 2, 3, 4
- `NPS Realistic Workload`: Multi-position workload simulation

**Running**: `cargo bench --bench simd_nps_benchmarks --features simd`

**Purpose**: Measure overall engine performance improvement from SIMD optimizations in realistic search scenarios.

### Instruction Validation Benchmarks

Location: `benches/simd_instruction_validation.rs`

**Purpose**: Verify that SIMD instructions are actually generated by the compiler.

**Validation Method**: 
1. Build with: `cargo build --release --features simd --bench simd_instruction_validation`
2. Disassemble: `objdump -d target/release/deps/simd_instruction_validation-* | grep -E "(pand|por|pxor|andnot|vand|vorr|veor)"`
3. Verify presence of SIMD instructions

**Expected Instructions**:
- x86_64: `pand`, `por`, `pxor`, `pandn` (SSE) or `vpand`, `vpor`, `vpxor`, `vpandn` (AVX2)
- ARM64: `vand`, `vorr`, `veor` (NEON)

## CI Integration

### Performance Regression Checks

Location: `.github/workflows/simd-performance-check.yml`

**Triggers**:
- Pull requests modifying SIMD-related code
- Pushes to main branch

**Actions**:
1. Run performance regression tests
2. Verify SIMD feature compiles
3. Fail build if regressions detected

**Running Locally**: 
```bash
cargo test --features simd simd_performance_regression_tests --release
```

## Performance Monitoring

### Benchmark Results

Benchmark results are stored in `target/criterion/` after running benchmarks.

**Viewing Results**: 
- HTML reports: `target/criterion/*/report/index.html`
- Compare runs: `cargo bench --bench simd_performance_benchmarks --features simd -- --save-baseline baseline`

### Performance Trends

Track performance over time by:
1. Running benchmarks on each commit
2. Comparing against baseline
3. Documenting significant changes

## Platform-Specific Targets

### x86_64 (SSE/AVX2)

**Expected Speedups**:
- Bitwise operations: 2-4x (SSE), potentially higher with AVX2
- Batch operations: 4-8x with proper vectorization
- Hardware popcount: Near-native speed

### ARM64 (NEON)

**Expected Speedups**:
- Bitwise operations: 2-4x
- Batch operations: 4-8x
- Hardware popcount: Near-native speed

### Scalar Fallback

**Expected**: Performance should match or slightly exceed pure scalar implementation (no regression).

## Validation Checklist

Before merging SIMD changes:

- [ ] All performance regression tests pass
- [ ] Benchmarks show expected speedup ratios
- [ ] SIMD instructions verified in disassembly
- [ ] No performance regressions detected
- [ ] CI performance checks pass
- [ ] Documentation updated with actual performance metrics
- [ ] NPS validation tests pass (20%+ improvement in release builds) (Task 5.12.2)
- [ ] NPS benchmarks show consistent improvement across different depths (Task 5.12.1)
- [ ] Realistic workload simulation validates improvement (Task 5.12.3)

## Future Enhancements

- Automated performance trend tracking
- Statistical significance testing for benchmarks
- Historical performance baseline comparison
- Platform-specific performance targets
- Integration with profiling tools

