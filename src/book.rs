//! Opening book in the YaneuraOu text `.db` format.
//!
//! The format is a header line `#YANEURAOU-DB2016 1.00`, then for each position
//! a line `sfen <board> <side> <hand> <ply>` followed by one line per book move:
//! `<move> <ponder> <value> <depth> <count>` (USI moves, `none` for no ponder,
//! value in centipawns from the mover's view). Books written by the format's
//! tools are sorted by the sfen line, so the file is binary-searched on disk
//! and never loaded: a 2.3M-position book costs no memory and no start-up time.
//!
//! Keys ignore the ply. Hands are written in the canonical order R B G S N L P
//! (Black's in upper case, then White's), which is what makes the key unique.
//! A book may hold only Black-to-move positions; a White-to-move position is
//! then looked up rotated 180 degrees with colours swapped, and the book move
//! rotated back ("flipped book").
//!
//! Every move read from the book is matched against our legal moves before it
//! can be played, so a corrupt or mismatched book can only fail to hit.

use crate::bitboards::BitboardBoard;
use crate::moves::MoveGenerator;
use crate::types::core::{Move, Piece, PieceType, Player, Position};
use crate::types::CapturedPieces;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const HEADER: &str = "#YANEURAOU-DB2016";

/// Hand order of the canonical SFEN.
const HAND_ORDER: [PieceType; 7] = [
    PieceType::Rook,
    PieceType::Bishop,
    PieceType::Gold,
    PieceType::Silver,
    PieceType::Knight,
    PieceType::Lance,
    PieceType::Pawn,
];

/// One book move, as stored (USI string, not yet checked for legality).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookEntry {
    pub usi: String,
    /// Centipawns, from the side to move's point of view.
    pub value: i32,
    pub depth: i32,
    pub count: u64,
}

/// A book move resolved against the current position.
#[derive(Debug, Clone)]
pub struct BookHit {
    pub mv: Move,
    pub value: i32,
    /// Number of book moves within the eval margin it was chosen from.
    pub candidates: usize,
}

/// A sorted YaneuraOu-format book file, searched on disk.
#[derive(Debug, Clone)]
pub struct Book {
    path: PathBuf,
    len: u64,
}

impl Book {
    /// Open a book, checking only the header (the file is not read further).
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        let mut first = String::new();
        BufReader::new(file).read_line(&mut first).map_err(|e| e.to_string())?;
        if !first.starts_with(HEADER) {
            return Err(format!("{}: not a YaneuraOu book (no {} header)", path.display(), HEADER));
        }
        Ok(Self { path: path.to_path_buf(), len })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Book moves stored for `key` (a position key from [`position_key`]).
    pub fn lookup(&self, key: &str) -> std::io::Result<Vec<BookEntry>> {
        let mut reader = BufReader::with_capacity(4096, File::open(&self.path)?);
        let (mut lo, mut hi) = (0u64, self.len);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            // The first position line starting at or after `mid`.
            let Some((start, line_key)) = next_position_line(&mut reader, mid)? else {
                hi = mid;
                continue;
            };
            match line_key.as_str().cmp(key) {
                Ordering::Less => lo = start + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return read_moves(&mut reader),
            }
        }
        Ok(Vec::new())
    }

    /// Choose a book move for this position: among the legal book moves whose
    /// value is within `eval_diff` of the best, uniformly at random. `None` if
    /// the position (or its flip, for White to move) is not in the book.
    pub fn probe(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured: &CapturedPieces,
        eval_diff: i32,
        rng: &mut impl rand::Rng,
    ) -> Option<BookHit> {
        let mut entries = self.lookup(&position_key(board, player, captured, false)).ok()?;
        if entries.is_empty() && player == Player::White {
            entries = self.lookup(&position_key(board, player, captured, true)).ok()?;
            for e in &mut entries {
                e.usi = flip_usi_move(&e.usi);
            }
        }
        if entries.is_empty() {
            return None;
        }

        let legal = MoveGenerator::new().generate_legal_moves(board, player, captured);
        let mut playable: Vec<(Move, i32)> = entries
            .iter()
            .filter_map(|e| {
                legal.iter().find(|m| m.to_usi_string() == e.usi).map(|m| (m.clone(), e.value))
            })
            .collect();
        let best = playable.iter().map(|(_, v)| *v).max()?;
        playable.retain(|(_, v)| *v >= best - eval_diff.max(0));
        let candidates = playable.len();
        let (mv, value) = playable.swap_remove(rng.gen_range(0..candidates));
        Some(BookHit { mv, value, candidates })
    }
}

