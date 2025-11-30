use shogi_engine::*;

/// Regression test for illegal move bug (move 15 forfeit)
/// This test verifies that get_best_move always returns legal moves
/// Note: We can't easily replay the exact game without a PGN parser,
/// but this test ensures the core legality check works
#[test]
fn test_illegal_move_regression() {
    let mut engine = ShogiEngine::new();
    
    // Play many moves to reach various positions
    // This simulates the conditions that led to the illegal move bug
    for move_num in 0..30 {
        if let Some(_) = engine.is_game_over() {
            break;
        }
        
        // Get best move
        let best_move = engine.get_best_move(3, 500, None);
        
        // CRITICAL: Verify the move is legal by trying to apply it
        // The apply_move method will check legality and return false if illegal
        if let Some(move_) = best_move {
            // Apply the move - this will verify legality internally
            let applied = engine.apply_move(&move_);
            assert!(
                applied,
                "ILLEGAL MOVE DETECTED at move {}!\n\
                 Move: {} was returned by get_best_move but is not legal",
                move_num + 1,
                move_.to_usi_string()
            );
        } else {
            // No move means game is over
            assert!(engine.is_game_over().is_some());
            break;
        }
    }
}

/// Fuzz test: Generate random positions and verify get_best_move returns legal moves
#[test]
fn test_move_legality_fuzzing() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    for test_num in 0..50 {
        let mut engine = ShogiEngine::new();
        
        // Play random moves to reach a random position
        for _ in 0..rng.gen_range(5..20) {
            if let Some(_) = engine.is_game_over() {
                // Game ended, start over
                engine = ShogiEngine::new();
                continue;
            }
            
            // Get best move and verify it's legal
            let best_move = engine.get_best_move(2, 100, None);
            
            if let Some(move_) = best_move {
                // Apply move - this will verify legality
                let applied = engine.apply_move(&move_);
                assert!(
                    applied,
                    "Test {}: get_best_move returned illegal move: {}",
                    test_num,
                    move_.to_usi_string()
                );
            } else {
                // No move means game is over
                break;
            }
        }
    }
}

/// Test that get_best_move always returns legal moves
#[test]
fn test_get_best_move_legality() {
    let mut engine = ShogiEngine::new();
    
    for _ in 0..20 {
        if let Some(result) = engine.is_game_over() {
            break;
        }
        
        let best_move = engine.get_best_move(3, 100, None);
        
        if let Some(move_) = best_move {
            // Verify move is legal by applying it
            // The apply_move method will check legality internally
            let applied = engine.apply_move(&move_);
            assert!(
                applied,
                "get_best_move returned illegal move: {}",
                move_.to_usi_string()
            );
        } else {
            // No move means game is over
            assert!(engine.is_game_over().is_some());
            break;
        }
    }
}


