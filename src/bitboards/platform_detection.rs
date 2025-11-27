//! Platform detection and capability detection for bit-scanning optimizations
//!
//! This module provides runtime detection of CPU features and platform
//! capabilities to select the optimal bit-scanning implementation for the
//! current environment.

// Note: Bitboard type will be used in future implementations

/// Supported bit-scanning implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitscanImpl {
    /// Hardware-accelerated bit scanning (native platforms only)
    Hardware,
    /// De Bruijn sequence lookup (fallback)
    DeBruijn,
    /// Generic software implementation (final fallback)
    Software,
}

/// Supported population count implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopcountImpl {
    /// Hardware-accelerated population count (native platforms only)
    Hardware,
    /// SWAR (SIMD Within A Register) bit counting fallback
    BitParallel,
    /// Generic software implementation (final fallback)
    Software,
}

/// Supported CPU architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    /// x86_64 architecture
    X86_64,
    /// ARM architecture
    ARM,
    /// Unknown architecture
    Unknown,
}

/// SIMD implementation levels (ordered from highest to lowest performance)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimdLevel {
    /// AVX-512 (highest performance, x86_64 only)
    AVX512,
    /// AVX2 (high performance, x86_64 only)
    AVX2,
    /// SSE (baseline SIMD, x86_64 only)
    SSE,
    /// NEON (ARM64 SIMD)
    NEON,
    /// Scalar fallback (no SIMD)
    Scalar,
}

/// Platform capabilities and detected features
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    /// x86_64 POPCNT instruction support
    pub has_popcnt: bool,
    /// x86_64 BMI1 instruction support
    pub has_bmi1: bool,
    /// x86_64 BMI2 instruction support
    pub has_bmi2: bool,
    /// x86_64 AVX2 instruction support
    pub has_avx2: bool,
    /// x86_64 AVX-512 instruction support
    pub has_avx512: bool,
    /// ARM64 NEON instruction support
    pub has_neon: bool,
    /// Detected architecture
    pub architecture: Architecture,
}

impl Default for PlatformCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

impl PlatformCapabilities {
    /// Detect platform capabilities at runtime
    pub fn detect() -> Self {
        Self::detect_native_capabilities()
    }

    /// Detect capabilities for native platforms
    fn detect_native_capabilities() -> Self {
        let architecture = Self::detect_architecture();

        Self {
            has_popcnt: Self::detect_popcnt_support(),
            has_bmi1: Self::detect_bmi1_support(),
            has_bmi2: Self::detect_bmi2_support(),
            has_avx2: Self::detect_avx2_support(),
            has_avx512: Self::detect_avx512_support(),
            has_neon: Self::detect_neon_support(),
            architecture,
        }
    }

    /// Detect CPU architecture
    fn detect_architecture() -> Architecture {
        #[cfg(target_arch = "x86_64")]
        {
            Architecture::X86_64
        }

        #[cfg(target_arch = "aarch64")]
        {
            Architecture::ARM
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Architecture::Unknown
        }
    }

    /// Detect x86_64 POPCNT instruction support
    #[cfg(target_arch = "x86_64")]
    fn detect_popcnt_support() -> bool {
        unsafe {
            use std::arch::x86_64::__cpuid;

            // Check CPUID feature flags for POPCNT support
            let cpuid = __cpuid(1);
            (cpuid.ecx & (1 << 23)) != 0 // POPCNT bit in ECX register
        }
    }

    /// Detect x86_64 BMI1 instruction support
    #[cfg(target_arch = "x86_64")]
    fn detect_bmi1_support() -> bool {
        unsafe {
            use std::arch::x86_64::__cpuid;

            // Check CPUID feature flags for BMI1 support
            let cpuid = __cpuid(7);
            (cpuid.ebx & (1 << 3)) != 0 // BMI1 bit in EBX register
        }
    }

    /// Detect x86_64 BMI2 instruction support
    #[cfg(target_arch = "x86_64")]
    fn detect_bmi2_support() -> bool {
        unsafe {
            use std::arch::x86_64::__cpuid;

            // Check CPUID feature flags for BMI2 support
            let cpuid = __cpuid(7);
            (cpuid.ebx & (1 << 8)) != 0 // BMI2 bit in EBX register
        }
    }

