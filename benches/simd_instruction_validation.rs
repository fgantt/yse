use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shogi_engine::bitboards::SimdBitboard;

/// This benchmark is designed to be analyzed with objdump/llvm-objdump
/// to verify that SIMD instructions are actually being generated.
/// 
/// To verify SIMD instructions:
/// 1. Build with: cargo build --release --features simd --bench simd_instruction_validation
/// 2. Disassemble: objdump -d target/release/deps/simd_instruction_validation-*.exe | grep -E "(pand|por|pxor|andnot|vand|vorr|veor)"
///    Or on macOS: otool -tv target/release/deps/simd_instruction_validation-* | grep -E "(pand|por|pxor|andnot|vand|vorr|veor)"
/// 
/// Expected instructions:
/// - x86_64: pand, por, pxor, pandn (SSE) or vpand, vpor, vpxor, vpandn (AVX2)
/// - ARM64: vand, vorr, veor (NEON)

fn bench_simd_instructions(c: &mut Criterion) {
    let test_value1 = 0x0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F_0F0F;
    let test_value2 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
    
    let bb1 = SimdBitboard::from_u128(test_value1);
    let bb2 = SimdBitboard::from_u128(test_value2);

    let mut group = c.benchmark_group("simd_instruction_validation");
    group.sample_size(100);

    // These operations should generate SIMD instructions when simd feature is enabled
    group.bench_function("and_instruction", |b| {
        b.iter(|| {
            black_box(bb1 & bb2)
        });
    });

    group.bench_function("or_instruction", |b| {
        b.iter(|| {
            black_box(bb1 | bb2)
        });
    });

    group.bench_function("xor_instruction", |b| {
        b.iter(|| {
            black_box(bb1 ^ bb2)
        });
    });

    group.bench_function("not_instruction", |b| {
        b.iter(|| {
            black_box(!bb1)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_simd_instructions);
criterion_main!(benches);

