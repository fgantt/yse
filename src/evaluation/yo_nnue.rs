//! Standard-format networks (`eval/nn.bin`): YaneuraOu's `halfkp_256x2-32-32` and its mirrored
//! `halfkpvm_256x2-32-32` variant, told apart by file size. Behind `yo-diagnostic`: other people's
//! networks are a diagnostic only; our own networks in this format (S48, `trainer/ --arch halfkp`)
//! are ours. The `halfkp_*` helpers below are the feature layout the trainer's tables come from.
//!
//! Session 35 asked how much of the gap to YaneuraOu is evaluation and how much is search.
//! With `SHOGI_YO_NNUE=/path/to/nn.bin` set, `PositionEvaluator::evaluate` returns this
//! network's score instead of ours, so our search runs on YaneuraOu's evaluation. Never shipped.
//!
//! Format and features follow YaneuraOu V9.00 (`source/eval/nnue/`): header (version, hash,
//! architecture string), feature transformer (hash, 256 i16 biases, 69,660 x 256 i16 weights),
//! network (hash, then 512->32, 32->32, 32->1 affine layers, i32 biases and i8 weights stored
//! row-major). HalfKP_vm: king on files 6-9 is mirrored to files 4-1 (with the board pieces,
//! not the hand). Evaluated from scratch each call (no incremental accumulator).

use crate::bitboards::BitboardBoard;
use crate::types::board::CapturedPieces;
use crate::types::core::{PieceType, Player, Position};
use std::io::Read;
use std::sync::OnceLock;

const HALF: usize = 256;
const FE_HAND_END: usize = 90;
const FE_END: usize = 1548;
/// Input rows of the mirrored variant (45 king squares) and of plain HalfKP (81).
const INPUTS_VM: usize = 45 * FE_END;
pub const INPUTS_HALFKP: usize = 81 * FE_END;
pub const HALFKP_FE_END: usize = FE_END;
const VERSION: u32 = 0x7AF3_2F16;
const FV_SCALE: i32 = 16;

pub struct YoNnue {
    /// HalfKP_vm (king on files 6-9 mirrored); false for plain HalfKP.
    mirror: bool,
    ft_bias: Vec<i16>,
    ft_w: Vec<i16>,
    b1: Vec<i32>,
    w1: Vec<i8>,
    b2: Vec<i32>,
    w2: Vec<i8>,
    b3: i32,
    w3: Vec<i8>,
}

/// The network named by `SHOGI_YO_NNUE`, loaded on first use; `None` when unset.
pub fn global() -> Option<&'static YoNnue> {
    static NET: OnceLock<Option<YoNnue>> = OnceLock::new();
    NET.get_or_init(|| {
        let path = std::env::var("SHOGI_YO_NNUE").ok()?;
        match YoNnue::load(&path) {
            Ok(n) => {
                eprintln!("info string YaneuraOu network loaded: {path}");
                Some(n)
            }
            Err(e) => {
                eprintln!("info string YaneuraOu network failed to load ({path}): {e}");
                None
            }
        }
    })
    .as_ref()
}

struct Reader(Vec<u8>, usize);

impl Reader {
    fn take<const N: usize>(&mut self) -> std::io::Result<[u8; N]> {
        let s = self.0.get(self.1..self.1 + N).ok_or_else(|| err("truncated file"))?;
        self.1 += N;
        Ok(s.try_into().unwrap())
    }
    fn u32(&mut self) -> std::io::Result<u32> { Ok(u32::from_le_bytes(self.take::<4>()?)) }
    fn i32(&mut self) -> std::io::Result<i32> { Ok(i32::from_le_bytes(self.take::<4>()?)) }
    fn i16(&mut self) -> std::io::Result<i16> { Ok(i16::from_le_bytes(self.take::<2>()?)) }
    fn i8(&mut self) -> std::io::Result<i8> { Ok(self.take::<1>()?[0] as i8) }
    fn i32s(&mut self, n: usize) -> std::io::Result<Vec<i32>> { (0..n).map(|_| self.i32()).collect() }
    fn i16s(&mut self, n: usize) -> std::io::Result<Vec<i16>> { (0..n).map(|_| self.i16()).collect() }
    fn i8s(&mut self, n: usize) -> std::io::Result<Vec<i8>> { (0..n).map(|_| self.i8()).collect() }
}

