#![cfg(feature = "simd")]
use shogi_engine::bitboards::{platform_detection, SimdBitboard};

#[test]
fn test_simd_platform_detection_integration() {
    // Test that SimdBitboard can access platform detection
    let simd_level = SimdBitboard::get_detected_simd_level();
    let has_simd = SimdBitboard::has_simd_support();
    let platform_info = SimdBitboard::get_platform_info();

    // Verify that we get valid results
    assert!(!platform_info.is_empty());
    assert!(platform_info.contains("Architecture:"));

    // On supported platforms, we should have SIMD support
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        assert!(has_simd, "Should have SIMD support on x86_64 or aarch64");
        assert_ne!(
            simd_level,
            platform_detection::SimdLevel::Scalar,
            "Should detect a SIMD level on supported platforms"
        );
    }
}

#[test]
fn test_platform_capabilities_for_simd() {
    let caps = platform_detection::get_platform_capabilities();
    let simd_level = caps.get_simd_level();
    let recommended = caps.get_recommended_simd_impl();

    // Verify we get valid recommendations
    assert!(!recommended.is_empty());

    // Verify SIMD level matches architecture
    match caps.architecture {
        platform_detection::Architecture::X86_64 => {
            assert!(matches!(
                simd_level,
                platform_detection::SimdLevel::SSE
                    | platform_detection::SimdLevel::AVX2
                    | platform_detection::SimdLevel::AVX512
            ));
        }
        platform_detection::Architecture::ARM => {
            assert!(matches!(
                simd_level,
                platform_detection::SimdLevel::NEON | platform_detection::SimdLevel::Scalar
            ));
        }
        platform_detection::Architecture::Unknown => {
            assert_eq!(simd_level, platform_detection::SimdLevel::Scalar);
        }
    }
}

#[test]
fn test_avx2_detection_integration() {
    let caps = platform_detection::get_platform_capabilities();

    #[cfg(target_arch = "x86_64")]
    {
        // AVX2 detection should work (may be true or false depending on CPU)
        let should_use = caps.should_use_avx2();
        assert_eq!(should_use, caps.has_avx2, "should_use_avx2 should match has_avx2");

        // If AVX2 is available, it should be recommended over SSE
        if caps.has_avx2 {
            let simd_level = caps.get_simd_level();
            assert!(matches!(
                simd_level,
                platform_detection::SimdLevel::AVX2 | platform_detection::SimdLevel::AVX512
            ));
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        assert!(!caps.should_use_avx2(), "AVX2 should not be recommended on non-x86_64");
    }
}

#[test]
fn test_avx512_detection_integration() {
    let caps = platform_detection::get_platform_capabilities();

    #[cfg(target_arch = "x86_64")]
    {
        // AVX-512 detection should work
        let should_use = caps.should_use_avx512();
        assert_eq!(should_use, caps.has_avx512, "should_use_avx512 should match has_avx512");

        // AVX-512 requires AVX2 as prerequisite
        if caps.has_avx512 {
            assert!(caps.has_avx2, "AVX-512 requires AVX2 support");
            let simd_level = caps.get_simd_level();
            assert_eq!(simd_level, platform_detection::SimdLevel::AVX512);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        assert!(!caps.should_use_avx512(), "AVX-512 should not be recommended on non-x86_64");
    }
}

#[test]
fn test_neon_detection_integration() {
    let caps = platform_detection::get_platform_capabilities();

    #[cfg(target_arch = "aarch64")]
    {
        // NEON should always be available on aarch64
        assert!(caps.has_neon, "NEON should be available on aarch64");
        let simd_level = caps.get_simd_level();
        assert_eq!(simd_level, platform_detection::SimdLevel::NEON);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        assert!(!caps.has_neon, "NEON should not be available on non-ARM64");
    }
}

#[test]
fn test_simd_operations_with_platform_detection() {
    // Test that SIMD operations work correctly with platform detection
    let bb1 = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F);
    let bb2 = SimdBitboard::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333);

    // Perform SIMD operations
    let result_and = bb1 & bb2;
    let result_or = bb1 | bb2;
    let result_xor = bb1 ^ bb2;
    let result_not = !bb1;

    // Verify correctness (operations should work regardless of detected SIMD level)
    assert_eq!(
        result_and.to_u128(),
        0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F & 0x3333_3333_3333_3333_3333_3333_3333_3333
    );
    assert_eq!(
        result_or.to_u128(),
        0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F | 0x3333_3333_3333_3333_3333_3333_3333_3333
    );
    assert_eq!(
        result_xor.to_u128(),
        0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F ^ 0x3333_3333_3333_3333_3333_3333_3333_3333
    );
    assert_eq!(result_not.to_u128(), !0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F);

    // Verify platform detection still works after operations
    let simd_level = SimdBitboard::get_detected_simd_level();
    assert_ne!(
        simd_level,
        platform_detection::SimdLevel::Scalar,
        "Should detect SIMD level on supported platforms"
    );
}
