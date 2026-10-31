//! Data path for the PyTorch trainer (`trainer/`, Session 38).
//!
//! * `pack`: JSONL corpus -> fixed 100-byte records, held-out games (every 20th
//!   game id, as the offline trainer) in `val.bin`, the rest shuffled into
//!   `train.bin`.
//! * `tables`: the feature-index lookup tables the trainer builds its inputs
//!   from, computed with the engine's own index functions.
//! * `check`: for the first records of a corpus, the packed bytes, the engine's
//!   active features for both perspectives and (with `--weights`) the engine's
//!   evaluation, so the trainer can prove it computes the same features and,
//!   after export, the same scores.
//!
//! * `pack-psv` / `psv-check`: the same records from PackedSfenValue files, the
//!   40-byte format of YaneuraOu-family training sets (e.g. nodchip's MIT-licensed
//!   Hao and Suisho5 sets on Hugging Face), with our own decoder.
//!
//! Record layout (little endian): board[81] u8 (0 empty, else 1 + 14 * colour +
//! `PieceType::as_index`, colour 0 Black), hand[14] u8 (Black's P L N S G B R,
//! then White's), side to move u8 (0 Black), teacher eval i16 (side to move's
//! view, clamped to +-32000), outcome i8 (+1 Black won, -1 White, 0 draw), pad u8.

use clap::{Parser, Subcommand};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{
    feature_index_pov, hand_feature_index_pov, king_bucket, to_mover_frame, NNUEEvaluator, NNUEWeights,
    HAND_FEATURE_BASE_FLAT, HAND_PIECE_TYPES, KING_FEATURE_STRIDE, MAX_HAND_COUNT,
};
use shogi_engine::evaluation::nnue_training::extract_features_frame;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::types::board::CapturedPieces;
use shogi_engine::types::core::{PieceType, Player, Position};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const RECORD: usize = 100;
const VAL_STRIDE: u32 = 20;
const PIECE_TYPES: [PieceType; 14] = [
    PieceType::Pawn,
    PieceType::Lance,
    PieceType::Knight,
    PieceType::Silver,
    PieceType::Gold,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::King,
    PieceType::PromotedPawn,
    PieceType::PromotedLance,
    PieceType::PromotedKnight,
    PieceType::PromotedSilver,
    PieceType::PromotedBishop,
    PieceType::PromotedRook,
];

#[derive(Parser)]
#[command(name = "nnue-pack")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Pack a JSONL corpus into `train.bin` (shuffled) and `val.bin`.
    Pack {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        /// Temporary shuffle buckets (each must fit in memory).
        #[arg(long, default_value_t = 128)]
        buckets: usize,
        #[arg(long, default_value_t = 38)]
        seed: u64,
        /// Keep only the training games with `(game_id / 20) % 2 == 0` (about half);
        /// held-out games are all kept, so validation matches the full pool's.
        #[arg(long)]
        half: bool,
        /// Skip positions that are not quiet: the side to move is in check, or its
        /// move in the game (read from the next record) captured a piece.
        #[arg(long)]
        quiet: bool,
    },
    /// Write the feature lookup tables as JSON.
    Tables {
        #[arg(long, default_value_t = 9)]
        king_buckets: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// Pack PackedSfenValue files: the last file is held out as `val.bin`.
    PackPsv {
        #[arg(long, num_args = 1.., required = true)]
        inputs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 128)]
        buckets: usize,
        #[arg(long, default_value_t = 38)]
        seed: u64,
        /// Skip positions in check or whose stored best move captures.
        #[arg(long)]
        quiet: bool,
    },
    /// Decode PackedSfenValue records and check them: every position legal and
    /// parsable, the stored move legal in it; prints statistics and, with
    /// `--jsonl`, writes the first `n` as corpus lines (sfen, score, move).
    PsvCheck {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 100_000)]
        n: usize,
        #[arg(long)]
        jsonl: Option<PathBuf>,
    },
    /// Packed bytes, engine features and engine evals for the first records.
    Check {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long, default_value_t = 1000)]
        n: usize,
        #[arg(long, default_value_t = 9)]
        king_buckets: usize,
        #[arg(long)]
        weights: Option<PathBuf>,
    },
}

struct Rec {
    sfen: String,
    player: Player,
    eval_cp: Option<i32>,
    outcome: i8,
    game_id: u32,
    ply: u32,
}

/// Which records `pack` keeps.
#[derive(Clone, Copy)]
struct Filter {
    half: bool,
    quiet: bool,
}