fn err(m: &str) -> std::io::Error { std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string()) }

impl YoNnue {
    pub fn load(path: &str) -> std::io::Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut bytes)?;
        let mut r = Reader(bytes, 0);
        if r.u32()? != VERSION {
            return Err(err("not a YaneuraOu NNUE file"));
        }
        let _hash = r.u32()?;
        let arch_len = r.u32()? as usize;
        r.1 += arch_len;
        let _ft_hash = r.u32()?;
        let ft_bias = r.i16s(HALF)?;
        // The rest after the feature transformer: net hash, then 32 + 32x512 + 32 + 32x32 + 1 + 32.
        let tail = 4 + 32 * 4 + 32 * 2 * HALF + 32 * 4 + 32 * 32 + 4 + 32;
        let ft_bytes = r.0.len().checked_sub(r.1 + tail).ok_or_else(|| err("truncated file"))?;
        let mirror = match ft_bytes / (2 * HALF) {
            INPUTS_VM => true,
            INPUTS_HALFKP => false,
            _ => return Err(err("unexpected file size (not halfkp(vm)_256x2-32-32)")),
        };
        let ft_w = r.i16s(ft_bytes / 2)?;
        let _net_hash = r.u32()?;
        let b1 = r.i32s(32)?;
        let w1 = r.i8s(32 * 2 * HALF)?;
        let b2 = r.i32s(32)?;
        let w2 = r.i8s(32 * 32)?;
        let b3 = r.i32()?;
        let w3 = r.i8s(32)?;
        if r.1 != r.0.len() {
            return Err(err("unexpected file size (not halfkp(vm)_256x2-32-32)"));
        }
        Ok(Self { mirror, ft_bias, ft_w, b1, w1, b2, w2, b3, w3 })
    }

    /// Score in YaneuraOu's units from `stm`'s point of view.
    pub fn evaluate(&self, board: &BitboardBoard, stm: Player, captured: &CapturedPieces) -> i32 {
        // BonaPieces from Black's view (fb) and White's view (fw), and both kings.
        let mut fb = [0u16; 40];
        let mut fw = [0u16; 40];
        let mut n = 0;
        let (mut kb, mut kw) = (0usize, 0usize);
        for row in 0..9u8 {
            for col in 0..9u8 {
                let Some(p) = board.get_piece(Position::new(row, col)) else { continue };
                if p.piece_type == PieceType::King {
                    if p.player == Player::Black {
                        kb = halfkp_king_square(Player::Black, row, col);
                    } else {
                        kw = halfkp_king_square(Player::White, row, col);
                    }
                    continue;
                }
                fb[n] = halfkp_board_feature(Player::Black, p.player, p.piece_type, row, col) as u16;
                fw[n] = halfkp_board_feature(Player::White, p.player, p.piece_type, row, col) as u16;
                n += 1;
            }
        }
        for (holder, list) in [(Player::Black, &captured.black), (Player::White, &captured.white)] {
            for pt in [
                PieceType::Pawn, PieceType::Lance, PieceType::Knight, PieceType::Silver,
                PieceType::Gold, PieceType::Bishop, PieceType::Rook,
            ] {
                let count = list.iter().filter(|&&q| q == pt).count();
                for k in 1..=count {
                    if n >= 38 { break; }
                    fb[n] = halfkp_hand_feature(Player::Black, holder, pt, k) as u16;
                    fw[n] = halfkp_hand_feature(Player::White, holder, pt, k) as u16;
                    n += 1;
                }
            }
        }
        let acc_b = self.accumulate(kb, &fb[..n]);
        let acc_w = self.accumulate(kw, &fw[..n]);
        let (us, them) = if stm == Player::Black { (&acc_b, &acc_w) } else { (&acc_w, &acc_b) };
        let mut input = [0u8; 2 * HALF];
        for j in 0..HALF {
            input[j] = us[j].clamp(0, 127) as u8;
            input[HALF + j] = them[j].clamp(0, 127) as u8;
        }
        let mut h1 = [0u8; 32];
        for o in 0..32 {
            let row = &self.w1[o * 2 * HALF..(o + 1) * 2 * HALF];
            let mut s = self.b1[o];
            for i in 0..2 * HALF {
                s += row[i] as i32 * input[i] as i32;
            }
            h1[o] = (s >> 6).clamp(0, 127) as u8;
        }
        let mut h2 = [0u8; 32];
        for o in 0..32 {
            let row = &self.w2[o * 32..(o + 1) * 32];
            let mut s = self.b2[o];
            for i in 0..32 {
                s += row[i] as i32 * h1[i] as i32;
            }
            h2[o] = (s >> 6).clamp(0, 127) as u8;
        }
        let mut out = self.b3;
        for i in 0..32 {
            out += self.w3[i] as i32 * h2[i] as i32;
        }
        out / FV_SCALE
    }

    fn accumulate(&self, king: usize, pieces: &[u16]) -> [i16; HALF] {
        let mut acc = [0i16; HALF];
        acc.copy_from_slice(&self.ft_bias);
        let mirror = self.mirror && king >= 45;
        let k = if mirror { mir(king) } else { king };
        for &p in pieces {
            let mut p = p as usize;
            if mirror && p >= FE_HAND_END {
                let (pi, s) = ((p - FE_HAND_END) / 81, (p - FE_HAND_END) % 81);
                p = FE_HAND_END + pi * 81 + mir(s);
            }
            let row = &self.ft_w[(FE_END * k + p) * HALF..][..HALF];
            for j in 0..HALF {
                acc[j] = acc[j].wrapping_add(row[j]);
            }
        }
        acc
    }
}