/// From byte `from`, skip to the next line start (unless at 0), then return the
/// offset and key of the first `sfen` line, leaving the reader just after it.
fn next_position_line(
    reader: &mut BufReader<File>,
    from: u64,
) -> std::io::Result<Option<(u64, String)>> {
    let mut pos = from;
    let mut line = String::new();
    if from > 0 {
        reader.seek(SeekFrom::Start(from - 1))?;
        pos = from - 1 + reader.read_line(&mut line)? as u64;
    } else {
        reader.seek(SeekFrom::Start(0))?;
    }
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        if let Some(rest) = line.strip_prefix("sfen ") {
            return Ok(Some((pos, strip_ply(rest.trim_end()).to_string())));
        }
        pos += n as u64;
    }
}

/// The move lines following a position line.
fn read_moves(reader: &mut BufReader<File>) -> std::io::Result<Vec<BookEntry>> {
    let mut entries = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.starts_with("sfen ") {
            return Ok(entries);
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut f = line.split_whitespace();
        let Some(usi) = f.next() else { continue };
        let _ponder = f.next();
        let num = |s: Option<&str>| s.and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        entries.push(BookEntry {
            usi: usi.to_string(),
            value: num(f.next()) as i32,
            depth: num(f.next()) as i32,
            count: num(f.next()).max(0) as u64,
        });
    }
}

/// `board side hand [ply]` without the ply.
fn strip_ply(sfen: &str) -> &str {
    match sfen.rsplit_once(' ') {
        Some((head, last)) if last.bytes().all(|b| b.is_ascii_digit()) && head.matches(' ').count() >= 2 => {
            head
        }
        _ => sfen,
    }
}

/// The book key of a position: canonical SFEN without the ply. With `flip`,
/// the key of the position rotated 180 degrees with colours swapped (so a
/// White-to-move position becomes Black-to-move).
pub fn position_key(
    board: &BitboardBoard,
    player: Player,
    captured: &CapturedPieces,
    flip: bool,
) -> String {
    let mut key = String::with_capacity(96);
    for r in 0..9u8 {
        let mut empty = 0;
        for c in 0..9u8 {
            let pos = if flip { Position::new(8 - r, 8 - c) } else { Position::new(r, c) };
            match board.get_piece(pos) {
                Some(p) => {
                    if empty > 0 {
                        key.push_str(&empty.to_string());
                        empty = 0;
                    }
                    let owner = if flip { p.player.opposite() } else { p.player };
                    key.push_str(&Piece::new(p.piece_type, owner).to_fen_char());
                }
                None => empty += 1,
            }
        }
        if empty > 0 {
            key.push_str(&empty.to_string());
        }
        if r < 8 {
            key.push('/');
        }
    }
    let side = if flip { player.opposite() } else { player };
    key.push_str(if side == Player::Black { " b " } else { " w " });

    let (first, second) =
        if flip { (&captured.white, &captured.black) } else { (&captured.black, &captured.white) };
    let mut hand = String::new();
    for (pieces, owner) in [(first, Player::Black), (second, Player::White)] {
        for pt in HAND_ORDER {
            let n = pieces.iter().filter(|&&p| p == pt).count();
            if n > 1 {
                hand.push_str(&n.to_string());
            }
            if n > 0 {
                hand.push_str(&Piece::new(pt, owner).to_fen_char());
            }
        }
    }
    key.push_str(if hand.is_empty() { "-" } else { &hand });
    key
}

