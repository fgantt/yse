#[cfg(test)]
mod tests {
    use shogi_engine::bitboards::SimdBitboard;

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
}