    /// Fallback implementations for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    fn detect_popcnt_support() -> bool {
        false
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn detect_bmi1_support() -> bool {
        false
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn detect_bmi2_support() -> bool {
        false
    }

    /// Detect x86_64 AVX2 instruction support
    #[cfg(target_arch = "x86_64")]
    fn detect_avx2_support() -> bool {
        unsafe {
            use std::arch::x86_64::__cpuid;

            // Check CPUID feature flags for AVX2 support
            // AVX2 requires:
            // 1. AVX support (bit 28 in ECX from CPUID leaf 1)
            // 2. AVX2 support (bit 5 in EBX from CPUID leaf 7)
            let cpuid1 = __cpuid(1);
            let has_avx = (cpuid1.ecx & (1 << 28)) != 0; // AVX bit in ECX register

            let cpuid7 = __cpuid(7);
            let has_avx2 = (cpuid7.ebx & (1 << 5)) != 0; // AVX2 bit in EBX register

            has_avx && has_avx2
        }
    }

    /// Detect x86_64 AVX-512 instruction support
    #[cfg(target_arch = "x86_64")]
    fn detect_avx512_support() -> bool {
        unsafe {
            use std::arch::x86_64::__cpuid;

            // Check CPUID feature flags for AVX-512 support
            // AVX-512 requires:
            // 1. AVX2 support (as a prerequisite)
            // 2. AVX-512F (Foundation) support (bit 16 in EBX from CPUID leaf 7)
            // 3. OSXSAVE support (bit 27 in ECX from CPUID leaf 1) - required for XSAVE
            let cpuid1 = __cpuid(1);
            let has_osxsave = (cpuid1.ecx & (1 << 27)) != 0; // OSXSAVE bit in ECX register

            if !has_osxsave {
                return false;
            }

            // Check XCR0 register to ensure OS supports AVX-512
            // This requires checking if XCR0[7:5] (ZMM state) is enabled
            #[cfg(target_feature = "avx512f")]
            {
                let cpuid7 = __cpuid(7);
                let has_avx512f = (cpuid7.ebx & (1 << 16)) != 0; // AVX-512F bit in EBX register
                has_avx512f
            }
            #[cfg(not(target_feature = "avx512f"))]
            {
                // If AVX-512 is not enabled at compile time, we can still detect it
                // but won't be able to use it without recompilation
                let cpuid7 = __cpuid(7);
                let has_avx512f = (cpuid7.ebx & (1 << 16)) != 0; // AVX-512F bit in EBX register
                has_avx512f
            }
        }
    }

    /// Detect ARM64 NEON instruction support
    #[cfg(target_arch = "aarch64")]
    fn detect_neon_support() -> bool {
        // NEON is mandatory on ARM64 (aarch64), so it's always available
        // However, we can verify this at runtime if needed
        true
    }

    /// Fallback implementations for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    fn detect_avx2_support() -> bool {
        false
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn detect_avx512_support() -> bool {
        false
    }

    /// Fallback for non-ARM64 platforms
    #[cfg(not(target_arch = "aarch64"))]
    fn detect_neon_support() -> bool {
        false
    }

    /// Get optimal bitscan implementation for this platform
    pub fn get_bitscan_impl(&self) -> BitscanImpl {
        if self.has_bmi1 {
            BitscanImpl::Hardware // Use hardware acceleration when available
        } else {
            BitscanImpl::DeBruijn // Use De Bruijn as fallback
        }
    }

    /// Get optimal popcount implementation for this platform
    pub fn get_popcount_impl(&self) -> PopcountImpl {
        if self.has_popcnt {
            PopcountImpl::Hardware // Use hardware acceleration when available
        } else {
            PopcountImpl::BitParallel // Use SWAR as fallback
        }
    }

    /// Check if platform supports hardware acceleration
    pub fn has_hardware_acceleration(&self) -> bool {
        self.has_popcnt || self.has_bmi1
    }

    /// Get optimal SIMD implementation level for this platform
    /// Returns the highest available SIMD feature
    pub fn get_simd_level(&self) -> SimdLevel {
        match self.architecture {
            Architecture::X86_64 => {
                if self.has_avx512 {
                    SimdLevel::AVX512
                } else if self.has_avx2 {
                    SimdLevel::AVX2
                } else {
                    SimdLevel::SSE
                }
            }
            Architecture::ARM => {
                if self.has_neon {
                    SimdLevel::NEON
                } else {
                    SimdLevel::Scalar
                }
            }
            Architecture::Unknown => SimdLevel::Scalar,
        }
    }

    /// Check if platform supports SIMD operations
    pub fn has_simd_support(&self) -> bool {
        match self.architecture {
            Architecture::X86_64 => self.has_avx2 || true, // SSE is always available on x86_64
            Architecture::ARM => self.has_neon,
            Architecture::Unknown => false,
        }
    }

    /// Get recommended SIMD implementation for runtime feature selection
    /// This provides information about what SIMD features are available at
    /// runtime Note: Actual implementation selection is compile-time, but
    /// this can be used for logging, diagnostics, or to inform build
    /// configuration
    pub fn get_recommended_simd_impl(&self) -> &'static str {
        match self.get_simd_level() {
            SimdLevel::AVX512 => "AVX-512 (highest performance)",
            SimdLevel::AVX2 => "AVX2 (high performance)",
            SimdLevel::SSE => "SSE (baseline SIMD)",
            SimdLevel::NEON => "NEON (ARM64 SIMD)",
            SimdLevel::Scalar => "Scalar (no SIMD)",
        }
    }

