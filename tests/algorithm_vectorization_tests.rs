#![cfg(feature = "simd")]
/// Tests for algorithm vectorization using SIMD batch operations
///
/// These tests validate that vectorized algorithms work correctly and
/// provide performance improvements over scalar implementations.
use shogi_engine::bitboards::sliding_moves::SlidingMoveGenerator;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::types::{MagicTable, Piece, PieceType, Player, Position};
use std::sync::Arc;

#[test]
fn test_vectorized_batch_move_generation() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();

    let pieces: Vec<(Position, Piece)> = vec![
        (Position::new(0, 0), Piece::new(PieceType::Rook, Player::Black)),
        (Position::new(0, 8), Piece::new(PieceType::Rook, Player::Black)),
        (Position::new(8, 0), Piece::new(PieceType::Bishop, Player::Black)),
        (Position::new(8, 8), Piece::new(PieceType::Bishop, Player::Black)),
    ];
    let scalar_pieces: Vec<(Position, PieceType)> =
        pieces.iter().map(|(pos, piece)| (*pos, piece.piece_type)).collect();

    // Test vectorized batch generation
    let vectorized_moves =
        generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);

    // Test regular batch generation for comparison
    let regular_moves =
        generator.generate_sliding_moves_batch(&board, &scalar_pieces, Player::Black);

    // Results should be equivalent (same moves, possibly different order)
    assert_eq!(
        vectorized_moves.len(),
        regular_moves.len(),
        "Vectorized and regular batch generation should produce same number of moves"
    );

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

    let pieces: Vec<(Position, Piece)> = vec![];

    let moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    assert!(moves.is_empty(), "Empty pieces list should produce no moves");
}

#[test]
fn test_vectorized_batch_single_piece() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();

    let pieces = vec![(Position::new(4, 4), Piece::new(PieceType::Rook, Player::Black))];
    let scalar_pieces: Vec<(Position, PieceType)> =
        pieces.iter().map(|(pos, piece)| (*pos, piece.piece_type)).collect();

    let vectorized_moves =
        generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    let regular_moves =
        generator.generate_sliding_moves_batch(&board, &scalar_pieces, Player::Black);

    assert_eq!(
        vectorized_moves.len(),
        regular_moves.len(),
        "Single piece should produce same number of moves"
    );
}

#[test]
fn test_vectorized_batch_large_batch() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let board = BitboardBoard::new();

    // Test with more than 4 pieces (tests batching)
    let pieces: Vec<(Position, Piece)> = vec![
        (Position::new(0, 0), Piece::new(PieceType::Rook, Player::Black)),
        (Position::new(0, 4), Piece::new(PieceType::Rook, Player::Black)),
        (Position::new(0, 8), Piece::new(PieceType::Rook, Player::Black)),
        (Position::new(4, 0), Piece::new(PieceType::Bishop, Player::Black)),
        (Position::new(4, 4), Piece::new(PieceType::Bishop, Player::Black)),
        (Position::new(4, 8), Piece::new(PieceType::Bishop, Player::Black)),
        (Position::new(8, 0), Piece::new(PieceType::Rook, Player::Black)),
        (Position::new(8, 8), Piece::new(PieceType::Rook, Player::Black)),
    ];
    let scalar_pieces: Vec<(Position, PieceType)> =
        pieces.iter().map(|(pos, piece)| (*pos, piece.piece_type)).collect();

    let vectorized_moves =
        generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);
    let regular_moves =
        generator.generate_sliding_moves_batch(&board, &scalar_pieces, Player::Black);

    assert_eq!(
        vectorized_moves.len(),
        regular_moves.len(),
        "Large batch should produce same number of moves"
    );
}

#[test]
fn test_vectorized_moves_set_capture_metadata() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let mut board = BitboardBoard::empty();

    let from = Position::new(4, 4);
    let target = Position::new(4, 6);
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), from);
    board.place_piece(Piece::new(PieceType::Gold, Player::White), target);

    let pieces = vec![(from, Piece::new(PieceType::Rook, Player::Black))];
    let moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);

    let capture_move = moves
        .iter()
        .find(|m| m.from == Some(from) && m.to == target)
        .expect("capture move missing");
    assert!(capture_move.is_capture);
    assert_eq!(
        capture_move.captured_piece.as_ref().map(|p| p.piece_type).unwrap(),
        PieceType::Gold
    );

    let mut board_after = board.clone();
    board_after.make_move(capture_move);
    assert!(
        !board_after.is_square_occupied_by(target, Player::White),
        "Captured piece must be removed from board state"
    );
}

#[test]
fn test_vectorized_moves_include_promotions() {
    let magic_table = Arc::new(MagicTable::default());
    let generator = SlidingMoveGenerator::new(magic_table);
    let mut board = BitboardBoard::empty();

    let from = Position::new(2, 4);
    let to = Position::new(1, 4);
    board.place_piece(Piece::new(PieceType::Rook, Player::Black), from);

    let pieces = vec![(from, Piece::new(PieceType::Rook, Player::Black))];
    let moves = generator.generate_sliding_moves_batch_vectorized(&board, &pieces, Player::Black);

    let matching_moves: Vec<_> =
        moves.iter().filter(|m| m.from == Some(from) && m.to == to).collect();
    assert!(
        matching_moves.iter().any(|m| m.is_promotion),
        "Vectorized path should emit a promoted variant"
    );
    assert!(
        matching_moves.iter().any(|m| !m.is_promotion),
        "Vectorized path should also emit the non-promoted move"
    );
}
