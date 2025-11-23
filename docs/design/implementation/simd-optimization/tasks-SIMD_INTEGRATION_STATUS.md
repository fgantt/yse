# Tasks: SIMD Full Integration

This task list is generated from `SIMD_INTEGRATION_STATUS.md` to fully integrate SIMD optimizations into all engine paths.

## Relevant Files

- `src/evaluation/integration.rs` - Contains `IntegratedEvaluator::evaluate_pst()` that needs SIMD integration
- `src/evaluation/evaluation_simd.rs` - Contains `SimdEvaluator` with `evaluate_pst_batch()` implementation
- `src/evaluation/tactical_patterns.rs` - Contains `TacticalPatternRecognizer::detect_forks()` that needs SIMD integration
- `src/evaluation/tactical_patterns_simd.rs` - Contains `SimdPatternMatcher` with `detect_forks_batch()` implementation
- `src/bitboards/sliding_moves.rs` - Contains `generate_sliding_moves_batch_vectorized()` implementation
- `src/search/search_engine.rs` - Search engine that needs to use vectorized move generation
- `src/moves.rs` - Move generator that may need SIMD integration
- `src/config/mod.rs` - Configuration system for runtime feature flags
- `src/utils/telemetry.rs` - Telemetry system for performance monitoring
- `tests/simd_integration_tests.rs` - Integration tests for SIMD features (to be created)
- `tests/evaluation_integration_tests.rs` - Tests for evaluation integration
- `tests/tactical_patterns_tests.rs` - Tests for tactical pattern integration
- `benches/simd_integration_benchmarks.rs` - Benchmarks for SIMD integration (to be created)

### Notes

- Unit tests should be placed alongside the code files they are testing
- Use `cargo test --features simd` to run SIMD-specific tests
- Use `cargo bench --features simd` to run SIMD benchmarks
- All SIMD code should be gated with `#[cfg(feature = "simd")]` feature flags

## Tasks

- [ ] 1.0 Integrate SIMD Evaluation into IntegratedEvaluator
  - [ ] 1.1 Add conditional compilation for SIMD feature in `src/evaluation/integration.rs`
  - [ ] 1.2 Import `SimdEvaluator` when SIMD feature is enabled
  - [ ] 1.3 Modify `evaluate_pst()` to conditionally use `SimdEvaluator::evaluate_pst_batch()` when SIMD is enabled
  - [ ] 1.4 Implement telemetry conversion: convert `TaperedScore` from SIMD evaluator to `PieceSquareTelemetry` format
  - [ ] 1.5 Add fallback to scalar implementation when SIMD feature is disabled or unavailable
  - [ ] 1.6 Ensure backward compatibility: maintain same return type `(TaperedScore, PieceSquareTelemetry)`
  - [ ] 1.7 Add unit tests in `tests/evaluation_integration_tests.rs` to verify SIMD evaluation produces same results as scalar
  - [ ] 1.8 Add integration test to verify SIMD evaluation is actually used when feature is enabled
  - [ ] 1.9 Update documentation comments to indicate SIMD usage
  - [ ] 1.10 (Optional) Consider using memory optimization utilities (prefetching) in evaluation paths for additional performance

- [ ] 2.0 Integrate SIMD Pattern Matching into TacticalPatternRecognizer
  - [ ] 2.1 Add conditional compilation for SIMD feature in `src/evaluation/tactical_patterns.rs`
  - [ ] 2.2 Import `SimdPatternMatcher` when SIMD feature is enabled
  - [ ] 2.3 Modify `detect_forks()` to conditionally use `SimdPatternMatcher::detect_forks_batch()` when SIMD is enabled
  - [ ] 2.4 Convert `SimdPatternMatcher` return type `Vec<(Position, PieceType, u32)>` to `TaperedScore` by calculating fork bonuses
  - [ ] 2.5 Preserve existing `detect_drop_fork_threats()` scalar logic (not yet vectorized)
  - [ ] 2.6 Apply phase weights to SIMD-detected forks using `apply_phase_weights()`
  - [ ] 2.7 Add fallback to scalar implementation when SIMD feature is disabled
  - [ ] 2.8 Ensure backward compatibility: maintain same return type `TaperedScore`
  - [ ] 2.9 Add unit tests in `tests/tactical_patterns_tests.rs` to verify SIMD fork detection produces same results as scalar
  - [ ] 2.10 Add integration test to verify SIMD pattern matching is actually used when feature is enabled
  - [ ] 2.11 Update documentation comments to indicate SIMD usage

