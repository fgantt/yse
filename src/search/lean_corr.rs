//! Correction history for the lean search (on by default; `SHOGI_LEAN_CORR=0` turns it off).
//!
//! Session 40 found that our search gets far less from each node than
//! YaneuraOu's: even against a network of our class it needed ~16x the
//! reported nodes for parity, with a tree of the same width. Most strong engines
//! now adjust the static evaluation, before it drives pruning, by what searches
//! of similar positions found it got wrong. The evaluation's errors are
//! systematic in features the network sees badly; a small table indexed by
//! those features learns them during the game.
//!
//! Our version, written for shogi (the idea is public; the keys, units and
//! constants are our own):
//!   * pawn structure: both sides' pawn bitboards;
//!   * pieces in hand: both sides' counts (the material a drop can bring in,
//!     which a board-only key misses);
//!   * continuation: the opponent's last move (side, piece, target).
//! Each table is indexed by the side to move. The correction is the weighted
//! sum of the three entries. After a node's search, when its result is
//! consistent with the bound it was searched against and the best move is
//! quiet, each entry is nudged toward the difference between the search result
//! and the corrected evaluation, by a step that grows with depth.
//!
//! One table set per search thread, as with `lean_hist`.

use crate::bitboards::BitboardBoard;
use crate::types::core::{PieceType, Player};
use crate::types::CapturedPieces;

const SIZE: usize = 1 << 14;
/// Entry units per centipawn.
const GRAIN: i32 = 32;
/// Entries stay within +-LIMIT (+-256 cp).
const LIMIT: i32 = 8192;
/// (side, piece, target) move keys, as in `lean_hist::key`.
const PT: usize = 2 * PieceType::COUNT * 81;

/// `SHOGI_LEAN_CORR`: on by default since Session 40; `0` turns it off.
pub(super) fn corr_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("SHOGI_LEAN_CORR").map_or(true, |v| v != "0"))
}

/// `SHOGI_LEAN_CORR_W=pawn,hand,cont,step`: weights in quarters, and the update
/// step's divisor (a node at depth d moves an entry by d/step of the error,
/// capped). Defaults 2,2,3,7 from SPSA run v1 (hand-set before: 2,2,2,8).
fn params() -> [i32; 4] {
    static P: std::sync::OnceLock<[i32; 4]> = std::sync::OnceLock::new();
    *P.get_or_init(|| {
        let mut p = [2, 2, 3, 7];
        if let Ok(v) = std::env::var("SHOGI_LEAN_CORR_W") {
            for (slot, s) in p.iter_mut().zip(v.split(',')) {
                if let Ok(x) = s.trim().parse() {
                    *slot = x;
                }
            }
        }
        p[3] = p[3].max(1);
        p
    })
}

pub(super) struct Keys {
    pawn: usize,
    hand: usize,
    cont: Option<usize>,
}