/// The value after `"key":` up to the next `,` or `}` (strings keep their quotes).
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\":");
    let i = line.find(&needle)? + needle.len();
    let rest = line[i..].trim_start();
    if let Some(s) = rest.strip_prefix('"') {
        return Some(&s[..s.find('"')?]);
    }
    let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn parse(line: &str) -> Option<Rec> {
    let player = match field(line, "player")? {
        "black" => Player::Black,
        "white" => Player::White,
        _ => return None,
    };
    let eval = field(line, "eval_cp")?;
    Some(Rec {
        sfen: field(line, "sfen")?.to_string(),
        player,
        eval_cp: if eval == "null" { None } else { eval.parse().ok() },
        outcome: field(line, "outcome")?.parse().ok()?,
        game_id: field(line, "game_id")?.parse().ok()?,
        ply: field(line, "ply").and_then(|v| v.parse().ok()).unwrap_or(0),
    })
}

fn colour(p: Player) -> usize {
    match p {
        Player::Black => 0,
        Player::White => 1,
    }
}

fn encode(board: &BitboardBoard, captured: &CapturedPieces, rec: &Rec) -> [u8; RECORD] {
    let mut out = [0u8; RECORD];
    for sq in 0..81u8 {
        if let Some(piece) = board.get_piece(Position::from_u8(sq)) {
            out[sq as usize] = (1 + 14 * colour(piece.player) + piece.piece_type.as_index()) as u8;
        }
    }
    for (side, player) in [Player::Black, Player::White].into_iter().enumerate() {
        for (t, &pt) in HAND_PIECE_TYPES.iter().enumerate() {
            out[81 + 7 * side + t] = captured.count(pt, player).min(255) as u8;
        }
    }
    out[95] = colour(rec.player) as u8;
    let eval = rec.eval_cp.unwrap_or(0).clamp(-32000, 32000) as i16;
    out[96..98].copy_from_slice(&eval.to_le_bytes());
    out[98] = rec.outcome as u8;
    out
}

/// Pieces `player` holds in hand.
fn hand_total(captured: &CapturedPieces, player: Player) -> usize {
    HAND_PIECE_TYPES.iter().map(|&pt| captured.count(pt, player)).sum()
}

/// Parse and encode one line: (is held-out, record), or `None` to skip it.
/// `next` is the following line of the corpus, which `--quiet` reads to see
/// whether this position's move captured.
fn pack_line(line: &str, next: Option<&str>, filter: Filter) -> Option<(bool, [u8; RECORD])> {
    let rec = parse(line)?;
    rec.eval_cp?;
    let held_out = rec.game_id % VAL_STRIDE == 0;
    if filter.half && !held_out && (rec.game_id / VAL_STRIDE) % 2 != 0 {
        return None;
    }
    let (board, _side, captured) = BitboardBoard::from_fen(&rec.sfen).ok()?;
    if filter.quiet {
        if board.is_king_in_check(rec.player, &captured) {
            return None;
        }
        let next = next.and_then(parse).filter(|n| n.game_id == rec.game_id && n.ply == rec.ply + 1);
        if let Some(n) = next {
            let (_, _, after) = BitboardBoard::from_fen(&n.sfen).ok()?;
            if hand_total(&after, rec.player) > hand_total(&captured, rec.player) {
                return None;
            }
        }
    }
    Some((held_out, encode(&board, &captured, &rec)))
}

