//! Standard-format networks (`nn.bin`, the `halfkp_256x2-32-32` layout most published shogi networks
//! use, and its mirrored `halfkpvm` variant) as first-class networks of this engine.
//!
//! A standard file is converted on load into [`NNUEWeights`] with this engine's own feature set
//! (`pov,hand,king,dual` with 81 king buckets), so the incremental accumulator, the make/unmake
//! stacks and the SIMD layer-2 code all apply unchanged. What differs from our own networks is set
//! by [`NNUEWeights::clipped`]: activations clipped to [0, 127], each layer's bias added before
//! its shift, and the output divided by 16 (the format's own units, as YaneuraOu reports them).
//!
//! The layout below is the format's, written from its description (feature numbering, file order);
//! no code of any other engine is used. `load` with `SHOGI_YO_NNUE` (`yo_nnue`, diagnostic build)
//! evaluates the same file from scratch and is the reference this conversion is checked against.
//!
//! File: version `0x7AF32F16`, hash, architecture string, feature-transformer hash, 256 i16
//! biases, rows x 256 i16 weights (row = king square x 1548 + BonaPiece), network hash, then the
//! 512->32, 32->32 and 32->1 layers (i32 biases, i8 weights stored output-major).

use crate::evaluation::nnue::{
    king_bucket, MidLayer, NNUEError, NNUEFeatureSet, NNUEWeights, HAND_PIECE_TYPES,
    KING_FEATURE_STRIDE, MAX_HAND_COUNT, NUM_HAND_FEATURES_PER_SIDE, NUM_NNUE_FEATURES,
};
use crate::types::core::{PieceType, Player};

const HALF: usize = 256;
const H2: usize = 32;
const FE_HAND_END: usize = 90;
const FE_END: usize = 1548;
const INPUTS_VM: usize = 45 * FE_END;
/// Input rows of plain HalfKP (81 king squares x 1548 BonaPieces).
pub const INPUTS_HALFKP: usize = 81 * FE_END;
pub const HALFKP_FE_END: usize = FE_END;
/// First four bytes of every standard-format file.
pub const VERSION: u32 = 0x7AF3_2F16;
/// The format's activation ceiling (u8 activations, 1.0 = 127).
pub const CLIP: i32 = 127;
/// The format's output divisor (`FV_SCALE`).
pub const OUTPUT_SCALE: i32 = 16;

/// Whether `bytes` start like a standard-format file.
pub fn is_standard(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == VERSION
}

/// A parsed standard-format file, in the file's own layout.
pub struct StdNet {
    /// HalfKP_vm: a king on files 6-9 (the format's squares 45..80) is mirrored, with the board
    /// pieces (not the hand).
    pub mirror: bool,
    pub ft_bias: Vec<i16>,
    pub ft_w: Vec<i16>,
    pub b1: Vec<i32>,
    pub w1: Vec<i8>,
    pub b2: Vec<i32>,
    pub w2: Vec<i8>,
    pub b3: i32,
    pub w3: Vec<i8>,
}

