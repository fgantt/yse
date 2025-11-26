//! ARM NEON optimization benchmarks
//!
//! This benchmark suite measures the performance improvements from ARM NEON optimizations
//! including optimized batch operations and tree reduction for combine_all.
//!
//! **Note**: These benchmarks require ARM64 hardware (Mac M-series, ARM servers) for validation.
//! They will compile on x86_64 but won't run the optimized NEON code paths.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::{AlignedBitboardArray, SimdBitboard};

fn generate_test_data<const N: usize>() -> (AlignedBitboardArray<N>, AlignedBitboardArray<N>) {
    let mut a = AlignedBitboardArray::new();
    let mut b = AlignedBitboardArray::new();

    for i in 0..N {
        // Create different patterns for each bitboard
        let pattern_a = 0x0F0F_0F0F_0F0F_0F0F_u128.wrapping_mul(i as u128 + 1);
        let pattern_b = 0x3333_3333_3333_3333_u128.wrapping_mul(i as u128 + 1);

        *a.get_mut(i) = SimdBitboard::from_u128(pattern_a);
        *b.get_mut(i) = SimdBitboard::from_u128(pattern_b);
    }

    (a, b)
}

fn generate_combine_all_data<const N: usize>() -> AlignedBitboardArray<N> {
    let mut arr = AlignedBitboardArray::new();

    for i in 0..N {
        // Create different patterns for each bitboard
        let pattern = 0x0F0F_0F0F_0F0F_0F0F_u128.wrapping_mul(i as u128 + 1);
        *arr.get_mut(i) = SimdBitboard::from_u128(pattern);
    }

    arr
}

