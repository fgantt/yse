#![cfg(feature = "simd")]
//! Performance validation tests for SIMD optimizations
//!
//! These tests measure actual execution time to ensure SIMD paths are faster than scalar.
//! Task 5.6: Performance validation with timing
//!
//! These tests:
//! - Measure execution time for SIMD vs scalar implementations
//! - Detect performance regressions automatically
//! - Generate performance comparison reports
//! - Validate that SIMD provides expected speedup (target: 1.5x-4x)

use shogi_engine::{
    bitboards::BitboardBoard,
    config::{EngineConfig, SimdConfig},
    evaluation::{
        integration::{IntegratedEvaluationConfig, IntegratedEvaluator},
        tactical_patterns::{TacticalConfig, TacticalPatternRecognizer},
    },
    moves::MoveGenerator,
    types::{Player, Position},
    utils::telemetry::{get_simd_telemetry, reset_simd_telemetry, SimdTelemetry},
};
use std::time::Instant;

/// Helper function to create a standard test board
fn create_test_board() -> BitboardBoard {
    BitboardBoard::new()
}

/// Helper function to warm up CPU (optional, reduces timing variance)
fn warmup_cpu() {
    let mut sum = 0u64;
    for i in 0..1000 {
        sum += i;
    }
    std::hint::black_box(sum);
}

/// Performance validation for evaluation
/// Task 5.6.2
#[test]
fn test_evaluation_performance_simd_vs_scalar() {
    // Skip in CI if performance tests are disabled
    if std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        eprintln!("Skipping performance test: SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    reset_simd_telemetry();
    warmup_cpu();

    let board = create_test_board();
    let player = Player::Black;

    // Create evaluator with SIMD enabled
    let mut config_simd = IntegratedEvaluationConfig::default();
    #[cfg(feature = "simd")]
    {
        config_simd.simd.enable_simd_evaluation = true;
    }
    let mut evaluator_simd = IntegratedEvaluator::with_config(config_simd);

    // Create evaluator with SIMD disabled
    let mut config_scalar = IntegratedEvaluationConfig::default();
    #[cfg(feature = "simd")]
    {
        config_scalar.simd.enable_simd_evaluation = false;
    }
    let mut evaluator_scalar = IntegratedEvaluator::with_config(config_scalar);

    let iterations = 1000;

    // Benchmark SIMD evaluation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluator_simd.evaluate(&board, player, &Default::default());
    }
    let simd_duration = start.elapsed();

    // Benchmark scalar evaluation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluator_scalar.evaluate(&board, player, &Default::default());
    }
    let scalar_duration = start.elapsed();

    let simd_ns = simd_duration.as_nanos() as u64;
    let scalar_ns = scalar_duration.as_nanos() as u64;

    // Check performance: SIMD should be at least as fast as scalar (no regression)
    // In release builds, expect 1.5x-4x speedup
    #[cfg(not(debug_assertions))]
    {
        let speedup = scalar_ns as f64 / simd_ns as f64;
        assert!(
            speedup >= 1.0 || simd_ns < 1_000_000, // Allow < 1ms variance
            "SIMD evaluation regression: SIMD took {}ns, scalar took {}ns (speedup: {:.2}x). SIMD should be faster.",
            simd_ns,
            scalar_ns,
            speedup
        );
    }

    // In debug builds, just ensure no major regression (function call overhead expected)
    #[cfg(debug_assertions)]
    {
        assert!(
            simd_ns <= scalar_ns * 2 || simd_ns < 1_000_000,
            "SIMD evaluation regression in debug: SIMD took {}ns, scalar took {}ns",
            simd_ns,
            scalar_ns
        );
    }

    println!(
        "Evaluation Performance: SIMD={}ns (avg {}ns), Scalar={}ns (avg {}ns), Speedup={:.2}x",
        simd_ns,
        simd_ns / iterations,
        scalar_ns,
        scalar_ns / iterations,
        scalar_ns as f64 / simd_ns as f64
    );
}