struct Reader<'a>(&'a [u8], usize);

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], NNUEError> {
        let s = self.0.get(self.1..self.1 + n).ok_or(NNUEError::InvalidFormat)?;
        self.1 += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, NNUEError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn i32s(&mut self, n: usize) -> Result<Vec<i32>, NNUEError> {
        Ok(self.bytes(4 * n)?.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
    }
    fn i16s(&mut self, n: usize) -> Result<Vec<i16>, NNUEError> {
        Ok(self.bytes(2 * n)?.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect())
    }
    fn i8s(&mut self, n: usize) -> Result<Vec<i8>, NNUEError> {
        Ok(self.bytes(n)?.iter().map(|&b| b as i8).collect())
    }
}

impl StdNet {
    pub fn parse(bytes: &[u8]) -> Result<Self, NNUEError> {
        let mut r = Reader(bytes, 0);
        if r.u32()? != VERSION {
            return Err(NNUEError::InvalidFormat);
        }
        let _hash = r.u32()?;
        let arch_len = r.u32()? as usize;
        let arch = String::from_utf8_lossy(r.bytes(arch_len)?).into_owned();
        if !arch.contains("256x2") || !arch.contains("HalfKP") {
            return Err(NNUEError::InvalidFormat);
        }
        let _ft_hash = r.u32()?;
        let ft_bias = r.i16s(HALF)?;
        // After the feature transformer: net hash, then 32 + 32x512 + 32 + 32x32 + 1 + 32 values.
        let tail = 4 + H2 * 4 + H2 * 2 * HALF + H2 * 4 + H2 * H2 + 4 + H2;
        let ft_bytes = bytes.len().checked_sub(r.1 + tail).ok_or(NNUEError::InvalidFormat)?;
        let mirror = match ft_bytes / (2 * HALF) {
            INPUTS_VM if ft_bytes % (2 * HALF) == 0 => true,
            INPUTS_HALFKP if ft_bytes % (2 * HALF) == 0 => false,
            _ => return Err(NNUEError::InvalidFormat),
        };
        let ft_w = r.i16s(ft_bytes / 2)?;
        let _net_hash = r.u32()?;
        let b1 = r.i32s(H2)?;
        let w1 = r.i8s(H2 * 2 * HALF)?;
        let b2 = r.i32s(H2)?;
        let w2 = r.i8s(H2 * H2)?;
        let b3 = r.i32s(1)?[0];
        let w3 = r.i8s(H2)?;
        if r.1 != bytes.len() {
            return Err(NNUEError::InvalidFormat);
        }
        Ok(Self { mirror, ft_bias, ft_w, b1, w1, b2, w2, b3, w3 })
    }

    /// The feature-transformer row of king square `k` and BonaPiece `p` (both the format's), with
    /// the mirroring of HalfKP_vm applied.
    fn ft_row(&self, k: usize, p: usize) -> &[i16] {
        let (k, p) = if self.mirror && k >= 45 {
            let p = if p >= FE_HAND_END {
                let (pi, s) = ((p - FE_HAND_END) / 81, (p - FE_HAND_END) % 81);
                FE_HAND_END + pi * 81 + mir(s)
            } else {
                p
            };
            (mir(k), p)
        } else {
            (k, p)
        };
        &self.ft_w[(FE_END * k + p) * HALF..][..HALF]
    }

    /// This engine's weights computing the same function, with the feature set they need.
    ///
    /// Our king-conditioned POV layout: per own-king square seen from the perspective (81 buckets),
    /// [`KING_FEATURE_STRIDE`] rows: board features `(mover/opponent, piece type, square)` in the
    /// perspective's frame, a side-to-move row (unused), then the hand thermometer. Each row is the
    /// format's row of the same piece: our frame is the format's (both turn the board 180 degrees
    /// for White), promoted minor pieces share gold's rows, kings have no rows (zero).
    pub fn to_weights(&self) -> Result<(NNUEWeights, NNUEFeatureSet), NNUEError> {
        let zero = vec![0i16; HALF];
        let mut rows = Vec::with_capacity(81 * KING_FEATURE_STRIDE);
        for bucket_sq in 0..81u8 {
            debug_assert_eq!(king_bucket(bucket_sq, 81), bucket_sq as usize);
            let k = halfkp_king_square(Player::Black, bucket_sq / 9, bucket_sq % 9);
            for f in 0..KING_FEATURE_STRIDE {
                let p = our_row_to_bonapiece(f);
                rows.push(p.map_or_else(|| zero.clone(), |p| self.ft_row(k, p).to_vec()));
            }
        }
        // Layer 2 and the middle layer are stored [inputs][outputs] here, output-major in the file.
        let w2: Vec<Vec<i16>> = (0..2 * HALF)
            .map(|i| (0..H2).map(|o| i16::from(self.w1[o * 2 * HALF + i])).collect())
            .collect();
        let mid = MidLayer {
            weights: (0..H2).map(|i| (0..H2).map(|o| i16::from(self.w2[o * H2 + i])).collect()).collect(),
            biases: self.b2.clone(),
            shift: 6,
        };
        let mut w = NNUEWeights {
            input_weights_1: rows,
            hidden_biases_1: self.ft_bias.iter().map(|&b| i32::from(b)).collect(),
            input_weights_2: Some(w2),
            hidden_biases_2: Some(self.b1.clone()),
            flat_weights_2: Vec::new(),
            l2_pairs: Vec::new(),
            l2_block_colsum_max: 0,
            l2_colsum_max: 0,
            output_weights: self.w3.iter().map(|&x| i16::from(x)).collect(),
            output_bias: self.b3,
            psqt: Vec::new(),
            l2_shift: 6,
            l3_shift: 0,
            mid_layers: Vec::new(),
            clipped: true,
        };
        w.set_mid_layers(vec![mid])?;
        w.rebuild_flat_weights_2();
        let fs = NNUEFeatureSet { pov: true, stm: false, halfkp: false, hand: true, king: true, dual: true };
        Ok((w, fs))
    }
}

/// Load a standard-format file's bytes as this engine's weights.
pub fn load(bytes: &[u8]) -> Result<(NNUEWeights, NNUEFeatureSet), NNUEError> {
    StdNet::parse(bytes)?.to_weights()
}

/// The BonaPiece (in the perspective's own frame) of row `f` of one king bucket of our layout, or
/// `None` for rows the format has no counterpart for (kings, the side-to-move row).
fn our_row_to_bonapiece(f: usize) -> Option<usize> {
    const PER_PLAYER: usize = 14 * 81;
    if f < NUM_NNUE_FEATURES {
        let owner = if f / PER_PLAYER == 0 { Player::Black } else { Player::White };
        let pt = BY_INDEX[(f % PER_PLAYER) / 81];
        debug_assert_eq!(pt.as_index(), (f % PER_PLAYER) / 81);
        if pt == PieceType::King {
            return None;
        }
        let sq = (f % 81) as u8;
        return Some(halfkp_board_feature(Player::Black, owner, pt, sq / 9, sq % 9));
    }
    let h = f.checked_sub(NUM_NNUE_FEATURES + 1)?;
    if h >= 2 * NUM_HAND_FEATURES_PER_SIDE {
        return None;
    }
    let holder = if h / NUM_HAND_FEATURES_PER_SIDE == 0 { Player::Black } else { Player::White };
    let mut i = h % NUM_HAND_FEATURES_PER_SIDE;
    for (t, &pt) in HAND_PIECE_TYPES.iter().enumerate() {
        let n = MAX_HAND_COUNT[t] as usize;
        if i < n {
            return Some(halfkp_hand_feature(Player::Black, holder, pt, i + 1));
        }
        i -= n;
    }
    None
}

/// Piece types in `PieceType::as_index` order (our board feature numbering).
const BY_INDEX: [PieceType; 14] = [
    PieceType::Pawn, PieceType::Lance, PieceType::Knight, PieceType::Silver, PieceType::Gold,
    PieceType::Bishop, PieceType::Rook, PieceType::King, PieceType::PromotedPawn,
    PieceType::PromotedLance, PieceType::PromotedKnight, PieceType::PromotedSilver,
    PieceType::PromotedBishop, PieceType::PromotedRook,
];

fn mir(sq: usize) -> usize {
    (8 - sq / 9) * 9 + sq % 9
}

/// The format's square index of our (row, col): files right to left, ranks top down.
fn yo_square(row: u8, col: u8) -> usize {
    (8 - col as usize) * 9 + row as usize
}

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

/// BonaPiece base for a board piece (promoted minor pieces count as gold).
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

/// (own, opponent's) BonaPiece bases for a piece in hand.
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