fn mix(mut x: u64) -> u64 {
    // splitmix64 finaliser
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn side(p: Player) -> usize {
    usize::from(p == Player::White)
}

/// Table keys for the node where `player` is to move; `prev_key` is the
/// `lean_hist` key of the move that led here (`lean_hist::NO_KEY` for none).
pub(super) fn keys(
    board: &BitboardBoard,
    captured: &CapturedPieces,
    player: Player,
    prev_key: u16,
) -> Keys {
    let s = side(player);
    let pieces = board.get_pieces();
    let pawn_idx = PieceType::Pawn.as_index();
    let pb = pieces[0][pawn_idx].to_u128();
    let pw = pieces[1][pawn_idx].to_u128();
    let ph = mix((pb as u64) ^ mix((pb >> 64) as u64 ^ 0x9e37_79b9_7f4a_7c15))
        ^ mix((pw as u64).rotate_left(17) ^ mix((pw >> 64) as u64 ^ 0x2545_f491_4f6c_dd1d));
    // Hand counts packed: pawn 5 bits, lance/knight/silver/gold 3 each, bishop/rook 2.
    let pack = |hand: &[PieceType]| -> u64 {
        let mut c = [0u64; 7];
        for p in hand {
            let i = p.as_index();
            if i < 7 {
                c[i] += 1;
            }
        }
        c[0] | c[1] << 5 | c[2] << 8 | c[3] << 11 | c[4] << 14 | c[5] << 17 | c[6] << 19
    };
    let hh = mix(pack(&captured.black) | pack(&captured.white) << 21);
    let cont = (prev_key != super::lean_hist::NO_KEY).then(|| s * PT + prev_key as usize);
    Keys {
        pawn: s * SIZE + (ph as usize & (SIZE - 1)),
        hand: s * SIZE + (hh as usize & (SIZE - 1)),
        cont,
    }
}

pub(super) struct LeanCorr {
    pawn: Vec<i16>,
    hand: Vec<i16>,
    cont: Vec<i16>,
}

thread_local! {
    static CORR: std::cell::RefCell<Option<(u64, Box<LeanCorr>)>> = const { std::cell::RefCell::new(None) };
}

/// Tables of the Lazy SMP helpers between moves, by helper id. Helpers are new
/// threads on every move (`lazy_smp`), so their thread-local tables would start
/// empty each time (Session 40); a helper takes its tables from here when it
/// starts (`unpark`) and leaves them here when it ends (`park`).
static PARKED: std::sync::Mutex<Vec<Option<(u64, Box<LeanCorr>)>>> = std::sync::Mutex::new(Vec::new());

/// Gives this thread helper `slot`'s tables (an older game's are dropped on use).
pub(super) fn unpark(slot: usize) {
    let t = PARKED.lock().ok().and_then(|mut v| v.get_mut(slot).and_then(Option::take));
    CORR.with(|c| *c.borrow_mut() = t);
}

/// Leaves this thread's tables for the next helper `slot`.
pub(super) fn park(slot: usize) {
    let t = CORR.with(|c| c.borrow_mut().take());
    if let Ok(mut v) = PARKED.lock() {
        if v.len() <= slot {
            v.resize_with(slot + 1, || None);
        }
        v[slot] = t;
    }
}

/// Runs `f` on this thread's tables, creating them on first use in a game
/// (`lean_hist::new_game`).
pub(super) fn with<R>(f: impl FnOnce(&mut LeanCorr) -> R) -> R {
    CORR.with(|c| {
        let mut c = c.borrow_mut();
        let epoch = super::lean_hist::game_epoch();
        if c.as_ref().is_some_and(|(e, _)| *e != epoch) {
            *c = None;
        }
        f(&mut c.get_or_insert_with(|| {
            let t = LeanCorr { pawn: vec![0; 2 * SIZE], hand: vec![0; 2 * SIZE], cont: vec![0; 2 * PT] };
            (epoch, Box::new(t))
        }).1)
    })
}

fn nudge(e: &mut i16, bonus: i32) {
    let v = i32::from(*e);
    *e = (v + bonus - v * bonus.abs() / LIMIT).clamp(-LIMIT, LIMIT) as i16;
}

impl LeanCorr {
    /// Centipawns to add to the static evaluation at a node with keys `k`.
    pub(super) fn correction(&self, k: &Keys) -> i32 {
        let [wp, wh, wc, _] = params();
        let c = k.cont.map_or(0, |i| i32::from(self.cont[i]));
        (wp * i32::from(self.pawn[k.pawn]) + wh * i32::from(self.hand[k.hand]) + wc * c)
            / (4 * GRAIN)
    }

    /// Records that a search to `depth` returned `error` cp more than the
    /// corrected static evaluation.
    pub(super) fn update(&mut self, k: &Keys, error: i32, depth: i32) {
        let step = params()[3];
        let bonus = (error * GRAIN * depth / step).clamp(-LIMIT / 4, LIMIT / 4);
        nudge(&mut self.pawn[k.pawn], bonus);
        nudge(&mut self.hand[k.hand], bonus);
        if let Some(i) = k.cont {
            nudge(&mut self.cont[i], bonus);
        }
    }
}