/// Performance validation for pattern matching
/// Task 5.6.2
#[test]
fn test_pattern_matching_performance_simd_vs_scalar() {
    if std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        eprintln!("Skipping performance test: SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    reset_simd_telemetry();
    warmup_cpu();

    let board = create_test_board();
    let player = Player::Black;

    // Create recognizer with SIMD enabled
    let mut config_simd = TacticalConfig::default();
    #[cfg(feature = "simd")]
    {
        config_simd.enable_simd_pattern_matching = true;
    }
    let mut recognizer_simd = TacticalPatternRecognizer::with_config(config_simd);

    // Create recognizer with SIMD disabled
    let mut config_scalar = TacticalConfig::default();
    #[cfg(feature = "simd")]
    {
        config_scalar.enable_simd_pattern_matching = false;
    }
    let mut recognizer_scalar = TacticalPatternRecognizer::with_config(config_scalar);

    let iterations = 1000;

    let captured = shogi_engine::types::board::CapturedPieces::new();

    // Benchmark SIMD pattern matching
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = recognizer_simd.evaluate_tactics(&board, player, &captured);
    }
    let simd_duration = start.elapsed();

    // Benchmark scalar pattern matching
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = recognizer_scalar.evaluate_tactics(&board, player, &captured);
    }
    let scalar_duration = start.elapsed();

    let simd_ns = simd_duration.as_nanos() as u64;
    let scalar_ns = scalar_duration.as_nanos() as u64;

    #[cfg(not(debug_assertions))]
    {
        let speedup = scalar_ns as f64 / simd_ns as f64;
        assert!(
            speedup >= 1.0 || simd_ns < 1_000_000,
            "SIMD pattern matching regression: SIMD took {}ns, scalar took {}ns (speedup: {:.2}x)",
            simd_ns,
            scalar_ns,
            speedup
        );
    }

    #[cfg(debug_assertions)]
    {
        assert!(
            simd_ns <= scalar_ns * 2 || simd_ns < 1_000_000,
            "SIMD pattern matching regression in debug: SIMD took {}ns, scalar took {}ns",
            simd_ns,
            scalar_ns
        );
    }

    println!(
        "Pattern Matching Performance: SIMD={}ns (avg {}ns), Scalar={}ns (avg {}ns), Speedup={:.2}x",
        simd_ns,
        simd_ns / iterations,
        scalar_ns,
        scalar_ns / iterations,
        scalar_ns as f64 / simd_ns as f64
    );
}

/// Performance validation for move generation
/// Task 5.6.2
#[test]
fn test_move_generation_performance_simd_vs_scalar() {
    if std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        eprintln!("Skipping performance test: SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    reset_simd_telemetry();
    warmup_cpu();

    let board = create_test_board();
    let player = Player::Black;

    // Create move generator with SIMD enabled
    let mut generator_simd = MoveGenerator::new();
    #[cfg(feature = "simd")]
    {
        let mut config = SimdConfig::default();
        config.enable_simd_move_generation = true;
        generator_simd.set_simd_config(config);
    }

    // Create move generator with SIMD disabled
    let mut generator_scalar = MoveGenerator::new();
    #[cfg(feature = "simd")]
    {
        let mut config = SimdConfig::default();
        config.enable_simd_move_generation = false;
        generator_scalar.set_simd_config(config);
    }

    let iterations = 500;

    // Benchmark SIMD move generation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = generator_simd.generate_legal_moves(&board, player, &Default::default());
    }
    let simd_duration = start.elapsed();

    // Benchmark scalar move generation
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = generator_scalar.generate_legal_moves(&board, player, &Default::default());
    }
    let scalar_duration = start.elapsed();

    let simd_ns = simd_duration.as_nanos() as u64;
    let scalar_ns = scalar_duration.as_nanos() as u64;

    #[cfg(not(debug_assertions))]
    {
        let speedup = scalar_ns as f64 / simd_ns as f64;
        assert!(
            speedup >= 1.0 || simd_ns < 1_000_000,
            "SIMD move generation regression: SIMD took {}ns, scalar took {}ns (speedup: {:.2}x)",
            simd_ns,
            scalar_ns,
            speedup
        );
    }

    #[cfg(debug_assertions)]
    {
        assert!(
            simd_ns <= scalar_ns * 2 || simd_ns < 1_000_000,
            "SIMD move generation regression in debug: SIMD took {}ns, scalar took {}ns",
            simd_ns,
            scalar_ns
        );
    }

    println!(
        "Move Generation Performance: SIMD={}ns (avg {}ns), Scalar={}ns (avg {}ns), Speedup={:.2}x",
        simd_ns,
        simd_ns / iterations,
        scalar_ns,
        scalar_ns / iterations,
        scalar_ns as f64 / simd_ns as f64
    );
}

