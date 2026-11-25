//! Check a standard-format network (`nn.bin`) as this engine loads it (`nnue_std`: converted to
//! our incremental accumulator) against the diagnostic from-scratch evaluation of the same file
//! (`yo_nnue`, the format's arithmetic written out directly).
//!
//!     cargo run --release --features yo-diagnostic --bin std_nnue_check -- --net nn.bin \
//!         --corpus nnue_corpus_yaneura_d10.jsonl [--positions 2000] [--search 20 --depth 4]
//!
//! 1. Static: on `--positions` corpus positions, with each side to move, both scores must be equal.
//! 2. Incremental: a fixed-depth search on `--search` positions with verify mode on, so every
//!    evaluated node's incremental score is compared with a from-scratch one.
//! Exits non-zero on any mismatch.

#[cfg(feature = "yo-diagnostic")]
fn main() {
    use clap::Parser;
    use shogi_engine::bitboards::BitboardBoard;
    use shogi_engine::evaluation::nnue::NNUEEvaluator;
    use shogi_engine::evaluation::yo_nnue::YoNnue;
    use shogi_engine::search::search_engine::SearchEngine;
    use shogi_engine::search::IterativeDeepening;
    use std::io::BufRead;

    #[derive(Parser)]
    struct Args {
        #[arg(long)]
        net: String,
        #[arg(long)]
        corpus: std::path::PathBuf,
        #[arg(long, default_value_t = 2000)]
        positions: usize,
        #[arg(long, default_value_t = 20)]
        search: usize,
        #[arg(long, default_value_t = 4)]
        depth: u8,
    }
    let a = Args::parse();
    let t = std::time::Instant::now();
    let mut ours = NNUEEvaluator::load(&a.net).expect("load through nnue_std");
    println!("loaded (ours) in {:.1} s, feature set {:?}", t.elapsed().as_secs_f64(), ours.declared_feature_set());
    let reference = YoNnue::load(&a.net).expect("load through yo_nnue");

    let file = std::fs::File::open(&a.corpus).expect("corpus");
    let mut sfens = Vec::new();
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let v: serde_json::Value = serde_json::from_str(&line).expect("json");
        if let Some(s) = v.get("sfen").and_then(|s| s.as_str()) {
            sfens.push(s.to_string());
        }
    }
    let step = (sfens.len() / a.positions.max(1)).max(1);
    let picked: Vec<&String> = sfens.iter().step_by(step).take(a.positions).collect();

    let (mut n, mut bad, mut sum_abs) = (0u64, 0u64, 0i64);
    for sfen in &picked {
        let Ok((b, stm, c)) = BitboardBoard::from_fen(sfen) else { continue };
        for side in [stm, stm.opposite()] {
            let x = ours.evaluate_for(&b, side, &c);
            let y = reference.evaluate(&b, side, &c);
            n += 1;
            sum_abs += i64::from(y.abs());
            if x != y {
                bad += 1;
                if bad <= 5 {
                    println!("MISMATCH {sfen} side {side:?}: ours {x} reference {y}");
                }
            }
        }
    }
    println!("static: {n} scores, {bad} mismatches, mean |score| {:.0}", sum_abs as f64 / n.max(1) as f64);

    let (mut checks, mut mism) = (0u64, 0u64);
    for sfen in picked.iter().take(a.search) {
        let Ok((b, stm, c)) = BitboardBoard::from_fen(sfen) else { continue };
        let mut engine = SearchEngine::new(None, 16);
        let ev = engine.get_evaluator_mut();
        ev.enable_nnue_with_weights(&a.net).expect("load");
        ev.nnue_set_verify(true);
        let mut id = IterativeDeepening::new(a.depth, 600_000, None);
        let _ = id.search(&mut engine, &b, &c, stm);
        let (k, m, first) = engine.get_evaluator_mut().nnue_verify_stats().unwrap();
        checks += k;
        mism += m;
        if m > 0 {
            println!("INCREMENTAL MISMATCH {sfen}: {m}/{k}; first: {}", first.unwrap_or_default());
        }
    }
    println!("incremental: {checks} checked nodes, {mism} mismatches");
    if bad > 0 || mism > 0 || n == 0 || checks == 0 {
        std::process::exit(1);
    }
    println!("OK");
}

#[cfg(not(feature = "yo-diagnostic"))]
fn main() {
    eprintln!("std_nnue_check needs --features yo-diagnostic (it compares against yo_nnue)");
    std::process::exit(2);
}
