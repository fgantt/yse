//! NNUE Self-Play Training
//!
//! This binary trains NNUE weights using self-play games and TD(λ) learning.

use shogi_engine::evaluation::nnue::{NNUEWeights, NNUEAccumulator};
use shogi_engine::evaluation::nnue_training::{NNUETrainer, NNUETrainingConfig, TrainingGame, TrainingPosition, extract_active_features, GameResult};
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::search::IterativeDeepening;
use shogi_engine::search::shogi_hash::ShogiHashHandler;
use shogi_engine::types::core::Player;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::BitboardBoard;
use shogi_engine::moves::MoveGenerator;
use std::time::Instant;

/// Play a self-play game and return training data
fn play_self_play_game(
    search_engine: &mut SearchEngine,
    evaluator: &mut PositionEvaluator,
    weights: &NNUEWeights,
    config: &NNUETrainingConfig,
) -> TrainingGame {
    let mut board = BitboardBoard::new();
    let mut captured_pieces = CapturedPieces::new();
    let mut positions = Vec::new();
    let mut current_player = Player::Black;
    let mut move_count = 0;
    
    // Track position hashes for repetition detection
    let mut hash_calculator = ShogiHashHandler::new_default();

    // Create NNUE accumulator for tracking
    let (hidden_size_1, hidden_size_2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(hidden_size_1, hidden_size_2);
    
    // Refresh accumulator for initial position
    accumulator.refresh(&board, weights);

    // Track position occurrences for 3-fold repetition detection
    let mut position_counts = std::collections::HashMap::<u64, u8>::new();
    
    while move_count < config.max_moves_per_game {
        // Check for game over
        if let Some(result) = check_game_over(&board, &captured_pieces, current_player) {
            // Result is already in the correct format (from Black's perspective)
            return TrainingGame {
                positions,
                result,
            };
        }

        // Check for 3-fold repetition (Shogi rule: 3-fold repetition = draw)
        let position_hash = hash_calculator.get_position_hash(&board, current_player, &captured_pieces);
        let count = position_counts.entry(position_hash).or_insert(0);
        *count += 1;
        if *count >= 3 {
            // 3-fold repetition detected - draw
            return TrainingGame {
                positions,
                result: GameResult::Draw,
            };
        }

        // Evaluate current position
        let evaluation = evaluator.evaluate(&board, current_player, &captured_pieces);

        // Extract active features
        let active_features = extract_active_features(&board);

        // Store position
        positions.push(TrainingPosition {
            active_features,
            accumulator: accumulator.clone(),
            evaluation,
            player: current_player,
            move_made: None, // Will be set after move is made
            outcome: None,
            td_target: None,
        });

        // Get best move using iterative deepening search
        let mut iterative_search = IterativeDeepening::new(config.search_depth, config.time_per_move_ms, None);
        let best_move = iterative_search.search(search_engine, &board, &captured_pieces, current_player);
        
        if let Some((mv, _score)) = best_move {
            // Update last position's move
            if let Some(last_pos) = positions.last_mut() {
                last_pos.move_made = Some(mv.clone());
            }
            
            // Update accumulator incrementally
            let from_piece = mv.from.and_then(|pos| board.get_piece(pos));
            let captured_piece = if mv.is_capture {
                board.get_piece(mv.to)
            } else {
                None
            };
            
            if let Some(piece) = from_piece {
                accumulator.update_move(
                    mv.from,
                    mv.to,
                    piece,
                    captured_piece,
                    weights,
                );
            }

            // Make move
            if let Some(captured) = board.make_move(&mv) {
                captured_pieces.add_piece(captured.piece_type, current_player);
            }

            current_player = current_player.opposite();
            move_count += 1;
        } else {
            // No legal moves - game over
            break;
        }
    }

    // Game ended due to move limit - treat as draw
    TrainingGame {
        positions,
        result: GameResult::Draw,
    }
}

/// Check if game is over
/// Returns GameResult from the perspective of the player whose turn it is
fn check_game_over(
    board: &BitboardBoard,
    captured_pieces: &CapturedPieces,
    player: Player,
) -> Option<GameResult> {
    let move_generator = MoveGenerator::new();
    let legal_moves = move_generator.generate_legal_moves(board, player, captured_pieces);

    if legal_moves.is_empty() {
        let in_check = board.is_king_in_check(player, captured_pieces);
        Some(if in_check {
            // Checkmate - current player loses
            if player == Player::Black {
                GameResult::Loss  // Black is mated
            } else {
                GameResult::Win   // White is mated, so Black wins
            }
        } else {
            GameResult::Draw
        })
    } else {
        None
    }
}

fn main() {
    println!("=== NNUE Self-Play Training ===\n");

    // Try to load existing weights, or create new ones
    let weights = match NNUEWeights::load("nnue_weights_trained.json") {
        Ok(w) => {
            println!("✓ Loaded existing weights from nnue_weights_trained.json");
            w
        }
        Err(_) => {
            println!("Creating new random weights");
            NNUEWeights::new(256, 32)
        }
    };
    
    let mut evaluator = PositionEvaluator::new();
    evaluator.enable_nnue_with_weights_internal(weights.clone());

    // Create search engine
    let mut search_engine = SearchEngine::new(None, 1);

    // Training configuration - optimized for continued training
    // NOTE: For faster training, reduce search_depth (4-5), time_per_move_ms (200-300),
    //       and games_per_iteration (10-15). Use release mode for 2-3x speedup.
    let mut config = NNUETrainingConfig::default();
    config.games_per_iteration = 15; // Balanced: good diversity, reasonable speed
    config.iterations = 200; // More iterations for continued training
    config.learning_rate = 0.01; // Reduced learning rate for more stable training (was 0.1)
    config.lambda = 0.7; // TD(λ) parameter
    config.search_depth = 5; // Reduced from 6 for faster training (can increase to 6 for stronger play)
    config.time_per_move_ms = 300; // Reduced from 500 for faster training (can increase to 500 for stronger play)
    config.max_moves_per_game = 200; // Increased for longer games
    config.min_batch_size = 500; // Reduced batch size for more frequent updates

    println!("Training Configuration:");
    println!("  Architecture: 256 -> 32 -> 1");
    println!("  Games per iteration: {}", config.games_per_iteration);
    println!("  Total iterations: {}", config.iterations);
    println!("  Learning rate: {}", config.learning_rate);
    println!("  Lambda (TD): {}", config.lambda);
    println!("  Search depth: {}", config.search_depth);
    println!("  Max moves per game: {}", config.max_moves_per_game);
    println!("  Min batch size: {}", config.min_batch_size);
    println!();

    // Create trainer
    let mut trainer = NNUETrainer::new(weights, config.clone());

    let start_time = Instant::now();

    // Training loop
    for iteration in 0..config.iterations {
        let iter_start = Instant::now();
        let mut game_results = vec![0; 3]; // [wins, losses, draws]

        // Get current weights from trainer
        let current_weights = trainer.get_weights().clone();
        
        // Update evaluator with current weights
        evaluator.enable_nnue_with_weights_internal(current_weights.clone());
        // Also update search engine's evaluator
        search_engine.get_evaluator_mut().enable_nnue_with_weights_internal(current_weights.clone());
        
        // Play self-play games
        for _game_num in 0..config.games_per_iteration {
            let game = play_self_play_game(&mut search_engine, &mut evaluator, &current_weights, &config);
            
            // Count results
            match game.result {
                GameResult::Win => game_results[0] += 1,
                GameResult::Loss => game_results[1] += 1,
                GameResult::Draw => game_results[2] += 1,
            }

            // Add to trainer (this updates weights internally)
            trainer.add_training_game(game);
        }

        // Update evaluator with latest weights after batch
        let new_weights = trainer.get_weights().clone();
        evaluator.enable_nnue_with_weights_internal(new_weights.clone());
        search_engine.get_evaluator_mut().enable_nnue_with_weights_internal(new_weights);

        // Get training statistics
        let stats = trainer.get_stats();
        let _iter_duration = iter_start.elapsed();

        println!(
            "Iteration {}/{}: {} games (W:{} L:{} D:{}), {} positions, avg game: {:.1} moves",
            iteration + 1,
            config.iterations,
            config.games_per_iteration,
            game_results[0],
            game_results[1],
            game_results[2],
            stats.positions_processed,
            stats.avg_game_length
        );
        println!(
            "  Training: {} weight updates, avg TD error: {:.6}, avg weight change: {:.6}, max change: {:.6}",
            stats.weight_updates,
            stats.avg_td_error,
            stats.avg_weight_change,
            stats.max_weight_change
        );
        
        // Show if training is making progress
        if iteration > 0 && iteration % 10 == 0 {
            // Compare with previous iterations
            println!("  Progress: TD error {}, weight changes {}", 
                if stats.avg_td_error < 0.001 { "decreasing" } else { "stable" },
                if stats.avg_weight_change > 0.0001 { "active" } else { "minimal" }
            );
        }

        // Save weights periodically
        if (iteration + 1) % 10 == 0 {
            let save_path = format!("nnue_weights_iter_{}.json", iteration + 1);
            if let Err(e) = trainer.get_weights().save(&save_path) {
                eprintln!("Failed to save weights: {}", e);
            } else {
                println!("  ✓ Saved weights to {} ({:.1} MB)", save_path, 
                    std::fs::metadata(&save_path).map(|m| m.len() as f64 / 1_000_000.0).unwrap_or(0.0));
            }
        }
        
        // Print summary every 10 iterations
        if (iteration + 1) % 10 == 0 {
            let summary_stats = trainer.get_stats();
            println!("\n  Training Summary:");
            println!("    Total games: {}", summary_stats.games_processed);
            println!("    Total positions: {}", summary_stats.positions_processed);
            println!("    Total weight updates: {}", summary_stats.weight_updates);
            println!("    Avg positions/game: {:.1}", summary_stats.avg_positions_per_game);
            println!();
        }
    }

    let total_duration = start_time.elapsed();
    println!("\n=== Training Complete ===");
    println!("Total time: {:.2}s", total_duration.as_secs_f64());

    // Save final weights
    let final_path = "nnue_weights_trained.json";
    if let Err(e) = trainer.get_weights().save(final_path) {
        eprintln!("Failed to save final weights: {}", e);
    } else {
        println!("Final weights saved to {}", final_path);
    }
}

