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

/// What reverse futility returns (`SHOGI_RFP_RETURN`): 0 (default) the static
/// evaluation, 1 beta, 2 halfway between them. Session 37 found the static
/// evaluation, often far above beta, surfacing as root fail-highs that the
/// re-search then contradicted by 400-500 cp.
fn rfp_return() -> u8 {
    static MODE: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| {
        std::env::var("SHOGI_RFP_RETURN").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
    })
}

/// Rules of the lean search that `SHOGI_LEAN_OFF` can switch off, one bit each.
mod off {
    pub const RFP: u16 = 1;
    pub const RAZOR: u16 = 2;
    pub const NULL: u16 = 4;
    pub const HIST: u16 = 8;
    pub const LMP: u16 = 16;
    pub const FUT: u16 = 32;
    pub const SEE_QUIET: u16 = 64;
    pub const SEE_CAPTURE: u16 = 128;
    pub const LMR: u16 = 256;
    pub const Q_DELTA: u16 = 512;
    pub const Q_SEE: u16 = 1024;
}

/// Diagnostic: whether a pruning or reduction rule is switched off.
/// `SHOGI_LEAN_OFF` is a comma list of rfp, razor, null, hist, lmp, fut, see_quiet,
/// see_capture, lmr, q_delta, q_see (unset: everything on, the search unchanged).
/// Session 37 used it to ask which rule hides the opponent's attack.
fn lean_off(rule: u16) -> bool {
    static OFF: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    let bits = *OFF.get_or_init(|| {
        std::env::var("SHOGI_LEAN_OFF").map_or(0, |v| {
            v.split(',')
                .map(|s| match s.trim() {
                    "rfp" => off::RFP,
                    "razor" => off::RAZOR,
                    "null" => off::NULL,
                    "hist" => off::HIST,
                    "lmp" => off::LMP,
                    "fut" => off::FUT,
                    "see_quiet" => off::SEE_QUIET,
                    "see_capture" => off::SEE_CAPTURE,
                    "lmr" => off::LMR,
                    "q_delta" => off::Q_DELTA,
                    "q_see" => off::Q_SEE,
                    "" => 0,
                    other => panic!("SHOGI_LEAN_OFF: unknown rule {other}"),
                })
                .fold(0, |a, b| a | b)
        })
    });
    bits & rule != 0
}

/// Where the "improving" flag applies (`SHOGI_IMPROVING`, a bit set): 1 reverse
/// futility margin, 2 late-move pruning count, 4 one more reduction when not
/// improving. 0 (default) leaves the search unchanged.
fn improving_uses() -> u8 {
    static ON: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SHOGI_IMPROVING").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
    })
}

const EVAL_STACK_LEN: usize = 256;

thread_local! {
    /// Static evaluation by ply on the current path (`-INF` in check), one per
    /// search thread.
    static EVAL_STACK: std::cell::RefCell<[i32; EVAL_STACK_LEN]> =
        const { std::cell::RefCell::new([-INF; EVAL_STACK_LEN]) };
}

