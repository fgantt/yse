//! NNUE Output-Layer Symmetriser (Session 10)
//!
//! Loads a set of pre-trained NNUE weights, zeroes `output_bias`, and
//! re-initialises `output_weights` from a zero-mean normal distribution so
//! the network can produce symmetric (positive *and* negative) predictions.
//!
//! Background: Session 9 (`docs/nnue-phase2/SESSION_LOG_009.md`) showed that
//! training from the Session-3 PST-imitation init left the output layer
//! biased — `output_bias ≈ 3400` plus `output_weights ∈ [-56, 3]`, so the
//! network could only deviate downward from a positive constant. With
//! `target_eval_scale = 2400` the achievable `prediction` range stops at
//! roughly [0.04, 0.22] while teacher targets fall in ±0.5, capping td_err
//! near 0.47. The hand-off plan for Session 10 was to break this by
//! re-initialising the output layer symmetrically; this binary is that step.
//!
//! Use:
//!     nnue-init-symmetric \
//!         --input  nnue_weights_trained.json \
//!         --output nnue_weights_s10_init.json \
//!         --sigma  20.0
//!
//! `sigma` is the std-dev (in i16 weight units) of the new
//! `output_weights`. Defaults to 20, which is roughly where Session-9
//! training landed (|w_max| ≈ 56 at 30 epochs) and keeps |raw_output|
//! values in the same order of magnitude as before symmetrisation —
//! we are *unblocking* the negative side, not retuning the magnitude.
//!
//! All non-output-layer weights are passed through unchanged.

use clap::Parser;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::evaluation::nnue::{NNUEAccumulator, NNUEWeights};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "nnue-init-symmetric")]
#[command(about = "Symmetrise the output layer of a trained NNUE weight file")]
struct Cli {
    /// Path to the input weights JSON file (e.g. nnue_weights_trained.json).
    #[arg(long)]
    input: PathBuf,

    /// Where to write the symmetrised weights.
    #[arg(long, default_value = "nnue_weights_s10_init.json")]
    output: PathBuf,

    /// Std-dev of the new `output_weights`, in i16 units. The samples are
    /// rounded to i16 and clamped to [-127, 127].
    #[arg(long, default_value_t = 20.0)]
    sigma: f64,

    /// New value for `output_bias`. Default 0 — pulls the network's average
    /// prediction back to zero so SGD can move it in either direction.
    #[arg(long, default_value_t = 0)]
    output_bias: i32,

    /// RNG seed for reproducibility (0 = system entropy).
    #[arg(long, default_value_t = 1)]
    seed: u64,
}

fn output_weight_stats(label: &str, w: &[i16]) {
    let n = w.len();
    if n == 0 {
        println!("  {:<12} (empty)", label);
        return;
    }
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    let mut sum: i64 = 0;
    let mut sum_abs: i64 = 0;
    for &x in w {
        let xi = x as i32;
        if xi < min {
            min = xi;
        }
        if xi > max {
            max = xi;
        }
        sum += xi as i64;
        sum_abs += xi.unsigned_abs() as i64;
    }
    let mean = sum as f64 / n as f64;
    let mean_abs = sum_abs as f64 / n as f64;
    println!(
        "  {:<12} n={:>3}  range=[{:>4},{:>4}]  mean={:>+7.2}  mean|w|={:>5.2}",
        label, n, min, max, mean, mean_abs
    );
}

fn eval_fen(weights: &NNUEWeights, fen: &str) -> Option<i32> {
    let (board, _p, _cap) = BitboardBoard::from_fen(fen).ok()?;
    let (h1, h2) = weights.hidden_sizes();
    let mut acc = NNUEAccumulator::new(h1, h2);
    acc.refresh(&board, weights);
    Some(acc.evaluate(weights))
}

fn print_sample_evals(label: &str, weights: &NNUEWeights) {
    let fens = [
        ("startpos", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
        (
            "mid-game",
            "ln1gk2nl/1r4gb1/p1ppp1spp/1p3pp2/9/2P2PP2/PPBPPSP1P/1G3GSR1/LN2K2NL b - 1",
        ),
        ("black-winning", "4k4/9/4S4/9/9/9/9/9/4K4 b GGBB 1"),
    ];
    println!("Sample-position cp evals ({}):", label);
    for (name, fen) in fens.iter() {
        match eval_fen(weights, fen) {
            Some(cp) => println!("  {:<14} cp = {:>+6}", name, cp),
            None => println!("  {:<14} (failed to parse fen)", name),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    println!("=== NNUE Output-Layer Symmetriser (Session 10) ===");
    println!("  Input:        {}", cli.input.display());
    println!("  Output:       {}", cli.output.display());
    println!("  Sigma:        {}", cli.sigma);
    println!("  Output bias:  {}", cli.output_bias);
    println!("  Seed:         {}", cli.seed);
    println!();

    let mut weights = NNUEWeights::load(&cli.input)?;
    let (h1, h2) = weights.hidden_sizes();
    println!("Loaded weights with arch {}->{}->1", h1, h2);
    println!();

    println!("Before symmetrisation:");
    println!("  output_bias = {}", weights.output_bias);
    output_weight_stats("output_w", &weights.output_weights);
    print_sample_evals("before", &weights);
    println!();

    let mut rng = if cli.seed == 0 {
        StdRng::from_entropy()
    } else {
        StdRng::seed_from_u64(cli.seed)
    };
    let dist = Normal::new(0.0, cli.sigma)?;
    let new_output: Vec<i16> = (0..weights.output_weights.len())
        .map(|_| {
            let x: f64 = dist.sample(&mut rng);
            x.round().clamp(-127.0, 127.0) as i16
        })
        .collect();
    weights.output_weights = new_output;
    weights.output_bias = cli.output_bias;

    println!("After symmetrisation:");
    println!("  output_bias = {}", weights.output_bias);
    output_weight_stats("output_w", &weights.output_weights);
    print_sample_evals("after", &weights);
    println!();

    weights.save(&cli.output)?;
    println!("Wrote {}", cli.output.display());
    Ok(())
}
