//! Telemetry utilities: thin, centralized surface for debug/trace logging.
//!
//! This re-exports existing debug utilities behind a stable path and can be
//! extended with higher-level helpers without touching call sites.

pub use crate::debug_utils::{is_debug_enabled, set_debug_enabled, trace_log, debug_log};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};

/// SIMD telemetry statistics
///
/// Tracks usage of SIMD vs scalar implementations across different components.
///
/// # Task 4.0 (Task 5.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdTelemetry {
    /// Number of SIMD evaluation calls
    pub simd_evaluation_calls: u64,
    /// Number of scalar evaluation calls
    pub scalar_evaluation_calls: u64,
    /// Number of SIMD pattern matching calls
    pub simd_pattern_calls: u64,
    /// Number of scalar pattern matching calls
    pub scalar_pattern_calls: u64,
    /// Number of SIMD move generation calls
    pub simd_move_gen_calls: u64,
    /// Number of scalar move generation calls
    pub scalar_move_gen_calls: u64,
}

impl Default for SimdTelemetry {
    fn default() -> Self {
        Self {
            simd_evaluation_calls: 0,
            scalar_evaluation_calls: 0,
            simd_pattern_calls: 0,
            scalar_pattern_calls: 0,
            simd_move_gen_calls: 0,
            scalar_move_gen_calls: 0,
        }
    }
}

/// Thread-safe SIMD telemetry tracker
///
/// Uses atomic counters for thread-safe tracking across multiple threads.
///
/// # Task 4.0 (Task 5.1)
#[derive(Debug)]
pub struct SimdTelemetryTracker {
    simd_evaluation_calls: AtomicU64,
    scalar_evaluation_calls: AtomicU64,
    simd_pattern_calls: AtomicU64,
    scalar_pattern_calls: AtomicU64,
    simd_move_gen_calls: AtomicU64,
    scalar_move_gen_calls: AtomicU64,
}

impl SimdTelemetryTracker {
    /// Create a new telemetry tracker
    pub fn new() -> Self {
        Self {
            simd_evaluation_calls: AtomicU64::new(0),
            scalar_evaluation_calls: AtomicU64::new(0),
            simd_pattern_calls: AtomicU64::new(0),
            scalar_pattern_calls: AtomicU64::new(0),
            simd_move_gen_calls: AtomicU64::new(0),
            scalar_move_gen_calls: AtomicU64::new(0),
        }
    }

    /// Record a SIMD evaluation call
    pub fn record_simd_evaluation(&self) {
        self.simd_evaluation_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a scalar evaluation call
    pub fn record_scalar_evaluation(&self) {
        self.scalar_evaluation_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a SIMD pattern matching call
    pub fn record_simd_pattern(&self) {
        self.simd_pattern_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a scalar pattern matching call
    pub fn record_scalar_pattern(&self) {
        self.scalar_pattern_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a SIMD move generation call
    pub fn record_simd_move_gen(&self) {
        self.simd_move_gen_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a scalar move generation call
    pub fn record_scalar_move_gen(&self) {
        self.scalar_move_gen_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current telemetry snapshot
    pub fn snapshot(&self) -> SimdTelemetry {
        SimdTelemetry {
            simd_evaluation_calls: self.simd_evaluation_calls.load(Ordering::Relaxed),
            scalar_evaluation_calls: self.scalar_evaluation_calls.load(Ordering::Relaxed),
            simd_pattern_calls: self.simd_pattern_calls.load(Ordering::Relaxed),
            scalar_pattern_calls: self.scalar_pattern_calls.load(Ordering::Relaxed),
            simd_move_gen_calls: self.simd_move_gen_calls.load(Ordering::Relaxed),
            scalar_move_gen_calls: self.scalar_move_gen_calls.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters
    pub fn reset(&self) {
        self.simd_evaluation_calls.store(0, Ordering::Relaxed);
        self.scalar_evaluation_calls.store(0, Ordering::Relaxed);
        self.simd_pattern_calls.store(0, Ordering::Relaxed);
        self.scalar_pattern_calls.store(0, Ordering::Relaxed);
        self.simd_move_gen_calls.store(0, Ordering::Relaxed);
        self.scalar_move_gen_calls.store(0, Ordering::Relaxed);
    }
}

impl Default for SimdTelemetryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Global SIMD telemetry tracker
///
/// Thread-safe global tracker for SIMD usage statistics.
///
/// # Task 4.0 (Task 5.1)
pub static SIMD_TELEMETRY: SimdTelemetryTracker = SimdTelemetryTracker {
    simd_evaluation_calls: std::sync::atomic::AtomicU64::new(0),
    scalar_evaluation_calls: std::sync::atomic::AtomicU64::new(0),
    simd_pattern_calls: std::sync::atomic::AtomicU64::new(0),
    scalar_pattern_calls: std::sync::atomic::AtomicU64::new(0),
    simd_move_gen_calls: std::sync::atomic::AtomicU64::new(0),
    scalar_move_gen_calls: std::sync::atomic::AtomicU64::new(0),
};

/// Get current SIMD telemetry snapshot
///
/// # Task 4.0 (Task 5.5)
pub fn get_simd_telemetry() -> SimdTelemetry {
    SIMD_TELEMETRY.snapshot()
}

/// Reset SIMD telemetry counters
pub fn reset_simd_telemetry() {
    SIMD_TELEMETRY.reset();
}

/// Convenience helper to trace with lazy formatting.
#[inline]
pub fn tracef<F: FnOnce() -> String>(feature: &str, msg: F) {
    if is_debug_enabled() {
        trace_log(feature, &msg());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_no_panic_when_disabled() {
        set_debug_enabled(false);
        tracef("TEST", || "ok".to_string());
        debug_log("ok");
    }
}


