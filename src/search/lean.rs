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

/// Reductions by depth and move number, from `ln(depth) * ln(move) / 2.25`
/// (`lmr_div`).
fn lmr_table() -> &'static [[u8; 64]; 64] {
    static T: std::sync::OnceLock<[[u8; 64]; 64]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let div = f64::from(super::lean_params::lean_params().lmr_div) / 100.0;
        let mut t = [[0u8; 64]; 64];
        for (d, row) in t.iter_mut().enumerate().skip(1) {
            for (m, r) in row.iter_mut().enumerate().skip(1) {
                *r = (0.5 + (d as f64).ln() * (m as f64).ln() / div) as u8;
            }
        }
        t
    })
}

/// Council root stack (`SHOGI_ROOT_STACK`, a bit set; 0 = off, the search as in `8934982`):
/// 1 LMR for late quiet root moves, 2 `lean_hist` ordering at the root, 4 root TT flag
/// against the original window, 8 aspiration windows in Lazy SMP helpers, 16 correction
/// history on the quiescence stand-pat, 32 helpers keep their quiescence table across moves.
/// Default 63 (all): S40 gate vs 0, 1 s, 4 threads, 86-8-55, LLR +3.05 PASS.
pub(super) fn root_stack() -> u32 {
    static ON: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SHOGI_ROOT_STACK").ok().and_then(|v| v.parse().ok()).unwrap_or(63)
    })
}

/// Reduction for the `move_number`-th root move (1-based) at `depth`: the interior
/// table, one less as for any PV node.
pub(super) fn root_reduction(depth: u8, move_number: usize) -> i32 {
    i32::from(lmr_table()[(depth as usize).min(63)][move_number.min(63)]) - 1
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

/// Diagnostic: `SHOGI_TRACE_LINE="m1 m2 ..."` (USI moves from the root) prints to stderr what the
/// search does at each node along that line: table cutoffs, node-level pruning, where the line's
/// move sits in the move order, whether it is pruned or reduced, and its score. Single thread
/// only (the matched prefix is per thread, but the output would interleave). Session 37 asked why
/// YaneuraOu sees a drop attack in 0.5M nodes that we miss at 32M.
fn trace_line() -> Option<&'static [String]> {
    static LINE: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    LINE.get_or_init(|| {
        std::env::var("SHOGI_TRACE_LINE")
            .ok()
            .map(|v| v.split_whitespace().map(String::from).collect())
    })
    .as_deref()
}

