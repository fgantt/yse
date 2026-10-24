//! Audit a YaneuraOu-format book against our move generator (ignored: needs a
//! book file). Every position must parse, our canonical key must equal the
//! file's key (or lookups would miss), and every book move must be legal.
//!
//!     YSE_BOOK=book/user_book1.db cargo test --release --test book_audit -- --ignored --nocapture

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::book::{position_key, Book};
use shogi_engine::moves::MoveGenerator;
use std::io::{BufRead, BufReader};

#[test]
#[ignore]
fn audit_book_file() {
    let Ok(path) = std::env::var("YSE_BOOK") else {
        eprintln!("YSE_BOOK not set; skipping");
        return;
    };
    let book = Book::open(std::path::Path::new(&path)).expect("open book");
    let reader = BufReader::new(std::fs::File::open(&path).unwrap());
    let gen = MoveGenerator::new();
    let (mut positions, mut moves, mut illegal, mut bad_sfen, mut key_mismatch) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut legal: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut sampled = 0u64;
    for line in reader.lines() {
        let line = line.unwrap();
        if let Some(sfen) = line.strip_prefix("sfen ") {
            positions += 1;
            current = None;
            legal.clear();
            match BitboardBoard::from_fen(sfen) {
                Ok((b, p, c)) => {
                    let key = position_key(&b, p, &c, false);
                    let file_key = sfen.rsplit_once(' ').map_or(sfen, |(h, _)| h);
                    if key != file_key {
                        key_mismatch += 1;
                        if key_mismatch <= 5 {
                            eprintln!("key mismatch:\n  file {}\n  ours {}", file_key, key);
                        }
                    }
                    legal = gen.generate_legal_moves(&b, p, &c).iter().map(|m| m.to_usi_string()).collect();
                    if positions % 20_000 == 0 {
                        assert!(!book.lookup(&key).unwrap().is_empty(), "lookup missed {}", key);
                        sampled += 1;
                    }
                    current = Some(key);
                }
                Err(_) => bad_sfen += 1,
            }
        } else if current.is_some() && !line.starts_with('#') {
            if let Some(mv) = line.split_whitespace().next() {
                moves += 1;
                if !legal.iter().any(|l| l == mv) {
                    illegal += 1;
                    if illegal <= 5 {
                        eprintln!("illegal {} in {}", mv, current.as_ref().unwrap());
                    }
                }
            }
        }
    }
    eprintln!(
        "positions {} moves {} illegal {} bad_sfen {} key_mismatch {} sampled_lookups {}",
        positions, moves, illegal, bad_sfen, key_mismatch, sampled
    );
    assert_eq!(bad_sfen, 0);
    assert_eq!(key_mismatch, 0);
    assert_eq!(illegal, 0);
}
