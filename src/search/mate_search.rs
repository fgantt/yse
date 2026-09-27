//! Mate search: df-pn (depth-first proof-number search) over checking sequences.
//!
//! The main search sees 5-7 plies at a game time control; forced mates in
//! shogi routinely run 11-25. In a won handicap game YaneuraOu saw a forced
//! mate from move 35 that the engine took until move 58 to deliver, letting it
//! slip once on the way. A mate search finds such lines directly, independent
//! of the evaluation: the attacker considers only checking moves, the defender
//! every legal reply, and a position is proved when every defence runs out of
//! legal moves.
//!
//! df-pn (Nagai 2002) keeps, for every position, a *proof number* (how many
//! positions must still be proved to prove it) and a *disproof number*, and
//! always expands the branch closest to a proof. That lets it solve long, narrow
//! mates far faster than a depth-limited search. Numbers are memoised by
//! position hash (board, side to move and hands), so transpositions are shared.
//!
//! Shogi-specific points:
//! * Pawn-drop mate is illegal; the legal move generator already excludes it.
//! * Perpetual check is illegal for the checking side, so a position repeated
//!   on the current path is treated as not mate.
//! * A defender with no legal move is mated whether or not it is in check at an
//!   AND node, since every AND node here follows a check.

use crate::bitboards::BitboardBoard;
use crate::moves::MoveGenerator;
use crate::search::shogi_hash::ShogiHashHandler;
use crate::types::board::CapturedPieces;
use crate::types::core::{Move, Player};
use std::collections::HashMap;

const INF: u32 = u32::MAX / 4;

#[derive(Clone, Copy, Debug)]
struct Entry {
    pn: u32,
    dn: u32,
}

/// A df-pn mate solver with a node budget.
pub struct MateSearch {
    mg: MoveGenerator,
    hasher: ShogiHashHandler,
    tt: HashMap<u64, Entry>,
    nodes: u64,
    node_limit: u64,
    tt_limit: usize,
    path: Vec<u64>,
}

struct Child {
    mv: Move,
    board: BitboardBoard,
    captured: CapturedPieces,
    hash: u64,
    /// Numbers for the child before it is first expanded.
    init: Entry,
}

impl MateSearch {
    pub fn new(node_limit: u64) -> Self {
        Self {
            mg: MoveGenerator::new(),
            hasher: ShogiHashHandler::new_default(),
            tt: HashMap::new(),
            nodes: 0,
            node_limit,
            tt_limit: 4_000_000,
            path: Vec::new(),
        }
    }

    /// Positions expanded by the last `solve`.
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    /// A forced mate for `attacker` (to move) from this position, as a move
    /// line, if one is proved within the node budget. The line takes the
    /// shortest mate the proof contains against the longest defence.
    pub fn solve(
        &mut self,
        board: &BitboardBoard,
        captured: &CapturedPieces,
        attacker: Player,
    ) -> Option<Vec<Move>> {
        self.tt.clear();
        self.nodes = 0;
        self.path.clear();
        let root_hash = self.hasher.get_position_hash(board, attacker, captured);
        let (pn, _) = self.mid(board, captured, attacker, true, root_hash, INF, INF);
        if pn != 0 {
            return None;
        }
        let mut memo = HashMap::new();
        let mut line = Vec::new();
        self.extract(board, captured, attacker, true, &mut memo, &mut line, 0);
        if line.is_empty() {
            None
        } else {
            Some(line)
        }
    }

    fn lookup(&self, hash: u64) -> Entry {
        self.tt.get(&hash).copied().unwrap_or(Entry { pn: 1, dn: 1 })
    }

    fn lookup_or(&self, hash: u64, init: Entry) -> Entry {
        self.tt.get(&hash).copied().unwrap_or(init)
    }

    fn store(&mut self, hash: u64, e: Entry) {
        if self.tt.len() >= self.tt_limit {
            // Keep proofs and disproofs, drop everything unsolved.
            self.tt.retain(|_, v| v.pn == 0 || v.dn == 0);
        }
        self.tt.insert(hash, e);
    }

