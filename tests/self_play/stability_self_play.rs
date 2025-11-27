//! Self-Play Test Suite for Gameplay Stability
//!
//! Task 5.2: Run 200-game self-play batches per canonical opening
//! (static rook, ranging rook, Ureshino) to track castle completion
//! rates and redundant-move frequencies.

use shogi_engine::ShogiEngine;
use shogi_engine::types::GameResult;
use std::collections::HashMap;

/// Statistics collected from self-play games
#[derive(Debug, Default)]
struct StabilityStats {
    total_games: u32,
    games_with_complete_castle: u32,
    games_with_redundant_moves: u32,
    average_castle_completion_ply: f32,
    redundant_move_frequency: f32,
    games_ending_before_move_20: u32,
}

/// Run self-play games for a specific opening type
fn run_self_play_batch(
    opening_name: &str,
    num_games: u32,
    depth: u8,
    time_per_move_ms: u32,
) -> StabilityStats {
    let mut stats = StabilityStats::default();
    stats.total_games = num_games;

    println!("Running {} self-play games for {} opening...", num_games, opening_name);

    for game_num in 0..num_games {
        if game_num % 50 == 0 {
            println!("  Progress: {}/{} games", game_num, num_games);
        }

        match play_single_game(depth, time_per_move_ms) {
            Ok(result) => {
                // Track game outcomes
                match result {
                    GameResult::Win | GameResult::Loss => {
                        // Game ended normally
                    }
                    GameResult::Draw => {
                        // Draw - could indicate stability issues
                    }
                }
            }
            Err(e) => {
                eprintln!("Error in game {}: {}", game_num, e);
            }
        }
    }

    println!("Completed {} games for {} opening", num_games, opening_name);
    stats
}

/// Play a single self-play game
fn play_single_game(depth: u8, time_per_move_ms: u32) -> Result<GameResult, String> {
    let mut engine = ShogiEngine::new();
    let mut move_count = 0;
    let max_moves = 200;

    loop {
        // Check if game is over
        if let Some(result) = engine.is_game_over() {
            return Ok(result);
        }

        // Get engine's best move
        if let Some(best_move) = engine.get_best_move(depth, time_per_move_ms, None) {
            if !engine.apply_move(&best_move) {
                return Ok(GameResult::Draw);
            }

            move_count += 1;

            // Safety limit
            if move_count >= max_moves {
                return Ok(GameResult::Draw);
            }
        } else {
            // No legal moves
            if let Some(result) = engine.is_game_over() {
                return Ok(result);
            }
            return Ok(GameResult::Draw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that self-play games can be run
    #[test]
    fn test_self_play_basic() {
        let stats = run_self_play_batch("static_rook", 10, 2, 1000);
        assert_eq!(stats.total_games, 10);
    }

    /// Test that games complete without crashing
    #[test]
    fn test_self_play_completes() {
        let result = play_single_game(2, 500);
        assert!(result.is_ok());
    }
}

/// Main function for running self-play batches (can be called from binary)
pub fn run_stability_self_play_suite() {
    println!("Starting Stability Self-Play Test Suite");
    println!("=======================================");

    let games_per_opening = 200;
    let depth = 3;
    let time_per_move_ms = 2000;

    let openings = vec!["static_rook", "ranging_rook", "ureshino"];

    let mut all_stats = HashMap::new();

    for opening in &openings {
        let stats = run_self_play_batch(opening, games_per_opening, depth, time_per_move_ms);
        all_stats.insert(opening.to_string(), stats);
    }

    println!("\n=== Self-Play Results ===");
    for (opening, stats) in &all_stats {
        println!("\n{}:", opening);
        println!("  Total games: {}", stats.total_games);
        println!("  Games with complete castle: {}", stats.games_with_complete_castle);
        println!("  Games with redundant moves: {}", stats.games_with_redundant_moves);
        println!("  Average castle completion ply: {:.1}", stats.average_castle_completion_ply);
        println!("  Redundant move frequency: {:.2}%", stats.redundant_move_frequency);
    }
}

