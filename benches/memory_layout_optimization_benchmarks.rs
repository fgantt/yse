//! Memory Layout Optimization Benchmarks
//!
//! Optimization 5.5: Benchmark memory layout improvements.
//!
//! This benchmark suite measures the performance improvements from memory layout
//! optimizations, including SoA vs AoA layouts and cache-aligned structures.

#![cfg(feature = "simd")]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::memory_optimization::cache_friendly::{
    AttackPatternSoA, BitboardSoA, PstSoA,
};
use shogi_engine::bitboards::{batch_ops::AlignedBitboardArray, BitboardBoard, SimdBitboard};
use shogi_engine::evaluation::piece_square_tables::PieceSquareTables;
use shogi_engine::types::core::{PieceType, Player, Position};

/// Benchmark PST table access: AoA vs SoA layout
fn bench_pst_layout_comparison(c: &mut Criterion) {
    let pst = PieceSquareTables::new();
    let piece_type = PieceType::Rook;

    // Create SoA layout from AoA
    let (aoa_mg, aoa_eg) = pst.get_tables(piece_type);
    let pst_soa = PstSoA::from_aoa(aoa_mg, aoa_eg);

    // Test positions
    let positions: Vec<(u8, u8)> =
        (0..9).flat_map(|row| (0..9).map(move |col| (row, col))).collect();

    let mut group = c.benchmark_group("PST Layout Comparison");

    // Benchmark AoA access (current implementation)
    group.bench_function("AoA Access", |b| {
        b.iter(|| {
            let mut total_mg = 0i32;
            let mut total_eg = 0i32;
            for &(row, col) in &positions {
                let mg = aoa_mg[row as usize][col as usize];
                let eg = aoa_eg[row as usize][col as usize];
                total_mg += black_box(mg);
                total_eg += black_box(eg);
            }
            (total_mg, total_eg)
        })
    });

    // Benchmark SoA access (optimized)
    group.bench_function("SoA Access", |b| {
        b.iter(|| {
            let mut total_mg = 0i32;
            let mut total_eg = 0i32;
            for &(row, col) in &positions {
                let (mg, eg) = pst_soa.get(row, col);
                total_mg += black_box(mg);
                total_eg += black_box(eg);
            }
            (total_mg, total_eg)
        })
    });

    // Benchmark SoA batch access
    group.bench_function("SoA Batch Access", |b| {
        b.iter(|| {
            let batch = pst_soa.get_batch(&positions);
            let mut total_mg = 0i32;
            let mut total_eg = 0i32;
            for (mg, eg) in batch {
                total_mg += black_box(mg);
                total_eg += black_box(eg);
            }
            (total_mg, total_eg)
        })
    });

    group.finish();
}

/// Benchmark attack pattern storage: Standard vs SoA
fn bench_attack_pattern_storage(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let piece_type = PieceType::Rook;
    let player = Player::Black;

    // Generate attack patterns for multiple positions
    let positions: Vec<Position> = (0..9)
        .flat_map(|row| (0..9).map(move |col| Position::new(row, col)))
        .take(16) // Use 16 positions for batch operations
        .collect();

    let attack_patterns: Vec<SimdBitboard> = positions
        .iter()
        .map(|&pos| {
            let attacks = board.get_attack_pattern(pos, piece_type);
            SimdBitboard::from_u128(attacks.to_u128())
        })
        .collect();

    // Create SoA structure
    let attack_soa = AttackPatternSoA::<16>::from_bitboards(&attack_patterns);

    // Create AlignedBitboardArray for comparison
    let mut aligned_array = AlignedBitboardArray::<16>::new();
    for (i, &bb) in attack_patterns.iter().take(16).enumerate() {
        aligned_array.as_mut_array()[i] = bb;
    }

    let mut group = c.benchmark_group("Attack Pattern Storage");

    // Benchmark standard Vec access
    group.bench_function("Vec Access", |b| {
        b.iter(|| {
            let mut combined = SimdBitboard::empty();
            for &bb in &attack_patterns {
                combined |= black_box(bb);
            }
            combined
        })
    });

    // Benchmark AlignedBitboardArray access
    group.bench_function("AlignedArray Access", |b| {
        b.iter(|| {
            let mut combined = SimdBitboard::empty();
            for i in 0..16 {
                combined |= black_box(*aligned_array.get(i));
            }
            combined
        })
    });

    // Benchmark SoA access
    group.bench_function("SoA Access", |b| {
        b.iter(|| {
            let mut combined = SimdBitboard::empty();
            for i in 0..16 {
                combined |= black_box(attack_soa.get(i));
            }
            combined
        })
    });

    group.finish();
}

