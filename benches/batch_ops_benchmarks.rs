use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::{SimdBitboard, batch_ops::AlignedBitboardArray};

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

criterion_group!(benches, bench_batch_and, bench_batch_or, bench_batch_xor, bench_batch_various_sizes);
criterion_main!(benches);