fn pack(
    corpus: &Path,
    out_dir: &Path,
    nbuckets: usize,
    seed: u64,
    filter: Filter,
) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let bucket_path = |b: usize| out_dir.join(format!("bucket_{b:03}.tmp"));
    let mut buckets: Vec<BufWriter<File>> = (0..nbuckets)
        .map(|b| File::create(bucket_path(b)).map(|f| BufWriter::with_capacity(1 << 20, f)))
        .collect::<Result<_, _>>()?;
    let mut val = BufWriter::new(File::create(out_dir.join("val.bin"))?);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let (mut n_train, mut n_val, mut skipped) = (0u64, 0u64, 0u64);
    let mut lines = BufReader::with_capacity(1 << 24, File::open(corpus)?).lines();
    let started = std::time::Instant::now();
    // The last line of each chunk waits for the next chunk, which holds its successor.
    let mut carry: Option<String> = None;
    loop {
        let mut chunk: Vec<String> = carry.take().into_iter().collect();
        let fresh = chunk.len();
        chunk.extend(lines.by_ref().take(1 << 20).collect::<Result<Vec<_>, _>>()?);
        let done = chunk.len() == fresh;
        if !done {
            carry = chunk.pop();
        }
        let packed: Vec<Option<(bool, [u8; RECORD])>> = (0..chunk.len())
            .into_par_iter()
            .map(|i| {
                let next = chunk.get(i + 1).or(carry.as_ref()).map(String::as_str);
                pack_line(&chunk[i], next, filter)
            })
            .collect();
        for p in packed {
            match p {
                Some((true, r)) => {
                    val.write_all(&r)?;
                    n_val += 1;
                }
                Some((false, r)) => {
                    buckets[rng.gen_range(0..nbuckets)].write_all(&r)?;
                    n_train += 1;
                }
                None => skipped += 1,
            }
        }
        eprintln!(
            "  {:>11} train {:>10} val {:>8} skipped  {:.0} s",
            n_train,
            n_val,
            skipped,
            started.elapsed().as_secs_f64()
        );
        if done {
            break;
        }
    }
    val.flush()?;
    for b in buckets.iter_mut() {
        b.flush()?;
    }
    drop(buckets);
    // Second pass: shuffle each bucket in memory and append it to train.bin.
    let mut train = BufWriter::with_capacity(1 << 24, File::create(out_dir.join("train.bin"))?);
    for b in 0..nbuckets {
        let mut bytes = Vec::new();
        File::open(bucket_path(b))?.read_to_end(&mut bytes)?;
        let mut idx: Vec<u32> = (0..(bytes.len() / RECORD) as u32).collect();
        idx.shuffle(&mut rng);
        for &i in &idx {
            let i = i as usize * RECORD;
            train.write_all(&bytes[i..i + RECORD])?;
        }
        std::fs::remove_file(bucket_path(b))?;
    }
    train.flush()?;
    println!("train {n_train}  val {n_val}  skipped {skipped}");
    Ok(())
}

// --- PackedSfenValue (YaneuraOu-family training data) -------------------------
//
// 40 bytes: a 256-bit packed position, score i16 (side to move's view), best move
// u16, game ply u16, game result i8 (side to move's view: 1 win, 0 draw, -1 loss),
// padding u8. The position is a little-endian bit stream: side to move (1 bit),
// Black's and White's king squares (7 bits each), then every other square from
// 1a, 1b, ... to 9i (square index (file - 1) * 9 + rank - 1) as a Huffman code
// (0 = empty), and then the pieces in hand until the 256 bits are used up. A board
// piece is its code, a promotion bit (not for gold) and a colour bit (1 = White);
// a piece in hand drops the code's first bit, so moving a piece to a hand keeps
// the total at 256 bits. Codes in the order bits are read:
//   P 1,0   L 1,1,0,0   N 1,1,0,1   S 1,1,1,0   G 1,1,1,1,0   B 1,1,1,1,1,0   R 1,1,1,1,1,1
// The move: to square in bits 0-6, from square in bits 7-13, bit 14 a drop
// (the from field then holds the piece type, P=1 L N S B R G=7), bit 15 promotion.

const PSV: usize = 40;

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        let b = (*self.data.get(self.pos / 8)? >> (self.pos % 8)) & 1;
        self.pos += 1;
        Some(u32::from(b))
    }
    fn bits(&mut self, n: usize) -> Option<u32> {
        (0..n).try_fold(0, |acc, i| Some(acc | (self.bit()? << i)))
    }
    /// A piece code without its leading bit (which the caller has read): the
    /// SFEN letter of the piece, or `None` past the end.
    fn piece_tail(&mut self) -> Option<char> {
        if self.bit()? == 0 {
            return Some('P');
        }
        if self.bit()? == 0 {
            return Some(if self.bit()? == 0 { 'L' } else { 'N' });
        }
        if self.bit()? == 0 {
            return Some('S');
        }
        if self.bit()? == 0 {
            return Some('G');
        }
        Some(if self.bit()? == 0 { 'B' } else { 'R' })
    }
}

fn square_name(sq: u32) -> String {
    format!("{}{}", sq / 9 + 1, (b'a' + (sq % 9) as u8) as char)
}

struct Psv {
    sfen: String,
    player: Player,
    usi_move: String,
    score: i16,
    /// Game result from Black's view.
    outcome: i8,
    ply: u16,
}

