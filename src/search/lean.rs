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

/// Selected main search: 0 = unset (environment decides), 1 = lean, 2 = classic.
static SEARCH_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Select the main search for the whole process (USI `SearchAlgorithm`).
pub fn set_lean_search(lean: bool) {
    SEARCH_MODE.store(if lean { 1 } else { 2 }, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the lean search is selected. Default lean; `SHOGI_LEAN=0` selects the classic
/// search unless the USI option has been set.
pub fn lean_enabled() -> bool {
    match SEARCH_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            static ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *ENV.get_or_init(|| std::env::var("SHOGI_LEAN").map_or(true, |v| v != "0"))
        }
    }
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
            return self.lean_q_entry(
                board,
                captured_pieces,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
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
            let q = self.lean_q_entry(
                board,
                captured_pieces,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
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
        if diag() {
            let d = (depth as usize).min(15);
            DIAG_LEGAL[d].fetch_add(legal_moves.len() as u64, Ordering::Relaxed);
            DIAG_MOVE_NODES[d].fetch_add(1, Ordering::Relaxed);
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
            // A check that loses material: not extended, and prunable like any
            // other move (see `lean_check_see`).
            let losing_check = mv.gives_check && lean_check_see() && {
                let see = if mv.is_capture {
                    crate::search::move_ordering::see_calculation::calculate_see_internal(
                        mv, board,
                    )
                } else {
                    crate::search::move_ordering::see_calculation::see_quiet(mv, board)
                };
                see < 0
            };

            // Shallow pruning, once a move has been searched, never of checks.
            if !in_check
                && best_score > -MATE_THRESHOLD
                && (!mv.gives_check || losing_check)
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
                    && !losing_check
                    && !search_toggles().no_check_ext
                    && self.check_extensions_on_path < self.current_depth.max(1) as u32,
            );
            let new_depth = depth - 1 + ext;

            if diag() {
                DIAG_SEARCHED[(depth as usize).min(15)].fetch_add(1, Ordering::Relaxed);
            }
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
                if diag() {
                    DIAG_REDUCTION[(depth as usize).min(15)].fetch_add(r as u64, Ordering::Relaxed);
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

/// Plies of captures (and, at the first ply, checks) the lean quiescence searches.
const LEAN_Q_MAX_PLY: i32 = 8;

/// One slot of the lean quiescence table: a direct-mapped cache of bounds, one per
/// thread (parallel workers each have their own), replaced always.
#[derive(Clone, Copy, Default)]
struct QSlot {
    key: u64,
    score: i32,
    /// 1 = exact, 2 = lower bound, 3 = upper bound; 0 = empty.
    flag: u8,
    /// Whether the search below included checks (it is then usable for either kind).
    checks: bool,
}

const Q_SLOTS: usize = 1 << 18;

thread_local! {
    static QTABLE: std::cell::RefCell<Vec<QSlot>> =
        std::cell::RefCell::new(vec![QSlot::default(); Q_SLOTS]);
}

/// Whether checks that lose material (negative SEE) lose the check extension
/// and the exemption from pruning (`SHOGI_LEAN_CHECK_SEE=0` restores both).
/// With full hands a position has dozens of checking drops, most of which hang
/// the piece; extended and unpruned, they made the tree grow ~3.7x per ply
/// against YaneuraOu's ~2 (Session 34, four-piece loss of 2026-10-11, move 50:
/// 6.7M nodes to depth 11, 2.2-2.7M with this).
fn lean_check_see() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("SHOGI_LEAN_CHECK_SEE").map_or(true, |v| v != "0"))
}

/// Whether the lean quiescence searches quiet checks at its first ply
/// (`SHOGI_LEAN_Q_CHECKS=0` turns them off; experiment).
fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |v| v != "0")
}

/// Quiescence: checks that lose material by SEE are pruned like other moves
/// (1: all checks, 2: quiet checks only).
fn lean_q_losing_checks_pruned() -> u8 {
    static ON: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SHOGI_Q_PRUNE_LOSING_CHECKS").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
    })
}

/// Quiescence in check: quiet evasions that lose material are skipped once
/// some evasion has a non-mated score.
fn lean_q_evasion_pruning() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| env_flag("SHOGI_Q_EVASION_PRUNING", false))
}

fn lean_q_checks() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("SHOGI_LEAN_Q_CHECKS").map_or(true, |v| v != "0"))
}

/// Whether the lean search uses its own quiescence (default; `SHOGI_LEAN_Q=0`
/// selects the shared one, for comparison).
fn lean_q_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("SHOGI_LEAN_Q").map_or(true, |v| v != "0"))
}