thread_local! {
    /// How many leading moves of the current path follow the traced line.
    static TRACE_MATCHED: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Whether the node at `ply` lies on the traced line.
fn trace_node(ply: i32) -> bool {
    trace_line().is_some() && TRACE_MATCHED.with(std::cell::Cell::get) == ply
}

/// Whether `mv`, at the node at `ply`, is the traced line's next move.
pub(super) fn trace_move(ply: i32, mv: &Move) -> bool {
    trace_node(ply)
        && trace_line()
            .and_then(|l| l.get(usize::try_from(ply).ok()?))
            .is_some_and(|m| *m == mv.to_usi_string())
}

/// Marks the path as following the line through `ply` moves (around a line move's search).
pub(super) fn trace_set(ply: i32) {
    TRACE_MATCHED.with(|m| m.set(ply));
}

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
        let pv_node = is_pv_window(alpha, beta);
        let lp = super::lean_params::lean_params();
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

        let hist = super::lean_hist::hist_uses();
        if depth <= 0 {
            // Quiescence's stand-pat correction reads the continuation key of
            // the move into this node (`prev_key(ply)`); record it here too, or
            // it reads whichever sibling last stood at this ply (council review,
            // 2026-11-09).
            if hist != 0 {
                super::lean_hist::with(|h| h.enter(ply, prev_move));
            }
            let q = self.lean_q_entry(
                board,
                captured_pieces,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                hash_history,
                true,
            );
            if trace_node(ply) {
                eprintln!("TRACE ply={ply} [{alpha},{beta}] quiescence -> {q}");
            }
            return q;
        }

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
                    if trace_node(ply) {
                        eprintln!(
                            "TRACE ply={ply} d={depth} [{alpha},{beta}] table cutoff {s} \
                             (entry depth {}, {:?})",
                            e.depth, e.flag
                        );
                    }
                    return s;
                }
            }
        }

        // Correction history (`lean_corr`): the evaluation adjusted by what
        // searches of positions with the same pawns, hand or previous move found.
        let corr_keys = (super::lean_corr::corr_enabled() && !in_check && excluded.is_none())
            .then(|| {
                let prev = if hist != 0 {
                    super::lean_hist::with(|h| h.prev_key(ply))
                } else {
                    super::lean_hist::NO_KEY
                };
                super::lean_corr::keys(board, captured_pieces, player, prev)
            });
        let static_eval = if in_check {
            -INF
        } else {
            let raw = self.evaluate_position(board, player, captured_pieces);
            match &corr_keys {
                Some(k) if raw.abs() < MATE_THRESHOLD => (raw
                    + super::lean_corr::with(|c| c.correction(k)))
                .clamp(-MATE_THRESHOLD + 1, MATE_THRESHOLD - 1),
                _ => raw,
            }
        };
        let prunable =
            !pv_node && !in_check && excluded.is_none() && !search_toggles().no_pruning;
        let uses = improving_uses();
        let improving = uses == 0 || record_and_test_improving(ply, static_eval);
        let improving_for = |bit: u8| uses & bit == 0 || improving;
        let traced = trace_node(ply);
        if traced {
            eprintln!(
                "TRACE ply={ply} d={depth} [{alpha},{beta}] {} eval={static_eval} tt={}",
                if pv_node { "pv" } else if cut_node { "cut" } else { "all" },
                tt_move.as_ref().map_or_else(|| "-".to_string(), Move::to_usi_string)
            );
        }

        // Reverse futility: far enough above beta that a quiet move will not
        // bring it back down within a few plies.
        if prunable
            && !lean_off(off::RFP)
            && depth <= lp.rfp_depth
            && static_eval - lp.rfp_margin * (depth - i32::from(uses & 1 != 0 && improving)) >= beta
            && static_eval < MATE_THRESHOLD
        {
            if traced {
                eprintln!("TRACE ply={ply} d={depth} reverse futility (eval {static_eval} vs beta {beta})");
            }
            return match rfp_return() {
                1 => beta,
                2 => (static_eval + beta) / 2,
                _ => static_eval,
            };
        }

        // Razoring: hopeless at the frontier, let quiescence decide.
        if prunable && !lean_off(off::RAZOR) && depth <= lp.razor_depth && static_eval + lp.razor_margin * depth < alpha {
            let q = self.lean_q_entry(
                board,
                captured_pieces,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                hash_history,
                true,
            );
            if q < alpha {
                if traced {
                    eprintln!("TRACE ply={ply} d={depth} razored -> {q}");
                }
                return q;
            }
        }

        // Null move: pass; if the opponent still cannot reach beta, prune.
        if prunable
            && null_ok
            && depth >= lp.null_depth
            && static_eval >= beta
            && !search_toggles().no_null_move
            && !lean_off(off::NULL)
            && beta > -MATE_THRESHOLD
        {
            let r = lp.null_base
                + depth / lp.null_depth_div
                + ((static_eval - beta) / lp.null_eval_div).min(lp.null_eval_max);
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
                if traced {
                    eprintln!("TRACE ply={ply} d={depth} null-move cutoff {s} (r={r})");
                }
                return if s >= MATE_THRESHOLD { beta } else { s };
            }
        }

        // ProbCut: a capture whose reduced search clears beta by a margin
        // would almost surely clear beta at full depth.
        if prunable && lean_probcut() && depth >= lp.probcut_depth && beta.abs() < MATE_THRESHOLD {
            let pc_beta = beta + lp.probcut_margin;
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
                    // The capture child is never entered as a node: give its
                    // quiescence the child's ply and move, or it reads a
                    // `node_ply` and continuation key left by whatever node the
                    // null-move search visited last (council review round 3).
                    self.node_ply = ply + 1;
                    if hist != 0 {
                        super::lean_hist::with(|h| h.enter(ply + 1, Some(mv)));
                    }
                    let mut v = -self.lean_q_entry(
                        board,
                        &new_captured,
                        player.opposite(),
                        -pc_beta,
                        -pc_beta + 1,
                        start_time,
                        time_limit_ms,
                        hash_history,
                        false,
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
        // Off by default (`SHOGI_LEAN_IID=1`): until 2026-11-09 this re-entered
        // the node with its own position still on the path, so the repetition
        // test returned at once and IID never searched anything (council
        // review). It now steps off the path as the singular search does, but
        // a working IID has never been measured, so it needs a screen first.
        if lean_iid() && !lean_iir() && tt_move.is_none() && pv_node && depth >= 5 {
            hash_history.pop();
            let base = hash_history.len();
            self.lean_inner(
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
            hash_history.truncate(base);
            hash_history.push(path_entry(position_hash, in_check));
            if self.search_aborted {
                return alpha;
            }
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
            let line_mv = traced && trace_move(ply, mv);
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
                        && depth <= lp.hist_prune_depth
                        && super::lean_hist::with(|h| h.quiet_score(mv, ply)) < -lp.hist_prune_mul * depth
                    {
                        if line_mv {
                            eprintln!("TRACE ply={ply} d={depth} line move {} #{move_count}/{} pruned: history", mv.to_usi_string(), moves.len());
                        }
                        continue;
                    }
                    // Late move pruning.
                    let lmp_count = (lp.lmp_base.max(0) as usize + (depth * depth) as usize) / (1 + usize::from(!improving_for(2)));
                    if !pv_node && !lean_off(off::LMP) && depth <= lp.lmp_depth && move_count > lmp_count {
                        if line_mv {
                            eprintln!("TRACE ply={ply} d={depth} line move {} #{move_count}/{} pruned: late-move (limit {lmp_count})", mv.to_usi_string(), moves.len());
                        }
                        continue;
                    }
                    // Futility.
                    if !pv_node && !lean_off(off::FUT) && depth <= lp.fut_depth && static_eval + lp.fut_base + lp.fut_mul * depth <= alpha {
                        if line_mv {
                            eprintln!("TRACE ply={ply} d={depth} line move {} #{move_count}/{} pruned: futility (eval {static_eval}, alpha {alpha})", mv.to_usi_string(), moves.len());
                        }
                        continue;
                    }
                    // Quiet moves and drops that lose material.
                    if depth <= lp.see_quiet_depth
                        && !lean_off(off::SEE_QUIET)
                        && crate::search::move_ordering::see_calculation::see_quiet(mv, board)
                            < -lp.see_quiet_mul * depth
                    {
                        if line_mv {
                            eprintln!("TRACE ply={ply} d={depth} line move {} #{move_count}/{} pruned: losing quiet/drop (SEE)", mv.to_usi_string(), moves.len());
                        }
                        continue;
                    }
                } else if depth <= lp.see_cap_depth
                    && !lean_off(off::SEE_CAPTURE)
                    && crate::search::move_ordering::see_calculation::calculate_see_internal(
                        mv, board,
                    ) < -lp.see_cap_mul * depth
                {
                    if line_mv {
                        eprintln!("TRACE ply={ply} d={depth} line move {} #{move_count}/{} pruned: losing capture (SEE)", mv.to_usi_string(), moves.len());
                    }
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
            if line_mv {
                trace_set(ply + 1);
            }
            let mut traced_r = 0;

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
                        r -= super::lean_hist::with(|h| h.quiet_score(mv, ply)) / lp.lmr_hist_div;
                    }
                    r = r.clamp(0, new_depth - 1);
                }
                traced_r = r;
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
            if line_mv {
                trace_set(ply);
                eprintln!(
                    "TRACE ply={ply} d={depth} [{alpha},{beta}] line move {} #{move_count}/{} \
                     r={traced_r} ext={ext} -> {score} (best before {best_score})",
                    mv.to_usi_string(),
                    moves.len()
                );
            }
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
                        if traced && !line_mv {
                            eprintln!(
                                "TRACE ply={ply} d={depth} cut by {} #{move_count}/{} -> {score}",
                                mv.to_usi_string(),
                                moves.len()
                            );
                        }
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
        // Correction history learns from results consistent with their bound
        // (see `corr_learnable` for which).
        if let Some(k) = &corr_keys {
            if !self.search_aborted
                && corr_learnable(best_move.as_ref(), best_score)
                && !(flag == TranspositionFlag::LowerBound && best_score <= static_eval)
                && !(flag == TranspositionFlag::UpperBound && best_score >= static_eval)
            {
                super::lean_corr::with(|c| c.update(k, best_score - static_eval, depth));
            }
        }
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
/// Whether `(alpha, beta)` is wider than a null window, i.e. a PV node. In
/// i64: a full window (MIN_SCORE, MAX_SCORE) overflowed i32 and, in release
/// builds, wrapped negative, so it read as a null window and the node was
/// searched as non-PV (council review, 2026-11-09).
fn is_pv_window(alpha: i32, beta: i32) -> bool {
    i64::from(beta) - i64::from(alpha) > 1
}

/// Whether correction history may learn from a node's result: the best move is
/// quiet (a capture's or a promotion's gain is not the evaluation's error), and
/// the score is not one of hand repetition (±30,000) or mate. Promotions and
/// hand-repetition scores used to be learned (council review, 2026-11-09).
fn corr_learnable(best_move: Option<&Move>, best_score: i32) -> bool {
    best_move.is_none_or(|m| !m.is_capture && !m.is_promotion)
        && best_score.abs() < super::HAND_REPETITION_SCORE
}

/// Whether PV nodes without a table move run internal iterative deepening
/// (`SHOGI_LEAN_IID=1`; see the call in `lean_inner`).
fn lean_iid() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| env_flag("SHOGI_LEAN_IID", false))
}

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
    /// Quiescence plies the search below had left: `LEAN_Q_MAX_PLY - qply`
    /// if the ply cap cut it short somewhere, else `LEAN_Q_MAX_PLY` (complete).
    /// A probe uses the entry only with no more plies left than that: a result
    /// the cap truncated near the cap saw fewer continuations (council review
    /// round 2, 2026-11-09: entries used to be used at any ply).
    left: u8,
}

