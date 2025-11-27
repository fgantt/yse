//! AVX-512 Performance Benchmarks
//!
//! This benchmark suite compares AVX-512 vs AVX2 vs SSE performance for batch
//! operations.
//!
//! **Note**: These benchmarks require AVX-512 capable hardware to run. On
//! systems without AVX-512, the benchmarks will fall back to AVX2 or SSE
//! automatically.
//!
//! Run with: `cargo bench --features simd --bench avx512_benchmarks`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, SimdBitboard};

/// Generate test data for batch operations
fn generate_test_data<const N: usize>() -> (AlignedBitboardArray<N>, AlignedBitboardArray<N>) {
    let mut a = AlignedBitboardArray::new();
    let mut b = AlignedBitboardArray::new();

    for i in 0..N {
        // Create different patterns for each bitboard
        let pattern_a = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_u128.wrapping_mul(i as u128 + 1);
        let pattern_b = 0x3333_3333_3333_3333_3333_3333_3333_3333_u128.wrapping_mul(i as u128 + 1);

        a.set(i, SimdBitboard::from_u128(pattern_a));
        b.set(i, SimdBitboard::from_u128(pattern_b));
    }

    (a, b)
}

fn bench_batch_and_avx512_vs_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_and_avx512_vs_avx2");
    group.sample_size(1000);

    // Test different batch sizes
    for size in [4, 8, 16, 32, 64] {
        let (a, b) = generate_test_data::<64>();
        let a_slice = &a.as_slice()[..size];
        let b_slice = &b.as_slice()[..size];

        // Create arrays of the correct size
        let a_arr = match size {
            4 => {
                let mut arr = AlignedBitboardArray::<4>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..4]);
                arr
            }
            8 => {
                let mut arr = AlignedBitboardArray::<8>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..8]);
                arr
            }
            16 => {
                let mut arr = AlignedBitboardArray::<16>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..16]);
                arr
            }
            32 => {
                let mut arr = AlignedBitboardArray::<32>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..32]);
                arr
            }
            64 => {
                let mut arr = AlignedBitboardArray::<64>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..64]);
                arr
            }
            _ => unreachable!(),
        };

        let b_arr = match size {
            4 => {
                let mut arr = AlignedBitboardArray::<4>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..4]);
                arr
            }
            8 => {
                let mut arr = AlignedBitboardArray::<8>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..8]);
                arr
            }
            16 => {
                let mut arr = AlignedBitboardArray::<16>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..16]);
                arr
            }
            32 => {
                let mut arr = AlignedBitboardArray::<32>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..32]);
                arr
            }
            64 => {
                let mut arr = AlignedBitboardArray::<64>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..64]);
                arr
            }
            _ => unreachable!(),
        };

        match size {
            4 => {
                group.bench_with_input(BenchmarkId::new("size_4", size), &size, |b, _| {
                    b.iter(|| {
                        let a = black_box(&a_arr);
                        let b = black_box(&b_arr);
                        black_box(a.batch_and(b))
                    });
                });
            }
            8 => {
                group.bench_with_input(BenchmarkId::new("size_8", size), &size, |b, _| {
                    b.iter(|| {
                        let a = black_box(&a_arr);
                        let b = black_box(&b_arr);
                        black_box(a.batch_and(b))
                    });
                });
            }
            16 => {
                group.bench_with_input(BenchmarkId::new("size_16", size), &size, |b, _| {
                    b.iter(|| {
                        let a = black_box(&a_arr);
                        let b = black_box(&b_arr);
                        black_box(a.batch_and(b))
                    });
                });
            }
            32 => {
                group.bench_with_input(BenchmarkId::new("size_32", size), &size, |b, _| {
                    b.iter(|| {
                        let a = black_box(&a_arr);
                        let b = black_box(&b_arr);
                        black_box(a.batch_and(b))
                    });
                });
            }
            64 => {
                group.bench_with_input(BenchmarkId::new("size_64", size), &size, |b, _| {
                    b.iter(|| {
                        let a = black_box(&a_arr);
                        let b = black_box(&b_arr);
                        black_box(a.batch_and(b))
                    });
                });
            }
            _ => unreachable!(),
        }
    }

    group.finish();
}

fn bench_batch_or_avx512_vs_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_or_avx512_vs_avx2");
    group.sample_size(1000);

    for size in [16, 32, 64] {
        let (a, b) = generate_test_data::<64>();
        let a_slice = &a.as_slice()[..size];
        let b_slice = &b.as_slice()[..size];

        let a_arr = match size {
            16 => {
                let mut arr = AlignedBitboardArray::<16>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..16]);
                arr
            }
            32 => {
                let mut arr = AlignedBitboardArray::<32>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..32]);
                arr
            }
            64 => {
                let mut arr = AlignedBitboardArray::<64>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..64]);
                arr
            }
            _ => unreachable!(),
        };

        let b_arr = match size {
            16 => {
                let mut arr = AlignedBitboardArray::<16>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..16]);
                arr
            }
            32 => {
                let mut arr = AlignedBitboardArray::<32>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..32]);
                arr
            }
            64 => {
                let mut arr = AlignedBitboardArray::<64>::new();
                arr.as_mut_slice().copy_from_slice(&b_slice[..64]);
                arr
            }
            _ => unreachable!(),
        };

        group.bench_with_input(BenchmarkId::new("size", size), &size, |b, _| {
            b.iter(|| {
                let a = black_box(&a_arr);
                let b = black_box(&b_arr);
                black_box(a.batch_or(b))
            });
        });
    }

    group.finish();
}

fn bench_combine_all_avx512_vs_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("combine_all_avx512_vs_avx2");
    group.sample_size(1000);

    for size in [16, 32, 64] {
        let (a, _) = generate_test_data::<64>();
        let a_slice = &a.as_slice()[..size];

        let arr = match size {
            16 => {
                let mut arr = AlignedBitboardArray::<16>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..16]);
                arr
            }
            32 => {
                let mut arr = AlignedBitboardArray::<32>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..32]);
                arr
            }
            64 => {
                let mut arr = AlignedBitboardArray::<64>::new();
                arr.as_mut_slice().copy_from_slice(&a_slice[..64]);
                arr
            }
            _ => unreachable!(),
        };

        group.bench_with_input(BenchmarkId::new("size", size), &size, |b, _| {
            b.iter(|| {
                let arr = black_box(&arr);
                black_box(arr.combine_all())
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_and_avx512_vs_avx2,
    bench_batch_or_avx512_vs_avx2,
    bench_combine_all_avx512_vs_avx2
);
criterion_main!(benches);
