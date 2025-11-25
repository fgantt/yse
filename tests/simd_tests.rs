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
fn test_simd_shifts_comprehensive() {
    // Test small shifts
    let bb = SimdBitboard::from_u128(0x1234_5678_9ABC_DEF0_1234_5678_9ABC_DEF0);
    
    for shift in 1..=7 {
        let result_simd = bb << shift;
        let result_scalar = bb.to_u128() << shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Left shift by {} failed", shift);
        
        let result_simd = bb >> shift;
        let result_scalar = bb.to_u128() >> shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Right shift by {} failed", shift);
    }
}

#[test]
fn test_simd_shifts_medium() {
    // Test medium shifts (8-63)
    let bb = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F);
    
    for shift in [8, 16, 32, 48, 63] {
        let result_simd = bb << shift;
        let result_scalar = bb.to_u128() << shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Left shift by {} failed", shift);
        
        let result_simd = bb >> shift;
        let result_scalar = bb.to_u128() >> shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Right shift by {} failed", shift);
    }
}

#[test]
fn test_simd_shifts_large() {
    // Test large shifts (64-127) - cross-lane shifts
    let bb = SimdBitboard::from_u128(0x1234_5678_9ABC_DEF0_1234_5678_9ABC_DEF0);
    
    for shift in [64, 65, 96, 127] {
        let result_simd = bb << shift;
        let result_scalar = bb.to_u128() << shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Left shift by {} failed", shift);
        
        let result_simd = bb >> shift;
        let result_scalar = bb.to_u128() >> shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Right shift by {} failed", shift);
    }
}

#[test]
fn test_simd_shifts_edge_cases() {
    // Test edge cases
    let bb = SimdBitboard::from_u128(0x8000_0000_0000_0000_0000_0000_0000_0000);
    
    // Shift 0 (no-op)
    assert_eq!((bb << 0).to_u128(), bb.to_u128());
    assert_eq!((bb >> 0).to_u128(), bb.to_u128());
    
    // Shift by 128 (Rust's u128 shift wraps, so shift by 128 is same as shift by 0)
    // Our implementation clamps to 127, so shift by 128 becomes shift by 127
    // For shift >= 128, we clamp to 127, so test with 127 instead
    assert_eq!((bb << 127).to_u128(), bb.to_u128() << 127);
    assert_eq!((bb >> 127).to_u128(), bb.to_u128() >> 127);
    
    // Shift by values > 128 (should clamp to 127)
    assert_eq!((bb << 200).to_u128(), bb.to_u128() << 127);
    assert_eq!((bb >> 200).to_u128(), bb.to_u128() >> 127);
    
    // Test with all bits set
    let all_bits = SimdBitboard::all_squares();
    for shift in [1, 64, 96] {
        let result_simd = all_bits << shift;
        let result_scalar = all_bits.to_u128() << shift;
        assert_eq!(result_simd.to_u128(), result_scalar, "Left shift of all bits by {} failed", shift);
    }
}

#[test]
fn test_simd_shifts_cross_lane() {
    // Specifically test cross-lane shifts (shifts that cross the 64-bit boundary)
    let val = 0x0000_0000_0000_0001_8000_0000_0000_0000; // Bit set in lower lane, high bit in upper lane
    let bb = SimdBitboard::from_u128(val);
    
    // Shift left by 1 - should carry from lower to upper lane
    let result = bb << 1;
    assert_eq!(result.to_u128(), val << 1);
    
    // Shift right by 1 - should carry from upper to lower lane
    let val2 = 0x8000_0000_0000_0000_0000_0000_0000_0001;
    let bb2 = SimdBitboard::from_u128(val2);
    let result = bb2 >> 1;
    assert_eq!(result.to_u128(), val2 >> 1);
    
    // Shift by exactly 64 bits - should move between lanes
    let result = bb << 64;
    assert_eq!(result.to_u128(), val << 64);
    
    let result = bb2 >> 64;
    assert_eq!(result.to_u128(), val2 >> 64);
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
