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

fn bench_trailing_zeros(c: &mut Criterion) {
    let test_value = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let bb = SimdBitboard::from_u128(test_value);

    let mut group = c.benchmark_group("trailing_zeros");
    group.sample_size(1000);

    group.bench_function("simd_trailing_zeros", |b| {
        b.iter(|| {
            black_box(bb.trailing_zeros())
        });
    });

    group.bench_function("scalar_trailing_zeros", |b| {
        b.iter(|| {
            black_box(test_value.trailing_zeros())
        });
    });

    group.finish();
}

fn bench_leading_zeros(c: &mut Criterion) {
    let test_value = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let bb = SimdBitboard::from_u128(test_value);

    let mut group = c.benchmark_group("leading_zeros");
    group.sample_size(1000);

    group.bench_function("simd_leading_zeros", |b| {
        b.iter(|| {
            black_box(bb.leading_zeros())
        });
    });

    group.bench_function("scalar_leading_zeros", |b| {
        b.iter(|| {
            black_box(test_value.leading_zeros())
        });
    });

    group.finish();
}

fn bench_is_empty(c: &mut Criterion) {
    let empty_bb = SimdBitboard::empty();
    let non_empty_bb = SimdBitboard::from_u128(0x1);

    let mut group = c.benchmark_group("is_empty");
    group.sample_size(1000);

    group.bench_function("simd_is_empty_true", |b| {
        b.iter(|| {
            black_box(empty_bb.is_empty())
        });
    });

    group.bench_function("simd_is_empty_false", |b| {
        b.iter(|| {
            black_box(non_empty_bb.is_empty())
        });
    });

    group.finish();
}

// Batch operations benchmarks (comprehensive coverage)
fn bench_batch_operations(c: &mut Criterion) {
    use shogi_engine::bitboards::batch_ops::AlignedBitboardArray;
    
    // Test with different array sizes
    for size in [4, 8, 16] {
        let mut a_data = [SimdBitboard::empty(); 16];
        let mut b_data = [SimdBitboard::empty(); 16];
        
        for i in 0..16 {
            a_data[i] = SimdBitboard::from_u128(0x0F0F_0F0F_0F0F_0F0F ^ (i as u128));
            b_data[i] = SimdBitboard::from_u128(0x3333_3333_3333_3333 ^ (i as u128));
        }
        
        match size {
            4 => {
                let a = AlignedBitboardArray::<4>::from_slice(&a_data[0..4]);
                let b = AlignedBitboardArray::<4>::from_slice(&b_data[0..4]);
                
                let mut group = c.benchmark_group(&format!("batch_operations_size_{}", size));
                group.sample_size(500);
                
                group.bench_function("simd_batch_and", |bencher| {
                    bencher.iter(|| black_box(a.batch_and(&b)));
                });
                
                group.bench_function("simd_batch_or", |bencher| {
                    bencher.iter(|| black_box(a.batch_or(&b)));
                });
                
                group.bench_function("simd_batch_xor", |bencher| {
                    bencher.iter(|| black_box(a.batch_xor(&b)));
                });
                
                group.finish();
            }
            8 => {
                let a = AlignedBitboardArray::<8>::from_slice(&a_data[0..8]);
                let b = AlignedBitboardArray::<8>::from_slice(&b_data[0..8]);
                
                let mut group = c.benchmark_group(&format!("batch_operations_size_{}", size));
                group.sample_size(500);
                
                group.bench_function("simd_batch_and", |bencher| {
                    bencher.iter(|| black_box(a.batch_and(&b)));
                });
                
                group.bench_function("simd_batch_or", |bencher| {
                    bencher.iter(|| black_box(a.batch_or(&b)));
                });
                
                group.bench_function("simd_batch_xor", |bencher| {
                    bencher.iter(|| black_box(a.batch_xor(&b)));
                });
                
                group.finish();
            }
            16 => {
                let a = AlignedBitboardArray::<16>::from_slice(&a_data);
                let b = AlignedBitboardArray::<16>::from_slice(&b_data);
                
                let mut group = c.benchmark_group(&format!("batch_operations_size_{}", size));
                group.sample_size(500);
                
                group.bench_function("simd_batch_and", |bencher| {
                    bencher.iter(|| black_box(a.batch_and(&b)));
                });
                
                group.bench_function("simd_batch_or", |bencher| {
                    bencher.iter(|| black_box(a.batch_or(&b)));
                });
                
                group.bench_function("simd_batch_xor", |bencher| {
                    bencher.iter(|| black_box(a.batch_xor(&b)));
                });
                
                group.finish();
            }
            _ => {}
        }
    }
}

criterion_group!(
    benches, 
    bench_bitwise_operations, 
    bench_count_ones, 
    bench_combined_operations,
    bench_trailing_zeros,
    bench_leading_zeros,
    bench_is_empty,
    bench_batch_operations
);
criterion_main!(benches);