impl SearchEngine {
    /// Entry from the lean main search at depth 0.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lean_q_entry(
        &mut self,
        board: &mut BitboardBoard,
        captured: &CapturedPieces,
        player: Player,
        alpha: i32,
        beta: i32,
        start_time: &TimeSource,
        time_limit_ms: u32,
    ) -> i32 {
        if lean_q_enabled() {
            self.lean_qsearch(board, captured, player, alpha, beta, start_time, time_limit_ms, 0)
        } else {
            self.quiescence_search(
                board,
                captured,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                QUIESCENCE_ENTRY_DEPTH,
            )
        }
    }

    /// Quiescence for the lean search: stand pat, then captures and promotions
    /// (and checks at the first ply) in most-valuable-victim order, skipping those
    /// that lose material by SEE or cannot reach alpha. In check, every evasion.
    /// Fail-soft; bounds are stored against the original window.
    #[allow(clippy::too_many_arguments)]
    fn lean_qsearch(
        &mut self,
        board: &mut BitboardBoard,
        captured: &CapturedPieces,
        player: Player,
        mut alpha: i32,
        beta: i32,
        start_time: &TimeSource,
        time_limit_ms: u32,
        qply: i32,
    ) -> i32 {
        self.quiescence_stats.nodes_searched += 1;
        self.search_statistics.increment_quiescence_nodes();
        if diag() {
            DIAG_QUIESCENCE_NODES.fetch_add(1, Ordering::Relaxed);
        }
        if self.should_stop(start_time, time_limit_ms) {
            return self.evaluator.evaluate(board, player, captured);
        }
        let in_check = board.is_king_in_check(player, captured);
        if qply >= LEAN_Q_MAX_PLY && !in_check {
            return self.evaluator.evaluate(board, player, captured);
        }
        let with_checks = qply == 0 && lean_q_checks();

        let key = self.hash_calculator.get_position_hash(board, player, captured);
        let slot = (key as usize) & (Q_SLOTS - 1);
        let hit = QTABLE.with(|t| t.borrow()[slot]);
        if hit.flag != 0 && hit.key == key && (hit.checks || !with_checks) {
            let s = hit.score;
            match hit.flag {
                1 => return s,
                2 if s >= beta => return s,
                3 if s <= alpha => return s,
                _ => {}
            }
        }

        let alpha_orig = alpha;
        let mut best = -INF;
        let mut stand_pat = -INF;
        if !in_check {
            stand_pat = self.evaluator.evaluate(board, player, captured);
            if stand_pat >= beta {
                return stand_pat;
            }
            best = stand_pat;
            alpha = alpha.max(stand_pat);
        }

        if diag() && in_check {
            DIAG_Q[0].fetch_add(1, Ordering::Relaxed);
        }
        let mut moves = if in_check {
            self.move_generator.generate_legal_moves(board, player, captured)
        } else {
            self.generate_noisy_moves(board, player, captured, with_checks)
        };
        if in_check && moves.is_empty() {
            return MATED_SCORE + self.node_ply + qply;
        }
        // Most valuable victim, then least valuable attacker; promotions next.
        moves.sort_by_cached_key(|m| {
            let promo = if m.is_promotion { 300 } else { 0 };
            -(m.captured_piece_value() * 16 + promo * 8 - m.piece_value())
        });

        for mv in &moves {
            let see_filtered = if in_check {
                // A quiet evasion that loses material, once some evasion holds.
                lean_q_evasion_pruning()
                    && !mv.is_capture
                    && best > -MATE_THRESHOLD
            } else {
                !mv.gives_check
                    || match lean_q_losing_checks_pruned() {
                        1 => true,
                        2 => !mv.is_capture,
                        _ => false,
                    }
            };
            if !in_check && !mv.gives_check {
                let promo = if mv.is_promotion { 300 } else { 0 };
                // Even winning the piece outright does not reach alpha.
                if stand_pat + mv.captured_piece_value() + promo + 200 <= alpha {
                    continue;
                }
            }
            if see_filtered {
                let see = if mv.is_capture {
                    crate::search::move_ordering::see_calculation::calculate_see_internal(
                        mv, board,
                    )
                } else {
                    crate::search::move_ordering::see_calculation::see_quiet(mv, board)
                };
                if see < 0 {
                    continue;
                }
            }

            if diag() {
                let quiet = !mv.is_capture && !mv.is_promotion;
                let drop = mv.from.is_none();
                let (all, q, d) = if in_check {
                    (Some(1), 2, 3)
                } else {
                    (None, 4, 5)
                };
                if let Some(a) = all {
                    DIAG_Q[a].fetch_add(1, Ordering::Relaxed);
                }
                if quiet && (in_check || mv.gives_check) {
                    DIAG_Q[q].fetch_add(1, Ordering::Relaxed);
                    if drop {
                        DIAG_Q[d].fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            let move_info = board.make_move_with_info(mv);
            self.nnue_make_move(&move_info);
            let mut new_captured = captured.clone();
            if let Some(ref c) = move_info.captured_piece {
                new_captured.add_piece(c.piece_type, player);
            } else if mv.from.is_none() {
                new_captured.remove_piece(mv.piece_type, player);
            }
            let score = -self.lean_qsearch(
                board,
                &new_captured,
                player.opposite(),
                -beta,
                -alpha,
                start_time,
                time_limit_ms,
                qply + 1,
            );
            self.nnue_unmake_move();
            board.unmake_move(&move_info);
            if self.search_aborted {
                return best.max(score);
            }

            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        break;
                    }
                }
            }
        }
        // In check with every evasion skipped cannot happen (none are pruned).
        if best == -INF {
            best = alpha_orig;
        }

        if !self.search_aborted && best.abs() < MATE_THRESHOLD {
            let flag = if best >= beta {
                2
            } else if best <= alpha_orig {
                3
            } else {
                1
            };
            QTABLE.with(|t| {
                t.borrow_mut()[slot] = QSlot { key, score: best, flag, checks: with_checks };
            });
        }
        best
    }
}
