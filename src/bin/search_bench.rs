//! Fixed-depth search benchmark and behavioural fingerprint.
//!
//! Runs the same `SearchEngine` + `IterativeDeepening` path that `elo-tester`
//! uses, at a fixed depth with an effectively unlimited time budget, over a
//! deterministic set of corpus positions. For each position it prints the best
//! move, score, and node count, then a nodes/second summary.
//!
//! The per-position lines are a fingerprint: a pure performance change must
//! leave them byte-identical. Diff `--fingerprint` output before and after.

use clap::Parser;
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::moves::MoveGenerator;
use shogi_engine::search::search_engine::SearchEngine;
use shogi_engine::search::IterativeDeepening;
use shogi_engine::BitboardBoard;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "search-bench")]
#[command(about = "Fixed-depth search benchmark with a diffable behavioural fingerprint")]
struct Cli {
    /// JSONL corpus to draw positions from (only the `sfen` field is used).
    #[arg(long, default_value = "nnue_corpus_yaneura_d10.jsonl")]
    corpus: PathBuf,

    /// Number of positions to search.
    #[arg(long, default_value_t = 30)]
    positions: usize,

    /// Take every Nth corpus record, to spread positions across many games.
    #[arg(long, default_value_t = 2011)]
    stride: usize,

    /// Fixed search depth.
    #[arg(long, default_value_t = 3)]
    depth: u8,

    /// Transposition table size in MB (fresh table per position).
    #[arg(long, default_value_t = 32)]
    tt_mb: usize,

    /// Optional NNUE weights; PST evaluation when omitted.
    #[arg(long)]
    nnue_weights: Option<PathBuf>,

    #[arg(long)]
    use_pov_features: bool,

    #[arg(long)]
    use_hand_features: bool,

    #[arg(long)]
    use_stm_feature: bool,

    /// Write the per-position fingerprint (move, score, nodes) here.
    #[arg(long)]
    fingerprint: Option<PathBuf>,
    /// Search this one position instead of drawing from the corpus.
    #[arg(long)]
    sfen: Option<String>,
}

fn load_sfens(cli: &Cli) -> std::io::Result<Vec<String>> {
    if let Some(s) = &cli.sfen {
        return Ok(vec![s.clone()]);
    }
    let f = BufReader::new(File::open(&cli.corpus)?);
    let mut out = Vec::new();
    for (i, line) in f.lines().enumerate() {
        if out.len() >= cli.positions {
            break;
        }
        if i % cli.stride != 0 {
            continue;
        }
        let line = line?;
        let key = "\"sfen\":\"";
        if let Some(start) = line.find(key) {
            let rest = &line[start + key.len()..];
            if let Some(end) = rest.find('"') {
                out.push(rest[..end].to_string());
            }
        }
    }
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let sfens = load_sfens(&cli)?;
    eprintln!(
        "search-bench: {} positions, depth {}, {}",
        sfens.len(),
        cli.depth,
        match &cli.nnue_weights {
            Some(p) => format!("NNUE {}", p.display()),
            None => "PST".to_string(),
        }
    );

    let mut fp_lines = Vec::new();
    let mut total_nodes: u64 = 0;
    let mut total_ms: f64 = 0.0;

    for (idx, sfen) in sfens.iter().enumerate() {
        let (board, player, captured) = match BitboardBoard::from_fen(sfen) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("skip {}: {}", idx, e);
                continue;
            }
        };

        // The search overrides the caller's time budget for in-check positions with
        // few replies (2 s for <=5, 5 s for <=10), so those results depend on machine
        // speed even at "fixed" depth. Flag them so fingerprint diffs can be read.
        let in_check = board.is_king_in_check(player, &captured);
        let n_legal = MoveGenerator::new().generate_legal_moves(&board, player, &captured).len();
        let time_bound = in_check && n_legal <= 10;

        let mut engine = SearchEngine::new(None, cli.tt_mb);
        if let Some(w) = &cli.nnue_weights {
            let eval: &mut PositionEvaluator = engine.get_evaluator_mut();
            eval.enable_nnue_with_weights(w)?;
            if cli.use_stm_feature {
                eval.nnue_set_use_stm_feature(true);
            }
            if cli.use_pov_features {
                eval.nnue_set_use_pov(true);
            }
            if cli.use_hand_features {
                eval.nnue_set_use_hand_features(true);
            }
        }

        // A very large budget makes the search depth-bound, so the result depends
        // only on the position and the code, not on machine speed or load.
        let mut id = IterativeDeepening::new(cli.depth, 600_000, None);
        let t0 = Instant::now();
        let best = id.search(&mut engine, &board, &captured, player);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let nodes = engine.get_nodes_searched();

        let (mv, score) = match best {
            Some((m, s)) => (m.to_usi_string(), s),
            None => ("none".to_string(), 0),
        };
        let tag = if time_bound { " TIME-BOUND(check)" } else { "" };
        println!(
            "{:>3} {:?} best={:<8} score={:>7} nodes={:>9} ms={:>9.1} legal={:>3}{}",
            idx, player, mv, score, nodes, ms, n_legal, tag
        );
        {
            let q = engine.get_quiescence_stats();
            let nm = engine.get_null_move_stats();
            let lmr = engine.get_lmr_stats();
            println!(
                "      qnodes={} null: tried={} cut={} | lmr: considered={} reduced={} researched={}",
                q.nodes_searched,
                nm.attempts,
                nm.cutoffs,
                lmr.moves_considered,
                lmr.reductions_applied,
                lmr.researches_triggered
            );
            let iid = engine.get_iid_stats();
            println!(
                "      iid: searches={} nodes={}",
                iid.iid_searches_performed, iid.total_iid_nodes
            );
            let mo = &lmr.move_ordering_stats;
            println!(
                "      cutoffs={} first-move={:.1}% mean-index={:.2}",
                mo.total_cutoffs,
                100.0 * mo.first_move_cutoffs as f64 / mo.total_cutoffs.max(1) as f64,
                mo.cutoff_index_sum as f64 / mo.cutoff_index_count.max(1) as f64
            );
        }
        fp_lines.push(format!("{} {} {} {}{}", idx, mv, score, nodes, tag));
        total_nodes += nodes;
        total_ms += ms;
    }

    println!(
        "TOTAL positions={} nodes={} ms={:.0} nps={:.0}",
        fp_lines.len(),
        total_nodes,
        total_ms,
        total_nodes as f64 / (total_ms / 1000.0).max(1e-9)
    );

    if let Some(p) = &cli.fingerprint {
        let mut f = File::create(p)?;
        for l in &fp_lines {
            writeln!(f, "{}", l)?;
        }
    }
    Ok(())
}
