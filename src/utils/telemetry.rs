//! Telemetry utilities: thin, centralized surface for debug/trace logging.
//!
//! This re-exports existing debug utilities behind a stable path and can be
//! extended with higher-level helpers without touching call sites.

pub use crate::debug_utils::{debug_log, is_debug_enabled, set_debug_enabled, trace_log};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// SIMD telemetry statistics
///
/// Tracks usage of SIMD vs scalar implementations across different components.
///
/// # Task 4.0 (Task 5.1)
/// # Task 5.6: Added timing measurements
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
    /// Total time spent in SIMD evaluation (nanoseconds)
    #[serde(skip_serializing_if = "is_zero")]
    pub simd_evaluation_time_ns: u64,
    /// Total time spent in scalar evaluation (nanoseconds)
    #[serde(skip_serializing_if = "is_zero")]
    pub scalar_evaluation_time_ns: u64,
    /// Total time spent in SIMD pattern matching (nanoseconds)
    #[serde(skip_serializing_if = "is_zero")]
    pub simd_pattern_time_ns: u64,
    /// Total time spent in scalar pattern matching (nanoseconds)
    #[serde(skip_serializing_if = "is_zero")]
    pub scalar_pattern_time_ns: u64,
    /// Total time spent in SIMD move generation (nanoseconds)
    #[serde(skip_serializing_if = "is_zero")]
    pub simd_move_gen_time_ns: u64,
    /// Total time spent in scalar move generation (nanoseconds)
    #[serde(skip_serializing_if = "is_zero")]
    pub scalar_move_gen_time_ns: u64,
}

/// Helper function for serde skip_serializing_if
#[inline]
fn is_zero(v: &u64) -> bool {
    *v == 0
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
            simd_evaluation_time_ns: 0,
            scalar_evaluation_time_ns: 0,
            simd_pattern_time_ns: 0,
            scalar_pattern_time_ns: 0,
            simd_move_gen_time_ns: 0,
            scalar_move_gen_time_ns: 0,
        }
    }
}

impl SimdTelemetry {
    /// Calculate average time per SIMD evaluation call (nanoseconds)
    pub fn avg_simd_evaluation_time_ns(&self) -> f64 {
        if self.simd_evaluation_calls > 0 {
            self.simd_evaluation_time_ns as f64 / self.simd_evaluation_calls as f64
        } else {
            0.0
        }
    }

    /// Calculate average time per scalar evaluation call (nanoseconds)
    pub fn avg_scalar_evaluation_time_ns(&self) -> f64 {
        if self.scalar_evaluation_calls > 0 {
            self.scalar_evaluation_time_ns as f64 / self.scalar_evaluation_calls as f64
        } else {
            0.0
        }
    }

    /// Calculate evaluation speedup ratio (scalar time / SIMD time)
    pub fn evaluation_speedup_ratio(&self) -> f64 {
        let simd_avg = self.avg_simd_evaluation_time_ns();
        let scalar_avg = self.avg_scalar_evaluation_time_ns();
        if simd_avg > 0.0 && scalar_avg > 0.0 {
            scalar_avg / simd_avg
        } else {
            1.0
        }
    }

    /// Calculate average time per SIMD pattern call (nanoseconds)
    pub fn avg_simd_pattern_time_ns(&self) -> f64 {
        if self.simd_pattern_calls > 0 {
            self.simd_pattern_time_ns as f64 / self.simd_pattern_calls as f64
        } else {
            0.0
        }
    }

    /// Calculate average time per scalar pattern call (nanoseconds)
    pub fn avg_scalar_pattern_time_ns(&self) -> f64 {
        if self.scalar_pattern_calls > 0 {
            self.scalar_pattern_time_ns as f64 / self.scalar_pattern_calls as f64
        } else {
            0.0
        }
    }

