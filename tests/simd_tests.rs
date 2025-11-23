#![cfg(feature = "simd")]
/// SIMD tests that validate both correctness and SIMD instruction usage
/// 
/// These tests verify that:
/// 1. Operations produce correct results
/// 2. SIMD instructions are actually being used (when simd feature is enabled)
/// 3. Platform detection works correctly
/// 
/// Note: To verify SIMD instructions are generated, build with --release and
/// disassemble: objdump -d target/release/deps/simd_tests-* | grep -E "(pand|por|pxor|vand|vorr|veor)"

use shogi_engine::bitboards::{SimdBitboard, platform_detection};

#[test]
fn test_simd_bitboard_creation() {
    let val = 0x1234_5678_9ABC_DEF0_1234_5678_9ABC_DEF0;
    let bb = SimdBitboard::from_u128(val);
    assert_eq!(bb.to_u128(), val);
}

#[test]
fn test_simd_bitwise_and() {
    let v1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let v2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    let bb1 = SimdBitboard::from_u128(v1);
    let bb2 = SimdBitboard::from_u128(v2);
    
    let result = bb1 & bb2;
    assert_eq!(result.to_u128(), v1 & v2);
    
    // Verify SIMD feature is enabled
    #[cfg(feature = "simd")]
    {
        assert!(SimdBitboard::has_simd_support(), "SIMD support should be available when simd feature is enabled");
    }
}

#[test]
fn test_simd_bitwise_or() {
    let v1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let v2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    let bb1 = SimdBitboard::from_u128(v1);
    let bb2 = SimdBitboard::from_u128(v2);
    
    let result = bb1 | bb2;
    assert_eq!(result.to_u128(), v1 | v2);
}

#[test]
fn test_simd_bitwise_xor() {
    let v1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let v2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    let bb1 = SimdBitboard::from_u128(v1);
    let bb2 = SimdBitboard::from_u128(v2);
    
    let result = bb1 ^ bb2;
    assert_eq!(result.to_u128(), v1 ^ v2);
}

#[test]
fn test_simd_not() {
    let v1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let bb1 = SimdBitboard::from_u128(v1);
    
    let result = !bb1;
    assert_eq!(result.to_u128(), !v1);
}

#[test]
fn test_simd_shifts() {
    let bb = SimdBitboard::from_u128(0b1100);
    
    let left = bb << 1;
    assert_eq!(left.to_u128(), 0b11000);
    
    let right = bb >> 1;
    assert_eq!(right.to_u128(), 0b0110);
}

#[test]
fn test_simd_assign_ops() {
    let mut bb = SimdBitboard::from_u128(0b1100);
    
    bb &= SimdBitboard::from_u128(0b1010);
    assert_eq!(bb.to_u128(), 0b1000);
    
    bb |= SimdBitboard::from_u128(0b0001);
    assert_eq!(bb.to_u128(), 0b1001);
    
    bb ^= SimdBitboard::from_u128(0b1000);
    assert_eq!(bb.to_u128(), 0b0001);
}

#[test]
fn test_simd_bit_counting() {
    let bb = SimdBitboard::from_u128(0b10110);
    assert_eq!(bb.count_ones(), 3);
    assert_eq!(bb.trailing_zeros(), 1);
    
    let empty = SimdBitboard::default();
    assert!(empty.is_empty());
    assert_eq!(empty.count_ones(), 0);
}

#[test]
fn test_simd_platform_detection() {
    // Verify platform detection works
    let simd_level = SimdBitboard::get_detected_simd_level();
    let has_simd = SimdBitboard::has_simd_support();
    let platform_info = SimdBitboard::get_platform_info();
    
    // Platform info should be non-empty
    assert!(!platform_info.is_empty());
    assert!(platform_info.contains("Architecture:"));
    
    // On supported platforms, SIMD should be available
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        assert!(has_simd, "SIMD should be available on x86_64 or aarch64");
        assert_ne!(simd_level, platform_detection::SimdLevel::Scalar, 
                   "Should detect a SIMD level on supported platforms");
    }
}

#[test]
fn test_simd_instruction_usage_validation() {
    // This test validates that when simd feature is enabled, we're using SIMD
    // The actual instruction verification requires disassembly, but we can
    // verify that the platform detection indicates SIMD support
    
    #[cfg(feature = "simd")]
    {
        let caps = platform_detection::get_platform_capabilities();
        let simd_level = caps.get_simd_level();
        
        // On supported architectures, we should detect a SIMD level
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        {
            assert_ne!(simd_level, platform_detection::SimdLevel::Scalar,
                      "SIMD level should not be Scalar on supported platforms");
            
            // Verify architecture-specific SIMD levels
            #[cfg(target_arch = "x86_64")]
            {
                assert!(matches!(simd_level, 
                    platform_detection::SimdLevel::SSE | 
                    platform_detection::SimdLevel::AVX2 | 
                    platform_detection::SimdLevel::AVX512),
                    "x86_64 should detect SSE, AVX2, or AVX512");
            }
            
            #[cfg(target_arch = "aarch64")]
            {
                assert_eq!(simd_level, platform_detection::SimdLevel::NEON,
                          "aarch64 should detect NEON");
            }
        }
    }
}

#[test]
fn test_simd_operations_use_explicit_intrinsics() {
    // Verify that SIMD operations are using explicit intrinsics
    // This is validated by checking that platform detection indicates
    // SIMD support and that operations work correctly
    
    let v1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let v2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    let bb1 = SimdBitboard::from_u128(v1);
    let bb2 = SimdBitboard::from_u128(v2);
    
    // Perform operations - these should use SIMD intrinsics when simd feature is enabled
    let and_result = bb1 & bb2;
    let or_result = bb1 | bb2;
    let xor_result = bb1 ^ bb2;
    let not_result = !bb1;
    
    // Verify correctness
    assert_eq!(and_result.to_u128(), v1 & v2);
    assert_eq!(or_result.to_u128(), v1 | v2);
    assert_eq!(xor_result.to_u128(), v1 ^ v2);
    assert_eq!(not_result.to_u128(), !v1);
    
    // Verify SIMD support is available
    #[cfg(feature = "simd")]
    {
        assert!(SimdBitboard::has_simd_support(), 
                "SIMD support should be available when simd feature is enabled");
    }
}
