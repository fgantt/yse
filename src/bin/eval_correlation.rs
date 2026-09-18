//! How well does an evaluator track the teacher?
//!
//! Scores the classical evaluator and/or an NNUE network against the corpus's
//! teacher evaluations, on the same held-out games the offline trainer validates
//! on (every 20th game), with the trainer's metrics: Pearson against the clamped
//! teacher score, and Spearman against the raw score. Putting the classical
//! evaluator and a network on one axis shows how far the network has to go.

use clap::Parser;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::PositionEvaluator;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "eval-correlation")]
#[command(about = "Correlate evaluators with teacher evals on held-out corpus games")]
struct Cli {
    #[arg(long, default_value = "nnue_corpus_yaneura_d10.jsonl")]
    corpus: PathBuf,
    /// NNUE weight file (v2 files configure themselves). Omit for classical only.
    #[arg(long)]
    nnue_weights: Option<PathBuf>,
    /// Must match the trainer's `--eval-clamp`.
    #[arg(long, default_value_t = 2000)]
    eval_clamp: i32,
    /// Must match the trainer's `VALIDATION_GAME_STRIDE`.
    #[arg(long, default_value_t = 20)]
    game_stride: u64,
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", key);
    let i = line.find(&needle)? + needle.len();
    let rest = line[i..].trim_start();
    let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('"'))
}

fn pearson(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len() as f64;
    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
    let (mut num, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) {
        num += (x - mx) * (y - my);
        sx += (x - mx).powi(2);
        sy += (y - my).powi(2);
    }
    num / (sx * sy).sqrt()
}

fn ranks(xs: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..xs.len()).collect();
    idx.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap());
    let mut out = vec![0.0; xs.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && xs[idx[j + 1]] == xs[idx[i]] {
            j += 1;
        }
        for k in i..=j {
            out[idx[k]] = (i + j) as f64 / 2.0 + 1.0;
        }
        i = j + 1;
    }
    out
}