    /// Calculate pattern matching speedup ratio (scalar time / SIMD time)
    pub fn pattern_speedup_ratio(&self) -> f64 {
        let simd_avg = self.avg_simd_pattern_time_ns();
        let scalar_avg = self.avg_scalar_pattern_time_ns();
        if simd_avg > 0.0 && scalar_avg > 0.0 {
            scalar_avg / simd_avg
        } else {
            1.0
        }
    }

    /// Calculate average time per SIMD move generation call (nanoseconds)
    pub fn avg_simd_move_gen_time_ns(&self) -> f64 {
        if self.simd_move_gen_calls > 0 {
            self.simd_move_gen_time_ns as f64 / self.simd_move_gen_calls as f64
        } else {
            0.0
        }
    }

    /// Calculate average time per scalar move generation call (nanoseconds)
    pub fn avg_scalar_move_gen_time_ns(&self) -> f64 {
        if self.scalar_move_gen_calls > 0 {
            self.scalar_move_gen_time_ns as f64 / self.scalar_move_gen_calls as f64
        } else {
            0.0
        }
    }

    /// Calculate move generation speedup ratio (scalar time / SIMD time)
    pub fn move_gen_speedup_ratio(&self) -> f64 {
        let simd_avg = self.avg_simd_move_gen_time_ns();
        let scalar_avg = self.avg_scalar_move_gen_time_ns();
        if simd_avg > 0.0 && scalar_avg > 0.0 {
            scalar_avg / simd_avg
        } else {
            1.0
        }
    }
}