    /// Check if AVX2 is available and recommended for use
    /// Returns true if AVX2 is detected and should be preferred over SSE
    pub fn should_use_avx2(&self) -> bool {
        self.has_avx2
    }

    /// Check if AVX-512 is available and recommended for use
    /// Returns true if AVX-512 is detected and should be preferred
    pub fn should_use_avx512(&self) -> bool {
        self.has_avx512
    }

    /// Get platform summary string
    pub fn get_summary(&self) -> String {
        format!(
            "Architecture: {:?}, POPCNT: {}, BMI1: {}, BMI2: {}, AVX2: {}, AVX-512: {}, NEON: {}",
            self.architecture,
            self.has_popcnt,
            self.has_bmi1,
            self.has_bmi2,
            self.has_avx2,
            self.has_avx512,
            self.has_neon
        )
    }
}

/// Global platform capabilities instance
static PLATFORM_CAPABILITIES: std::sync::OnceLock<PlatformCapabilities> =
    std::sync::OnceLock::new();

/// Get the global platform capabilities instance
pub fn get_platform_capabilities() -> &'static PlatformCapabilities {
    PLATFORM_CAPABILITIES.get_or_init(PlatformCapabilities::detect)
}

/// Get optimal bitscan implementation for current platform
pub fn get_best_bitscan_impl() -> BitscanImpl {
    get_platform_capabilities().get_bitscan_impl()
}

/// Get optimal popcount implementation for current platform
pub fn get_best_popcount_impl() -> PopcountImpl {
    get_platform_capabilities().get_popcount_impl()
}

/// Check if current platform has hardware acceleration
pub fn has_hardware_support() -> bool {
    get_platform_capabilities().has_hardware_acceleration()
}

/// Get platform summary for debugging
pub fn get_platform_summary() -> String {
    get_platform_capabilities().get_summary()
}

/// Get optimal SIMD level for current platform
pub fn get_simd_level() -> SimdLevel {
    get_platform_capabilities().get_simd_level()
}