fn decode_psv(r: &[u8]) -> Option<Psv> {
    let mut b = Bits { data: &r[..32], pos: 0 };
    let player = if b.bit()? == 0 { Player::Black } else { Player::White };
    let kings = [b.bits(7)?, b.bits(7)?];
    let mut board: [Option<String>; 81] = std::array::from_fn(|_| None);
    for (c, &k) in kings.iter().enumerate() {
        *board.get_mut(k as usize)? = Some(if c == 0 { "K" } else { "k" }.to_string());
    }
    for sq in 0..81 {
        if board[sq].is_some() {
            continue;
        }
        if b.bit()? == 0 {
            continue;
        }
        let p = b.piece_tail()?;
        let promoted = p != 'G' && b.bit()? == 1;
        let white = b.bit()? == 1;
        let letter = if white { p.to_ascii_lowercase() } else { p };
        board[sq] = Some(if promoted { format!("+{letter}") } else { letter.to_string() });
    }
    // Hand: Black's and White's counts per piece letter.
    let order = ['R', 'B', 'G', 'S', 'N', 'L', 'P'];
    let mut hand = [[0u32; 7]; 2];
    while b.pos < 256 {
        let p = b.piece_tail()?;
        if p != 'G' {
            b.bit()?; // promotion bit, always 0 in hand
        }
        let white = b.bit()? as usize;
        hand[white][order.iter().position(|&o| o == p)?] += 1;
    }
    if b.pos != 256 {
        return None;
    }
    let mut sfen = String::new();
    for rank in 0..9 {
        let mut empty = 0;
        for file in (0..9).rev() {
            match &board[file * 9 + rank] {
                Some(p) => {
                    if empty > 0 {
                        sfen += &empty.to_string();
                        empty = 0;
                    }
                    sfen += p;
                }
                None => empty += 1,
            }
        }
        if empty > 0 {
            sfen += &empty.to_string();
        }
        if rank < 8 {
            sfen.push('/');
        }
    }
    sfen += if player == Player::Black { " b " } else { " w " };
    let mut hs = String::new();
    for (c, counts) in hand.iter().enumerate() {
        for (i, &n) in counts.iter().enumerate() {
            if n > 0 {
                if n > 1 {
                    hs += &n.to_string();
                }
                hs.push(if c == 0 { order[i] } else { order[i].to_ascii_lowercase() });
            }
        }
    }
    sfen += if hs.is_empty() { "-" } else { &hs };
    let m = u16::from_le_bytes([r[34], r[35]]);
    let (to, from) = (u32::from(m & 0x7f), u32::from((m >> 7) & 0x7f));
    let usi_move = if m & (1 << 14) != 0 {
        let pt = if from > 80 { from - 80 } else { from };
        let letter = *['P', 'L', 'N', 'S', 'B', 'R', 'G'].get(pt.checked_sub(1)? as usize)?;
        format!("{letter}*{}", square_name(to))
    } else {
        let promo = if m & (1 << 15) != 0 { "+" } else { "" };
        format!("{}{}{promo}", square_name(from), square_name(to))
    };
    let result = r[38] as i8;
    Some(Psv {
        sfen,
        player,
        usi_move,
        score: i16::from_le_bytes([r[32], r[33]]),
        outcome: if player == Player::Black { result } else { -result },
        ply: u16::from_le_bytes([r[36], r[37]]),
    })
}

/// Whether `mv` (USI) is legal in the position, and whether it captures.
fn move_facts(board: &BitboardBoard, player: Player, captured: &CapturedPieces, mv: &str)
    -> (bool, bool) {
    let legal = MoveGenerator::new()
        .generate_legal_moves(board, player, captured)
        .iter()
        .any(|m| m.to_usi_string() == mv);
    let capture = !mv.contains('*')
        && mv
            .get(2..4)
            .and_then(|t| Position::from_usi_string(t).ok())
            .and_then(|t| board.get_piece(t))
            .is_some();
    (legal, capture)
}

