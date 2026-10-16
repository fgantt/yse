//! Move statistics for the lean search (`SHOGI_LEAN_HIST`): history tables that
//! order moves and adjust reductions, replacing the general move orderer there.
//!
//! Session 35 put YaneuraOu's own network inside our search and still lost to
//! YaneuraOu at ~7k nodes: with the evaluation equal, our tree needed ~5x the
//! nodes for the same depth, and only ~80% of cutoffs came from the first move
//! searched. The general orderer's history was a hash map keyed by piece, from
//! and to -- the same entry for both players -- with positive scores only and
//! the whole table cut by 10% every 1,000 updates, so moves that failed were
//! never marked down and what it had learned was soon forgotten.
//!
//! These are the statistics most strong engines keep, in our own form:
//!   * quiet history by side, origin (a square, or the dropped piece) and target;
//!   * continuation history: a quiet move's record after the opponent's last
//!     move and after our own move before it, each keyed by (side, piece, target);
//!   * capture history by (side, piece, target, captured piece);
//!   * two killers per ply and a counter move per opponent move.
//! Each update moves an entry toward +/-`LIMIT` by a share that shrinks as it
//! nears the limit, so entries stay bounded without periodic aging. The move
//! that causes a cutoff is rewarded and the moves of the same kind searched
//! before it are penalised.
//!
//! One table set per search thread. Helper threads start fresh each move; the
//! main thread keeps its tables across moves.

use crate::bitboards::BitboardBoard;
use crate::search::move_ordering::see_calculation::{calculate_see_internal, QuietSee};
use crate::types::core::{Move, PieceType};

/// Which parts are on (`SHOGI_LEAN_HIST`, a bit set): 1 move ordering (default),
/// 2 reductions adjusted by history, 4 shallow pruning of quiet moves with bad
/// history. 0 restores the general move orderer.
pub(super) fn hist_uses() -> u8 {
    static ON: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SHOGI_LEAN_HIST").ok().and_then(|v| v.parse().ok()).unwrap_or(1)
    })
}

const LIMIT: i32 = 16_000;
const MAX_PLY: usize = 256;
/// (side, piece, target) keys: 2 x 14 x 81.
const PT: usize = 2 * PieceType::COUNT * 81;
/// Origins: 81 squares, then the 7 droppable pieces.
const ORIGINS: usize = 81 + 7;
/// No move (the root's parent, or a null move).
pub(super) const NO_KEY: u16 = u16::MAX;

pub(super) struct LeanHist {
    quiet: Vec<i16>,
    /// Continuation history, `[prev key][key]`, one table per distance back (1, 2).
    cont: [Vec<i16>; 2],
    capture: Vec<i16>,
    killers: [[Option<Move>; 2]; MAX_PLY],
    counter: Vec<Option<Move>>,
    /// Key of the move that led to the node at each ply.
    path: [u16; MAX_PLY],
}