/// Benchmark batch operations with different layouts
fn bench_batch_operations(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let piece_type = PieceType::Rook;

    // Generate attack patterns
    let positions: Vec<Position> = (0..4).map(|i| Position::new(i, i)).collect();

    let attack_patterns: Vec<SimdBitboard> = positions
        .iter()
        .map(|&pos| {
            let attacks = board.get_attack_pattern(pos, piece_type);
            SimdBitboard::from_u128(attacks.to_u128())
        })
        .collect();

    // Create different layouts
    let aligned_array = AlignedBitboardArray::<4>::from_slice(&attack_patterns[..4]);
    let bitboard_soa = {
        let mut soa = BitboardSoA::<4>::new();
        for (i, &bb) in attack_patterns.iter().take(4).enumerate() {
            soa.set(i, bb);
        }
        soa
    };

    let mut group = c.benchmark_group("Batch Operations");

    // Benchmark AlignedBitboardArray batch operations
    group.bench_function("AlignedArray Batch OR", |b| {
        b.iter(|| black_box(aligned_array.combine_all()))
    });

    // Benchmark SoA batch operations
    group.bench_function("SoA Batch OR", |b| {
        b.iter(|| {
            let mut combined = SimdBitboard::empty();
            for i in 0..4 {
                combined |= black_box(bitboard_soa.get(i));
            }
            combined
        })
    });

    group.finish();
}

/// Benchmark memory access patterns for different batch sizes
fn bench_memory_access_patterns(c: &mut Criterion) {
    let board = BitboardBoard::new();
    let piece_type = PieceType::Rook;

    let mut group = c.benchmark_group("Memory Access Patterns");

    for batch_size in [4, 8, 16, 32].iter() {
        let batch_size_usize = *batch_size as usize;
        // Generate attack patterns
        let positions: Vec<Position> = (0..batch_size_usize)
            .map(|i| Position::new((i % 9) as u8, (i / 9) as u8))
            .collect();

        let attack_patterns: Vec<SimdBitboard> = positions
            .iter()
            .map(|&pos| {
                let attacks = board.get_attack_pattern(pos, piece_type);
                SimdBitboard::from_u128(attacks.to_u128())
            })
            .collect();

        // Benchmark Vec access
        group.bench_with_input(
            BenchmarkId::new("Vec Access", batch_size),
            &batch_size_usize,
            |b, &size| {
                b.iter(|| {
                    let mut combined = SimdBitboard::empty();
                    for &bb in attack_patterns.iter().take(size) {
                        combined |= black_box(bb);
                    }
                    combined
                })
            },
        );

        // Benchmark AlignedBitboardArray access
        if batch_size_usize <= 32 {
            let mut aligned_array = AlignedBitboardArray::<32>::new();
            for (i, &bb) in attack_patterns.iter().take(batch_size_usize).enumerate() {
                aligned_array.as_mut_array()[i] = bb;
            }

            group.bench_with_input(
                BenchmarkId::new("AlignedArray Access", batch_size),
                &batch_size_usize,
                |b, &size| {
                    b.iter(|| {
                        let mut combined = SimdBitboard::empty();
                        for i in 0..size {
                            combined |= black_box(*aligned_array.get(i));
                        }
                        combined
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pst_layout_comparison,
    bench_attack_pattern_storage,
    bench_batch_operations,
    bench_memory_access_patterns
);
criterion_main!(benches);