fn psv_check(input: &Path, n: usize, jsonl: Option<&Path>) -> std::io::Result<()> {
    let bytes = std::fs::read(input)?;
    let total = bytes.len() / PSV;
    let n = n.min(total);
    let rows: Vec<(Option<Psv>, bool, bool, bool)> = (0..n)
        .into_par_iter()
        .map(|i| {
            let Some(p) = decode_psv(&bytes[i * PSV..(i + 1) * PSV]) else {
                return (None, false, false, false);
            };
            let Ok((board, side, captured)) = BitboardBoard::from_fen(&p.sfen) else {
                return (Some(p), false, false, false);
            };
            let in_check = board.is_king_in_check(side, &captured);
            let (legal, capture) = move_facts(&board, side, &captured, &p.usi_move);
            (Some(p), legal, capture, in_check)
        })
        .collect();
    let decoded = rows.iter().filter(|r| r.0.is_some()).count();
    let legal = rows.iter().filter(|r| r.1).count();
    let captures = rows.iter().filter(|r| r.1 && r.2).count();
    let checks = rows.iter().filter(|r| r.3).count();
    println!("{total} records in file; checked {n}: decoded {decoded}, stored move legal {legal}, \
              capture {captures}, in check {checks}");
    let scores: Vec<f64> = rows.iter().filter_map(|r| r.0.as_ref()).map(|p| f64::from(p.score)).collect();
    let mean_abs = scores.iter().map(|s| s.abs().min(3000.0)).sum::<f64>() / scores.len().max(1) as f64;
    println!("mean |score| (clamped 3000) {mean_abs:.0}");
    for r in rows.iter().filter(|r| !r.1).take(5) {
        match &r.0 {
            Some(p) => println!("  BAD sfen {} move {}", p.sfen, p.usi_move),
            None => println!("  BAD undecodable"),
        }
    }
    if let Some(path) = jsonl {
        let mut out = BufWriter::new(File::create(path)?);
        for (i, r) in rows.iter().enumerate() {
            if let Some(p) = &r.0 {
                let row = serde_json::json!({"i": i, "sfen": p.sfen, "score": p.score,
                    "move": p.usi_move, "ply": p.ply, "outcome": p.outcome});
                writeln!(out, "{row}")?;
            }
        }
    }
    Ok(())
}

fn pack_psv(
    inputs: &[PathBuf],
    out_dir: &Path,
    nbuckets: usize,
    seed: u64,
    quiet: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let bucket_path = |b: usize| out_dir.join(format!("bucket_{b:03}.tmp"));
    let mut buckets: Vec<BufWriter<File>> = (0..nbuckets)
        .map(|b| File::create(bucket_path(b)).map(|f| BufWriter::with_capacity(1 << 20, f)))
        .collect::<Result<_, _>>()?;
    let mut val = BufWriter::new(File::create(out_dir.join("val.bin"))?);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let (mut n_train, mut n_val, mut skipped) = (0u64, 0u64, 0u64);
    let started = std::time::Instant::now();
    for (fi, path) in inputs.iter().enumerate() {
        let held_out = fi + 1 == inputs.len();
        let bytes = std::fs::read(path)?;
        let packed: Vec<Option<[u8; RECORD]>> = bytes
            .par_chunks_exact(PSV)
            .map(|r| {
                let p = decode_psv(r)?;
                let (board, side, captured) = BitboardBoard::from_fen(&p.sfen).ok()?;
                if quiet {
                    if board.is_king_in_check(side, &captured) {
                        return None;
                    }
                    let (legal, capture) = move_facts(&board, side, &captured, &p.usi_move);
                    if !legal || capture {
                        return None;
                    }
                }
                let rec = Rec {
                    sfen: p.sfen,
                    player: p.player,
                    eval_cp: Some(i32::from(p.score)),
                    outcome: p.outcome,
                    game_id: 0,
                    ply: u32::from(p.ply),
                };
                Some(encode(&board, &captured, &rec))
            })
            .collect();
        for r in packed {
            match r {
                Some(r) if held_out => {
                    val.write_all(&r)?;
                    n_val += 1;
                }
                Some(r) => {
                    buckets[rng.gen_range(0..nbuckets)].write_all(&r)?;
                    n_train += 1;
                }
                None => skipped += 1,
            }
        }
        eprintln!("  file {}/{}: train {n_train} val {n_val} skipped {skipped}  {:.0} s",
                  fi + 1, inputs.len(), started.elapsed().as_secs_f64());
    }
    val.flush()?;
    for b in buckets.iter_mut() {
        b.flush()?;
    }
    drop(buckets);
    let mut train = BufWriter::with_capacity(1 << 24, File::create(out_dir.join("train.bin"))?);
    for b in 0..nbuckets {
        let mut bytes = Vec::new();
        File::open(bucket_path(b))?.read_to_end(&mut bytes)?;
        let mut idx: Vec<u32> = (0..(bytes.len() / RECORD) as u32).collect();
        idx.shuffle(&mut rng);
        for &i in &idx {
            let i = i as usize * RECORD;
            train.write_all(&bytes[i..i + RECORD])?;
        }
        std::fs::remove_file(bucket_path(b))?;
    }
    train.flush()?;
    println!("train {n_train}  val {n_val}  skipped {skipped}");
    Ok(())
}

