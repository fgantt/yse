//! Threat inputs for the NNUE: attacks on the squares around each king.
//!
//! The piece-square features say where pieces stand, not what they attack, and
//! a 256-wide first layer learns attack patterns poorly. Session 32 found our
//! evaluation's error in the user's games tracks attacks on each king zone
//! (a linear model of these features cut held-out error 22%), but applying that
//! model as a correction lost Elo. These features let the network learn the
//! interaction instead.
//!
//! They are computed from the board at every evaluation (not incrementally) and
//! appended as [`NUM_THREAT_FEATURES`] input rows after the king-bucketed block:
//! a network has them when its row count modulo
//! [`KING_FEATURE_STRIDE`](crate::evaluation::nnue::KING_FEATURE_STRIDE) is
//! [`NUM_THREAT_FEATURES`]. Indices are relative to a perspective (`frame`), with
//! squares seen from that side as in the POV piece features.
//!
//! For each king (`side` 0 = the frame's own, 1 = the opponent's) and each square
//! of its 3x3 zone that is on the board (`off` 0..9, row-major in the frame,
//! 4 = the king's square):
//!
//! * `side*27 + off*3 + min(a,3) - 1` when `a` (the enemy's attackers of that
//!   square) is at least 1;
//! * `54 + side*9 + off` when the square is attacked and no piece of the king's
//!   side other than the king defends it;
//! * `72 + side*9 + k - 1` for `k` in `1..=n`, `n` the number of attacked zone
//!   squares (a thermometer).

use crate::bitboards::BitboardBoard;
use crate::types::{Player, Position};

pub const NUM_THREAT_FEATURES: usize = 90;

/// At most 9 attacker-count + 9 undefended + 9 thermometer features per king.
pub const MAX_ACTIVE_THREAT_FEATURES: usize = 54;

/// Attack counts on both kings' zones, in board coordinates: computed once per
/// position and read by both perspectives.
#[derive(Debug, Clone, Copy, Default)]
pub struct KingZones {
    /// `[king colour][offset]`: enemy attackers of the square, or `u8::MAX`
    /// when the square is off the board.
    attackers: [[u8; 9]; 2],
    /// `[king colour][offset]`: defenders other than the king.
    defenders: [[u8; 9]; 2],
}

/// Active threat features of one perspective (indices relative to the block).
#[derive(Debug, Clone, Copy)]
pub struct ThreatList {
    items: [u16; MAX_ACTIVE_THREAT_FEATURES],
    len: usize,
}

impl ThreatList {
    pub fn new() -> Self {
        Self { items: [0; MAX_ACTIVE_THREAT_FEATURES], len: 0 }
    }

    #[inline]
    fn push(&mut self, f: usize) {
        self.items[self.len] = f as u16;
        self.len += 1;
    }

    pub fn as_slice(&self) -> &[u16] {
        &self.items[..self.len]
    }
}

impl Default for ThreatList {
    fn default() -> Self {
        Self::new()
    }
}

fn colour_idx(p: Player) -> usize {
    if p == Player::Black { 0 } else { 1 }
}

impl KingZones {
    /// `None` when a king is missing (never in play).
    pub fn compute(board: &BitboardBoard) -> Option<Self> {
        let mut z = Self { attackers: [[u8::MAX; 9]; 2], defenders: [[0; 9]; 2] };
        for king_colour in [Player::Black, Player::White] {
            let k = board.find_king_position(king_colour)?;
            let ki = colour_idx(king_colour);
            let king_bit = 1u128 << k.to_index();
            for dr in -1i8..=1 {
                for dc in -1i8..=1 {
                    let (r, c) = (k.row as i8 + dr, k.col as i8 + dc);
                    if !(0..9).contains(&r) || !(0..9).contains(&c) {
                        continue;
                    }
                    let sq = Position::new(r as u8, c as u8);
                    let off = ((dr + 1) * 3 + (dc + 1)) as usize;
                    let a = board.attackers_of(sq, king_colour.opposite()).count_ones();
                    let d = (board.attackers_of(sq, king_colour) & !king_bit).count_ones();
                    z.attackers[ki][off] = a.min(254) as u8;
                    z.defenders[ki][off] = d.min(255) as u8;
                }
            }
        }
        Some(z)
    }