/// Benchmark batch AND operations with different array sizes
fn bench_batch_and(c: &mut Criterion) {
    let mut group = c.benchmark_group("arm_neon_batch_and");

    // Benchmark each size separately to avoid const generic type issues
    {
        let (a, b) = generate_test_data::<4>();
        group.bench_with_input(BenchmarkId::new("optimized", 4), &4, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<4>();
                black_box(a.batch_and(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<8>();
        group.bench_with_input(BenchmarkId::new("optimized", 8), &8, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<8>();
                black_box(a.batch_and(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<16>();
        group.bench_with_input(BenchmarkId::new("optimized", 16), &16, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<16>();
                black_box(a.batch_and(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<32>();
        group.bench_with_input(BenchmarkId::new("optimized", 32), &32, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<32>();
                black_box(a.batch_and(&b))
            });
        });
    }

    group.finish();
}

/// Benchmark batch OR operations with different array sizes
fn bench_batch_or(c: &mut Criterion) {
    let mut group = c.benchmark_group("arm_neon_batch_or");

    {
        let (a, b) = generate_test_data::<4>();
        group.bench_with_input(BenchmarkId::new("optimized", 4), &4, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<4>();
                black_box(a.batch_or(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<8>();
        group.bench_with_input(BenchmarkId::new("optimized", 8), &8, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<8>();
                black_box(a.batch_or(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<16>();
        group.bench_with_input(BenchmarkId::new("optimized", 16), &16, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<16>();
                black_box(a.batch_or(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<32>();
        group.bench_with_input(BenchmarkId::new("optimized", 32), &32, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<32>();
                black_box(a.batch_or(&b))
            });
        });
    }

    group.finish();
}

/// Benchmark batch XOR operations with different array sizes
fn bench_batch_xor(c: &mut Criterion) {
    let mut group = c.benchmark_group("arm_neon_batch_xor");

    {
        let (a, b) = generate_test_data::<4>();
        group.bench_with_input(BenchmarkId::new("optimized", 4), &4, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<4>();
                black_box(a.batch_xor(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<8>();
        group.bench_with_input(BenchmarkId::new("optimized", 8), &8, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<8>();
                black_box(a.batch_xor(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<16>();
        group.bench_with_input(BenchmarkId::new("optimized", 16), &16, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<16>();
                black_box(a.batch_xor(&b))
            });
        });
    }

    {
        let (a, b) = generate_test_data::<32>();
        group.bench_with_input(BenchmarkId::new("optimized", 32), &32, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<32>();
                black_box(a.batch_xor(&b))
            });
        });
    }

    group.finish();
}

/// Benchmark combine_all operations with different array sizes
/// This tests the tree reduction optimization
fn bench_combine_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("arm_neon_combine_all");

    {
        let arr = generate_combine_all_data::<4>();
        group.bench_with_input(BenchmarkId::new("tree_reduction", 4), &4, |bencher, _| {
            bencher.iter(|| {
                let arr = generate_combine_all_data::<4>();
                black_box(arr.combine_all())
            });
        });
    }

    {
        let arr = generate_combine_all_data::<8>();
        group.bench_with_input(BenchmarkId::new("tree_reduction", 8), &8, |bencher, _| {
            bencher.iter(|| {
                let arr = generate_combine_all_data::<8>();
                black_box(arr.combine_all())
            });
        });
    }

    {
        let arr = generate_combine_all_data::<16>();
        group.bench_with_input(BenchmarkId::new("tree_reduction", 16), &16, |bencher, _| {
            bencher.iter(|| {
                let arr = generate_combine_all_data::<16>();
                black_box(arr.combine_all())
            });
        });
    }

    {
        let arr = generate_combine_all_data::<32>();
        group.bench_with_input(BenchmarkId::new("tree_reduction", 32), &32, |bencher, _| {
            bencher.iter(|| {
                let arr = generate_combine_all_data::<32>();
                black_box(arr.combine_all())
            });
        });
    }

    {
        let arr = generate_combine_all_data::<64>();
        group.bench_with_input(BenchmarkId::new("tree_reduction", 64), &64, |bencher, _| {
            bencher.iter(|| {
                let arr = generate_combine_all_data::<64>();
                black_box(arr.combine_all())
            });
        });
    }

    group.finish();
}

/// Benchmark comparison: optimized batch operations vs scalar fallback
/// This helps measure the improvement over scalar operations
fn bench_batch_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("arm_neon_batch_vs_scalar");

    // Size 8
    {
        // Scalar implementation
        group.bench_with_input(BenchmarkId::new("scalar", 8), &8, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<8>();
                let mut result = AlignedBitboardArray::<8>::new();
                for i in 0..8 {
                    *result.get_mut(i) = *a.get(i) & *b.get(i);
                }
                black_box(result)
            });
        });

        // Optimized NEON implementation
        group.bench_with_input(BenchmarkId::new("neon_optimized", 8), &8, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<8>();
                black_box(a.batch_and(&b))
            });
        });
    }

    // Size 16
    {
        // Scalar implementation
        group.bench_with_input(BenchmarkId::new("scalar", 16), &16, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<16>();
                let mut result = AlignedBitboardArray::<16>::new();
                for i in 0..16 {
                    *result.get_mut(i) = *a.get(i) & *b.get(i);
                }
                black_box(result)
            });
        });

        // Optimized NEON implementation
        group.bench_with_input(BenchmarkId::new("neon_optimized", 16), &16, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<16>();
                black_box(a.batch_and(&b))
            });
        });
    }

    // Size 32
    {
        // Scalar implementation
        group.bench_with_input(BenchmarkId::new("scalar", 32), &32, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<32>();
                let mut result = AlignedBitboardArray::<32>::new();
                for i in 0..32 {
                    *result.get_mut(i) = *a.get(i) & *b.get(i);
                }
                black_box(result)
            });
        });

        // Optimized NEON implementation
        group.bench_with_input(BenchmarkId::new("neon_optimized", 32), &32, |bencher, _| {
            bencher.iter(|| {
                let (a, b) = generate_test_data::<32>();
                black_box(a.batch_and(&b))
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_and,
    bench_batch_or,
    bench_batch_xor,
    bench_combine_all,
    bench_batch_vs_scalar
);

criterion_main!(benches);
