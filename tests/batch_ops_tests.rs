#![cfg(feature = "simd")]
use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, SimdBitboard};

#[test]
fn test_aligned_array_creation() {
    let arr = AlignedBitboardArray::<4>::new();
    assert_eq!(arr.len(), 4);
    assert!(!arr.is_empty());

    for i in 0..4 {
        assert!(arr.get(i).is_empty());
    }
}

#[test]
fn test_aligned_array_from_slice() {
    let bitboards = [
        SimdBitboard::from_u128(0x1111),
        SimdBitboard::from_u128(0x2222),
        SimdBitboard::from_u128(0x3333),
        SimdBitboard::from_u128(0x4444),
    ];

    let arr = AlignedBitboardArray::<4>::from_slice(&bitboards);
    assert_eq!(arr.len(), 4);

    for i in 0..4 {
        assert_eq!(arr.get(i).to_u128(), bitboards[i].to_u128());
    }
}

#[test]
fn test_aligned_array_get_set() {
    let mut arr = AlignedBitboardArray::<4>::new();

    arr.set(0, SimdBitboard::from_u128(0xAAAA));
    arr.set(1, SimdBitboard::from_u128(0xBBBB));
    arr.set(2, SimdBitboard::from_u128(0xCCCC));
    arr.set(3, SimdBitboard::from_u128(0xDDDD));

    assert_eq!(arr.get(0).to_u128(), 0xAAAA);
    assert_eq!(arr.get(1).to_u128(), 0xBBBB);
    assert_eq!(arr.get(2).to_u128(), 0xCCCC);
    assert_eq!(arr.get(3).to_u128(), 0xDDDD);
}

#[test]
fn test_batch_and_small() {
    let a = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F),
        SimdBitboard::from_u128(0x3333),
        SimdBitboard::from_u128(0x5555),
        SimdBitboard::from_u128(0xAAAA),
    ]);

    let b = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0xFFFF),
        SimdBitboard::from_u128(0x0000),
        SimdBitboard::from_u128(0xFFFF),
        SimdBitboard::from_u128(0x0000),
    ]);

    let result = a.batch_and(&b);

    assert_eq!(result.get(0).to_u128(), 0x0F0F & 0xFFFF);
    assert_eq!(result.get(1).to_u128(), 0x3333 & 0x0000);
    assert_eq!(result.get(2).to_u128(), 0x5555 & 0xFFFF);
    assert_eq!(result.get(3).to_u128(), 0xAAAA & 0x0000);
}

#[test]
fn test_batch_or_small() {
    let a = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F),
        SimdBitboard::from_u128(0x3333),
        SimdBitboard::from_u128(0x5555),
        SimdBitboard::from_u128(0xAAAA),
    ]);

    let b = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0xFFFF),
        SimdBitboard::from_u128(0x0000),
        SimdBitboard::from_u128(0xFFFF),
        SimdBitboard::from_u128(0x0000),
    ]);

    let result = a.batch_or(&b);

    assert_eq!(result.get(0).to_u128(), 0x0F0F | 0xFFFF);
    assert_eq!(result.get(1).to_u128(), 0x3333 | 0x0000);
    assert_eq!(result.get(2).to_u128(), 0x5555 | 0xFFFF);
    assert_eq!(result.get(3).to_u128(), 0xAAAA | 0x0000);
}

#[test]
fn test_batch_xor_small() {
    let a = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0F0F),
        SimdBitboard::from_u128(0x3333),
        SimdBitboard::from_u128(0x5555),
        SimdBitboard::from_u128(0xAAAA),
    ]);

    let b = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0xFFFF),
        SimdBitboard::from_u128(0x0000),
        SimdBitboard::from_u128(0xFFFF),
        SimdBitboard::from_u128(0x0000),
    ]);

    let result = a.batch_xor(&b);

    assert_eq!(result.get(0).to_u128(), 0x0F0F ^ 0xFFFF);
    assert_eq!(result.get(1).to_u128(), 0x3333 ^ 0x0000);
    assert_eq!(result.get(2).to_u128(), 0x5555 ^ 0xFFFF);
    assert_eq!(result.get(3).to_u128(), 0xAAAA ^ 0x0000);
}

#[test]
fn test_batch_and_medium() {
    // Test with 8 bitboards
    let mut a_data = [SimdBitboard::empty(); 8];
    let mut b_data = [SimdBitboard::empty(); 8];

    for i in 0..8 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F << (i * 4));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333 << (i * 4));
    }

    let a = AlignedBitboardArray::<8>::from_slice(&a_data);
    let b = AlignedBitboardArray::<8>::from_slice(&b_data);

    let result = a.batch_and(&b);

    for i in 0..8 {
        let expected = a_data[i] & b_data[i];
        assert_eq!(result.get(i).to_u128(), expected.to_u128(), "Mismatch at index {}", i);
    }
}