const Q_SLOTS: usize = 1 << 18;

thread_local! {
    /// Repetitions quiescence has found on this thread. A node whose subtree
    /// found one depends on the path, so it is not stored (compare before and
    /// after the subtree).
    static Q_REPETITIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Times quiescence stopped at `LEAN_Q_MAX_PLY` on this thread (see
    /// `QSlot::left`).
    static Q_CAP_HITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// A thread's quiescence table and the game (`lean_hist::game_epoch`) it holds
/// entries from. Entries include the correction-history term, which
/// `usinewgame` resets, so a new game starts with an empty table (council
/// review, 2026-11-09: it used to survive every game).
type QTable = (u64, Vec<QSlot>);

thread_local! {
    static QTABLE: std::cell::RefCell<QTable> =
        std::cell::RefCell::new((super::lean_hist::game_epoch(), vec![QSlot::default(); Q_SLOTS]));
}

static PARKED_Q: std::sync::Mutex<Vec<Option<QTable>>> = std::sync::Mutex::new(Vec::new());

/// Runs `f` on this thread's quiescence table, emptied first if it holds an
/// earlier game's entries.
fn with_qtable<R>(f: impl FnOnce(&mut Vec<QSlot>) -> R) -> R {
    QTABLE.with(|q| {
        let mut q = q.borrow_mut();
        let epoch = super::lean_hist::game_epoch();
        if q.0 != epoch || q.1.len() != Q_SLOTS {
            q.1.clear();
            q.1.resize(Q_SLOTS, QSlot::default());
            q.0 = epoch;
        }
        f(&mut q.1)
    })
}

/// Gives this (helper) thread the quiescence table helper `slot` left last move.
pub(super) fn q_unpark(slot: usize) {
    if root_stack() & 32 == 0 {
        return;
    }
    let t = PARKED_Q.lock().ok().and_then(|mut v| v.get_mut(slot).and_then(Option::take));
    if let Some(t) = t {
        QTABLE.with(|q| *q.borrow_mut() = t);
    }
}

/// Leaves this thread's quiescence table for the next helper `slot`.
pub(super) fn q_park(slot: usize) {
    if root_stack() & 32 == 0 {
        return;
    }
    let t = QTABLE.with(|q| std::mem::take(&mut *q.borrow_mut()));
    if t.1.len() != Q_SLOTS {
        return;
    }
    if let Ok(mut v) = PARKED_Q.lock() {
        if v.len() <= slot {
            v.resize_with(slot + 1, || None);
        }
        v[slot] = Some(t);
    }
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
        path: &mut Vec<u64>,
        root_on_path: bool,
    ) -> i32 {
        if lean_q_enabled() {
            // Quiescence keeps every node of its line on `path` (the repetition
            // test reads it as consecutive plies), so a root not already there
            // (ProbCut's capture) goes on first.
            let len = path.len();
            if !root_on_path {
                let key = self.hash_calculator.get_position_hash(board, player, captured);
                path.push(path_entry(key, board.is_king_in_check(player, captured)));
            }
            let v = self.lean_qsearch(
                board,
                captured,
                player,
                alpha,
                beta,
                start_time,
                time_limit_ms,
                0,
                path,
            );
            path.truncate(len);
            v
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
    ///
    /// `path` is the search path, the quiescence root on it. Every node below
    /// the root is tested for repetition and perpetual check, as `lean_node`
    /// tests its nodes (a quiet check and the king's reply can close a cycle;
    /// so can a capture undone by a drop), and is on the path while its
    /// children search, so the path holds consecutive plies. Quiescence used to
    /// test nothing (council review, 2026-11-09; round 2: only quiet nodes,
    /// on a path with gaps).
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
        path: &mut Vec<u64>,
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
        let key = self.hash_calculator.get_position_hash(board, player, captured);
        // Repetition first, also at the ply cap (round 3: a cycle closing at
        // the cap used to get the static evaluation).
        if qply > 0 {
            if let Some(score) = self.path_repetition(path, key, in_check) {
                Q_REPETITIONS.with(|r| r.set(r.get() + 1));
                return score;
            }
        }
        if qply >= LEAN_Q_MAX_PLY && !in_check {
            Q_CAP_HITS.with(|c| c.set(c.get() + 1));
            return self.evaluator.evaluate(board, player, captured);
        }
        let with_checks = qply == 0 && lean_q_checks();
        let left = u8::try_from((LEAN_Q_MAX_PLY - qply).max(0)).unwrap_or(0);
        let slot = (key as usize) & (Q_SLOTS - 1);
        let hit = with_qtable(|t| t[slot]);
        if hit.flag != 0
            && hit.key == key
            && (hit.checks || !with_checks)
            && hit.left >= left
        {
            let s = hit.score;
            let usable = match hit.flag {
                1 => true,
                2 => s >= beta,
                3 => s <= alpha,
                _ => false,
            };
            if usable {
                // A result the cap truncated truncates whatever uses it
                // (round 3: its ancestors used to be stored as complete).
                if hit.left < LEAN_Q_MAX_PLY as u8 {
                    Q_CAP_HITS.with(|c| c.set(c.get() + 1));
                }
                return s;
            }
        }

        let alpha_orig = alpha;
        let mut best = -INF;
        let mut stand_pat = -INF;
        if !in_check {
            stand_pat = self.evaluator.evaluate(board, player, captured);
            if root_stack() & 16 != 0
                && super::lean_corr::corr_enabled()
                && stand_pat.abs() < MATE_THRESHOLD
            {
                let ply = self.node_ply + qply;
                let prev = if qply == 0 && super::lean_hist::hist_uses() != 0 {
                    super::lean_hist::with(|h| h.prev_key(ply))
                } else {
                    super::lean_hist::NO_KEY
                };
                let k = super::lean_corr::keys(board, captured, player, prev);
                stand_pat = (stand_pat + super::lean_corr::with(|c| c.correction(&k)))
                    .clamp(-MATE_THRESHOLD + 1, MATE_THRESHOLD - 1);
            }
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

        // On the path while the children search (the root already is).
        let path_len = path.len();
        if qply > 0 {
            path.push(path_entry(key, in_check));
        }
        let repetitions_before = Q_REPETITIONS.with(std::cell::Cell::get);
        let cap_hits_before = Q_CAP_HITS.with(std::cell::Cell::get);
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
                path,
            );
            self.nnue_unmake_move();
            board.unmake_move(&move_info);
            if self.search_aborted {
                path.truncate(path_len);
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
        path.truncate(path_len);
        // In check with every evasion skipped cannot happen (none are pruned).
        if best == -INF {
            best = alpha_orig;
        }

        // A result that met a repetition below holds only for this path.
        let path_free = Q_REPETITIONS.with(std::cell::Cell::get) == repetitions_before;
        if !self.search_aborted && best.abs() < MATE_THRESHOLD && path_free {
            let flag = if best >= beta {
                2
            } else if best <= alpha_orig {
                3
            } else {
                1
            };
            let complete = Q_CAP_HITS.with(std::cell::Cell::get) == cap_hits_before;
            let left = if complete { LEAN_Q_MAX_PLY as u8 } else { left };
            with_qtable(|t| t[slot] = QSlot { key, score: best, flag, checks: with_checks, left });
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moves::MoveGenerator;

    fn startpos_moves() -> Vec<Move> {
        MoveGenerator::new().generate_legal_moves(
            &BitboardBoard::new(),
            Player::Black,
            &CapturedPieces::new(),
        )
    }

    /// The full window is a PV window (it overflowed i32 and read as null).
    #[test]
    fn full_window_is_pv() {
        assert!(is_pv_window(MIN_SCORE, MAX_SCORE));
        assert!(is_pv_window(MAX_SCORE.saturating_neg(), MIN_SCORE.saturating_neg()));
        assert!(is_pv_window(-100, 100));
        assert!(!is_pv_window(99, 100));
    }

    /// Correction history does not learn from captures, promotions, hand
    /// repetition or mate scores.
    #[test]
    fn correction_learns_only_quiet_evaluation_errors() {
        let quiet = startpos_moves().into_iter().find(|m| !m.is_capture).expect("a quiet move");
        let mut promo = quiet.clone();
        promo.is_promotion = true;
        let mut capture = quiet.clone();
        capture.is_capture = true;
        assert!(corr_learnable(Some(&quiet), 150));
        assert!(corr_learnable(None, -150));
        assert!(!corr_learnable(Some(&promo), 150));
        assert!(!corr_learnable(Some(&capture), 150));
        assert!(!corr_learnable(Some(&quiet), super::super::HAND_REPETITION_SCORE));
        assert!(!corr_learnable(Some(&quiet), -super::super::HAND_REPETITION_SCORE));
        assert!(!corr_learnable(None, MATE_THRESHOLD + 5));
    }

    /// `usinewgame` empties the quiescence table too (its entries hold the
    /// correction-history term, which a new game resets).
    #[test]
    fn new_game_empties_quiescence_table() {
        let _epoch = super::super::lean_hist::TEST_EPOCH_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        with_qtable(|t| t[7] = QSlot { key: 42, score: 1, flag: 1, checks: false, left: 8 });
        assert_eq!(with_qtable(|t| t[7].key), 42);
        super::super::lean_hist::new_game();
        let slot = with_qtable(|t| t[7]);
        assert_eq!((slot.key, slot.flag), (0, 0));
    }

    /// A depth-0 node records the move that reached it before quiescence reads
    /// its continuation key (it used to read a sibling's).
    #[test]
    fn quiescence_entry_records_its_move() {
        let moves = startpos_moves();
        let (sibling, mv) = (&moves[0], &moves[1]);
        assert_ne!(super::super::lean_hist::key(sibling), super::super::lean_hist::key(mv));
        let mut engine = SearchEngine::new(None, 16);
        let mut board = BitboardBoard::new();
        let mut captured = CapturedPieces::new();
        let root = engine.hash_calculator.get_position_hash(&board, Player::Black, &captured);
        engine.root_entry = path_entry(root, false);
        engine.root_path_len = 1;
        board.make_move_with_hand(mv, &mut captured);
        super::super::lean_hist::with(|h| h.enter(1, Some(sibling)));
        let mut path = vec![engine.root_entry];
        let start = TimeSource::now();
        engine.lean_node(
            &mut board,
            &captured,
            Player::White,
            0,
            -1000,
            1000,
            &start,
            60_000,
            &mut path,
            false,
            true,
            Some(mv),
        );
        let got = super::super::lean_hist::with(|h| h.prev_key(1));
        assert_eq!(got, super::super::lean_hist::key(mv));
    }

    /// Quiescence sees a repetition its quiet check and the king's reply close
    /// with the search path (it used to see none): Black, far behind, draws by
    /// checking from 5a, since the king's only escape repeats an earlier
    /// position.
    #[test]
    fn quiescence_finds_repetition_with_the_path() {
        let pos = |sfen: &str| BitboardBoard::from_fen(sfen).expect("sfen");
        // X: White king 1b, Black rook 5a, Black to move; Y: rook back to 5c,
        // White to move; Z: king back to 1a, Black to move (the q root).
        let (x, xp, xc) = pos("4R4/7pk/9/9/9/9/r8/r8/gg6K b - 1");
        let (y, yp, yc) = pos("9/7pk/4R4/9/9/9/r8/r8/gg6K w - 1");
        let (mut z, zp, zc) = pos("8k/7p1/4R4/9/9/9/r8/r8/gg6K b - 1");
        let mut engine = SearchEngine::new(None, 16);
        let entry = |e: &SearchEngine, b: &BitboardBoard, p: Player, c: &CapturedPieces| {
            path_entry(e.hash_calculator.get_position_hash(b, p, c), b.is_king_in_check(p, c))
        };
        let mut path =
            vec![entry(&engine, &x, xp, &xc), entry(&engine, &y, yp, &yc), entry(&engine, &z, zp, &zc)];
        engine.root_entry = path[0];
        engine.root_path_len = 1;
        let start = TimeSource::now();
        let stand_pat = engine.evaluator.evaluate(&z, zp, &zc);
        assert!(stand_pat < -500, "Black should be far behind: {stand_pat}");
        let q = engine.lean_q_entry(&mut z, &zc, zp, -30_000, 30_000, &start, 60_000, &mut path, true);
        assert_eq!(q, 0, "the checking line repeats X");
        assert_eq!(path.len(), 3);
        // The draw holds only on this path: Z's result is not cached.
        let zkey = engine.hash_calculator.get_position_hash(&z, zp, &zc);
        let slot = with_qtable(|t| t[(zkey as usize) & (Q_SLOTS - 1)]);
        assert!(slot.flag == 0 || slot.key != zkey, "path-dependent result cached");
    }

    /// A node that uses a truncated cached result is itself truncated: it is
    /// stored with its own plies left, not as complete.
    #[test]
    fn truncated_cache_hit_truncates_its_parent() {
        // A: Black's rook on 5h can take the pawn on 5e (its only noisy move),
        // giving B with White to move.
        let (mut a, ap, ac) = BitboardBoard::from_fen("8k/9/9/9/4p4/9/9/4R4/K8 b - 1").expect("sfen");
        let (b, bp, bc) = BitboardBoard::from_fen("8k/9/9/9/4R4/9/9/9/K8 w P 1").expect("sfen");
        let mut engine = SearchEngine::new(None, 16);
        let akey = engine.hash_calculator.get_position_hash(&a, ap, &ac);
        let bkey = engine.hash_calculator.get_position_hash(&b, bp, &bc);
        let bslot = (bkey as usize) & (Q_SLOTS - 1);
        let aslot = (akey as usize) & (Q_SLOTS - 1);
        assert_ne!(aslot, bslot);
        // B's entry: exact, from one ply before the cap (1 ply left).
        with_qtable(|t| {
            t[bslot] = QSlot { key: bkey, score: -77, flag: 1, checks: false, left: 1 };
            t[aslot] = QSlot::default();
        });
        let start = TimeSource::now();
        let mut path = vec![path_entry(akey ^ 2, false)];
        let qply = LEAN_Q_MAX_PLY - 2;
        engine.lean_qsearch(&mut a, &ac, ap, -30_000, 30_000, &start, 60_000, qply, &mut path);
        let stored = with_qtable(|t| t[aslot]);
        assert_eq!(stored.key, akey, "A was not stored");
        assert_eq!(i32::from(stored.left), LEAN_Q_MAX_PLY - qply, "stored as complete");
    }

    /// A repetition at the ply cap is scored as one, not by the evaluation.
    #[test]
    fn repetition_at_the_ply_cap() {
        let (mut z, zp, zc) = BitboardBoard::from_fen("8k/7p1/4R4/9/9/9/r8/r8/gg6K b - 1").expect("sfen");
        let mut engine = SearchEngine::new(None, 16);
        let key = engine.hash_calculator.get_position_hash(&z, zp, &zc);
        // Z, then an opponent position not in check, then Z again: a plain
        // repetition (with the opponent in check it would be our perpetual).
        let mut path = vec![path_entry(key ^ 2, false), path_entry(key, false), path_entry(key ^ 4, false)];
        let start = TimeSource::now();
        let eval = engine.evaluator.evaluate(&z, zp, &zc);
        assert_ne!(eval, 0);
        let q = engine.lean_qsearch(&mut z, &zc, zp, -30_000, 30_000, &start, 60_000, LEAN_Q_MAX_PLY, &mut path);
        assert_eq!(q, 0, "the repetition was not seen at the cap");
        assert_eq!(path.len(), 3);
    }

    /// A cached quiescence result is used only where it had at least as many
    /// plies left as the probe needs (an entry from near the ply cap saw fewer
    /// continuations).
    #[test]
    fn quiescence_cache_respects_the_horizon() {
        let (mut b, p, c) = BitboardBoard::from_fen("4k4/9/9/9/9/9/9/9/4K4 b - 1").expect("sfen");
        let mut engine = SearchEngine::new(None, 16);
        let key = engine.hash_calculator.get_position_hash(&b, p, &c);
        let slot = (key as usize) & (Q_SLOTS - 1);
        let start = TimeSource::now();
        let mut path = vec![path_entry(key, false)];
        let fake = 4_321;
        with_qtable(|t| t[slot] = QSlot { key, score: fake, flag: 1, checks: true, left: 1 });
        let q = engine.lean_q_entry(&mut b, &c, p, -30_000, 30_000, &start, 60_000, &mut path, true);
        assert_ne!(q, fake, "an entry with 1 ply left was used at the root");
        with_qtable(|t| t[slot] = QSlot { key, score: fake, flag: 1, checks: true, left: 8 });
        let q = engine.lean_q_entry(&mut b, &c, p, -30_000, 30_000, &start, 60_000, &mut path, true);
        assert_eq!(q, fake, "a full-horizon entry is used");
    }

    /// The root search is fail-soft: a fail-low returns the best score found,
    /// below alpha (it returned alpha itself, so the aspiration loops widened
    /// from the old bound).
    #[test]
    fn root_fail_low_returns_true_bound() {
        let mut engine = SearchEngine::new(None, 16);
        let mut board = BitboardBoard::new();
        let (mv, score) = engine
            .search_at_depth(&mut board, &CapturedPieces::new(), Player::Black, 2, 60_000, 20_000, 20_001)
            .expect("a move");
        assert!(score < 20_000, "fail-low returned {score}");
        // The move on a fail-low is the first-ordered one, as before.
        assert!(startpos_moves().iter().any(|m| m.to_usi_string() == mv.to_usi_string()));
    }
}
