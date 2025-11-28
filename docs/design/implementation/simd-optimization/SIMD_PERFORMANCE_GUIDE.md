## SIMD Performance Guide

This guide consolidates the latest SIMD performance data, tuning levers, and troubleshooting tactics for the YSE engine. It complements the deep-dive evaluation in `SIMD_IMPLEMENTATION_EVALUATION.md` and the task tracking in `tasks-SIMD_INTEGRATION_STATUS.md` / `tasks-SIMD_FUTURE_IMPROVEMENTS.md`, so read those when you need exhaustive context.

---

### 1. Audience & Prerequisites
- Engine developers who need deterministic SIMD speedups across evaluation, tactical analysis, and move generation.
- Familiarity with runtime configuration (`SimdConfig`), Criterion benchmarks, and the telemetry helpers in `src/utils/telemetry.rs`.

---

### 2. Quick Start Checklist
1. **Enable the `simd` feature** at compile time (`cargo test --features simd …`).
2. **Verify runtime flags**: ensure the relevant `SimdConfig` booleans are `true` before profiling.
3. **Warm up telemetry**: call `reset_simd_telemetry()` before each measurement run to capture clean counters.
4. **Record a scalar baseline** by temporarily disabling each flag—this is critical because early evaluations showed a 40% regression when pseudo-SIMD wrappers were used without real intrinsics.
5. **Automate validation** via `tests/simd_performance_validation_tests.rs` (micro timings) and the NPS validation tests/benches for end-to-end throughput.

---

### 3. Performance Characteristics (DT2.1)

| Component | Workload | SIMD Backend | Typical Speedup | Notes |
| --- | --- | --- | --- | --- |
| Bitwise core ops | Batch AND/OR/XOR | SSE2 → AVX2 auto-switch, NEON on ARM | 2–4× | Explicit intrinsics replace the legacy `u128` wrapper that showed a 0.59× slowdown in the 2024 evaluation. |
| Evaluation | PST scoring batches | Portable SIMD + SoA PST layout | 2–3× | Prefetching + aligned PST storage deliver another ~8% per `tasks-SIMD_FUTURE_IMPROVEMENTS.md`. |
| Tactical patterns | Forks, pins, skewers | SIMD filters + scalar scoring | 2–4× | SIMD quickly filters candidate pieces; scalar code still assigns bonuses for accuracy. |
| Move generation | Sliding piece batches | AVX2/NEON vectorized ray lookups | 2–4× | Requires magic table availability; falls back to scalar per `tasks-SIMD_INTEGRATION_STATUS.md`. |
| NPS end-to-end | Full search workloads | Mixed | ≥20% | Enforced by `tests/simd_nps_validation.rs` and `benches/simd_nps_benchmarks.rs`. |

Use the Criterion suites (`benches/simd_integration_benchmarks.rs`, `benches/simd_nps_benchmarks.rs`, etc.) to collect hardware-specific numbers; update `tests/performance_baselines/simd_performance_baseline.json` whenever you refresh expectations.

---

### 4. Performance Tuning Workflow (DT2.2)
1. **Measure**  
   - Run scalar + SIMD micro benches (`cargo bench --features simd simd_integration_benchmarks`).  
   - Capture telemetry snapshots (`search_engine.get_simd_telemetry()`).
2. **Diagnose**  
   - Compare against the evaluation report; if a path regresses back toward the 0.59× slowdown, you likely disabled an intrinsic path or hit an alignment issue.  
   - Check `tasks-SIMD_INTEGRATION_STATUS.md` to ensure the relevant integration task is still marked complete.
3. **Tune knobs**  
   - Runtime: toggle `SimdConfig` flags individually to pinpoint regressions.  
   - Memory: ensure 64-byte alignment and prefetch hints from the memory optimization tasks are intact.  
   - Platform: confirm AVX2/NEON detection via `platform_detection::get_platform_capabilities()`.
4. **Validate**  
   - `cargo test --features simd --test simd_performance_validation_tests` (micro).  
   - `cargo test --features simd --test simd_nps_validation` (end-to-end).  
   - Review GitHub workflow `simd-performance-check.yml` for CI parity.

---

### 5. Platform-Specific Guidance (DT2.3)
- **x86_64**  
  - SSE2 baseline with automatic AVX2 upgrade for batch work; AVX-512 kicks in only for large batches (≥16 bitboards) to avoid throttling (see `tasks-SIMD_FUTURE_IMPROVEMENTS.md`, Optimization 1 + Research Task 1).  
  - Ensure `RUSTFLAGS="-C target-feature=+avx2"` when benchmarking AVX2 explicitly; otherwise rely on runtime detection.
- **ARM64 (NEON)**  
  - Batch ops process two bitboards per lane with interleaved loads and tree reduction per `ARM_NEON_OPTIMIZATION_ANALYSIS.md`.  
  - Validate on real hardware (Apple Silicon) because x86 cross-tests spoof NEON paths but cannot replicate cache behavior.
- **WebAssembly / Other**  
  - Web targets currently fall back to scalar; when wasm SIMD128 returns, re-run the same telemetry + validation workflow before enabling by default.

---

### 6. Troubleshooting (DT2.4)
- **SYMPTOM**: SIMD slower than scalar  
  - ACTIONS: Confirm intrinsics are compiled in (check `cargo rustc -- --print cfg | grep simd`), ensure release mode, verify teleport detection logs, re-run `tests/simd_performance_regression_tests.rs` to compare with JSON baselines derived from the 2024 evaluation report.
- **SYMPTOM**: Telemetry shows zero SIMD calls  
  - ACTIONS: Inspect runtime flags, ensure `reset_simd_telemetry()` was called before measurement, confirm feature gate `#[cfg(feature = "simd")]` compiled the path.
- **SYMPTOM**: AVX-512 hurts throughput  
  - ACTIONS: Reduce batch size threshold or disable via env (`SHOGI_DISABLE_AVX512=1`) until you profile on a machine with sufficient thermal headroom.
- **SYMPTOM**: Performance inconsistent across runs  
  - ACTIONS: Pin CPU governor, purge OS caches, and run benchmarks with warmed-up telemetry to avoid recording initialization costs.

---

### 7. Monitoring & Examples (DT2.5)
- **Telemetry Snapshot**  
  ```rust
  let before = reset_simd_telemetry();
  // run workload
  let after = get_simd_telemetry();
  println!("{:?}", after.diff(&before));
  ```
- **Regression Test**  
  - `cargo test --features simd --test simd_performance_regression_tests -- --nocapture`  
    Uses `tests/performance_baselines/simd_performance_baseline.json` to flag deviations, as recorded in the future-improvements task doc.
- **CI Workflow**  
  - `.github/workflows/simd-performance-check.yml` uploads Markdown summaries so reviewers can see when a change drifts toward the pre-intrinsic regression documented in `SIMD_IMPLEMENTATION_EVALUATION.md`.

---

### 8. Reference Map
- `docs/design/implementation/simd-optimization/SIMD_IMPLEMENTATION_EVALUATION.md` – historical baseline, root-cause analysis.
- `docs/design/implementation/simd-optimization/tasks-SIMD_INTEGRATION_STATUS.md` – integration checklist + telemetry hooks.
- `docs/design/implementation/simd-optimization/tasks-SIMD_FUTURE_IMPROVEMENTS.md` – completion notes for optimization/memory/prefetching work feeding this guide.

Keep this guide updated whenever you refresh performance baselines, add new SIMD backends, or change the telemetry schema.





