#![cfg(feature = "simd")]

//! Baseline-driven performance regression tests for SIMD bitboard operations.
//!
//! These tests implement **Test Task 2** from
//! `docs/design/implementation/simd-optimization/tasks-SIMD_FUTURE_IMPROVEMENTS.md`
//! by comparing the current implementation against the documented baselines in
//! `SIMD_IMPLEMENTATION_EVALUATION.md`. Each scenario reads its baseline ratio
//! (SIMD vs scalar) from `tests/performance_baselines/simd_performance_baseline.json`
//! so CI can detect regressions automatically.

use lazy_static::lazy_static;
use serde::Deserialize;
use shogi_engine::bitboards::SimdBitboard;
use std::time::Instant;

const BASELINE_PATH: &str = "tests/performance_baselines/simd_performance_baseline.json";
const NOISE_FLOOR_NS: u128 = 1_000_000; // 1ms - treat anything faster as noise

#[derive(Debug, Deserialize)]
struct PerformanceBaseline {
    version: String,
    description: String,
    scenarios: Vec<ScenarioBaseline>,
}

#[derive(Debug, Deserialize)]
struct ScenarioBaseline {
    name: String,
    operation: OperationKind,
    iterations: u64,
    baseline_ratio: f64,
    allowed_regression_pct: f64,
    max_avg_simd_ns: f64,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OperationKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNot,
    Popcount,
    Combined,
}

#[derive(Debug)]
struct PerfMeasurement {
    simd_total_ns: u128,
    scalar_total_ns: u128,
    iterations: u64,
}

impl PerfMeasurement {
    fn ratio(&self) -> f64 {
        if self.scalar_total_ns == 0 {
            return 1.0;
        }
        self.simd_total_ns as f64 / self.scalar_total_ns as f64
    }

    fn simd_avg_ns(&self) -> f64 {
        self.simd_total_ns as f64 / self.iterations as f64
    }

    fn scalar_avg_ns(&self) -> f64 {
        self.scalar_total_ns as f64 / self.iterations as f64
    }

    fn is_within_noise(&self) -> bool {
        self.simd_total_ns < NOISE_FLOOR_NS
    }
}

lazy_static! {
    static ref BASELINE: PerformanceBaseline = {
        let raw = include_str!("performance_baselines/simd_performance_baseline.json");
        serde_json::from_str(raw).unwrap_or_else(|err| {
            panic!("Invalid performance baseline ({}): {}", BASELINE_PATH, err)
        })
    };
}

#[test]
fn test_simd_performance_against_baseline() {
    println!("Loaded SIMD performance baseline {} - {}", BASELINE.version, BASELINE.description);

    let mut summary_rows = Vec::new();
    let mut alerts = Vec::new();
    let mut failures = Vec::new();

    for scenario in &BASELINE.scenarios {
        let measurement = run_scenario(scenario.operation, scenario.iterations);
        let ratio = measurement.ratio();
        let allowed_ratio =
            scenario.baseline_ratio * (1.0 + scenario.allowed_regression_pct / 100.0);
        let speedup = if measurement.simd_total_ns > 0 {
            measurement.scalar_total_ns as f64 / measurement.simd_total_ns as f64
        } else {
            1.0
        };

        let mut status = "PASS";
        if ratio > allowed_ratio && !measurement.is_within_noise() {
            status = "FAIL";
            failures.push(format!(
                "{} exceeded allowed ratio {:.3}x (measured {:.3}x, baseline {:.3}x, speedup {:.2}x)",
                scenario.name, allowed_ratio, ratio, scenario.baseline_ratio, speedup
            ));
        }

        if ratio > scenario.baseline_ratio && status == "PASS" {
            alerts.push(format!(
                "{} slowed down vs. baseline (ratio {:.3}x > {:.3}x)",
                scenario.name, ratio, scenario.baseline_ratio
            ));
        }

        if measurement.simd_avg_ns() > scenario.max_avg_simd_ns && !measurement.is_within_noise() {
            status = "FAIL";
            failures.push(format!(
                "{} average SIMD time {:.2}ns exceeded limit {:.2}ns",
                scenario.name,
                measurement.simd_avg_ns(),
                scenario.max_avg_simd_ns
            ));
        }

        let note = scenario.notes.as_deref().unwrap_or("-");
        summary_rows.push(format!(
            "| `{}` | {:.3}x | {:.3}x | {:.3}x | {:.2}ns | {:.2}ns | {} | {} |",
            scenario.name,
            ratio,
            scenario.baseline_ratio,
            allowed_ratio,
            measurement.simd_avg_ns(),
            measurement.scalar_avg_ns(),
            status,
            note
        ));
    }

    write_summary(&summary_rows, &alerts, &failures);

    if !failures.is_empty() {
        panic!("SIMD performance regressions detected:\n{}", failures.join("\n"));
    }
}

