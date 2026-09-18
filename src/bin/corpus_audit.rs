//! Audit a training corpus for positions that do not follow from each other.
//!
//! Each game in the corpus is a chain of positions one ply apart. For every
//! consecutive pair this checks that some legal move (by our move generator,
//! which matches the published perft counts) turns the first into the second.
//! A break means the recorded position is not the one the teacher was playing,
//! so its label belongs to a different position.

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::moves::MoveGenerator;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", key);
    let i = line.find(&needle)? + needle.len();
    let rest = line[i..].trim_start();
    let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('"'))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "nnue_corpus_yaneura_d10.jsonl".into());
    let mut games: BTreeMap<u64, Vec<(u32, String)>> = BTreeMap::new();
    for line in BufReader::new(std::fs::File::open(&path)?).lines() {
        let line = line?;
        let (Some(g), Some(p)) = (field(&line, "game_id"), field(&line, "ply")) else { continue };
        let Some(i) = line.find("\"sfen\":\"") else { continue };
        let rest = &line[i + 8..];
        let sfen = rest[..rest.find('"').unwrap_or(rest.len())].to_string();
        games.entry(g.parse()?).or_default().push((p.parse()?, sfen));
    }
    let mg = MoveGenerator::new();
    let (mut pairs, mut breaks, mut bad_games) = (0usize, 0usize, 0usize);
    let mut first_breaks: Vec<String> = Vec::new();
    for (gid, recs) in &games {
        let mut game_bad = false;
        for w in recs.windows(2) {
            let ((p0, s0), (p1, s1)) = (&w[0], &w[1]);
            if p1 != &(p0 + 1) {
                continue;
            }
            pairs += 1;
            let (board, stm, captured) = BitboardBoard::from_fen(s0)?;
            let target = s1.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            let found = mg.generate_legal_moves(&board, stm, &captured).iter().any(|mv| {
                let mut b = board.clone();
                let mut c = captured.clone();
                b.make_move_with_hand(mv, &mut c);
                let f = b.to_fen(stm.opposite(), &c);
                f.split_whitespace().take(3).collect::<Vec<_>>().join(" ") == target
            });
            if !found {
                breaks += 1;
                game_bad = true;
                if first_breaks.len() < 10 {
                    first_breaks.push(format!("game {gid} ply {p0}->{p1}: {s0}  =>  {s1}"));
                }
            }
        }
        bad_games += game_bad as usize;
    }
    println!("games {}  consecutive pairs {}  breaks {}  games with a break {}", games.len(), pairs, breaks, bad_games);
    for b in first_breaks {
        println!("  {b}");
    }
    Ok(())
}