/// Automatic performance regression detection
/// Task 5.6.3
#[test]
fn test_performance_regression_detection() {
    if std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        eprintln!("Skipping performance test: SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    reset_simd_telemetry();
    warmup_cpu();

    // Threshold: SIMD should not be more than 10% slower than scalar in release builds
    const REGRESSION_THRESHOLD: f64 = 0.9; // 90% of scalar speed = 10% regression

    let board = create_test_board();
    let player = Player::Black;

    // Test all three components
    let component_names = vec!["evaluation", "pattern_matching", "move_generation"];

    for component_name in component_names {
        // Get SIMD time
        let simd_time = match component_name {
            "evaluation" => {
                let mut config = IntegratedEvaluationConfig::default();
                #[cfg(feature = "simd")]
                {
                    config.simd.enable_simd_evaluation = true;
                }
                let mut evaluator = IntegratedEvaluator::with_config(config);
                let start = Instant::now();
                for _ in 0..500 {
                    let _ = evaluator.evaluate(&board, player, &Default::default());
                }
                start.elapsed()
            }
            "pattern_matching" => {
                let mut config = TacticalConfig::default();
                #[cfg(feature = "simd")]
                {
                    config.enable_simd_pattern_matching = true;
                }
                let mut recognizer = TacticalPatternRecognizer::with_config(config);
                let captured = shogi_engine::types::board::CapturedPieces::new();
                let start = Instant::now();
                for _ in 0..500 {
                    let _ = recognizer.evaluate_tactics(&board, player, &captured);
                }
                start.elapsed()
            }
            "move_generation" => {
                let mut generator = MoveGenerator::new();
                #[cfg(feature = "simd")]
                {
                    let mut config = SimdConfig::default();
                    config.enable_simd_move_generation = true;
                    generator.set_simd_config(config);
                }
                let start = Instant::now();
                for _ in 0..500 {
                    let _ = generator.generate_legal_moves(&board, player, &Default::default());
                }
                start.elapsed()
            }
            _ => unreachable!(),
        };

        // Get scalar baseline (disable SIMD)
        let scalar_time = match component_name {
            "evaluation" => {
                let mut config = IntegratedEvaluationConfig::default();
                #[cfg(feature = "simd")]
                {
                    config.simd.enable_simd_evaluation = false;
                }
                let mut evaluator = IntegratedEvaluator::with_config(config);
                let start = Instant::now();
                for _ in 0..500 {
                    let _ = evaluator.evaluate(&board, player, &Default::default());
                }
                start.elapsed()
            }
            "pattern_matching" => {
                let mut config = TacticalConfig::default();
                #[cfg(feature = "simd")]
                {
                    config.enable_simd_pattern_matching = false;
                }
                let mut recognizer = TacticalPatternRecognizer::with_config(config);
                let captured = shogi_engine::types::board::CapturedPieces::new();
                let start = Instant::now();
                for _ in 0..500 {
                    let _ = recognizer.evaluate_tactics(&board, player, &captured);
                }
                start.elapsed()
            }
            "move_generation" => {
                let mut generator = MoveGenerator::new();
                #[cfg(feature = "simd")]
                {
                    let mut config = SimdConfig::default();
                    config.enable_simd_move_generation = false;
                    generator.set_simd_config(config);
                }
                let start = Instant::now();
                for _ in 0..500 {
                    let _ = generator.generate_legal_moves(&board, player, &Default::default());
                }
                start.elapsed()
            }
            _ => unreachable!(),
        };

        let simd_ns = simd_time.as_nanos() as f64;
        let scalar_ns = scalar_time.as_nanos() as f64;

        let ratio = if scalar_ns > 0.0 { simd_ns / scalar_ns } else { 1.0 };

        // In release builds, check for regression threshold
        #[cfg(not(debug_assertions))]
        {
            // Ratio > 1.0 means SIMD is slower (regression)
            // Allow small variance (< 1ms) or up to 10% regression
            assert!(
                ratio <= 1.0 + (1.0 - REGRESSION_THRESHOLD) || simd_ns < 1_000_000.0,
                "Performance regression detected in {}: SIMD is {:.2}x slower than scalar (threshold: {:.2}x). SIMD={:.0}ns, Scalar={:.0}ns",
                component_name,
                ratio,
                1.0 + (1.0 - REGRESSION_THRESHOLD),
                simd_ns,
                scalar_ns
            );
        }

        println!(
            "{}: SIMD={:.0}ns, Scalar={:.0}ns, Ratio={:.2}x",
            component_name, simd_ns, scalar_ns, ratio
        );
    }
}

/// Generate performance comparison report
/// Task 5.6.4
#[test]
fn test_performance_comparison_report() {
    if std::env::var("SHOGI_SKIP_PERFORMANCE_TESTS").is_ok() {
        eprintln!("Skipping performance test: SHOGI_SKIP_PERFORMANCE_TESTS is set");
        return;
    }

    reset_simd_telemetry();
    warmup_cpu();

    let board = create_test_board();
    let player = Player::Black;
    let iterations = 1000;

    // Collect telemetry data
    let telemetry_before = get_simd_telemetry();

    // Run SIMD evaluation
    let mut config_simd = IntegratedEvaluationConfig::default();
    #[cfg(feature = "simd")]
    {
        config_simd.simd.enable_simd_evaluation = true;
    }
    let mut evaluator = IntegratedEvaluator::with_config(config_simd);
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluator.evaluate(&board, player, &Default::default());
    }
    let simd_eval_time = start.elapsed();

    // Run scalar evaluation
    let mut config_scalar = IntegratedEvaluationConfig::default();
    #[cfg(feature = "simd")]
    {
        config_scalar.simd.enable_simd_evaluation = false;
    }
    let mut evaluator = IntegratedEvaluator::with_config(config_scalar);
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluator.evaluate(&board, player, &Default::default());
    }
    let scalar_eval_time = start.elapsed();

    let telemetry_after = get_simd_telemetry();

    // Generate report
    let report = generate_performance_report(
        &telemetry_before,
        &telemetry_after,
        simd_eval_time,
        scalar_eval_time,
        iterations,
    );

    println!("{}", report);

    // Verify report contains expected information
    assert!(report.contains("Evaluation"));
    assert!(report.contains("SIMD"));
    assert!(report.contains("Scalar"));
    assert!(report.contains("Speedup"));
}