thread_local! {
    static HIST: std::cell::RefCell<Option<Box<LeanHist>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` on this thread's tables, creating them on first use.
pub(super) fn with<R>(f: impl FnOnce(&mut LeanHist) -> R) -> R {
    HIST.with(|h| {
        let mut h = h.borrow_mut();
        f(h.get_or_insert_with(|| {
            Box::new(LeanHist {
                quiet: vec![0; 2 * ORIGINS * 81],
                cont: [vec![0; PT * PT], vec![0; PT * PT]],
                capture: vec![0; PT * PieceType::COUNT],
                killers: std::array::from_fn(|_| [None, None]),
                counter: vec![None; PT],
                path: [NO_KEY; MAX_PLY],
            })
        }))
    })
}

/// (side, piece, target) key of a move.
pub(super) fn key(mv: &Move) -> u16 {
    ((mv.player as usize * PieceType::COUNT + mv.piece_type.as_index()) * 81
        + mv.to.to_index() as usize) as u16
}

fn origin(mv: &Move) -> usize {
    mv.from.map_or(81 + mv.piece_type.as_index().min(6), |f| f.to_index() as usize)
}

fn same(a: &Move, b: &Move) -> bool {
    a.from == b.from && a.to == b.to && a.piece_type == b.piece_type && a.is_promotion == b.is_promotion
}

/// Moves `e` toward the sign of `bonus` by `bonus`, less the share already used up.
fn nudge(e: &mut i16, bonus: i32) {
    let v = i32::from(*e);
    *e = (v + bonus - v * bonus.abs() / LIMIT) as i16;
}

/// Reward for the move that cut off at `depth` (and penalty for those before it).
fn bonus(depth: i32) -> i32 {
    (24 * depth * depth + 40 * depth).min(2400)
}

/// Ordering tiers: each exceeds anything the tiers below can score.
const TIER_TT: i32 = 1 << 30;
const TIER_GOOD_CAPTURE: i32 = 1 << 28;
const TIER_KILLER: i32 = 1 << 26;
const TIER_COUNTER: i32 = TIER_KILLER - 1;
const TIER_HANGING: i32 = -(1 << 26);
const TIER_BAD_CAPTURE: i32 = -(1 << 28);

impl LeanHist {
    fn quiet_idx(mv: &Move) -> usize {
        (mv.player as usize * ORIGINS + origin(mv)) * 81 + mv.to.to_index() as usize
    }

    fn capture_idx(mv: &Move) -> Option<usize> {
        let victim = mv.captured_piece.as_ref()?.piece_type.as_index();
        Some(key(mv) as usize * PieceType::COUNT + victim)
    }

    /// Records the move that led to the node at `ply` (`NO_KEY` for none).
    pub(super) fn enter(&mut self, ply: i32, prev: Option<&Move>) {
        if let Ok(p) = usize::try_from(ply) {
            if p < MAX_PLY {
                self.path[p] = prev.map_or(NO_KEY, key);
            }
        }
    }

    /// Keys of the moves 1 and 2 plies before the node at `ply`.
    fn prev_keys(&self, ply: i32) -> [u16; 2] {
        let get = |back: i32| {
            usize::try_from(ply - back).ok().filter(|&p| p < MAX_PLY && ply - back >= 1).map_or(NO_KEY, |p| self.path[p])
        };
        [get(0), get(1)]
    }

    /// History score of a quiet move at `ply` (quiet + both continuations).
    pub(super) fn quiet_score(&self, mv: &Move, ply: i32) -> i32 {
        let k = key(mv) as usize;
        let mut s = i32::from(self.quiet[Self::quiet_idx(mv)]);
        for (i, p) in self.prev_keys(ply).into_iter().enumerate() {
            if p != NO_KEY {
                s += i32::from(self.cont[i][p as usize * PT + k]);
            }
        }
        s
    }

    pub(super) fn is_killer(&self, mv: &Move, ply: i32) -> bool {
        usize::try_from(ply)
            .ok()
            .and_then(|p| self.killers.get(p))
            .is_some_and(|ks| ks.iter().flatten().any(|k| same(k, mv)))
    }

    /// `moves` sorted for searching at `ply`.
    pub(super) fn order(
        &self,
        moves: &[Move],
        board: &BitboardBoard,
        tt_move: Option<&Move>,
        ply: i32,
    ) -> Vec<Move> {
        let prev = self.prev_keys(ply)[0];
        let counter = (prev != NO_KEY).then(|| self.counter[prev as usize].as_ref()).flatten();
        let mut see = QuietSee::default();
        let mut scored: Vec<(i32, usize)> = moves
            .iter()
            .enumerate()
            .map(|(i, mv)| {
                let s = if tt_move.is_some_and(|t| same(t, mv)) {
                    TIER_TT
                } else if mv.is_capture {
                    let victim = mv.captured_piece.as_ref().map_or(0, |p| p.piece_type.base_value());
                    let ch = Self::capture_idx(mv).map_or(0, |i| i32::from(self.capture[i]));
                    let value = victim * 8 + ch / 4 + i32::from(mv.is_promotion) * 400;
                    if calculate_see_internal(mv, board) >= 0 {
                        TIER_GOOD_CAPTURE + value
                    } else {
                        TIER_BAD_CAPTURE + value
                    }
                } else if mv.is_promotion && mv.piece_type != PieceType::Silver {
                    TIER_GOOD_CAPTURE + self.quiet_score(mv, ply) / 64
                } else if self.is_killer(mv, ply) {
                    TIER_KILLER
                } else if counter.is_some_and(|c| same(c, mv)) {
                    TIER_COUNTER
                } else {
                    let h = self.quiet_score(mv, ply);
                    let opp = mv.player.opposite();
                    if board.is_square_attacked_by(mv.to, opp) {
                        let loss = see.see_quiet(mv, board);
                        if loss < 0 {
                            return (TIER_HANGING + loss * 64 + h / 64, i);
                        }
                    }
                    h
                };
                (s, i)
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, i)| moves[i].clone()).collect()
    }

    /// Updates after `best` caused a cutoff at `depth`, having searched
    /// `quiets` and `captures` (which include `best`) at `ply`.
    pub(super) fn cutoff(
        &mut self,
        best: &Move,
        quiets: &[Move],
        captures: &[Move],
        depth: i32,
        ply: i32,
    ) {
        let b = bonus(depth);
        let best_quiet = !best.is_capture;
        if best_quiet {
            let prev = self.prev_keys(ply);
            for mv in quiets {
                let sign = if same(mv, best) { b } else { -b };
                nudge(&mut self.quiet[Self::quiet_idx(mv)], sign);
                let k = key(mv) as usize;
                for (i, p) in prev.into_iter().enumerate() {
                    if p != NO_KEY {
                        nudge(&mut self.cont[i][p as usize * PT + k], sign);
                    }
                }
            }
            if let Ok(p) = usize::try_from(ply) {
                if p < MAX_PLY && !self.killers[p][0].as_ref().is_some_and(|k| same(k, best)) {
                    self.killers[p][1] = self.killers[p][0].take();
                    self.killers[p][0] = Some(best.clone());
                }
            }
            if prev[0] != NO_KEY {
                self.counter[prev[0] as usize] = Some(best.clone());
            }
        }
        for mv in captures {
            if let Some(i) = Self::capture_idx(mv) {
                nudge(&mut self.capture[i], if same(mv, best) { b } else { -b });
            }
        }
    }

    /// Clears the killers two plies below `ply` (as a node's children start).
    pub(super) fn clear_child_killers(&mut self, ply: i32) {
        if let Ok(p) = usize::try_from(ply + 2) {
            if p < MAX_PLY {
                self.killers[p] = [None, None];
            }
        }
    }
}
