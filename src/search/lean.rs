//! The lean main search (`SHOGI_LEAN=1`): principal variation search with the
//! pruning and reductions strong shogi and chess engines share, written as one
//! function instead of the layered managers of `negamax_with_context_inner`.
//!
//! Session 34 measured the old search's tree at ~4.6x nodes per ply on a
//! user-game position (YaneuraOu ~2.3x): depth 7 in 12M nodes against
//! YaneuraOu's depth 18 in 3M. Its pruning was gated on "king safety" and
//! "complexity" derived from the static score alone, so the side behind --
//! White in every handicap game -- was barely pruned; the root searched every
//! move with an open window; and the transposition table's move was only used
//! from entries at least as deep as the current search, so the previous
//! iteration's best move never ordered the next one.
//!
//! Everything else is shared with the old search: the iterative-deepening
//! driver, time control, quiescence, move generation and ordering, the
//! transposition table, the mate solver, and the repetition rules (the path
//! repetition with perpetual check, and the superior/inferior hand rule).
//!
//! A child module of `search_engine` so it can use the engine's private state.

use super::*;

/// Reductions by depth and move number, from `ln(depth) * ln(move) / 2.25`.
fn lmr_table() -> &'static [[u8; 64]; 64] {
    static T: std::sync::OnceLock<[[u8; 64]; 64]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [[0u8; 64]; 64];
        for (d, row) in t.iter_mut().enumerate().skip(1) {
            for (m, r) in row.iter_mut().enumerate().skip(1) {
                *r = (0.5 + (d as f64).ln() * (m as f64).ln() / 2.25) as u8;
            }
        }
        t
    })
}

/// Whether the lean search is selected (`SHOGI_LEAN`, read once).
pub(crate) fn lean_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SHOGI_LEAN").is_some())
}

const INF: i32 = 1_000_000;