    /// Children of a node: checking moves for the attacker (`or_node`), every
    /// legal reply for the defender.
    ///
    /// A check's position starts with its proof number set to the defender's
    /// number of legal replies (df-pn+ style): a check that leaves one escape is
    /// far closer to mate than one that leaves twenty, and with a flat (1, 1)
    /// start the search could not tell them apart -- in positions with pieces
    /// in hand on both sides it drowned in checking drops and interpositions.
    /// A check that leaves no reply is mate on the spot.
    fn children(
        &self,
        board: &BitboardBoard,
        captured: &CapturedPieces,
        side: Player,
        or_node: bool,
    ) -> Vec<Child> {
        let moves = self.mg.generate_legal_moves(board, side, captured);
        moves
            .into_iter()
            .filter(|m| !or_node || m.gives_check)
            .map(|mv| {
                let mut b = board.clone();
                let mut c = captured.clone();
                b.make_move_with_hand(&mv, &mut c);
                let hash = self.hasher.get_position_hash(&b, side.opposite(), &c);
                let init = if or_node {
                    let replies = self.mg.generate_legal_moves(&b, side.opposite(), &c).len() as u32;
                    if replies == 0 {
                        Entry { pn: 0, dn: INF }
                    } else {
                        Entry { pn: replies, dn: 1 }
                    }
                } else {
                    Entry { pn: 1, dn: 1 }
                };
                Child { mv, board: b, captured: c, hash, init }
            })
            .collect()
    }

    /// Proof and disproof numbers of a child, with positions repeated on the
    /// current path counted as not mate (perpetual check is illegal).
    fn child_numbers(&self, child: &Child) -> Entry {
        if self.path.contains(&child.hash) {
            // For the attacker's next move this is a dead end; the child is an
            // AND node (defender to move) when we are at an OR node and vice
            // versa, and "not mate" is pn = INF, dn = 0 from the attacker's
            // view in both cases.
            return Entry { pn: INF, dn: 0 };
        }
        self.lookup_or(child.hash, child.init)
    }

    /// Multiple iterative deepening: expand `board` until its numbers reach the
    /// thresholds. Returns the node's (pn, dn).
    #[allow(clippy::too_many_arguments)]
    fn mid(
        &mut self,
        board: &BitboardBoard,
        captured: &CapturedPieces,
        side: Player,
        or_node: bool,
        hash: u64,
        thpn: u32,
        thdn: u32,
    ) -> (u32, u32) {
        self.nodes += 1;
        let children = self.children(board, captured, side, or_node);
        if children.is_empty() {
            // Attacker without a check: not mate. Defender without a move: mated.
            let e = if or_node { Entry { pn: INF, dn: 0 } } else { Entry { pn: 0, dn: INF } };
            self.store(hash, e);
            return (e.pn, e.dn);
        }
        self.path.push(hash);
        let result = loop {
            // Numbers of this node from its children.
            let mut best = 0usize;
            let (mut best_val, mut second_val) = (INF, INF);
            let (mut min_pn, mut sum_dn, mut sum_pn, mut min_dn) = (INF, 0u32, 0u32, INF);
            for (i, ch) in children.iter().enumerate() {
                let e = self.child_numbers(ch);
                min_pn = min_pn.min(e.pn);
                min_dn = min_dn.min(e.dn);
                sum_pn = (sum_pn + e.pn).min(INF);
                sum_dn = (sum_dn + e.dn).min(INF);
                // OR node: expand the child closest to a proof (min pn);
                // AND node: the child closest to a disproof (min dn).
                let v = if or_node { e.pn } else { e.dn };
                if v < best_val {
                    second_val = best_val;
                    best_val = v;
                    best = i;
                } else if v < second_val {
                    second_val = v;
                }
            }
            let (pn, dn) = if or_node { (min_pn, sum_dn) } else { (sum_pn, min_dn) };
            if pn >= thpn || dn >= thdn || self.nodes >= self.node_limit {
                self.store(hash, Entry { pn, dn });
                break (pn, dn);
            }
            let ch = &children[best];
            let e = self.child_numbers(ch);
            // 1+epsilon trick: let the chosen child run until it is clearly
            // worse than its best sibling, not merely one unit worse, so the
            // search does not switch branches after every expansion.
            let eps = |v: u32| v.saturating_add(1).max(v.saturating_add(v / 4));
            let (cthpn, cthdn) = if or_node {
                (thpn.min(eps(second_val)), thdn.saturating_sub(dn).saturating_add(e.dn))
            } else {
                (thpn.saturating_sub(pn).saturating_add(e.pn), thdn.min(eps(second_val)))
            };
            let (cb, cc, chash) = (ch.board.clone(), ch.captured.clone(), ch.hash);
            self.mid(&cb, &cc, side.opposite(), !or_node, chash, cthpn, cthdn);
        };
        self.path.pop();
        result
    }