/// Records this node's static evaluation and returns whether it is above the
/// one two plies earlier -- the same side to move -- or four plies earlier when
/// that node was in check. With neither known (near the root, or both in
/// check), it counts as improving, as in Stockfish.
fn record_and_test_improving(ply: i32, static_eval: i32) -> bool {
    let Ok(p) = usize::try_from(ply) else { return true };
    if p >= EVAL_STACK_LEN {
        return true;
    }
    EVAL_STACK.with(|st| {
        let mut st = st.borrow_mut();
        st[p] = static_eval;
        if static_eval == -INF {
            return false;
        }
        let earlier = [2, 4].iter().find_map(|&k| {
            let e = *st.get(p.checked_sub(k)?)?;
            (e != -INF).then_some(e)
        });
        earlier.map_or(true, |e| static_eval > e)
    })
}

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

        // Repetition on the search path or of a game position (perpetual check
        // loses), as in negamax.
        if let Some(score) = self.path_repetition(hash_history, position_hash, in_check) {
            return score;
        }
        hash_history.push(path_entry(position_hash, in_check));
        let ply = (hash_history.len().saturating_sub(self.root_path_len.max(1))) as i32;
        self.node_ply = ply;
        // The table move, while a singular search asks how the others do without it.
        let excluded = excluded_at(ply);

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

        let hist = super::lean_hist::hist_uses();
        if hist != 0 {
            super::lean_hist::with(|h| {
                h.enter(ply, prev_move);
                h.clear_child_killers(ply);
            });
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
            if !pv_node && excluded.is_none() && e.depth as i32 >= depth {
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
        let prunable =
            !pv_node && !in_check && excluded.is_none() && !search_toggles().no_pruning;
        let uses = improving_uses();
        let improving = uses == 0 || record_and_test_improving(ply, static_eval);
        let improving_for = |bit: u8| uses & bit == 0 || improving;

        // Reverse futility: far enough above beta that a quiet move will not
        // bring it back down within a few plies.
        if prunable
            && !lean_off(off::RFP)
            && depth <= 6
            && static_eval - 120 * (depth - i32::from(uses & 1 != 0 && improving)) >= beta
            && static_eval < MATE_THRESHOLD
        {
            return match rfp_return() {
                1 => beta,
                2 => (static_eval + beta) / 2,
                _ => static_eval,
            };
        }

        // Razoring: hopeless at the frontier, let quiescence decide.
        if prunable && !lean_off(off::RAZOR) && depth <= 2 && static_eval + 250 * depth < alpha {
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
            && !lean_off(off::NULL)
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

        // ProbCut: a capture whose reduced search clears beta by a margin
        // would almost surely clear beta at full depth.
        if prunable && lean_probcut() && depth >= 5 && beta.abs() < MATE_THRESHOLD {
            let pc_beta = beta + 200;
            let tt_says_below = tt_entry.as_ref().is_some_and(|e| {
                e.depth as i32 >= depth - 3
                    && e.flag != TranspositionFlag::LowerBound
                    && mate_score_from_tt(e.score, ply) < pc_beta
            });
            if !tt_says_below {
                let captures =
                    self.move_generator.generate_legal_captures(board, player, captured_pieces);
                for mv in &captures {
                    let see =
                        crate::search::move_ordering::see_calculation::calculate_see_internal(
                            mv, board,
                        );
                    if static_eval + see < pc_beta {
                        continue;
                    }
                    let move_info = board.make_move_with_info(mv);
                    self.nnue_make_move(&move_info);
                    let mut new_captured = captured_pieces.clone();
                    if let Some(ref c) = move_info.captured_piece {
                        new_captured.add_piece(c.piece_type, player);
                    }
                    let path_len = hash_history.len();
                    let mut v = -self.lean_q_entry(
                        board,
                        &new_captured,
                        player.opposite(),
                        -pc_beta,
                        -pc_beta + 1,
                        start_time,
                        time_limit_ms,
                    );
                    hash_history.truncate(path_len);
                    if v >= pc_beta {
                        v = -self.lean_node(
                            board,
                            &new_captured,
                            player.opposite(),
                            depth - 4,
                            -pc_beta,
                            -pc_beta + 1,
                            start_time,
                            time_limit_ms,
                            hash_history,
                            !cut_node,
                            true,
                            Some(mv),
                        );
                        hash_history.truncate(path_len);
                    }
                    self.nnue_unmake_move();
                    board.unmake_move(&move_info);
                    if self.search_aborted {
                        return alpha;
                    }
                    if v >= pc_beta {
                        return v - (pc_beta - beta);
                    }
                }
            }
        }

        // Internal iterative deepening: a PV node without a table move. With
        // `SHOGI_LEAN_IIR`, any node without one is searched a ply shallower instead.
        let mut tt_move = tt_move;
        let depth = if lean_iir() && excluded.is_none() && tt_move.is_none() && depth >= 4 {
            depth - 1
        } else {
            depth
        };
        if !lean_iir() && tt_move.is_none() && pv_node && depth >= 5 {
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
        // Mate in one: a checking move that leaves the opponent no legal reply.
        // Found by search too, unless pruning drops it first -- a checking drop
        // that SEE scores as losing material is pruned like any other move.
        if lean_mate1() && excluded.is_none() {
            if let Some(score) = self.lean_mate_in_one(board, captured_pieces, player, &legal_moves, ply)
            {
                return score;
            }
        }
        if diag() {
            let d = (depth as usize).min(15);
            DIAG_LEGAL[d].fetch_add(legal_moves.len() as u64, Ordering::Relaxed);
            DIAG_MOVE_NODES[d].fetch_add(1, Ordering::Relaxed);
        }
        let moves = if hist & 1 != 0 {
            super::lean_hist::with(|h| h.order(&legal_moves, board, tt_move.as_ref(), ply))
        } else {
            self.initialize_move_orderer();
            self.order_moves_for_negamax(
                &legal_moves,
                board,
                captured_pieces,
                player,
                depth.clamp(1, 255) as u8,
                alpha,
                beta,
                tt_move.as_ref(),
                prev_move,
            )
        };
        // Singular extension: if every other move falls well short of the table
        // move's score in a reduced search, the table move gets one more ply.
        // If even the others reach beta, more than one move cuts: return.
        let mut singular = false;
        if lean_se() && excluded.is_none() && depth >= se_param("SHOGI_LEAN_SE_DEPTH", 6) {
            if let (Some(e), Some(ttm)) = (tt_entry.as_ref(), tt_move.as_ref()) {
                let s = mate_score_from_tt(e.score, ply);
                if e.depth as i32 >= depth - 3
                    && e.flag != TranspositionFlag::UpperBound
                    && s.abs() < MATE_THRESHOLD
                {
                    let sing_beta = s - se_param("SHOGI_LEAN_SE_MARGIN", 2) * depth;
                    set_excluded(ply, Some(ttm.clone()));
                    // The same node again: off the path, or it would be a repetition.
                    hash_history.pop();
                    let base = hash_history.len();
                    let v = self.lean_inner(
                        board,
                        captured_pieces,
                        player,
                        (depth - 1) / 2,
                        sing_beta - 1,
                        sing_beta,
                        start_time,
                        time_limit_ms,
                        hash_history,
                        cut_node,
                        false,
                        prev_move,
                    );
                    hash_history.truncate(base);
                    hash_history.push(path_entry(position_hash, in_check));
                    set_excluded(ply, None);
                    if self.search_aborted {
                        return alpha;
                    }
                    SE_TESTS.fetch_add(1, Ordering::Relaxed);
                    if v < sing_beta {
                        SE_EXT.fetch_add(1, Ordering::Relaxed);
                        singular = true;
                    } else if sing_beta >= beta {
                        SE_MULTICUT.fetch_add(1, Ordering::Relaxed);
                        return sing_beta;
                    }
                }
            }
        }

        let mut quiets_tried: Vec<Move> = Vec::new();
        let mut captures_tried: Vec<Move> = Vec::new();

        let alpha_orig = alpha;
        let mut best_score = -INF;
        let mut best_move: Option<Move> = None;
        let mut move_count = 0usize;
        let lmr = lmr_table();

        for mv in &moves {
            if excluded.as_ref().is_some_and(|x| self.moves_equal(mv, x)) {
                continue;
            }
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
                    // Quiet moves with a bad record, near the leaves.
                    if hist & 4 != 0
                        && !lean_off(off::HIST)
                        && !pv_node
                        && depth <= 3
                        && super::lean_hist::with(|h| h.quiet_score(mv, ply)) < -4000 * depth
                    {
                        continue;
                    }
                    // Late move pruning.
                    let lmp_count = (3 + (depth * depth) as usize) / (1 + usize::from(!improving_for(2)));
                    if !pv_node && !lean_off(off::LMP) && depth <= 8 && move_count > lmp_count {
                        continue;
                    }
                    // Futility.
                    if !pv_node && !lean_off(off::FUT) && depth <= 6 && static_eval + 100 + 150 * depth <= alpha {
                        continue;
                    }
                    // Quiet moves and drops that lose material.
                    if depth <= 6
                        && !lean_off(off::SEE_QUIET)
                        && crate::search::move_ordering::see_calculation::see_quiet(mv, board)
                            < -80 * depth
                    {
                        continue;
                    }
                } else if depth <= 6
                    && !lean_off(off::SEE_CAPTURE)
                    && crate::search::move_ordering::see_calculation::calculate_see_internal(
                        mv, board,
                    ) < -150 * depth
                {
                    continue;
                }
            }

            // Check and singular extensions, bounded per path as in negamax.
            let ext = i32::from(
                ((mv.gives_check && !losing_check && !search_toggles().no_check_ext)
                    || (singular && is_tt))
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
                if depth >= 2
                    && !lean_off(off::LMR)
                    && move_count > 1 + usize::from(pv_node)
                    && quiet
                    && !mv.gives_check
                {
                    r = i32::from(lmr[(depth as usize).min(63)][move_count.min(63)]);
                    if pv_node {
                        r -= 1;
                    }
                    if cut_node {
                        r += 1;
                    }
                    if !improving_for(4) {
                        r += 1;
                    }
                    let killer = if hist & 1 != 0 {
                        super::lean_hist::with(|h| h.is_killer(mv, ply))
                    } else {
                        self.is_killer_move(mv)
                    };
                    if in_check || killer {
                        r -= 1;
                    }
                    if hist & 2 != 0 {
                        r -= super::lean_hist::with(|h| h.quiet_score(mv, ply)) / 8000;
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
            if hist & 1 != 0 {
                if mv.is_capture {
                    captures_tried.push(mv.clone());
                } else if quiet {
                    quiets_tried.push(mv.clone());
                }
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
                        if hist & 1 != 0 {
                            super::lean_hist::with(|h| {
                                h.cutoff(mv, &quiets_tried, &captures_tried, depth, ply);
                            });
                        } else if quiet {
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
        // A search without the table move must not overwrite what the table knows.
        if excluded.is_none() {
            self.maybe_buffer_tt_store(entry, d, flag);
        }
        best_score
    }
}

/// Whether the table move may get a singular extension (`SHOGI_LEAN_SE=1`).
pub static SE_TESTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SE_EXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SE_MULTICUT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Singular-extension tuning: `SHOGI_LEAN_SE_DEPTH` (minimum depth) and
/// `SHOGI_LEAN_SE_MARGIN` (centipawns per ply below the table score).
fn se_param(name: &'static str, default: i32) -> i32 {
    static CACHE: std::sync::OnceLock<(i32, i32)> = std::sync::OnceLock::new();
    let (d, m) = *CACHE.get_or_init(|| {
        let get = |n: &str, dv: i32| std::env::var(n).ok().and_then(|v| v.parse().ok()).unwrap_or(dv);
        (get("SHOGI_LEAN_SE_DEPTH", 6), get("SHOGI_LEAN_SE_MARGIN", 2))
    });
    let _ = default;
    if name == "SHOGI_LEAN_SE_DEPTH" { d } else { m }
}

fn lean_se() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| env_flag("SHOGI_LEAN_SE", false))
}

thread_local! {
    /// The move a singular search leaves out, by ply, one per search thread.
    static EXCLUDED: std::cell::RefCell<Vec<Option<Move>>> =
        std::cell::RefCell::new(vec![None; EVAL_STACK_LEN]);
}

fn excluded_at(ply: i32) -> Option<Move> {
    let p = usize::try_from(ply).ok()?;
    EXCLUDED.with(|x| x.borrow().get(p).cloned().flatten())
}

fn set_excluded(ply: i32, mv: Option<Move>) {
    if let Ok(p) = usize::try_from(ply) {
        EXCLUDED.with(|x| {
            if let Some(slot) = x.borrow_mut().get_mut(p) {
                *slot = mv;
            }
        });
    }
}

/// Whether nodes without a table move are reduced a ply (`SHOGI_LEAN_IIR=1`).
fn lean_iir() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| env_flag("SHOGI_LEAN_IIR", false))
}

/// Whether non-PV nodes try ProbCut (`SHOGI_LEAN_PROBCUT=1`).
fn lean_probcut() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| env_flag("SHOGI_LEAN_PROBCUT", false))
}

