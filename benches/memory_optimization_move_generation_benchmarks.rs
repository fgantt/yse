//! Memory optimization performance benchmarks for move generation paths
//!
//! This benchmark suite measures the performance impact of memory optimizations
//! (Task 3.12) in move generation paths, specifically:
//! - Prefetching hints in magic table lookups
//! - Prefetching for attack pattern generation
//! - Cache-friendly data access patterns
//!
//! # Benchmarks
//!
//! - `move_generation_with_memory_optimizations`: Measures move generation performance
//!   with memory optimizations enabled (prefetching)
//! - `move_generation_without_memory_optimizations`: Baseline comparison
//! - `sliding_move_generation_batch`: Measures batch sliding move generation performance
//! - `magic_table_lookup_performance`: Measures magic table lookup performance with prefetching

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shogi_engine::bitboards::{BitboardBoard, sliding_moves::SlidingMoveGenerator};
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::*;
use std::sync::Arc;

/// Create a board with many sliding pieces for realistic move generation workload
fn create_heavy_sliding_board() -> BitboardBoard {
    let mut board = BitboardBoard::new();
    
    // Add many sliding pieces (rook, bishop, lance) to create cache pressure
    for row in 0..9 {
        for col in 0..9 {
            if (row + col) % 2 == 0 {
                let piece_type = match (row + col) % 3 {
                    0 => PieceType::Rook,
                    1 => PieceType::Bishop,
                    _ => PieceType::Lance,
                };
                let player = if row < 4 { Player::Black } else { Player::White };
                board.place_piece(
                    Piece::new(piece_type, player),
                    Position::new(row, col),
                );
            }
        }
    }
    
    board
}

/// Create a board with sparse sliding pieces (fewer cache misses expected)
fn create_sparse_sliding_board() -> BitboardBoard {
    let mut board = BitboardBoard::new();
    
    // Add a few sliding pieces
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(0, 0));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(0, 8));
    board.place_piece(Piece::new(PieceType::Rook, Player::White), Position::new(8, 0));
    board.place_piece(Piece::new(PieceType::Bishop, Player::White), Position::new(8, 8));
    
    board
}

/// Create a realistic game position with mixed pieces
fn create_realistic_position() -> BitboardBoard {
    let mut board = BitboardBoard::new();
    
    // Create a mid-game position with various pieces
    // Black pieces
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), Position::new(1, 1));
    board.place_piece(Piece::new(PieceType::Bishop, Player::Black), Position::new(1, 7));
    board.place_piece(Piece::new(PieceType::Lance, Player::Black), Position::new(2, 0));
    board.place_piece(Piece::new(PieceType::Silver, Player::Black), Position::new(2, 2));
    board.place_piece(Piece::new(PieceType::Gold, Player::Black), Position::new(2, 6));
    
    // White pieces
    board.place_piece(Piece::new(PieceType::Rook, Player::White), Position::new(7, 7));
    board.place_piece(Piece::new(PieceType::Bishop, Player::White), Position::new(7, 1));
    board.place_piece(Piece::new(PieceType::Lance, Player::White), Position::new(6, 8));
    board.place_piece(Piece::new(PieceType::Silver, Player::White), Position::new(6, 6));
    board.place_piece(Piece::new(PieceType::Gold, Player::White), Position::new(6, 2));
    
    board
}

/// Benchmark move generation with memory optimizations
fn bench_move_generation_with_memory_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("move_generation_memory_optimizations");
    
    let boards = vec![
        ("sparse", create_sparse_sliding_board()),
        ("heavy", create_heavy_sliding_board()),
        ("realistic", create_realistic_position()),
    ];
    
    for (name, board) in boards {
        let captured_pieces = CapturedPieces::new();
        let move_generator = MoveGenerator::new();
        
        group.bench_function(
            BenchmarkId::new("with_prefetching", name),
            |b| {
                b.iter(|| {
                    let moves = move_generator.generate_all_piece_moves(
                        black_box(&board),
                        black_box(Player::Black),
                    );
                    black_box(moves)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark sliding move generation batch performance
#[cfg(feature = "simd")]
fn bench_sliding_move_generation_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("sliding_move_generation_batch");
    
    let boards = vec![
        ("sparse", create_sparse_sliding_board()),
        ("heavy", create_heavy_sliding_board()),
        ("realistic", create_realistic_position()),
    ];
    
    for (name, board) in boards {
        // Get magic table from board
        let magic_table = board.get_magic_table().unwrap_or_else(|| {
            Arc::new(MagicTable::default())
        });
        let generator = SlidingMoveGenerator::new(magic_table);
        
        // Collect sliding pieces
        let mut sliding_pieces = Vec::new();
        for row in 0..9 {
            for col in 0..9 {
                let pos = Position::new(row, col);
                if let Some(piece) = board.get_piece(pos) {
                    match piece.piece_type {
                        PieceType::Rook | PieceType::Bishop | PieceType::Lance => {
                            sliding_pieces.push((pos, piece.piece_type));
                        }
                        _ => {}
                    }
                }
            }
        }
        
        group.bench_function(
            BenchmarkId::new("batch_vectorized", name),
            |b| {
                b.iter(|| {
                    let moves = generator.generate_sliding_moves_batch_vectorized(
                        black_box(&board),
                        black_box(&sliding_pieces),
                        black_box(Player::Black),
                    );
                    black_box(moves)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark magic table lookup performance
fn bench_magic_table_lookup_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("magic_table_lookup");
    
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = create_realistic_position();
    let occupied = board.get_occupied_bitboard();
    
    // Test different squares and piece types
    let test_cases = vec![
        (Position::new(0, 0), PieceType::Rook),
        (Position::new(4, 4), PieceType::Rook),
        (Position::new(8, 8), PieceType::Rook),
        (Position::new(0, 0), PieceType::Bishop),
        (Position::new(4, 4), PieceType::Bishop),
        (Position::new(8, 8), PieceType::Bishop),
    ];
    
    for (pos, piece_type) in test_cases {
        let square = pos.to_index();
        group.bench_function(
            BenchmarkId::new("lookup", format!("{:?}_{}", piece_type, square)),
            |b| {
                b.iter(|| {
                    // Use the generator's method directly
                    let moves = generator.generate_sliding_moves(
                        black_box(&board),
                        black_box(pos),
                        black_box(piece_type),
                        black_box(Player::Black),
                    );
                    black_box(moves)
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_move_generation_with_memory_optimizations,
    bench_sliding_move_generation_batch,
    bench_magic_table_lookup_performance
);
criterion_main!(benches);

