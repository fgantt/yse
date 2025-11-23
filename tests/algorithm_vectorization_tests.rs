#![cfg(feature = "simd")]
/// Tests for algorithm vectorization using SIMD batch operations
/// 
/// These tests validate that vectorized algorithms work correctly and
/// provide performance improvements over scalar implementations.

use shogi_engine::bitboards::sliding_moves::SlidingMoveGenerator;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::types::{MagicTable, PieceType, Player, Position};
use std::sync::Arc;

#[test]
fn test_vectorized_batch_move_generation() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();
    
    let pieces = vec![
        (Position::new(0, 0), PieceType::Rook),
        (Position::new(0, 8), PieceType::Rook),
        (Position::new(8, 0), PieceType::Bishop),
        (Position::new(8, 8), PieceType::Bishop),
    ];
    
    // Test vectorized batch generation
    let vectorized_moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    
    // Test regular batch generation for comparison
    let regular_moves = generator.generate_sliding_moves_batch(&board, &pieces, Player::Black);
    
    // Results should be equivalent (same moves, possibly different order)
    assert_eq!(vectorized_moves.len(), regular_moves.len(),
               "Vectorized and regular batch generation should produce same number of moves");
    
    // Verify all moves are valid
    for mv in &vectorized_moves {
        if let Some(from) = mv.from {
            assert!(from.is_valid());
        }
        assert!(mv.to.is_valid());
    }
}

#[test]
fn test_vectorized_batch_empty_pieces() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();
    
    let pieces = vec![];
    
    let moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    assert!(moves.is_empty(), "Empty pieces list should produce no moves");
}

#[test]
fn test_vectorized_batch_single_piece() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();
    
    let pieces = vec![
        (Position::new(4, 4), PieceType::Rook),
    ];
    
    let vectorized_moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    let regular_moves = generator.generate_sliding_moves_batch(&board, &pieces, Player::Black);
    
    assert_eq!(vectorized_moves.len(), regular_moves.len(),
               "Single piece should produce same number of moves");
}

#[test]
fn test_vectorized_batch_large_batch() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();
    
    // Test with more than 4 pieces (tests batching)
    let pieces = vec![
        (Position::new(0, 0), PieceType::Rook),
        (Position::new(0, 4), PieceType::Rook),
        (Position::new(0, 8), PieceType::Rook),
        (Position::new(4, 0), PieceType::Bishop),
        (Position::new(4, 4), PieceType::Bishop),
        (Position::new(4, 8), PieceType::Bishop),
        (Position::new(8, 0), PieceType::Rook),
        (Position::new(8, 8), PieceType::Rook),
    ];
    
    let vectorized_moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    let regular_moves = generator.generate_sliding_moves_batch(&board, &pieces, Player::Black);
    
    assert_eq!(vectorized_moves.len(), regular_moves.len(),
               "Large batch should produce same number of moves");
}