/// Whether main-search nodes test for a mate in one (`SHOGI_LEAN_MATE1=1`).
fn lean_mate1() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| env_flag("SHOGI_LEAN_MATE1", false))
}

impl SearchEngine {
    /// The mate score if one of `legal_moves` checkmates at once.
    fn lean_mate_in_one(
        &mut self,
        board: &mut BitboardBoard,
        captured_pieces: &CapturedPieces,
        player: Player,
        legal_moves: &[Move],
        ply: i32,
    ) -> Option<i32> {
        for mv in legal_moves.iter().filter(|m| m.gives_check) {
            let info = board.make_move_with_info(mv);
            let mut after = captured_pieces.clone();
            if let Some(ref c) = info.captured_piece {
                after.add_piece(c.piece_type, player);
            } else if mv.from.is_none() {
                after.remove_piece(mv.piece_type, player);
            }
            let mate = !self.move_generator.has_legal_move(board, player.opposite(), &after);
            board.unmake_move(&info);
            if mate {
                return Some(-(MATED_SCORE + ply + 1));
            }
        }
        None
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
/// (1, default: all checks; 2: quiet checks only; 0: none). Session 35 SPRT
/// against 0 at 1 s/move, 16 threads: +95.6 +/- 67.7, PASS.
fn lean_q_losing_checks_pruned() -> u8 {
    static ON: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SHOGI_Q_PRUNE_LOSING_CHECKS").ok().and_then(|v| v.parse().ok()).unwrap_or(1)
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
            if !in_check && !mv.gives_check && !lean_off(off::Q_DELTA) {
                let promo = if mv.is_promotion { 300 } else { 0 };
                // Even winning the piece outright does not reach alpha.
                if stand_pat + mv.captured_piece_value() + promo + 200 <= alpha {
                    continue;
                }
            }
            if see_filtered && !lean_off(off::Q_SEE) {
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