/// Feature lookup tables (`-1` = no feature). Indices are rows of the
/// king-conditioned input layer: `piece[frame][code][sq] + king[frame][own king sq]`,
/// likewise for `hand[frame][holder][type][k]` (k = 1..=max).
fn tables(nbuckets: usize, out: &Path) -> std::io::Result<()> {
    let frames = [Player::Black, Player::White];
    let mut piece = vec![vec![vec![-1i64; 81]; 29]; 2];
    let mut hand = vec![vec![vec![vec![-1i64; 19]; 7]; 2]; 2];
    let mut king = vec![vec![0i64; 81]; 2];
    for (f, &frame) in frames.iter().enumerate() {
        for (c, &owner) in frames.iter().enumerate() {
            for (t, &pt) in PIECE_TYPES.iter().enumerate() {
                for sq in 0..81u8 {
                    piece[f][1 + 14 * c + t][sq as usize] =
                        feature_index_pov(frame, owner, pt, sq) as i64;
                }
            }
            for (t, &pt) in HAND_PIECE_TYPES.iter().enumerate() {
                for k in 1..=MAX_HAND_COUNT[t] {
                    if let Some(i) = hand_feature_index_pov(HAND_FEATURE_BASE_FLAT, frame, owner, pt, k)
                    {
                        hand[f][c][t][k as usize] = i as i64;
                    }
                }
            }
        }
        for sq in 0..81u8 {
            let in_frame = to_mover_frame(frame, frame, sq).1;
            king[f][sq as usize] = (king_bucket(in_frame, nbuckets) * KING_FEATURE_STRIDE) as i64;
        }
    }
    let json = serde_json::json!({
        "king_buckets": nbuckets,
        "stride": KING_FEATURE_STRIDE,
        "rows": nbuckets * KING_FEATURE_STRIDE,
        "max_hand": MAX_HAND_COUNT,
        "piece": piece,
        "hand": hand,
        "king": king,
        "material_psqt": NNUEWeights::material_psqt(nbuckets * KING_FEATURE_STRIDE),
    });
    serde_json::to_writer(BufWriter::new(File::create(out)?), &json)?;
    Ok(())
}

fn check(corpus: &Path, n: usize, nbuckets: usize, weights: Option<&Path>) -> std::io::Result<()> {
    let mut ev = match weights {
        Some(p) => Some(NNUEEvaluator::load(p).map_err(|e| std::io::Error::other(format!("{e:?}")))?),
        None => None,
    };
    let mut out = BufWriter::new(std::io::stdout().lock());
    let mut done = 0;
    for line in BufReader::new(File::open(corpus)?).lines() {
        if done >= n {
            break;
        }
        let line = line?;
        let Some(rec) = parse(&line) else { continue };
        if rec.eval_cp.is_none() {
            continue;
        }
        let Ok((board, _side, captured)) = BitboardBoard::from_fen(&rec.sfen) else { continue };
        let packed = encode(&board, &captured, &rec);
        let stm = extract_features_frame(&board, rec.player, &captured, true, nbuckets);
        let nstm = extract_features_frame(&board, rec.player.opposite(), &captured, true, nbuckets);
        let eval = ev.as_mut().map(|e| {
            e.invalidate();
            e.evaluate_for(&board, rec.player, &captured)
        });
        let hex: String = packed.iter().map(|b| format!("{b:02x}")).collect();
        let row = serde_json::json!({ "rec": hex, "stm": stm, "nstm": nstm, "eval": eval });
        writeln!(out, "{row}")?;
        done += 1;
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    match Cli::parse().cmd {
        Cmd::Pack { corpus, out_dir, buckets, seed, half, quiet } => {
            pack(&corpus, &out_dir, buckets, seed, Filter { half, quiet })
        }
        Cmd::Tables { king_buckets, out } => tables(king_buckets, &out),
        Cmd::PackPsv { inputs, out_dir, buckets, seed, quiet } => {
            pack_psv(&inputs, &out_dir, buckets, seed, quiet)
        }
        Cmd::PsvCheck { input, n, jsonl } => psv_check(&input, n, jsonl.as_deref()),
        Cmd::Check { corpus, n, king_buckets, weights } => {
            check(&corpus, n, king_buckets, weights.as_deref())
        }
    }
}
