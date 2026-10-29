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
    },
    /// Write the feature lookup tables as JSON.
    Tables {
        #[arg(long, default_value_t = 9)]
        king_buckets: usize,
        #[arg(long)]
        out: PathBuf,
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

/// Parse and encode one line: (is held-out, record), or `None` to skip it.
fn pack_line(line: &str) -> Option<(bool, [u8; RECORD])> {
    let rec = parse(line)?;
    rec.eval_cp?;
    let (board, _side, captured) = BitboardBoard::from_fen(&rec.sfen).ok()?;
    Some((rec.game_id % VAL_STRIDE == 0, encode(&board, &captured, &rec)))
}

fn pack(corpus: &Path, out_dir: &Path, nbuckets: usize, seed: u64) -> std::io::Result<()> {
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
    loop {
        let chunk: Vec<String> = lines.by_ref().take(1 << 20).collect::<Result<_, _>>()?;
        if chunk.is_empty() {
            break;
        }
        let packed: Vec<Option<(bool, [u8; RECORD])>> =
            chunk.par_iter().map(|l| pack_line(l)).collect();
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
        Cmd::Pack { corpus, out_dir, buckets, seed } => pack(&corpus, &out_dir, buckets, seed),
        Cmd::Tables { king_buckets, out } => tables(king_buckets, &out),
        Cmd::Check { corpus, n, king_buckets, weights } => {
            check(&corpus, n, king_buckets, weights.as_deref())
        }
    }
}
