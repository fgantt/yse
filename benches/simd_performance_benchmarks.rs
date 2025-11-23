use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::SimdBitboard;

/// Scalar implementation for comparison
#[inline(always)]
fn scalar_and(a: u128, b: u128) -> u128 {
    a & b
}

#[inline(always)]
fn scalar_or(a: u128, b: u128) -> u128 {
    a | b
}

#[inline(always)]
fn scalar_xor(a: u128, b: u128) -> u128 {
    a ^ b
}

#[inline(always)]
fn scalar_not(a: u128) -> u128 {
    !a
}

fn bench_bitwise_operations(c: &mut Criterion) {
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);

    let mut group = c.benchmark_group("bitwise_operations");
    group.sample_size(1000);

    // Benchmark SIMD AND
    group.bench_function("simd_and", |b| {
        b.iter(|| {
            black_box(bb1 & bb2)
        });
    });

    // Benchmark scalar AND
    group.bench_function("scalar_and", |b| {
        b.iter(|| {
            black_box(scalar_and(test_value1, test_value2))
        });
    });

    // Benchmark SIMD OR
    group.bench_function("simd_or", |b| {
        b.iter(|| {
            black_box(bb1 | bb2)
        });
    });

    // Benchmark scalar OR
    group.bench_function("scalar_or", |b| {
        b.iter(|| {
            black_box(scalar_or(test_value1, test_value2))
        });
    });

    // Benchmark SIMD XOR
    group.bench_function("simd_xor", |b| {
        b.iter(|| {
            black_box(bb1 ^ bb2)
        });
    });

    // Benchmark scalar XOR
    group.bench_function("scalar_xor", |b| {
        b.iter(|| {
            black_box(scalar_xor(test_value1, test_value2))
        });
    });

    // Benchmark SIMD NOT
    group.bench_function("simd_not", |b| {
        b.iter(|| {
            black_box(!bb1)
        });
    });

    // Benchmark scalar NOT
    group.bench_function("scalar_not", |b| {
        b.iter(|| {
            black_box(scalar_not(test_value1))
        });
    });

    group.finish();
}

fn bench_count_ones(c: &mut Criterion) {
    let test_value = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let bb = SimdBitboard::from_u128(test_value);

    let mut group = c.benchmark_group("count_ones");
    group.sample_size(1000);

    // Benchmark SIMD count_ones (uses hardware popcount)
    group.bench_function("simd_count_ones", |b| {
        b.iter(|| {
            black_box(bb.count_ones())
        });
    });

    // Benchmark scalar count_ones
    group.bench_function("scalar_count_ones", |b| {
        b.iter(|| {
            black_box(test_value.count_ones())
        });
    });

    group.finish();
}

fn bench_combined_operations(c: &mut Criterion) {
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    let test_value3 = 0x5555_5555_5555_5555_5555_5555_5555_5555;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);
    let bb3 = SimdBitboard::from_u128(test_value3);

    let mut group = c.benchmark_group("combined_operations");
    group.sample_size(1000);

    // Benchmark SIMD combined: (a & b) | (c & !a)
    group.bench_function("simd_combined", |b| {
        b.iter(|| {
            black_box((bb1 & bb2) | (bb3 & !bb1))
        });
    });

    // Benchmark scalar combined
    group.bench_function("scalar_combined", |b| {
        b.iter(|| {
            black_box((test_value1 & test_value2) | (test_value3 & !test_value1))
        });
    });

    group.finish();
}

criterion_group!(benches, bench_bitwise_operations, bench_count_ones, bench_combined_operations);
criterion_main!(benches);