fn report(name: &str, preds: &[f64], raw: &[f64], clamped: &[f64]) {
    println!(
        "{:<44} pearson[clamp] {:+.3}   spearman[raw] {:+.3}   n={}",
        name,
        pearson(preds, clamped),
        pearson(&ranks(preds), &ranks(raw)),
        preds.len()
    );
    // Scale: Pearson is scale-invariant, but the search compares scores with
    // fixed centipawn margins, so an evaluator's scale relative to real
    // centipawns matters. Fitted on positions where the teacher is within
    // +/-1500 cp (outside that the teacher is clamped and the fit meaningless).
    let (mut xs, mut ys) = (vec![], vec![]);
    for (p, t) in preds.iter().zip(clamped) {
        if t.abs() < 1500.0 {
            xs.push(*t);
            ys.push(*p);
        }
    }
    let n = xs.len() as f64;
    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
    let cov: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum::<f64>() / n;
    let var: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum::<f64>() / n;
    let sd_pred = (ys.iter().map(|y| (y - my).powi(2)).sum::<f64>() / n).sqrt();
    let sd_teacher = var.sqrt();
    println!(
        "{:<44}   scale: slope {:.2} (eval cp per teacher cp), offset {:+.0}, sd eval {:.0} vs teacher {:.0}",
        "", cov / var, my - cov / var * mx, sd_pred, sd_teacher
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut classical = PositionEvaluator::new();
    let mut nnue = match &cli.nnue_weights {
        Some(p) => {
            let mut ev = PositionEvaluator::new();
            ev.enable_nnue_with_weights(p)?;
            match ev.nnue_declared_feature_set() {
                Some(fs) => eprintln!("network {} declares features {}", p.display(), fs),
                None => eprintln!("warning: {} declares no feature set (legacy file)", p.display()),
            }
            Some(ev)
        }
        None => None,
    };

    let (mut raw, mut clamped, mut pc, mut pn) = (vec![], vec![], vec![], vec![]);
    for line in BufReader::new(std::fs::File::open(&cli.corpus)?).lines() {
        let line = line?;
        let gid: u64 = match field(&line, "game_id").and_then(|g| g.parse().ok()) {
            Some(g) => g,
            None => continue,
        };
        if gid % cli.game_stride != 0 {
            continue;
        }
        let cp: i32 = match field(&line, "eval_cp").and_then(|e| e.parse().ok()) {
            Some(c) => c,
            None => continue,
        };
        let sfen = match line.find("\"sfen\":\"") {
            Some(i) => {
                let rest = &line[i + 8..];
                &rest[..rest.find('"').unwrap_or(rest.len())]
            }
            None => continue,
        };
        let (board, stm, captured) = match BitboardBoard::from_fen(sfen) {
            Ok(x) => x,
            Err(_) => continue,
        };
        raw.push(cp as f64);
        clamped.push(cp.clamp(-cli.eval_clamp, cli.eval_clamp) as f64);
        pc.push(classical.evaluate(&board, stm, &captured) as f64);
        if let Some(ev) = nnue.as_mut() {
            pn.push(ev.evaluate(&board, stm, &captured) as f64);
        }
    }
    println!("held-out games: every {}th; clamp ±{}", cli.game_stride, cli.eval_clamp);
    material_sensitivity(&cli, &mut classical, nnue.as_mut())?;
    report("classical (tapered) evaluator", &pc, &raw, &clamped);
    if let Some(p) = &cli.nnue_weights {
        report(&format!("nnue {}", p.display()), &pn, &raw, &clamped);
    }
    Ok(())
}

/// Remove one of the opponent's pieces from each held-out position and measure
/// how much the side to move's evaluation rises. A sound evaluator gains roughly
/// the piece's value. The corpus comes from strong-engine games where material
/// rarely hangs, but the search spends most of its time in positions after
/// captures, so an evaluator that has not learned material values will throw
/// pieces away in play even if it ranks quiet game positions well.
fn material_sensitivity(
    cli: &Cli,
    classical: &mut PositionEvaluator,
    mut nnue: Option<&mut PositionEvaluator>,
) -> Result<(), Box<dyn std::error::Error>> {
    use shogi_engine::types::core::{PieceType, Position};
    use std::collections::BTreeMap;
    let mut gains: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    let mut seen = 0;
    for line in BufReader::new(std::fs::File::open(&cli.corpus)?).lines() {
        let line = line?;
        let gid: u64 = match field(&line, "game_id").and_then(|g| g.parse().ok()) {
            Some(g) => g,
            None => continue,
        };
        if gid % cli.game_stride != 0 || field(&line, "eval_cp").map_or(true, |e| e == "null") {
            continue;
        }
        seen += 1;
        if seen % 3 != 0 {
            continue; // thin the sample; removal per square is the expensive part
        }
        let sfen = match line.find("\"sfen\":\"") {
            Some(i) => {
                let rest = &line[i + 8..];
                rest[..rest.find('"').unwrap_or(rest.len())].to_string()
            }
            None => continue,
        };
        let (board, stm, captured) = match BitboardBoard::from_fen(&sfen) {
            Ok(x) => x,
            Err(_) => continue,
        };
        let base_c = classical.evaluate(&board, stm, &captured) as f64;
        let base_n = nnue.as_mut().map(|e| e.evaluate(&board, stm, &captured) as f64);
        for sq in 0..81u8 {
            let pos = Position::from_u8(sq);
            let piece = match board.get_piece(pos) {
                Some(p) if p.player != stm && p.piece_type != PieceType::King => p,
                _ => continue,
            };
            let mut b = board.clone();
            b.remove_piece(pos);
            if b.is_king_in_check(stm.opposite(), &captured) && false {
                continue;
            }
            let entry = gains.entry(format!("{:?}", piece.piece_type)).or_default();
            entry.0.push(classical.evaluate(&b, stm, &captured) as f64 - base_c);
            if let (Some(e), Some(bn)) = (nnue.as_mut(), base_n) {
                entry.1.push(e.evaluate(&b, stm, &captured) as f64 - bn);
            }
        }
    }
    let median = |v: &Vec<f64>| -> f64 {
        if v.is_empty() {
            return f64::NAN;
        }
        let mut w = v.clone();
        w.sort_by(|a, b| a.partial_cmp(b).unwrap());
        w[w.len() / 2]
    };
    println!("\nmaterial sensitivity: median eval gain (cp) when one opponent piece is removed");
    println!("{:<16} {:>7} {:>10} {:>10}", "piece", "n", "classical", "nnue");
    for (pt, (c, n)) in &gains {
        println!("{:<16} {:>7} {:>10.0} {:>10.0}", pt, c.len(), median(c), median(n));
    }
    println!();
    Ok(())
}
