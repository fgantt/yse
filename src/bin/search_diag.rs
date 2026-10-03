//! Why is our search tree so wide?
//!
//! At the same node count we reach roughly half YaneuraOu's depth, which puts
//! our effective branching factor near 6 against their ~2. Depth goes as
//! `log(nodes) / log(EBF)`, so that gap is worth several plies — far more than
//! the 28x nps gap, which at EBF 6 buys about 0.4 of a ply per doubling.
//!
//! This reports the three things that decide the tree's width, so the fix is
//! chosen rather than guessed:
//!
//!   * **cutoff index** — where in the move list beta cutoffs happen. A
//!     well-ordered search cuts on the first move it tries ~90% of the time.
//!     Every cutoff further down means the moves before it were searched for
//!     nothing. `average_cutoff_index` has sat in the metrics struct hardcoded
//!     to 0.0 ("would need calculation") for a long time, so this number has
//!     never been looked at.
//!   * **quiescence share** — how much of the tree is quiescence rather than
//!     the main search.
//!   * **per-ply node ratio** — the effective branching factor itself, measured
//!     by searching the same position to successive depths.

use clap::Parser;
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::search::search_engine::{
    IterativeDeepening, SearchEngine, DIAG_CUT_INDEX, DIAG_INTERIOR_NODES, DIAG_PVS_RESEARCHES,
    DIAG_PVS_SCOUTS, DIAG_QUIESCENCE_NODES,
};
use shogi_engine::search::statistics::{DIAG_LMR_REJECT, GLOBAL_SEARCH_NODES};
use std::sync::atomic::Ordering;

#[derive(Parser, Debug)]
#[command(name = "search-diag")]
#[command(about = "Report what makes the search tree wide")]
struct Cli {
    /// Positions to measure, as SFEN. Defaults to the start position plus a
    /// middlegame with pieces in hand, where branching is widest.
    #[arg(long)]
    sfen: Vec<String>,

    /// Deepest iteration to run.
    #[arg(long, default_value_t = 6)]
    max_depth: u8,

    /// NNUE weights. The search prunes on evaluation, so measuring tree shape
    /// under the classical evaluator would describe an engine we do not ship.
    #[arg(long, default_value = "nnue/default.bin")]
    nnue: String,
}