#[test]
fn test_batch_or_medium() {
    let mut a_data = [SimdBitboard::empty(); 8];
    let mut b_data = [SimdBitboard::empty(); 8];

    for i in 0..8 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F << (i * 4));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333 << (i * 4));
    }

    let a = AlignedBitboardArray::<8>::from_slice(&a_data);
    let b = AlignedBitboardArray::<8>::from_slice(&b_data);

    let result = a.batch_or(&b);

    for i in 0..8 {
        let expected = a_data[i] | b_data[i];
        assert_eq!(result.get(i).to_u128(), expected.to_u128(), "Mismatch at index {}", i);
    }
}

#[test]
fn test_batch_xor_medium() {
    let mut a_data = [SimdBitboard::empty(); 8];
    let mut b_data = [SimdBitboard::empty(); 8];

    for i in 0..8 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F << (i * 4));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333 << (i * 4));
    }

    let a = AlignedBitboardArray::<8>::from_slice(&a_data);
    let b = AlignedBitboardArray::<8>::from_slice(&b_data);

    let result = a.batch_xor(&b);

    for i in 0..8 {
        let expected = a_data[i] ^ b_data[i];
        assert_eq!(result.get(i).to_u128(), expected.to_u128(), "Mismatch at index {}", i);
    }
}

#[test]
fn test_batch_and_large() {
    // Test with 16 bitboards
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];

    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }

    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);

    let result = a.batch_and(&b);

    for i in 0..16 {
        let expected = a_data[i] & b_data[i];
        assert_eq!(result.get(i).to_u128(), expected.to_u128(), "Mismatch at index {}", i);
    }
}

#[test]
fn test_batch_operations_correctness() {
    // Comprehensive correctness test with various sizes
    for size in [1, 2, 4, 8, 16, 32] {
        match size {
            1 => test_batch_size::<1>(),
            2 => test_batch_size::<2>(),
            4 => test_batch_size::<4>(),
            8 => test_batch_size::<8>(),
            16 => test_batch_size::<16>(),
            32 => test_batch_size::<32>(),
            _ => unreachable!(),
        }
    }
}

fn test_batch_size<const N: usize>() {
    let mut a_data = [SimdBitboard::empty(); N];
    let mut b_data = [SimdBitboard::empty(); N];

    for i in 0..N {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ ((i * 17) as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ ((i * 23) as u128));
    }

    let a = AlignedBitboardArray::<N>::from_slice(&a_data);
    let b = AlignedBitboardArray::<N>::from_slice(&b_data);

    // Test AND
    let result_and = a.batch_and(&b);
    for i in 0..N {
        let expected = a_data[i] & b_data[i];
        assert_eq!(
            result_and.get(i).to_u128(),
            expected.to_u128(),
            "AND mismatch at index {} for size {}",
            i,
            N
        );
    }

    // Test OR
    let result_or = a.batch_or(&b);
    for i in 0..N {
        let expected = a_data[i] | b_data[i];
        assert_eq!(
            result_or.get(i).to_u128(),
            expected.to_u128(),
            "OR mismatch at index {} for size {}",
            i,
            N
        );
    }

    // Test XOR
    let result_xor = a.batch_xor(&b);
    for i in 0..N {
        let expected = a_data[i] ^ b_data[i];
        assert_eq!(
            result_xor.get(i).to_u128(),
            expected.to_u128(),
            "XOR mismatch at index {} for size {}",
            i,
            N
        );
    }
}

#[test]
fn test_batch_operations_empty() {
    let a = AlignedBitboardArray::<4>::new();
    let b = AlignedBitboardArray::<4>::new();

    let result_and = a.batch_and(&b);
    let result_or = a.batch_or(&b);
    let result_xor = a.batch_xor(&b);

    for i in 0..4 {
        assert!(result_and.get(i).is_empty());
        assert!(result_or.get(i).is_empty());
        assert!(result_xor.get(i).is_empty());
    }
}

#[test]
fn test_batch_operations_all_ones() {
    let all_ones = SimdBitboard::all_squares();
    let a = AlignedBitboardArray::<4>::from_slice(&[all_ones; 4]);
    let b = AlignedBitboardArray::<4>::from_slice(&[all_ones; 4]);

    let result_and = a.batch_and(&b);
    let result_or = a.batch_or(&b);
    let result_xor = a.batch_xor(&b);

    for i in 0..4 {
        assert_eq!(result_and.get(i).to_u128(), all_ones.to_u128());
        assert_eq!(result_or.get(i).to_u128(), all_ones.to_u128());
        assert_eq!(result_xor.get(i).to_u128(), 0); // XOR of same values is 0
    }
}

#[test]
fn test_aligned_array_clone() {
    let a = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x1111),
        SimdBitboard::from_u128(0x2222),
        SimdBitboard::from_u128(0x3333),
        SimdBitboard::from_u128(0x4444),
    ]);

    let cloned = a.clone();
    assert_eq!(a, cloned);

    for i in 0..4 {
        assert_eq!(a.get(i).to_u128(), cloned.get(i).to_u128());
    }
}

#[test]
fn test_aligned_array_debug() {
    let arr = AlignedBitboardArray::<4>::new();
    let debug_str = format!("{:?}", arr);
    assert!(debug_str.contains("AlignedBitboardArray"));
    assert!(debug_str.contains("len"));
}