fn mir(sq: usize) -> usize { (8 - sq / 9) * 9 + sq % 9 }

/// The standard format's square index of our (row, col): files right to left, ranks top down.
fn yo_square(row: u8, col: u8) -> usize { (8 - col as usize) * 9 + row as usize }

/// `frame`'s own king square in `frame`'s view (the row of the feature block it selects).
pub fn halfkp_king_square(frame: Player, row: u8, col: u8) -> usize {
    let sq = yo_square(row, col);
    if frame == Player::Black { sq } else { 80 - sq }
}

/// BonaPiece of `owner`'s board piece `pt` on (row, col), from `frame`'s view (not a king).
pub fn halfkp_board_feature(frame: Player, owner: Player, pt: PieceType, row: u8, col: u8) -> usize {
    let sq = yo_square(row, col);
    let f = board_base(pt);
    let (base, s) = if frame == Player::Black { (f, sq) } else { (f, 80 - sq) };
    if owner == frame { base + s } else { base + 81 + s }
}

/// BonaPiece of the `k`-th (1-based) `pt` in `holder`'s hand, from `frame`'s view.
pub fn halfkp_hand_feature(frame: Player, holder: Player, pt: PieceType, k: usize) -> usize {
    let (f, e) = hand_base(pt);
    (if holder == frame { f } else { e }) + k - 1
}

/// `f_*` BonaPiece base for a board piece (promoted minor pieces count as gold).
fn board_base(pt: PieceType) -> usize {
    match pt {
        PieceType::Pawn => 90,
        PieceType::Lance => 252,
        PieceType::Knight => 414,
        PieceType::Silver => 576,
        PieceType::Gold
        | PieceType::PromotedPawn
        | PieceType::PromotedLance
        | PieceType::PromotedKnight
        | PieceType::PromotedSilver => 738,
        PieceType::Bishop => 900,
        PieceType::PromotedBishop => 1062,
        PieceType::Rook => 1224,
        PieceType::PromotedRook => 1386,
        PieceType::King => unreachable!(),
    }
}

/// (`f_hand_*`, `e_hand_*`) BonaPiece bases for a piece in hand.
fn hand_base(pt: PieceType) -> (usize, usize) {
    match pt {
        PieceType::Pawn => (1, 20),
        PieceType::Lance => (39, 44),
        PieceType::Knight => (49, 54),
        PieceType::Silver => (59, 64),
        PieceType::Gold => (69, 74),
        PieceType::Bishop => (79, 82),
        _ => (85, 88),
    }
}
