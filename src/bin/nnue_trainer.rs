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
use rand::Rng;
use std::time::Instant;
use std::io::{self, Write};

/// Format seconds into a human-readable time string (HH:MM:SS)
fn format_time(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

/// Play random legal moves to create a diverse starting position.
///
/// Returns the number of random moves played. The board, captured_pieces,
/// current_player, and accumulator are all updated in place. This ensures
/// each self-play game starts from a different position, providing diverse
/// training data even when the PST evaluator is deterministic.
fn play_random_opening(
    board: &mut BitboardBoard,
    captured_pieces: &mut CapturedPieces,
    current_player: &mut Player,
    accumulator: &mut NNUEAccumulator,
    weights: &NNUEWeights,
    random_plies: usize,
) -> usize {
    let move_generator = MoveGenerator::new();
    let mut rng = rand::thread_rng();
    let mut plies_played = 0;

    // Play between 2 and random_plies random moves (variable per game for more diversity)
    let actual_plies = if random_plies > 2 {
        rng.gen_range(2..=random_plies)
    } else {
        random_plies
    };

    for _ in 0..actual_plies {
        let legal_moves = move_generator.generate_legal_moves(board, *current_player, captured_pieces);
        if legal_moves.is_empty() {
            break;
        }

        // Pick a random legal move
        let idx = rng.gen_range(0..legal_moves.len());
        let mv = &legal_moves[idx];

        // Update NNUE accumulator incrementally
        let from_piece = mv.from.and_then(|pos| board.get_piece(pos));
        let captured_piece = if mv.is_capture {
            board.get_piece(mv.to)
        } else {
            None
        };
        if let Some(piece) = from_piece {
            accumulator.update_move(mv.from, mv.to, piece, captured_piece, weights);
        }

        // Make the move
        if let Some(captured) = board.make_move(mv) {
            captured_pieces.add_piece(captured.piece_type, *current_player);
        }

        *current_player = current_player.opposite();
        plies_played += 1;
    }

    plies_played
}

/// Play a self-play game and return training data.
///
/// Uses PST evaluator for game play (strong, deterministic) while recording
/// NNUE accumulator states for training. This breaks the bootstrap problem:
/// the NNUE learns from PST evaluations (external oracle) rather than from
/// its own weak, near-zero evaluations.
///
/// Each game begins with 2-8 random legal moves to ensure diverse starting
/// positions, preventing all games from being identical.
fn play_self_play_game(
    search_engine: &mut SearchEngine,
    pst_evaluator: &mut PositionEvaluator,
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

    // Create NNUE accumulator for tracking (student network)
    let (hidden_size_1, hidden_size_2) = weights.hidden_sizes();
    let mut accumulator = NNUEAccumulator::new(hidden_size_1, hidden_size_2);

    // Refresh accumulator for initial position
    accumulator.refresh(&board, weights);

    // Play random opening moves for diverse starting positions.
    // This prevents all games from being identical (PST is deterministic).
    let random_plies = play_random_opening(
        &mut board,
        &mut captured_pieces,
        &mut current_player,
        &mut accumulator,
        weights,
        8, // Up to 8 random plies (4 moves per side)
    );
    move_count += random_plies;

    // Track position occurrences for 3-fold repetition detection
    let mut position_counts = std::collections::HashMap::<u64, u8>::new();

    while move_count < config.max_moves_per_game {
        // Check for game over
        if let Some(result) = check_game_over(&board, &captured_pieces, current_player) {
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
            return TrainingGame {
                positions,
                result: GameResult::Draw,
            };
        }

        // Get PST evaluation (oracle/teacher) - this is the target NNUE should learn
        let pst_eval = pst_evaluator.evaluate(&board, current_player, &captured_pieces);

        // Also compute NNUE evaluation for the current position (student prediction)
        let nnue_eval = accumulator.evaluate(weights);

        // Extract active features
        let active_features = extract_active_features(&board);

        // Store position with PST evaluation as training target and NNUE eval as current prediction
        positions.push(TrainingPosition {
            active_features,
            accumulator: accumulator.clone(),
            evaluation: nnue_eval,      // NNUE's current prediction (student)
            pst_evaluation: pst_eval,   // PST evaluation (teacher/oracle target)
            player: current_player,
            move_made: None,
            outcome: None,
            td_target: None,
        });

        // Search uses PST evaluator (not NNUE) for move selection
        let mut iterative_search = IterativeDeepening::new(config.search_depth, config.time_per_move_ms, None);
        let best_move = iterative_search.search(search_engine, &board, &captured_pieces, current_player);

        if let Some((mv, _score)) = best_move {
            // Update last position's move
            if let Some(last_pos) = positions.last_mut() {
                last_pos.move_made = Some(mv.clone());
            }

            // Update NNUE accumulator incrementally
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
    
    // PST evaluator for self-play (teacher/oracle) - no NNUE
    let mut pst_evaluator = PositionEvaluator::new();

    // Search engine uses PST evaluation for move selection (not NNUE).
    // TT size of 64 MB allows deeper search to reach intended depth within time limits.
    // (Previously 1 MB, which caused search to only reach depth 1.)
    let mut search_engine = SearchEngine::new(None, 64);

    // Training configuration - optimized for continued training
    // Production training config after Phase 2 fixes.
    // PST evaluator plays self-play games; NNUE learns to predict PST evaluations.
    let mut config = NNUETrainingConfig::default();
    config.games_per_iteration = 10; // Balanced: enough diversity without being slow
    config.iterations = 100; // Enough to measure convergence
    config.learning_rate = 0.005; // Moderate learning rate
    config.lambda = 0.7; // TD(λ) parameter
    config.search_depth = 3; // Depth 3 for fast game generation (PST at depth 1-3 is sufficient)
    config.time_per_move_ms = 50; // Fast per-move: prioritize more games over deeper search
    config.max_moves_per_game = 150; // Cap game length to prevent very slow late-game positions
    config.min_batch_size = 100; // Update after accumulating enough positions

    println!("Training Configuration:");
    println!("  Architecture: 256 -> 32 -> 1");
    println!("  Games per iteration: {}", config.games_per_iteration);
    println!("  Total iterations: {}", config.iterations);
    println!("  Learning rate: {}", config.learning_rate);
    println!("  Lambda (TD): {}", config.lambda);
    println!("  Search depth: {}", config.search_depth);
    println!("  Max moves per game: {}", config.max_moves_per_game);
    println!("  Min batch size: {}", config.min_batch_size);
    println!("  Time per move: {} ms", config.time_per_move_ms);
    println!("  TT size: 64 MB");
    println!("  Random opening plies: 2-8");
    println!();

    // Create trainer
    let mut trainer = NNUETrainer::new(weights, config.clone());

    let start_time = Instant::now();
    let mut iteration_times = Vec::new(); // Track iteration times for ETA calculation

    // Training loop
    for iteration in 0..config.iterations {
        let iter_start = Instant::now();
        let mut game_results = vec![0; 3]; // [wins, losses, draws]

        // Calculate progress
        let progress_pct = ((iteration + 1) as f64 / config.iterations as f64) * 100.0;
        let elapsed = start_time.elapsed();
        
        // Calculate ETA based on average iteration time
        let eta_seconds = if iteration > 0 && !iteration_times.is_empty() {
            let avg_iter_time = iteration_times.iter().sum::<u64>() as f64 / iteration_times.len() as f64;
            let remaining_iterations = config.iterations - (iteration + 1);
            avg_iter_time * remaining_iterations as f64
        } else {
            0.0
        };

        // Format time strings
        let elapsed_str = format_time(elapsed.as_secs());
        let eta_str = if eta_seconds > 0.0 {
            format_time(eta_seconds as u64)
        } else {
            "calculating...".to_string()
        };

        println!("\n{}", "=".repeat(80));
        println!("Iteration {}/{} ({:.1}% complete)", iteration + 1, config.iterations, progress_pct);
        println!("Time elapsed: {} | Estimated remaining: {}", elapsed_str, eta_str);
        println!("{}", "-".repeat(80));

        // Get current NNUE weights from trainer (for accumulator tracking during games)
        let current_weights = trainer.get_weights().clone();

        // Search engine uses PST evaluator (no NNUE) for move selection.
        // This ensures games are meaningful and produce real wins/losses.

        // Play self-play games using PST for moves, recording NNUE states for training
        for game_num in 0..config.games_per_iteration {
            let game = play_self_play_game(&mut search_engine, &mut pst_evaluator, &current_weights, &config);

            // Count results
            match game.result {
                GameResult::Win => game_results[0] += 1,
                GameResult::Loss => game_results[1] += 1,
                GameResult::Draw => game_results[2] += 1,
            }

            // Add to trainer (this updates weights internally)
            trainer.add_training_game(game);

            // Show progress within iteration
            if (game_num + 1) % 10 == 0 || (game_num + 1) == config.games_per_iteration {
                print!("\r  Games: {}/{}", game_num + 1, config.games_per_iteration);
                io::stdout().flush().unwrap();
            }
        }
        println!(); // New line after progress indicator

        // Get training statistics
        let stats = trainer.get_stats();
        let iter_duration = iter_start.elapsed();
        iteration_times.push(iter_duration.as_secs());
        
        // Keep only last 10 iteration times for ETA calculation
        if iteration_times.len() > 10 {
            iteration_times.remove(0);
        }

        println!("  Results: {} games (W:{} L:{} D:{}), {} positions, avg game: {:.1} moves",
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
        println!("  Iteration time: {:.1}s", iter_duration.as_secs_f64());
        
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
    println!("\n{}", "=".repeat(80));
    println!("=== Training Complete ===");
    println!("{}", "=".repeat(80));
    println!("Total time: {} ({:.2} seconds)", format_time(total_duration.as_secs()), total_duration.as_secs_f64());
    println!("Total iterations: {}", config.iterations);
    println!("Total games: {}", config.iterations * config.games_per_iteration);
    println!("Average time per iteration: {:.1}s", total_duration.as_secs_f64() / config.iterations as f64);

    // Save final weights
    let final_path = "nnue_weights_trained.json";
    if let Err(e) = trainer.get_weights().save(final_path) {
        eprintln!("Failed to save final weights: {}", e);
    } else {
        println!("Final weights saved to {}", final_path);
    }
}