#[test]
fn test_combine_all() {
    // Test combining multiple attack patterns
    let attacks = AlignedBitboardArray::<4>::from_slice(&[
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_0000_1111),
        SimdBitboard::from_u128(0x0000_0000_0000_0000_0000_0000_1111_0000),
        SimdBitboard::from_u128(0x0000_0000_0000_0000_1111_0000_0000_0000),
        SimdBitboard::from_u128(0x0000_0000_1111_0000_0000_0000_0000_0000),
    ]);

    let combined = attacks.combine_all();
    let expected = SimdBitboard::from_u128(0x0000_0000_1111_0000_1111_0000_1111_1111);

    assert_eq!(combined.to_u128(), expected.to_u128());
}

#[test]
fn test_combine_all_empty() {
    let attacks = AlignedBitboardArray::<4>::new();
    let combined = attacks.combine_all();
    assert!(combined.is_empty());
}

#[test]
fn test_combine_all_single() {
    let attacks = AlignedBitboardArray::<1>::from_slice(&[SimdBitboard::from_u128(0xAAAA)]);

    let combined = attacks.combine_all();
    assert_eq!(combined.to_u128(), 0xAAAA);
}

#[test]
fn test_avx2_correctness() {
    // Test that AVX2 implementation produces same results as scalar
    // This test validates that AVX2 batch operations are correct
    // even when processing 2 bitboards simultaneously

    use shogi_engine::bitboards::platform_detection;

    let caps = platform_detection::get_platform_capabilities();

    // Test with various sizes, including odd sizes to test edge cases
    for size in [2, 3, 4, 5, 8, 9, 16, 17, 32] {
        match size {
            2 => test_avx2_size::<2>(),
            3 => test_avx2_size::<3>(),
            4 => test_avx2_size::<4>(),
            5 => test_avx2_size::<5>(),
            8 => test_avx2_size::<8>(),
            9 => test_avx2_size::<9>(),
            16 => test_avx2_size::<16>(),
            17 => test_avx2_size::<17>(),
            32 => test_avx2_size::<32>(),
            _ => unreachable!(),
        }
    }

    // Test combine_all with AVX2
    test_avx2_combine_all();

    if caps.has_avx2 {
        println!("AVX2 is available - tested AVX2 implementation");
    } else {
        println!("AVX2 not available - tested SSE fallback");
    }
}

fn test_avx2_size<const N: usize>() {
    let mut a_data = [SimdBitboard::empty(); N];
    let mut b_data = [SimdBitboard::empty(); N];

    // Create diverse test data
    for i in 0..N {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ ((i * 17) as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ ((i * 23) as u128));
    }

    let a = AlignedBitboardArray::<N>::from_slice(&a_data);
    let b = AlignedBitboardArray::<N>::from_slice(&b_data);

    // Test batch operations (will use AVX2 if available, SSE otherwise)
    let result_and = a.batch_and(&b);
    let result_or = a.batch_or(&b);
    let result_xor = a.batch_xor(&b);

    // Verify correctness by comparing with scalar operations
    for i in 0..N {
        let expected_and = a_data[i] & b_data[i];
        let expected_or = a_data[i] | b_data[i];
        let expected_xor = a_data[i] ^ b_data[i];

        assert_eq!(
            result_and.get(i).to_u128(),
            expected_and.to_u128(),
            "AVX2/SSE AND mismatch at index {} for size {}",
            i,
            N
        );
        assert_eq!(
            result_or.get(i).to_u128(),
            expected_or.to_u128(),
            "AVX2/SSE OR mismatch at index {} for size {}",
            i,
            N
        );
        assert_eq!(
            result_xor.get(i).to_u128(),
            expected_xor.to_u128(),
            "AVX2/SSE XOR mismatch at index {} for size {}",
            i,
            N
        );
    }
}

fn test_avx2_combine_all() {
    // Test combine_all with various sizes
    for size in [2, 3, 4, 5, 8, 16, 32] {
        match size {
            2 => test_combine_all_size::<2>(),
            3 => test_combine_all_size::<3>(),
            4 => test_combine_all_size::<4>(),
            5 => test_combine_all_size::<5>(),
            8 => test_combine_all_size::<8>(),
            16 => test_combine_all_size::<16>(),
            32 => test_combine_all_size::<32>(),
            _ => unreachable!(),
        }
    }
}

fn test_combine_all_size<const N: usize>() {
    let mut data = [SimdBitboard::empty(); N];

    // Create diverse attack patterns
    for i in 0..N {
        data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ ((i as u128) << (i % 64)));
    }

    let arr = AlignedBitboardArray::<N>::from_slice(&data);

    // Test combine_all (will use AVX2 if available, SSE otherwise)
    let combined = arr.combine_all();

    // Verify correctness by comparing with scalar OR reduction
    let mut expected = SimdBitboard::empty();
    for i in 0..N {
        expected = expected | data[i];
    }

    assert_eq!(
        combined.to_u128(),
        expected.to_u128(),
        "AVX2/SSE combine_all mismatch for size {}",
        N
    );
}