const DEFAULT_SFENS: &[(&str, &str)] = &[
    ("startpos", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
    // From the 8-piece handicap game: many pieces in hand, so the drop moves
    // make the move list very long. This is the regime that decides games.
    (
        "midgame",
        "l5snl/4g1k2/2n1sp1p1/p1ppp1pPp/5P3/PPPPP1N1P/2GS5/1KGG5/LN1R4L b RBGSPp 1",
    ),
];

fn reset() {
    for c in DIAG_CUT_INDEX.iter() {
        c.store(0, Ordering::Relaxed);
    }
    DIAG_INTERIOR_NODES.store(0, Ordering::Relaxed);
    DIAG_QUIESCENCE_NODES.store(0, Ordering::Relaxed);
    DIAG_PVS_SCOUTS.store(0, Ordering::Relaxed);
    DIAG_PVS_RESEARCHES.store(0, Ordering::Relaxed);
    for c in DIAG_LMR_REJECT.iter() {
        c.store(0, Ordering::Relaxed);
    }
}

fn main() {
    let cli = Cli::parse();
    shogi_engine::search::search_engine::enable_search_diagnostics();
    let positions: Vec<(String, String)> = if cli.sfen.is_empty() {
        DEFAULT_SFENS.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect()
    } else {
        cli.sfen.iter().enumerate().map(|(i, s)| (format!("sfen{i}"), s.clone())).collect()
    };

    for (name, sfen) in &positions {
        let Ok((board, player, captured)) = BitboardBoard::from_fen(sfen) else {
            eprintln!("{name}: could not parse sfen");
            continue;
        };
        println!("\n=== {name} ===");
        println!("{:>5}  {:>12}  {:>7}  {:>9}", "depth", "nodes", "ratio", "q-share");
        let mut prev_nodes = 0u64;
        for depth in 1..=cli.max_depth {
            reset();
            let mut engine = SearchEngine::new(None, 64);
            let ev = engine.get_evaluator_mut();
            ev.enable_nnue_with_weights(&cli.nnue).expect("load nnue weights");
            assert!(
                ev.nnue_declared_feature_set().is_some(),
                "{} declares no feature set, so the engine would fall back to classical \
                 evaluation and this diagnostic would describe the wrong engine",
                cli.nnue
            );
            let mut id = IterativeDeepening::new(depth, 600_000, None);
            let _ = id.search(&mut engine, &board, &captured, player);
            let nm = engine.get_null_move_stats().clone();
            let nodes = GLOBAL_SEARCH_NODES.load(Ordering::Relaxed);
            let q = DIAG_QUIESCENCE_NODES.load(Ordering::Relaxed);
            let interior = DIAG_INTERIOR_NODES.load(Ordering::Relaxed);
            let ratio = if prev_nodes > 0 { nodes as f64 / prev_nodes as f64 } else { 0.0 };
            let qshare =
                if q + interior > 0 { 100.0 * q as f64 / (q + interior) as f64 } else { 0.0 };
            let scouts = DIAG_PVS_SCOUTS.load(Ordering::Relaxed);
            let researches = DIAG_PVS_RESEARCHES.load(Ordering::Relaxed);
            let rr = if scouts > 0 { 100.0 * researches as f64 / scouts as f64 } else { 0.0 };
            println!(
                "{depth:>5}  {nodes:>12}  {ratio:>7.2}  {qshare:>8.0}%  scouts {scouts:>8}  \
                 re-search {rr:>5.1}%"
            );
            prev_nodes = nodes;

            if depth == cli.max_depth {
                println!(
                    "\n  null move: attempts {}  tried {}  cutoffs {}  \
                     disabled-in-check {}  skipped-time-pressure {}",
                    nm.attempts, nm.null_moves_tried, nm.cutoffs_after_null_move,
                    nm.disabled_in_check, nm.skipped_time_pressure
                );
            }
            if depth == cli.max_depth {
                let cuts: Vec<u64> =
                    DIAG_CUT_INDEX.iter().map(|c| c.load(Ordering::Relaxed)).collect();
                let total: u64 = cuts.iter().sum();
                let lmr: Vec<u64> =
                    DIAG_LMR_REJECT.iter().map(|c| c.load(Ordering::Relaxed)).collect();
                let lmr_total: u64 = lmr.iter().sum();
                if lmr_total > 0 {
                    println!("\n  LMR decisions (depth {depth}):");
                    let labels = [
                        "APPLIED", "move# too low", "depth too low", "in check", "capture",
                        "promotion", "killer", "tt move", "escape",
                    ];
                    for (label, n) in labels.iter().zip(lmr.iter()) {
                        let pct = 100.0 * *n as f64 / lmr_total as f64;
                        println!("    {label:>14} {pct:>5.1}%  {n}");
                    }
                }
                if total > 0 {
                    println!("\n  beta cutoffs by move index (depth {depth}):");
                    let labels = ["1st", "2nd", "3rd", "4th", "5-8th", "9-16th", "17-32nd", "33+"];
                    for (label, n) in labels.iter().zip(cuts.iter()) {
                        let pct = 100.0 * *n as f64 / total as f64;
                        let bar = "#".repeat((pct / 2.0).round() as usize);
                        println!("    {label:>8} {pct:>5.1}%  {bar}");
                    }
                    println!(
                        "    first-move cutoff rate: {:.1}%  (a well-ordered search is ~90%)",
                        100.0 * cuts[0] as f64 / total as f64
                    );
                }
            }
        }
    }
}