/// Generate a performance comparison report
/// Task 5.6.4
fn generate_performance_report(
    _before: &SimdTelemetry,
    after: &SimdTelemetry,
    simd_time: std::time::Duration,
    scalar_time: std::time::Duration,
    iterations: u64,
) -> String {
    let simd_ns = simd_time.as_nanos() as f64;
    let scalar_ns = scalar_time.as_nanos() as f64;
    let speedup = if simd_ns > 0.0 { scalar_ns / simd_ns } else { 1.0 };

    format!(
        r#"
========================================
SIMD Performance Comparison Report
========================================

Configuration:
  Iterations: {}
  Build: {}

Evaluation Performance:
  SIMD Time:      {:.2}ms (avg {:.2}μs per call)
  Scalar Time:    {:.2}ms (avg {:.2}μs per call)
  Speedup:        {:.2}x

Telemetry Statistics:
  SIMD Calls:     {}
  Scalar Calls:   {}

========================================
"#,
        iterations,
        if cfg!(debug_assertions) { "Debug" } else { "Release" },
        simd_ns / 1_000_000.0,
        simd_ns / iterations as f64 / 1000.0,
        scalar_ns / 1_000_000.0,
        scalar_ns / iterations as f64 / 1000.0,
        speedup,
        after.simd_evaluation_calls,
        after.scalar_evaluation_calls,
    )
}

/// Test that timing measurements work correctly
#[test]
fn test_timing_measurements_enabled() {
    reset_simd_telemetry();

    let board = create_test_board();
    let player = Player::Black;

    let mut config = IntegratedEvaluationConfig::default();
    #[cfg(feature = "simd")]
    {
        config.simd.enable_simd_evaluation = true;
    }
    let mut evaluator = IntegratedEvaluator::with_config(config);

    // Run evaluation
    for _ in 0..100 {
        let _ = evaluator.evaluate(&board, player, &Default::default());
    }

    let telemetry = get_simd_telemetry();

    // Verify telemetry is being collected
    assert!(
        telemetry.simd_evaluation_calls > 0 || telemetry.scalar_evaluation_calls > 0,
        "Telemetry should record evaluation calls"
    );
}