- [ ] 3.0 Integrate Vectorized Move Generation into Search Engine
  - [ ] 3.1 Identify all locations in `src/search/search_engine.rs` where move generation occurs
  - [ ] 3.2 Determine if `MoveGenerator` needs to be modified to use `SlidingMoveGenerator` for batch operations
  - [ ] 3.3 Add conditional compilation for SIMD feature in move generation paths
  - [ ] 3.4 Modify move generation to collect sliding pieces (rook, bishop, lance) into batches
  - [ ] 3.5 Integrate `SlidingMoveGenerator::generate_sliding_moves_batch_vectorized()` for batch processing when SIMD is enabled
  - [ ] 3.6 Ensure non-sliding pieces continue to use existing generation logic
  - [ ] 3.7 Add fallback to regular batch generation when SIMD feature is disabled
  - [ ] 3.8 Verify move generation correctness: ensure vectorized generation produces same moves as scalar
  - [ ] 3.9 Add unit tests in `tests/simd_integration_tests.rs` to verify vectorized move generation correctness
  - [ ] 3.10 Add integration test to verify vectorized move generation is used in search engine when feature is enabled
  - [ ] 3.11 Update documentation comments to indicate SIMD usage
  - [ ] 3.12 (Optional) Consider integrating memory optimization utilities (prefetching, alignment) into critical move generation paths for additional performance gains

- [ ] 4.0 Add Runtime Feature Flags for SIMD Control
  - [ ] 4.1 Add `SimdConfig` struct to `src/config/mod.rs` with fields: `enable_simd_evaluation`, `enable_simd_pattern_matching`, `enable_simd_move_generation`
  - [ ] 4.2 Add `simd` field of type `SimdConfig` to `EngineConfig` struct
  - [ ] 4.3 Implement `Default` trait for `SimdConfig` with all features enabled by default when `simd` feature is enabled
  - [ ] 4.4 Add runtime checks in evaluation integration: check `config.simd.enable_simd_evaluation` before using SIMD
  - [ ] 4.5 Add runtime checks in pattern matching: check `config.simd.enable_simd_pattern_matching` before using SIMD
  - [ ] 4.6 Add runtime checks in move generation: check `config.simd.enable_simd_move_generation` before using SIMD
  - [ ] 4.7 Add `validate()` method to `SimdConfig` to ensure configuration is valid
  - [ ] 4.8 Add serialization/deserialization support for `SimdConfig` (Serialize/Deserialize traits)
  - [ ] 4.9 Add unit tests in `tests/config_tests.rs` to verify SIMD config can be enabled/disabled at runtime
  - [ ] 4.10 Add integration test to verify runtime flags actually control SIMD usage
  - [ ] 4.11 Update configuration documentation to describe SIMD runtime flags

- [ ] 5.0 Add Performance Monitoring and Validation
  - [ ] 5.1 Add `SimdTelemetry` struct to `src/utils/telemetry.rs` with fields: `simd_evaluation_calls`, `scalar_evaluation_calls`, `simd_pattern_calls`, `scalar_pattern_calls`, `simd_move_gen_calls`, `scalar_move_gen_calls`
  - [ ] 5.2 Add telemetry tracking in `IntegratedEvaluator::evaluate_pst()` to count SIMD vs scalar calls
  - [ ] 5.3 Add telemetry tracking in `TacticalPatternRecognizer::detect_forks()` to count SIMD vs scalar calls
  - [ ] 5.4 Add telemetry tracking in move generation to count SIMD vs scalar calls
  - [ ] 5.5 Add method to retrieve SIMD telemetry statistics from evaluator and search engine
  - [ ] 5.6 Add performance validation: ensure SIMD paths are actually faster than scalar (add timing measurements)
  - [ ] 5.7 Create `benches/simd_integration_benchmarks.rs` to benchmark SIMD vs scalar performance
  - [ ] 5.8 Add benchmark for SIMD evaluation vs scalar evaluation
  - [ ] 5.9 Add benchmark for SIMD pattern matching vs scalar pattern matching
  - [ ] 5.10 Add benchmark for vectorized move generation vs regular batch generation
  - [ ] 5.11 Add integration test in `tests/simd_integration_tests.rs` to verify SIMD telemetry is collected
  - [ ] 5.12 Add validation test to ensure SIMD provides expected performance improvement (target: 20%+ NPS improvement)
  - [ ] 5.13 Update `SIMD_INTEGRATION_STATUS.md` to reflect completed integration