fn write_summary(rows: &[String], alerts: &[String], failures: &[String]) {
    let Some(path) = std::env::var_os("SIMD_PERF_OUTPUT") else {
        return;
    };

    let mut report = String::new();
    report.push_str("# SIMD Performance Regression Results\n\n");
    report.push_str("| Scenario | Measured Ratio | Baseline Ratio | Allowed Ratio | SIMD avg (ns) | Scalar avg (ns) | Status | Notes |\n");
    report.push_str("|----------|----------------|----------------|---------------|---------------|-----------------|--------|-------|\n");
    for row in rows {
        report.push_str(row);
        report.push('\n');
    }

    if !alerts.is_empty() {
        report.push_str("\n## Alerts\n");
        for alert in alerts {
            report.push_str("- ");
            report.push_str(alert);
            report.push('\n');
        }
    }

    if !failures.is_empty() {
        report.push_str("\n## Failures\n");
        for failure in failures {
            report.push_str("- ");
            report.push_str(failure);
            report.push('\n');
        }
    }

    if let Err(err) = std::fs::write(&path, report) {
        eprintln!("Unable to write SIMD performance summary to {:?}: {}", path, err);
    }
}

fn run_scenario(operation: OperationKind, iterations: u64) -> PerfMeasurement {
    match operation {
        OperationKind::BitwiseAnd => run_binary_op(iterations, |a, b| a & b, |a, b| a & b),
        OperationKind::BitwiseOr => run_binary_op(iterations, |a, b| a | b, |a, b| a | b),
        OperationKind::BitwiseXor => run_binary_op(iterations, |a, b| a ^ b, |a, b| a ^ b),
        OperationKind::BitwiseNot => run_unary_op(iterations, |a| !a, |a| !a),
        OperationKind::Popcount => run_popcount(iterations),
        OperationKind::Combined => run_combined(iterations),
    }
}

fn run_binary_op<FSimd, FScalar>(
    iterations: u64,
    simd_op: FSimd,
    scalar_op: FScalar,
) -> PerfMeasurement
where
    FSimd: Fn(SimdBitboard, SimdBitboard) -> SimdBitboard + Copy,
    FScalar: Fn(u128, u128) -> u128 + Copy,
{
    let lhs = SimdBitboard::from_u128(TEST_VALUE1);
    let rhs = SimdBitboard::from_u128(TEST_VALUE2);
    let scalar_lhs = TEST_VALUE1;
    let scalar_rhs = TEST_VALUE2;

    let simd_duration = time_loop(iterations, || {
        let result = simd_op(lhs, rhs);
        std::hint::black_box(result);
    });

    let scalar_duration = time_loop(iterations, || {
        let result = scalar_op(scalar_lhs, scalar_rhs);
        std::hint::black_box(result);
    });

    PerfMeasurement {
        simd_total_ns: simd_duration.as_nanos(),
        scalar_total_ns: scalar_duration.as_nanos(),
        iterations,
    }
}

fn run_unary_op<FSimd, FScalar>(
    iterations: u64,
    simd_op: FSimd,
    scalar_op: FScalar,
) -> PerfMeasurement
where
    FSimd: Fn(SimdBitboard) -> SimdBitboard + Copy,
    FScalar: Fn(u128) -> u128 + Copy,
{
    let value = SimdBitboard::from_u128(TEST_VALUE1);
    let scalar_value = TEST_VALUE1;

    let simd_duration = time_loop(iterations, || {
        let result = simd_op(value);
        std::hint::black_box(result);
    });

    let scalar_duration = time_loop(iterations, || {
        let result = scalar_op(scalar_value);
        std::hint::black_box(result);
    });

    PerfMeasurement {
        simd_total_ns: simd_duration.as_nanos(),
        scalar_total_ns: scalar_duration.as_nanos(),
        iterations,
    }
}

fn run_popcount(iterations: u64) -> PerfMeasurement {
    let value = SimdBitboard::from_u128(TEST_VALUE3);
    let scalar_value = TEST_VALUE3;

    let simd_duration = time_loop(iterations, || {
        let result = value.count_ones();
        std::hint::black_box(result);
    });

    let scalar_duration = time_loop(iterations, || {
        let result = scalar_value.count_ones();
        std::hint::black_box(result);
    });

    PerfMeasurement {
        simd_total_ns: simd_duration.as_nanos(),
        scalar_total_ns: scalar_duration.as_nanos(),
        iterations,
    }
}

fn run_combined(iterations: u64) -> PerfMeasurement {
    let a = SimdBitboard::from_u128(TEST_VALUE1);
    let b = SimdBitboard::from_u128(TEST_VALUE2);
    let c = SimdBitboard::from_u128(TEST_VALUE3);

    let scalar_a = TEST_VALUE1;
    let scalar_b = TEST_VALUE2;
    let scalar_c = TEST_VALUE3;

    let simd_duration = time_loop(iterations, || {
        let result = (a & b) | (c & !a);
        std::hint::black_box(result);
    });

    let scalar_duration = time_loop(iterations, || {
        let result = (scalar_a & scalar_b) | (scalar_c & !scalar_a);
        std::hint::black_box(result);
    });

    PerfMeasurement {
        simd_total_ns: simd_duration.as_nanos(),
        scalar_total_ns: scalar_duration.as_nanos(),
        iterations,
    }
}

fn time_loop<F>(iterations: u64, mut f: F) -> std::time::Duration
where
    F: FnMut(),
{
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed()
}

const TEST_VALUE1: u128 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
const TEST_VALUE2: u128 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
const TEST_VALUE3: u128 = 0x5555_5555_5555_5555_5555_5555_5555_5555;