impl SearchEngine {
    /// A node of the lean search: the superior/inferior hand rule, then
    /// [`Self::lean_inner`]. Same path conventions as `negamax_with_context`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lean_node(
        &mut self,
        board: &mut BitboardBoard,
        captured_pieces: &CapturedPieces,
        player: Player,
        depth: i32,
        alpha: i32,
        beta: i32,
        start_time: &TimeSource,
        time_limit_ms: u32,
        hash_history: &mut Vec<u64>,
        cut_node: bool,
        null_ok: bool,
        prev_move: Option<&Move>,
    ) -> i32 {
        let entry = self.hand_path_entry(board, player, captured_pieces);
        if !search_toggles().no_hand_repetition {
            if let Some(score) = self.hand_repetition_score(&entry) {
                return score;
            }
        }
        self.hand_path.push(entry);
        let path_len = hash_history.len();
        let score = self.lean_inner(
            board,
            captured_pieces,
            player,
            depth,
            alpha,
            beta,
            start_time,
            time_limit_ms,
            hash_history,
            cut_node,
            null_ok,
            prev_move,
        );
        hash_history.truncate(path_len);
        self.hand_path.pop();
        score
    }

    #[allow(clippy::too_many_arguments)]
    fn lean_inner(
        &mut self,
        board: &mut BitboardBoard,
        captured_pieces: &CapturedPieces,
        player: Player,
        depth: i32,
        mut alpha: i32,
        mut beta: i32,
        start_time: &TimeSource,
        time_limit_ms: u32,
        hash_history: &mut Vec<u64>,
        cut_node: bool,
        null_ok: bool,
        prev_move: Option<&Move>,
    ) -> i32 {
        let pv_node = beta - alpha > 1;
        let position_hash = self.hash_calculator.get_position_hash(board, player, captured_pieces);
        let in_check = board.is_king_in_check(player, captured_pieces);

        // Repetition on the search path (perpetual check loses), as in negamax.
        if let Some(j) = hash_history.iter().rposition(|h| same_position(*h, position_hash)) {
            return self.repetition_score(hash_history, j, in_check);
        }
        hash_history.push(path_entry(position_hash, in_check));
        let ply = (hash_history.len().saturating_sub(self.root_path_len.max(1))) as i32;
        self.node_ply = ply;

        if depth <= 0 {
            return self.quiescence_search(
                board,
                captured_pieces,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                QUIESCENCE_ENTRY_DEPTH,
            );
        }

        self.search_statistics.increment_nodes();
        self.core_search_metrics.total_nodes += 1;
        self.search_statistics.update_seldepth(ply.clamp(0, 255) as u8);
        if diag() {
            DIAG_NODES[(depth as usize).min(15)].fetch_add(1, Ordering::Relaxed);
            DIAG_INTERIOR_NODES.fetch_add(1, Ordering::Relaxed);
        }
        if self.should_stop(start_time, time_limit_ms) {
            return alpha;
        }

        // Mate distance pruning.
        alpha = alpha.max(MATED_SCORE + ply);
        beta = beta.min(-(MATED_SCORE + ply + 1));
        if alpha >= beta {
            return alpha;
        }

        // Transposition table: any depth gives a move; enough depth, a bound.
        let tt_entry = if search_toggles().no_tt { None } else { self.tt_probe(position_hash, 0) };
        let tt_move = tt_entry.as_ref().and_then(|e| e.best_move.clone());
        if let Some(ref e) = tt_entry {
            if !pv_node && e.depth as i32 >= depth {
                let s = mate_score_from_tt(e.score, ply);
                let usable = match e.flag {
                    TranspositionFlag::Exact => true,
                    TranspositionFlag::LowerBound => s >= beta,
                    TranspositionFlag::UpperBound => s <= alpha,
                };
                if usable {
                    return s;
                }
            }
        }

        let static_eval =
            if in_check { -INF } else { self.evaluate_position(board, player, captured_pieces) };
        let prunable = !pv_node && !in_check && !search_toggles().no_pruning;

        // Reverse futility: far enough above beta that a quiet move will not
        // bring it back down within a few plies.
        if prunable && depth <= 6 && static_eval - 120 * depth >= beta && static_eval < MATE_THRESHOLD
        {
            return static_eval;
        }

        // Razoring: hopeless at the frontier, let quiescence decide.
        if prunable && depth <= 2 && static_eval + 250 * depth < alpha {
            let q = self.quiescence_search(
                board,
                captured_pieces,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                QUIESCENCE_ENTRY_DEPTH,
            );
            if q < alpha {
                return q;
            }
        }

        // Null move: pass; if the opponent still cannot reach beta, prune.
        if prunable
            && null_ok
            && depth >= 3
            && static_eval >= beta
            && !search_toggles().no_null_move
            && beta > -MATE_THRESHOLD
        {
            let r = 3 + depth / 3 + ((static_eval - beta) / 200).min(3);
            self.null_move_stats.attempts += 1;
            let path_len = hash_history.len();
            let s = -self.lean_node(
                board,
                captured_pieces,
                player.opposite(),
                depth - r,
                -beta,
                -beta + 1,
                start_time,
                time_limit_ms,
                hash_history,
                !cut_node,
                false,
                None,
            );
            hash_history.truncate(path_len);
            if s >= beta {
                self.null_move_stats.cutoffs += 1;
                return if s >= MATE_THRESHOLD { beta } else { s };
            }
        }

        // Internal iterative deepening: a PV node without a table move.
        let mut tt_move = tt_move;
        if tt_move.is_none() && pv_node && depth >= 5 {
            self.lean_node(
                board,
                captured_pieces,
                player,
                depth - 2,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                hash_history,
                cut_node,
                null_ok,
                prev_move,
            );
            tt_move = self.tt_probe(position_hash, 0).and_then(|e| e.best_move);
        }

        let legal_moves = self.move_generator.generate_legal_moves(board, player, captured_pieces);
        if legal_moves.is_empty() {
            // No legal move is a loss in shogi, in check or not.
            return MATED_SCORE + ply;
        }
        self.initialize_move_orderer();
        let moves = self.order_moves_for_negamax(
            &legal_moves,
            board,
            captured_pieces,
            player,
            depth.clamp(1, 255) as u8,
            alpha,
            beta,
            tt_move.as_ref(),
            prev_move,
        );

        let alpha_orig = alpha;
        let mut best_score = -INF;
        let mut best_move: Option<Move> = None;
        let mut move_count = 0usize;
        let lmr = lmr_table();

        for mv in &moves {
            move_count += 1;
            let quiet = !mv.is_capture && !mv.is_promotion;
            let is_tt = tt_move.as_ref().is_some_and(|t| self.moves_equal(mv, t));

            // Shallow pruning, once a move has been searched, never of checks.
            if !in_check
                && best_score > -MATE_THRESHOLD
                && !mv.gives_check
                && !is_tt
                && !search_toggles().no_pruning
            {
                if quiet {
                    // Late move pruning.
                    if !pv_node && depth <= 8 && move_count > (3 + (depth * depth) as usize) {
                        continue;
                    }
                    // Futility.
                    if !pv_node && depth <= 6 && static_eval + 100 + 150 * depth <= alpha {
                        continue;
                    }
                    // Quiet moves and drops that lose material.
                    if depth <= 6
                        && crate::search::move_ordering::see_calculation::see_quiet(mv, board)
                            < -80 * depth
                    {
                        continue;
                    }
                } else if depth <= 6
                    && crate::search::move_ordering::see_calculation::calculate_see_internal(
                        mv, board,
                    ) < -150 * depth
                {
                    continue;
                }
            }

            // Check extension, bounded per path as in negamax.
            let ext = i32::from(
                mv.gives_check
                    && !search_toggles().no_check_ext
                    && self.check_extensions_on_path < self.current_depth.max(1) as u32,
            );
            let new_depth = depth - 1 + ext;

            let move_info = board.make_move_with_info(mv);
            self.nnue_make_move(&move_info);
            let mut new_captured = captured_pieces.clone();
            if let Some(ref captured) = move_info.captured_piece {
                new_captured.add_piece(captured.piece_type, player);
            } else if mv.from.is_none() {
                new_captured.remove_piece(mv.piece_type, player);
            }
            self.check_extensions_on_path += ext as u32;
            let path_len = hash_history.len();

            let mut score;
            if move_count == 1 {
                score = -self.lean_node(
                    board,
                    &new_captured,
                    player.opposite(),
                    new_depth,
                    -beta,
                    -alpha,
                    start_time,
                    time_limit_ms,
                    hash_history,
                    !pv_node && !cut_node,
                    true,
                    Some(mv),
                );
            } else {
                // Late move reductions for quiet moves past the first few.
                let mut r = 0;
                if depth >= 2 && move_count > 1 + usize::from(pv_node) && quiet && !mv.gives_check
                {
                    r = i32::from(lmr[(depth as usize).min(63)][move_count.min(63)]);
                    if pv_node {
                        r -= 1;
                    }
                    if cut_node {
                        r += 1;
                    }
                    if in_check || self.is_killer_move(mv) {
                        r -= 1;
                    }
                    r = r.clamp(0, new_depth - 1);
                }
                score = -self.lean_node(
                    board,
                    &new_captured,
                    player.opposite(),
                    new_depth - r,
                    -alpha - 1,
                    -alpha,
                    start_time,
                    time_limit_ms,
                    hash_history,
                    true,
                    true,
                    Some(mv),
                );
                hash_history.truncate(path_len);
                if score > alpha && r > 0 {
                    score = -self.lean_node(
                        board,
                        &new_captured,
                        player.opposite(),
                        new_depth,
                        -alpha - 1,
                        -alpha,
                        start_time,
                        time_limit_ms,
                        hash_history,
                        !cut_node,
                        true,
                        Some(mv),
                    );
                    hash_history.truncate(path_len);
                }
                if pv_node && score > alpha && score < beta {
                    score = -self.lean_node(
                        board,
                        &new_captured,
                        player.opposite(),
                        new_depth,
                        -beta,
                        -alpha,
                        start_time,
                        time_limit_ms,
                        hash_history,
                        false,
                        true,
                        Some(mv),
                    );
                }
            }
            hash_history.truncate(path_len);
            self.check_extensions_on_path -= ext as u32;
            self.nnue_unmake_move();
            board.unmake_move(&move_info);

            if self.search_aborted {
                return best_score.max(alpha_orig);
            }

            if score > best_score {
                best_score = score;
                if score > alpha {
                    best_move = Some(mv.clone());
                    alpha = score;
                    if alpha >= beta {
                        if diag() {
                            DIAG_CUTS[(depth as usize).min(15)].fetch_add(1, Ordering::Relaxed);
                            DIAG_CUT_INDEX[cut_index_bucket(move_count - 1)]
                                .fetch_add(1, Ordering::Relaxed);
                        }
                        if quiet {
                            self.update_killer_moves(mv.clone());
                            let d = depth.clamp(1, 255) as u8;
                            self.advanced_move_orderer.set_current_depth(d);
                            self.advanced_move_orderer.add_killer_move(mv.clone());
                            self.advanced_move_orderer.update_history_score(mv, d, Some(board));
                            if let Some(from) = mv.from {
                                self.history_table[from.row as usize][from.col as usize] +=
                                    depth * depth;
                            }
                            if let Some(prev) = prev_move {
                                self.advanced_move_orderer
                                    .add_counter_move(prev.clone(), mv.clone());
                            }
                        }
                        break;
                    }
                }
            }
        }

        if best_score == -INF {
            // Every move was pruned: nothing searched beats the static view.
            return alpha_orig;
        }

        let flag = if best_score >= beta {
            TranspositionFlag::LowerBound
        } else if best_score > alpha_orig {
            TranspositionFlag::Exact
        } else {
            TranspositionFlag::UpperBound
        };
        // The table keeps non-exact entries at shallow depth only with a move
        // (see `maybe_buffer_tt_store`); a fail-low node offers its first move,
        // as negamax does.
        let stored_move = best_move.or_else(|| moves.first().cloned());
        let d = depth.clamp(1, 255) as u8;
        let entry = TranspositionEntry::new(
            mate_score_to_tt(best_score, ply),
            d,
            flag,
            stored_move,
            position_hash,
            0,
            crate::types::EntrySource::MainSearch,
        );
        self.maybe_buffer_tt_store(entry, d, flag);
        best_score
    }
}