    /// Length (plies) of the mate the table proves from a position: the
    /// attacker's quickest proved continuation against the defender's longest.
    /// `None` if the table does not prove it.
    #[allow(clippy::too_many_arguments)]
    fn mate_length(
        &self,
        board: &BitboardBoard,
        captured: &CapturedPieces,
        side: Player,
        or_node: bool,
        hash: u64,
        memo: &mut HashMap<(u64, bool), Option<u32>>,
        depth: u32,
    ) -> Option<u32> {
        if let Some(v) = memo.get(&(hash, or_node)) {
            return *v;
        }
        if depth > 64 {
            return None;
        }
        match self.tt.get(&hash) {
            Some(e) if e.pn == 0 => {}
            Some(_) => return None,
            // Not in the table: only a defender with no legal reply counts as
            // proved. A check that leaves no reply is scored mate when it is
            // generated (see `children`) and never expanded, so it is never
            // stored; anything else unstored is unproved.
            None => {
                let mated = !or_node && self.children(board, captured, side, false).is_empty();
                return if mated { Some(0) } else { None };
            }
        }
        memo.insert((hash, or_node), None); // guards against cycles
        let children = self.children(board, captured, side, or_node);
        let result = if children.is_empty() {
            if or_node { None } else { Some(0) }
        } else if or_node {
            children
                .iter()
                .filter_map(|c| {
                    self.mate_length(&c.board, &c.captured, side.opposite(), false, c.hash, memo, depth + 1)
                })
                .min()
                .map(|l| l + 1)
        } else {
            let mut longest = 0u32;
            let mut all = true;
            for c in &children {
                match self.mate_length(&c.board, &c.captured, side.opposite(), true, c.hash, memo, depth + 1) {
                    Some(l) => longest = longest.max(l),
                    None => {
                        all = false;
                        break;
                    }
                }
            }
            if all { Some(longest + 1) } else { None }
        };
        memo.insert((hash, or_node), result);
        result
    }

    /// Append the mate line from this position to `line`.
    #[allow(clippy::too_many_arguments)]
    fn extract(
        &self,
        board: &BitboardBoard,
        captured: &CapturedPieces,
        side: Player,
        or_node: bool,
        memo: &mut HashMap<(u64, bool), Option<u32>>,
        line: &mut Vec<Move>,
        depth: u32,
    ) {
        if depth > 64 {
            return;
        }
        let children = self.children(board, captured, side, or_node);
        let lengths: Vec<Option<u32>> = children
            .iter()
            .map(|c| self.mate_length(&c.board, &c.captured, side.opposite(), !or_node, c.hash, memo, depth + 1))
            .collect();
        let pick = if or_node {
            lengths.iter().enumerate().filter_map(|(i, l)| l.map(|l| (l, i))).min().map(|(_, i)| i)
        } else {
            lengths.iter().enumerate().filter_map(|(i, l)| l.map(|l| (l, i))).max().map(|(_, i)| i)
        };
        if let Some(i) = pick {
            let c = &children[i];
            line.push(c.mv.clone());
            self.extract(&c.board, &c.captured, side.opposite(), !or_node, memo, line, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solve(sfen: &str, nodes: u64) -> Option<Vec<String>> {
        let (board, player, captured) = BitboardBoard::from_fen(sfen).unwrap();
        MateSearch::new(nodes)
            .solve(&board, &captured, player)
            .map(|l| l.iter().map(|m| m.to_usi_string()).collect())
    }

    /// Mate in one from a user's 8-piece handicap game (6d6c+, 6b7b and others).
    #[test]
    fn finds_mate_in_one() {
        let line =
            solve("2+P+P5/3+P5/+N1k6/G2P4G/2S3+B2/1P3PP1P/+p5NPL/+p1PR1S2K/3G2G1L b 6P 172", 10_000)
                .expect("mate in one");
        assert_eq!(line.len(), 1, "{line:?}");
    }

    /// The start position has no forced mate.
    #[test]
    fn no_mate_from_start() {
        assert!(solve("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1", 20_000).is_none());
    }
}