    /// Active features of perspective `frame`, appended to `out`.
    pub fn features(&self, frame: Player, out: &mut ThreatList) {
        for side in 0..2 {
            let king_colour = if side == 0 { frame } else { frame.opposite() };
            let ki = colour_idx(king_colour);
            let mut n = 0;
            for off in 0..9 {
                // The frame turns the board 180 degrees for White, which
                // reverses the row-major offset order.
                let board_off = if frame == Player::Black { off } else { 8 - off };
                let a = self.attackers[ki][board_off];
                if a == u8::MAX || a == 0 {
                    continue;
                }
                n += 1;
                out.push(side * 27 + off * 3 + (a.min(3) as usize) - 1);
                if self.defenders[ki][board_off] == 0 {
                    out.push(54 + side * 9 + off);
                }
            }
            for k in 1..=n {
                out.push(72 + side * 9 + k - 1);
            }
        }
    }
}

/// Active threat features of `frame` for `board` (empty without both kings).
pub fn threat_features(board: &BitboardBoard, frame: Player) -> ThreatList {
    let mut out = ThreatList::new();
    if let Some(z) = KingZones::compute(board) {
        z.features(frame, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CapturedPieces;

    fn board(sfen: &str) -> BitboardBoard {
        let (b, _, _): (BitboardBoard, Player, CapturedPieces) =
            BitboardBoard::from_fen(sfen).expect("valid sfen");
        b
    }

    #[test]
    fn start_position_has_no_threats() {
        let b = board("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1");
        assert!(threat_features(&b, Player::Black).as_slice().is_empty());
        assert!(threat_features(&b, Player::White).as_slice().is_empty());
    }

    /// The two perspectives of a position and of its colour-mirror agree: the
    /// features are relative to the frame, like the POV piece features.
    #[test]
    fn mirror_symmetry() {
        // Black gold on 5c attacks the white king's zone; white silver on 4h
        // attacks the black king's.
        let a = board("4k4/9/4G4/9/9/9/9/5s3/4K4 b - 1");
        // The same position turned 180 degrees with colours swapped.
        let m = board("4k4/3S5/9/9/9/9/4g4/9/4K4 w - 1");
        let mut fa = threat_features(&a, Player::Black).as_slice().to_vec();
        let mut fm = threat_features(&m, Player::White).as_slice().to_vec();
        fa.sort_unstable();
        fm.sort_unstable();
        assert!(!fa.is_empty());
        assert_eq!(fa, fm);
        let mut ga = threat_features(&a, Player::White).as_slice().to_vec();
        let mut gm = threat_features(&m, Player::Black).as_slice().to_vec();
        ga.sort_unstable();
        gm.sort_unstable();
        assert_eq!(ga, gm);
    }

    #[test]
    fn counts_attackers_and_undefended() {
        // Black gold on 5c next to the white king on 5a: it attacks 4b, 5b, 6b
        // (and 4c/6c, outside the zone). None is defended except by the king.
        let b = board("4k4/9/4G4/9/9/9/9/9/4K4 b - 1");
        let f = threat_features(&b, Player::Black);
        let s = f.as_slice();
        // The enemy king is side 1 in Black's frame. 5b is offset 7 (row +1,
        // same file); 4b and 6b are offsets 6 and 8.
        for off in [6usize, 7, 8] {
            assert!(s.contains(&((27 + off * 3) as u16)), "attack on offset {off}");
            assert!(s.contains(&((54 + 9 + off) as u16)), "undefended offset {off}");
        }
        // Three attacked squares: thermometer levels 1-3 of side 1.
        for k in 0..3 {
            assert!(s.contains(&((72 + 9 + k) as u16)));
        }
        assert!(!s.contains(&((72 + 9 + 3) as u16)));
        assert!(s.iter().all(|&x| x < NUM_THREAT_FEATURES as u16));
    }
}