/// Thread-safe SIMD telemetry tracker
///
/// Uses atomic counters for thread-safe tracking across multiple threads.
///
/// # Task 4.0 (Task 5.1)
/// # Task 5.6: Added timing measurements
#[derive(Debug)]
pub struct SimdTelemetryTracker {
    simd_evaluation_calls: AtomicU64,
    scalar_evaluation_calls: AtomicU64,
    simd_pattern_calls: AtomicU64,
    scalar_pattern_calls: AtomicU64,
    simd_move_gen_calls: AtomicU64,
    scalar_move_gen_calls: AtomicU64,
    simd_evaluation_time_ns: AtomicU64,
    scalar_evaluation_time_ns: AtomicU64,
    simd_pattern_time_ns: AtomicU64,
    scalar_pattern_time_ns: AtomicU64,
    simd_move_gen_time_ns: AtomicU64,
    scalar_move_gen_time_ns: AtomicU64,
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
            simd_evaluation_time_ns: AtomicU64::new(0),
            scalar_evaluation_time_ns: AtomicU64::new(0),
            simd_pattern_time_ns: AtomicU64::new(0),
            scalar_pattern_time_ns: AtomicU64::new(0),
            simd_move_gen_time_ns: AtomicU64::new(0),
            scalar_move_gen_time_ns: AtomicU64::new(0),
        }
    }

    /// Record a SIMD evaluation call
    pub fn record_simd_evaluation(&self) {
        self.simd_evaluation_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a SIMD evaluation call with timing
    /// # Task 5.6.1
    pub fn record_simd_evaluation_with_time(&self, time_ns: u64) {
        self.simd_evaluation_calls.fetch_add(1, Ordering::Relaxed);
        self.simd_evaluation_time_ns.fetch_add(time_ns, Ordering::Relaxed);
    }

    /// Record a scalar evaluation call
    pub fn record_scalar_evaluation(&self) {
        self.scalar_evaluation_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a scalar evaluation call with timing
    /// # Task 5.6.1
    pub fn record_scalar_evaluation_with_time(&self, time_ns: u64) {
        self.scalar_evaluation_calls.fetch_add(1, Ordering::Relaxed);
        self.scalar_evaluation_time_ns.fetch_add(time_ns, Ordering::Relaxed);
    }

    /// Record a SIMD pattern matching call
    pub fn record_simd_pattern(&self) {
        self.simd_pattern_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a SIMD pattern matching call with timing
    /// # Task 5.6.1
    pub fn record_simd_pattern_with_time(&self, time_ns: u64) {
        self.simd_pattern_calls.fetch_add(1, Ordering::Relaxed);
        self.simd_pattern_time_ns.fetch_add(time_ns, Ordering::Relaxed);
    }

    /// Record a scalar pattern matching call
    pub fn record_scalar_pattern(&self) {
        self.scalar_pattern_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a scalar pattern matching call with timing
    /// # Task 5.6.1
    pub fn record_scalar_pattern_with_time(&self, time_ns: u64) {
        self.scalar_pattern_calls.fetch_add(1, Ordering::Relaxed);
        self.scalar_pattern_time_ns.fetch_add(time_ns, Ordering::Relaxed);
    }

    /// Record a SIMD move generation call
    pub fn record_simd_move_gen(&self) {
        self.simd_move_gen_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a SIMD move generation call with timing
    /// # Task 5.6.1
    pub fn record_simd_move_gen_with_time(&self, time_ns: u64) {
        self.simd_move_gen_calls.fetch_add(1, Ordering::Relaxed);
        self.simd_move_gen_time_ns.fetch_add(time_ns, Ordering::Relaxed);
    }

    /// Record a scalar move generation call
    pub fn record_scalar_move_gen(&self) {
        self.scalar_move_gen_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a scalar move generation call with timing
    /// # Task 5.6.1
    pub fn record_scalar_move_gen_with_time(&self, time_ns: u64) {
        self.scalar_move_gen_calls.fetch_add(1, Ordering::Relaxed);
        self.scalar_move_gen_time_ns.fetch_add(time_ns, Ordering::Relaxed);
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
            simd_evaluation_time_ns: self.simd_evaluation_time_ns.load(Ordering::Relaxed),
            scalar_evaluation_time_ns: self.scalar_evaluation_time_ns.load(Ordering::Relaxed),
            simd_pattern_time_ns: self.simd_pattern_time_ns.load(Ordering::Relaxed),
            scalar_pattern_time_ns: self.scalar_pattern_time_ns.load(Ordering::Relaxed),
            simd_move_gen_time_ns: self.simd_move_gen_time_ns.load(Ordering::Relaxed),
            scalar_move_gen_time_ns: self.scalar_move_gen_time_ns.load(Ordering::Relaxed),
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
        self.simd_evaluation_time_ns.store(0, Ordering::Relaxed);
        self.scalar_evaluation_time_ns.store(0, Ordering::Relaxed);
        self.simd_pattern_time_ns.store(0, Ordering::Relaxed);
        self.scalar_pattern_time_ns.store(0, Ordering::Relaxed);
        self.simd_move_gen_time_ns.store(0, Ordering::Relaxed);
        self.scalar_move_gen_time_ns.store(0, Ordering::Relaxed);
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
/// # Task 5.6: Added timing measurements
pub static SIMD_TELEMETRY: SimdTelemetryTracker = SimdTelemetryTracker {
    simd_evaluation_calls: std::sync::atomic::AtomicU64::new(0),
    scalar_evaluation_calls: std::sync::atomic::AtomicU64::new(0),
    simd_pattern_calls: std::sync::atomic::AtomicU64::new(0),
    scalar_pattern_calls: std::sync::atomic::AtomicU64::new(0),
    simd_move_gen_calls: std::sync::atomic::AtomicU64::new(0),
    scalar_move_gen_calls: std::sync::atomic::AtomicU64::new(0),
    simd_evaluation_time_ns: std::sync::atomic::AtomicU64::new(0),
    scalar_evaluation_time_ns: std::sync::atomic::AtomicU64::new(0),
    simd_pattern_time_ns: std::sync::atomic::AtomicU64::new(0),
    scalar_pattern_time_ns: std::sync::atomic::AtomicU64::new(0),
    simd_move_gen_time_ns: std::sync::atomic::AtomicU64::new(0),
    scalar_move_gen_time_ns: std::sync::atomic::AtomicU64::new(0),
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