/// Rotate a USI move by 180 degrees (`7g7f` -> `3c3d`, `P*5e` -> `P*5e`).
pub fn flip_usi_move(usi: &str) -> String {
    let flip_sq = |s: &[u8]| -> [u8; 2] { [b'0' + 10 - (s[0] - b'0'), b'a' + (b'i' - s[1])] };
    let b = usi.as_bytes();
    if b.len() < 4 {
        return usi.to_string();
    }
    let mut out = Vec::with_capacity(b.len());
    if b[1] == b'*' {
        out.extend_from_slice(&b[..2]);
        out.extend_from_slice(&flip_sq(&b[2..4]));
    } else {
        out.extend_from_slice(&flip_sq(&b[0..2]));
        out.extend_from_slice(&flip_sq(&b[2..4]));
    }
    out.extend_from_slice(&b[4..]);
    String::from_utf8(out).unwrap_or_else(|_| usi.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const START: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

    fn pos(sfen: &str) -> (BitboardBoard, Player, CapturedPieces) {
        BitboardBoard::from_fen(sfen).unwrap()
    }

    fn after(moves: &[&str]) -> (BitboardBoard, Player, CapturedPieces) {
        let (mut b, mut p, mut c) = pos(START);
        for m in moves {
            let mv = Move::from_usi_string(m, p, &b).unwrap();
            b.make_move_with_hand(&mv, &mut c);
            p = p.opposite();
        }
        (b, p, c)
    }

    fn write_book(dir: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("yse_book_test_{}_{}.db", dir, std::process::id()));
        let mut f = File::create(&path).unwrap();
        writeln!(f, "#YANEURAOU-DB2016 1.00").unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn key_of_start_position() {
        let (b, p, c) = pos(START);
        assert_eq!(
            position_key(&b, p, &c, false),
            "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -"
        );
    }

    #[test]
    fn hand_is_canonical_order() {
        // Captured order P then B for Black; the key writes B before P.
        let (b, p, c) = pos("lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/7R1/LNSGKGSNL b P2Bp 1");
        assert!(position_key(&b, p, &c, false).ends_with(" b 2BPp"));
    }

    #[test]
    fn flip_of_white_position_is_black_position() {
        // After 7g7f the flipped White-to-move position looks like Black having
        // played 3c3d... from Black's side: it is the start position with
        // White's 3c pawn on 3d, mirrored.
        let (b, p, c) = after(&["7g7f"]);
        assert_eq!(p, Player::White);
        assert_eq!(
            position_key(&b, p, &c, true),
            "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -"
        );
        assert_eq!(flip_usi_move("8c8d"), "2g2f");
        assert_eq!(flip_usi_move("2b8h+"), "8h2b+");
        assert_eq!(flip_usi_move("P*5e"), "P*5e");
        assert_eq!(flip_usi_move("B*1a"), "B*9i");
    }

    #[test]
    fn lookup_and_probe_on_sorted_file() {
        let start_key = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -";
        let flipped_after_7g7f = "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b -";
        let mut lines = vec![
            format!("sfen {} 0\n2g2f none 44 30 0\n7g7f none 44 30 0\n5i5h none -10 3 0\n9a9b none 99 1 0\n", start_key),
            // White's reply to 7g7f, stored flipped: 2g2f here is 8c8d for White.
            format!("sfen {} 0\n2g2f none 20 5 0\n", flipped_after_7g7f),
            "sfen 9/9/9/9/9/9/9/9/9 b - 0\nnone none 0 0 0\n".to_string(),
            "sfen zzz b - 0\n1a1b none 0 0 0\n".to_string(),
        ];
        lines.sort();
        let path = write_book("probe", &lines.concat());
        let book = Book::open(&path).unwrap();
        assert_eq!(book.lookup(start_key).unwrap().len(), 4);
        assert!(book.lookup("nope b -").unwrap().is_empty());
        assert!(book.lookup("zzz b -").unwrap().len() == 1);

        let mut rng = rand::rngs::mock::StepRng::new(0, 1);
        let (b, p, c) = pos(START);
        // 9a9b is illegal for Black and is dropped; 5i5h is outside the margin.
        let hit = book.probe(&b, p, &c, 0, &mut rng).unwrap();
        assert_eq!(hit.candidates, 2);
        assert!(["2g2f", "7g7f"].contains(&hit.mv.to_usi_string().as_str()));
        let hit = book.probe(&b, p, &c, 100, &mut rng).unwrap();
        assert_eq!(hit.candidates, 3);

        let (b, p, c) = after(&["7g7f"]);
        let hit = book.probe(&b, p, &c, 0, &mut rng).unwrap();
        assert_eq!(hit.mv.to_usi_string(), "8c8d");
        assert_eq!(hit.mv.player, Player::White);

        let (b, p, c) = after(&["2g2f"]);
        assert!(book.probe(&b, p, &c, 0, &mut rng).is_none());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn rejects_non_book_file() {
        let path = std::env::temp_dir().join(format!("yse_book_bad_{}.db", std::process::id()));
        std::fs::write(&path, "hello\n").unwrap();
        assert!(Book::open(&path).is_err());
        std::fs::remove_file(path).ok();
    }
}
