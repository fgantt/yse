use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::{SimdBitboard, batch_ops::AlignedBitboardArray, platform_detection};

/// Scalar implementation for comparison
fn scalar_batch_and<const N: usize>(
    a: &[SimdBitboard; N],
    b: &[SimdBitboard; N],
) -> [SimdBitboard; N] {
    let mut result = [SimdBitboard::empty(); N];
    for i in 0..N {
        result[i] = a[i] & b[i];
    }
    result
}

fn scalar_batch_or<const N: usize>(
    a: &[SimdBitboard; N],
    b: &[SimdBitboard; N],
) -> [SimdBitboard; N] {
    let mut result = [SimdBitboard::empty(); N];
    for i in 0..N {
        result[i] = a[i] | b[i];
    }
    result
}

fn scalar_batch_xor<const N: usize>(
    a: &[SimdBitboard; N],
    b: &[SimdBitboard; N],
) -> [SimdBitboard; N] {
    let mut result = [SimdBitboard::empty(); N];
    for i in 0..N {
        result[i] = a[i] ^ b[i];
    }
    result
}

/// Scalar implementation of combine_all for comparison
fn scalar_combine_all<const N: usize>(arr: &AlignedBitboardArray<N>) -> SimdBitboard {
    let mut result = SimdBitboard::empty();
    for i in 0..N {
        result = result | *arr.get(i);
    }
    result
}

fn bench_batch_and(c: &mut Criterion) {
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];
    
    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);

    let mut group = c.benchmark_group("batch_and");
    group.sample_size(1000);

    // Benchmark SIMD batch AND
    group.bench_function("simd_batch_and_16", |bencher| {
        bencher.iter(|| {
            black_box(a.batch_and(&b))
        });
    });

    // Benchmark scalar batch AND
    group.bench_function("scalar_batch_and_16", |bencher| {
        bencher.iter(|| {
            black_box(scalar_batch_and(&a_data, &b_data))
        });
    });

    group.finish();
}

fn bench_batch_or(c: &mut Criterion) {
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];
    
    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);

    let mut group = c.benchmark_group("batch_or");
    group.sample_size(1000);

    group.bench_function("simd_batch_or_16", |bencher| {
        bencher.iter(|| {
            black_box(a.batch_or(&b))
        });
    });

    group.bench_function("scalar_batch_or_16", |bencher| {
        bencher.iter(|| {
            black_box(scalar_batch_or(&a_data, &b_data))
        });
    });

    group.finish();
}

fn bench_batch_xor(c: &mut Criterion) {
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];
    
    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);

    let mut group = c.benchmark_group("batch_xor");
    group.sample_size(1000);

    group.bench_function("simd_batch_xor_16", |bencher| {
        bencher.iter(|| {
            black_box(a.batch_xor(&b))
        });
    });

    group.bench_function("scalar_batch_xor_16", |bencher| {
        bencher.iter(|| {
            black_box(scalar_batch_xor(&a_data, &b_data))
        });
    });

    group.finish();
}

fn bench_batch_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_various_sizes");
    group.sample_size(500);

    // Test different array sizes
    for size in [4, 8, 16, 32] {
        match size {
            4 => bench_batch_size::<4>(&mut group),
            8 => bench_batch_size::<8>(&mut group),
            16 => bench_batch_size::<16>(&mut group),
            32 => bench_batch_size::<32>(&mut group),
            _ => unreachable!(),
        }
    }

    group.finish();
}

fn bench_batch_size<const N: usize>(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    let mut a_data = [SimdBitboard::empty(); N];
    let mut b_data = [SimdBitboard::empty(); N];
    
    for i in 0..N {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<N>::from_slice(&a_data);
    let b = AlignedBitboardArray::<N>::from_slice(&b_data);

    group.bench_function(&format!("simd_batch_and_{}", N), |bencher| {
        bencher.iter(|| {
            black_box(a.batch_and(&b))
        });
    });

    group.bench_function(&format!("scalar_batch_and_{}", N), |bencher| {
        bencher.iter(|| {
            black_box(scalar_batch_and(&a_data, &b_data))
        });
    });
}

fn bench_combine_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("combine_all");
    group.sample_size(1000);

    // Test with different array sizes
    for size in [4, 8, 16, 32] {
        match size {
            4 => bench_combine_all_size::<4>(&mut group),
            8 => bench_combine_all_size::<8>(&mut group),
            16 => bench_combine_all_size::<16>(&mut group),
            32 => bench_combine_all_size::<32>(&mut group),
            _ => unreachable!(),
        }
    }

    group.finish();
}

fn bench_combine_all_size<const N: usize>(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    let mut data = [SimdBitboard::empty(); N];
    
    // Create diverse attack patterns
    for i in 0..N {
        data[i] = SimdBitboard::from_u128(
            0x0F0F_0F0F_0F0F_0F0F ^ ((i as u128) << (i % 64))
        );
    }
    
    let arr = AlignedBitboardArray::<N>::from_slice(&data);

    // Benchmark SIMD combine_all
    group.bench_function(&format!("simd_combine_all_{}", N), |bencher| {
        bencher.iter(|| {
            black_box(arr.combine_all())
        });
    });

    // Benchmark scalar combine_all
    group.bench_function(&format!("scalar_combine_all_{}", N), |bencher| {
        bencher.iter(|| {
            black_box(scalar_combine_all(&arr))
        });
    });
}

/// Benchmark AVX2 vs SSE performance for batch operations
/// This benchmark will automatically use AVX2 if available, otherwise SSE
/// The runtime selection is handled by the batch_ops module
fn bench_avx2_vs_sse(c: &mut Criterion) {
    let caps = platform_detection::get_platform_capabilities();
    let simd_level = caps.get_simd_level();
    
    let mut group = c.benchmark_group("avx2_vs_sse");
    group.sample_size(1000);
    
    // Print which SIMD level is being used
    eprintln!("SIMD Level: {:?}, AVX2 Available: {}", simd_level, caps.has_avx2);
    
    let mut a_data = [SimdBitboard::empty(); 16];
    let mut b_data = [SimdBitboard::empty(); 16];
    
    for i in 0..16 {
        a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
        b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
    }
    
    let a = AlignedBitboardArray::<16>::from_slice(&a_data);
    let b = AlignedBitboardArray::<16>::from_slice(&b_data);
    
    // These will automatically use AVX2 if available, SSE otherwise
    group.bench_function("batch_and_auto_select", |bencher| {
        bencher.iter(|| {
            black_box(a.batch_and(&b))
        });
    });
    
    group.bench_function("batch_or_auto_select", |bencher| {
        bencher.iter(|| {
            black_box(a.batch_or(&b))
        });
    });
    
    group.bench_function("batch_xor_auto_select", |bencher| {
        bencher.iter(|| {
            black_box(a.batch_xor(&b))
        });
    });
    
    // Test combine_all with AVX2/SSE auto-selection
    let arr = AlignedBitboardArray::<16>::from_slice(&a_data);
    group.bench_function("combine_all_auto_select", |bencher| {
        bencher.iter(|| {
            black_box(arr.combine_all())
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_batch_and, bench_batch_or, bench_batch_xor, bench_batch_various_sizes, bench_combine_all, bench_avx2_vs_sse);
criterion_main!(benches);

