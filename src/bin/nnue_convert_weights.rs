//! NNUE weight format converter (Session 18).
//!
//! Converts between JSON (`.json`) and bincode (`.bin`) NNUE weight files.
//! Format is selected by output-path extension (the same dispatch the runtime
//! `NNUEWeights::load` / `save` use). This lets the 437 MB HalfKP JSON file be
//! shrunk to ~50–80 MB and loaded an order of magnitude faster, without
//! changing any in-process behaviour.
//!
//! Usage:
//!     cargo run --release --bin nnue-convert-weights -- \
//!         --input /tmp/s16_halfkp_adam_fresh_30e.json \
//!         --output /tmp/s16_halfkp_adam_fresh_30e.bin

use clap::Parser;
use shogi_engine::evaluation::nnue::{NNUEFeatureSet, NNUEWeights};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "nnue-convert-weights")]
#[command(about = "Convert NNUE weights between JSON and bincode (Session 18)")]
struct Cli {
    /// Input weights file (`.json` or `.bin` — format detected by extension)
    #[arg(long)]
    input: PathBuf,

    /// Output weights file (`.json` or `.bin` — format selected by extension)
    #[arg(long)]
    output: PathBuf,

    /// Record the feature set the network was trained with, e.g. `pov,hand` or
    /// `stm,hand`, writing a v2 file the engine configures itself from. Without
    /// it, a v2 input's declared set is carried over and a legacy input stays
    /// legacy.
    #[arg(long)]
    feature_set: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    println!("=== NNUE Weight Converter (Session 18) ===");
    println!("  Input:  {}", cli.input.display());
    println!("  Output: {}", cli.output.display());
    println!();

    let in_size = std::fs::metadata(&cli.input)?.len();
    println!("Input size:  {} bytes  ({:.1} MB)", in_size, in_size as f64 / 1024.0 / 1024.0);

    let load_start = Instant::now();
    let (weights, declared) = NNUEWeights::load_with_features(&cli.input)?;
    match declared {
        Some(fs) => println!("Declared:    feature set {}", fs),
        None => println!("Declared:    none (legacy v1 file)"),
    }
    let features = match &cli.feature_set {
        Some(spec) => Some(NNUEFeatureSet::parse(spec)?),
        None => declared,
    };
    let load_elapsed = load_start.elapsed();
    println!("Load time:   {:.3} s", load_elapsed.as_secs_f64());

    let (h1, h2) = weights.hidden_sizes();
    println!(
        "Loaded:      input_rows={}  hidden_1={}  hidden_2={}",
        weights.input_weights_1.len(),
        h1,
        h2,
    );

    let save_start = Instant::now();
    match features {
        Some(fs) => {
            weights.save_with_features(&cli.output, fs)?;
            println!("Wrote:       v2 file, feature set {}", fs);
        }
        None => weights.save(&cli.output)?,
    }
    let save_elapsed = save_start.elapsed();
    println!("Save time:   {:.3} s", save_elapsed.as_secs_f64());

    let out_size = std::fs::metadata(&cli.output)?.len();
    println!("Output size: {} bytes  ({:.1} MB)", out_size, out_size as f64 / 1024.0 / 1024.0);
    println!(
        "Shrink:      {:.2}× ({:.1}% of original)",
        in_size as f64 / out_size.max(1) as f64,
        100.0 * out_size as f64 / in_size.max(1) as f64,
    );

    // Verify round-trip: load the output back and confirm bit-for-bit equality.
    let verify_start = Instant::now();
    let (reloaded, reloaded_features) = NNUEWeights::load_with_features(&cli.output)?;
    assert_eq!(reloaded_features, features, "feature set did not round-trip");
    let verify_elapsed = verify_start.elapsed();
    println!("Reload time: {:.3} s", verify_elapsed.as_secs_f64());

    assert_eq!(weights.input_weights_1, reloaded.input_weights_1, "input_weights_1 mismatch");
    assert_eq!(weights.hidden_biases_1, reloaded.hidden_biases_1, "hidden_biases_1 mismatch");
    assert_eq!(weights.input_weights_2, reloaded.input_weights_2, "input_weights_2 mismatch");
    assert_eq!(weights.hidden_biases_2, reloaded.hidden_biases_2, "hidden_biases_2 mismatch");
    assert_eq!(weights.output_weights, reloaded.output_weights, "output_weights mismatch");
    assert_eq!(weights.output_bias, reloaded.output_bias, "output_bias mismatch");
    println!("Round-trip:  OK (all weight tensors bit-identical)");

    Ok(())
}
