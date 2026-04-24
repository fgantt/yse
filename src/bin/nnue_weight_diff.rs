//! Compare two NNUE weight files and report:
//!   * per-layer L1 distance, max delta, and count of changed weights
//!   * min/max/mean of each file's weights (range check)
//!   * NNUE evaluations of the starting position with each set of weights
//!
//! Use: `nnue-weight-diff <baseline.json> <candidate.json>`

use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{NNUEAccumulator, NNUEWeights};
use std::env;

fn stats_i16(name: &str, a: &[i16], b: &[i16]) {
    let mut total = 0_i64;
    let mut max: i32 = 0;
    let mut changed = 0_usize;
    for (&x, &y) in a.iter().zip(b.iter()) {
        let d = (y as i32 - x as i32).abs();
        if d != 0 {
            changed += 1;
            total += d as i64;
            if d > max {
                max = d;
            }
        }
    }
    let min_a = a.iter().copied().min().unwrap_or(0);
    let max_a = a.iter().copied().max().unwrap_or(0);
    let min_b = b.iter().copied().min().unwrap_or(0);
    let max_b = b.iter().copied().max().unwrap_or(0);
    println!(
        "  {:<24} n={:>7}  changed={:>7} ({:5.1}%)  L1={:>10}  max_delta={:>5}  A=[{:>6},{:>6}]  B=[{:>6},{:>6}]",
        name,
        a.len(),
        changed,
        100.0 * changed as f64 / a.len().max(1) as f64,
        total,
        max,
        min_a,
        max_a,
        min_b,
        max_b
    );
}

fn stats_i32(name: &str, a: &[i32], b: &[i32]) {
    let mut total = 0_i64;
    let mut max: i32 = 0;
    let mut changed = 0_usize;
    for (&x, &y) in a.iter().zip(b.iter()) {
        let d = (y - x).abs();
        if d != 0 {
            changed += 1;
            total += d as i64;
            if d > max {
                max = d;
            }
        }
    }
    let min_a = a.iter().copied().min().unwrap_or(0);
    let max_a = a.iter().copied().max().unwrap_or(0);
    let min_b = b.iter().copied().min().unwrap_or(0);
    let max_b = b.iter().copied().max().unwrap_or(0);
    println!(
        "  {:<24} n={:>7}  changed={:>7} ({:5.1}%)  L1={:>10}  max_delta={:>5}  A=[{:>6},{:>6}]  B=[{:>6},{:>6}]",
        name,
        a.len(),
        changed,
        100.0 * changed as f64 / a.len().max(1) as f64,
        total,
        max,
        min_a,
        max_a,
        min_b,
        max_b
    );
}

fn eval_fen(weights: &NNUEWeights, fen: &str) -> Option<i32> {
    let (board, _p, _cap) = BitboardBoard::from_fen(fen).ok()?;
    let (h1, h2) = weights.hidden_sizes();
    let mut acc = NNUEAccumulator::new(h1, h2);
    acc.refresh(&board, weights);
    Some(acc.evaluate(weights))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <baseline.json> <candidate.json>", args[0]);
        std::process::exit(2);
    }
    let a = NNUEWeights::load(&args[1])?;
    let b = NNUEWeights::load(&args[2])?;
    println!("A: {}", args[1]);
    println!("B: {}", args[2]);
    let (h1, h2) = a.hidden_sizes();
    println!("Arch: {}->{}->1", h1, h2);
    println!();

    println!("Per-layer deltas:");
    let flat_a: Vec<i16> = a.input_weights_1.iter().flatten().copied().collect();
    let flat_b: Vec<i16> = b.input_weights_1.iter().flatten().copied().collect();
    stats_i16("input_weights_1", &flat_a, &flat_b);
    stats_i32("hidden_biases_1", &a.hidden_biases_1, &b.hidden_biases_1);
    if let (Some(w2a), Some(w2b)) = (&a.input_weights_2, &b.input_weights_2) {
        let fa: Vec<i16> = w2a.iter().flatten().copied().collect();
        let fb: Vec<i16> = w2b.iter().flatten().copied().collect();
        stats_i16("input_weights_2", &fa, &fb);
    }
    if let (Some(b2a), Some(b2b)) = (&a.hidden_biases_2, &b.hidden_biases_2) {
        stats_i32("hidden_biases_2", b2a, b2b);
    }
    stats_i16("output_weights", &a.output_weights, &b.output_weights);
    stats_i32("output_bias", &[a.output_bias], &[b.output_bias]);

    println!();
    println!("Sample position evaluations (cp):");
    let fens = [
        ("startpos", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
        (
            "mid-game",
            "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
        ),
        (
            "black-winning",
            "4k4/9/4S4/9/9/9/9/9/4K4 b GGBB 1",
        ),
    ];
    for (label, fen) in fens.iter() {
        let ea = eval_fen(&a, fen).unwrap_or(i32::MIN);
        let eb = eval_fen(&b, fen).unwrap_or(i32::MIN);
        println!("  {:<16} A={:>6}  B={:>6}  delta={:>+6}", label, ea, eb, eb - ea);
    }

    Ok(())
}