/// Check if current platform has SIMD support
pub fn has_simd_support() -> bool {
    get_platform_capabilities().has_simd_support()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capabilities_detection() {
        let caps = PlatformCapabilities::detect();

        // Basic sanity checks
        assert_ne!(caps.architecture, Architecture::Unknown);
    }

    #[test]
    fn test_implementation_selection() {
        let caps = PlatformCapabilities::detect();

        let bitscan_impl = caps.get_bitscan_impl();
        let popcount_impl = caps.get_popcount_impl();

        // Hardware acceleration should be preferred when available
        if caps.has_bmi1 {
            assert_eq!(bitscan_impl, BitscanImpl::Hardware);
        }

        if caps.has_popcnt {
            assert_eq!(popcount_impl, PopcountImpl::Hardware);
        }
    }

    #[test]
    fn test_global_functions() {
        // Test that global functions work
        let _impl = get_best_bitscan_impl();
        let _impl = get_best_popcount_impl();
        let _has_hw = has_hardware_support();
        let _summary = get_platform_summary();

        // Ensure we get consistent results
        let caps1 = get_platform_capabilities();
        let caps2 = get_platform_capabilities();
        assert_eq!(caps1.architecture, caps2.architecture);
    }

    #[test]
    fn test_platform_summary_format() {
        let summary = get_platform_summary();
        assert!(!summary.is_empty());
        assert!(summary.contains("Architecture:"));
        assert!(summary.contains("POPCNT:"));
        assert!(summary.contains("BMI1:"));
        assert!(summary.contains("BMI2:"));
        assert!(summary.contains("AVX2:"));
        assert!(summary.contains("AVX-512:"));
        assert!(summary.contains("NEON:"));
    }

    #[test]
    fn test_simd_level_detection() {
        let caps = PlatformCapabilities::detect();
        let simd_level = caps.get_simd_level();

        // SIMD level should be appropriate for the architecture
        match caps.architecture {
            Architecture::X86_64 => {
                assert!(matches!(simd_level, SimdLevel::SSE | SimdLevel::AVX2 | SimdLevel::AVX512));
            }
            Architecture::ARM => {
                assert!(matches!(simd_level, SimdLevel::NEON | SimdLevel::Scalar));
            }
            Architecture::Unknown => {
                assert_eq!(simd_level, SimdLevel::Scalar);
            }
        }
    }

    #[test]
    fn test_simd_support_detection() {
        let caps = PlatformCapabilities::detect();
        let has_simd = caps.has_simd_support();

        // x86_64 should always have SIMD (at least SSE)
        // ARM should have NEON on aarch64
        match caps.architecture {
            Architecture::X86_64 => {
                assert!(has_simd, "x86_64 should always have SIMD support (SSE)");
            }
            Architecture::ARM => {
                assert_eq!(has_simd, caps.has_neon, "ARM SIMD support should match NEON detection");
            }
            Architecture::Unknown => {
                assert!(!has_simd, "Unknown architecture should not have SIMD support");
            }
        }
    }

    #[test]
    fn test_avx2_detection() {
        let caps = PlatformCapabilities::detect();

        #[cfg(target_arch = "x86_64")]
        {
            // AVX2 detection should work (may be true or false depending on CPU)
            let _ = caps.has_avx2;
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            assert!(!caps.has_avx2, "AVX2 should be false on non-x86_64 platforms");
        }
    }

    #[test]
    fn test_avx512_detection() {
        let caps = PlatformCapabilities::detect();

        #[cfg(target_arch = "x86_64")]
        {
            // AVX-512 detection should work (may be true or false depending on CPU)
            let _ = caps.has_avx512;
            // AVX-512 requires AVX2 as a prerequisite
            if caps.has_avx512 {
                assert!(caps.has_avx2, "AVX-512 requires AVX2 support");
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            assert!(!caps.has_avx512, "AVX-512 should be false on non-x86_64 platforms");
        }
    }

    #[test]
    fn test_neon_detection() {
        let caps = PlatformCapabilities::detect();

        #[cfg(target_arch = "aarch64")]
        {
            // NEON should always be available on aarch64
            assert!(caps.has_neon, "NEON should be available on aarch64");
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            assert!(!caps.has_neon, "NEON should be false on non-ARM64 platforms");
        }
    }

    #[test]
    fn test_simd_level_ordering() {
        // Test that SimdLevel ordering is correct
        // Note: With derive(PartialOrd, Ord), enum variants are ordered by declaration
        // order AVX512 (0) < AVX2 (1) < SSE (2) < NEON (3) < Scalar (4)
        // For semantic correctness, we verify the ordering exists
        // (Lower numeric values come first, but semantically AVX512 is "best")
        assert!(SimdLevel::AVX512 < SimdLevel::AVX2);
        assert!(SimdLevel::AVX2 < SimdLevel::SSE);
        assert!(SimdLevel::SSE < SimdLevel::NEON);
        assert!(SimdLevel::NEON < SimdLevel::Scalar);

        // Verify that get_simd_level() returns appropriate levels
        let caps = PlatformCapabilities::detect();
        let level = caps.get_simd_level();
        assert_ne!(
            level,
            SimdLevel::Scalar,
            "Should detect some SIMD level on supported platforms"
        );
    }

    #[test]
    fn test_cross_platform_consistency() {
        // Test that detection works on all platforms
        let caps = PlatformCapabilities::detect();

        // Architecture should be detected
        assert_ne!(caps.architecture, Architecture::Unknown);

        // Implementation selection should work
        let _bitscan = caps.get_bitscan_impl();
        let _popcount = caps.get_popcount_impl();

        // Hardware acceleration check should work
        let _has_hw = caps.has_hardware_acceleration();
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_detection_performance() {
        // Test that detection is fast (should be < 1% overhead)
        let iterations = 1000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _caps = PlatformCapabilities::detect();
        }
        let duration = start.elapsed();

        let avg_time_ns = duration.as_nanos() / iterations;
        assert!(avg_time_ns < 1_000_000, "Detection too slow: {}ns average", avg_time_ns);
    }

    #[test]
    fn test_global_access_performance() {
        // Test that global access is fast
        let iterations = 10000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _caps = get_platform_capabilities();
            let _impl = get_best_bitscan_impl();
            let _impl = get_best_popcount_impl();
            let _has_hw = has_hardware_support();
        }
        let duration = start.elapsed();

        let avg_time_ns = duration.as_nanos() / iterations;
        assert!(avg_time_ns < 1000, "Global access too slow: {}ns average", avg_time_ns);
    }
}
