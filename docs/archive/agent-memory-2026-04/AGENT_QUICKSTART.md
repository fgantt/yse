# AI Agent Quick Start Guide

This guide helps AI agents quickly understand and work with the Yggdrasil Shogi Engine codebase.

## Project Identity

- **Name:** Yggdrasil Shogi Engine
- **Language:** Rust (Edition 2021)
- **Type:** Shogi (Japanese Chess) engine with USI protocol support
- **Primary Focus:** NNUE neural network evaluation (this is an `nnue` feature branch)

## Essential Commands

### Build
```bash
# Standard release build (recommended)
cargo build --release --bin usi-engine

# Build all targets
cargo build --release

# Debug build (faster compile, slower runtime)
cargo build
```

### Test
```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test usi_e2e_tests

# Run with all features
cargo test --all-features

# Run SIMD validation tests
cargo test --test simd_performance_validation_tests --features simd
```

### Run
```bash
# Run USI engine
./target/release/usi-engine

# Run NNUE trainer
cargo run --release --bin nnue_trainer

# Run analyzer
cargo run --release --bin analyzer
```

## Code Navigation Map

### Where to Find Things

| What | Location |
|------|----------|
| Engine entry point | `src/main.rs` |
| Core engine struct | `src/lib.rs:79-460` |
| USI protocol | `src/usi.rs` |
| Search algorithms | `src/search/` |
| NNUE implementation | `src/evaluation/nnue.rs` |
| NNUE training | `src/evaluation/nnue_training.rs` |
| Move generation | `src/moves.rs` |
| Bitboard operations | `src/bitboards/` |
| Core types | `src/types/` |
| Binary tools | `src/bin/` |

### Key Files by Task

**Working on search:**
- `src/search/search_engine.rs` - Main search engine
- `src/search/pvs.rs` - Principal variation search
- `src/search/iterative_deepening.rs` - ID framework
- `src/search/quiescence.rs` - Quiescence search
- `src/search/transposition_table.rs` - TT implementation

**Working on evaluation:**
- `src/evaluation/nnue.rs` - NNUE network
- `src/evaluation/nnue_training.rs` - Training code
- `src/evaluation/piece_square_tables.rs` - PSTs
- `src/evaluation/tapered_eval.rs` - Game phase interpolation

**Working on move generation:**
- `src/moves.rs` - MoveGenerator struct
- `src/bitboards/magic/` - Sliding piece attacks
- `src/bitboards/attack_patterns.rs` - Attack generation

**Working on USI protocol:**
- `src/usi.rs` - Protocol handler
- `src/lib.rs:881-984` - Position handling
- `src/lib.rs:991-1193+` - Option handling

## Common Modification Patterns

### Adding a New Search Feature

1. Create feature in `src/search/` or modify existing file
2. Integrate into `SearchEngine` in `search_engine.rs`
3. Optionally expose via USI option in `usi.rs`
4. Add tests in `tests/` or within module

### Modifying Evaluation

1. Edit relevant file in `src/evaluation/`
2. Update `PositionEvaluator::evaluate()` if needed
3. Consider game phase implications (`tapered_eval.rs`)
4. Test with `cargo test --lib`

### Adding USI Option

1. Add option declaration in `src/usi.rs:handle_usi()`
2. Add handler in `src/lib.rs:handle_setoption()`
3. Store value in `ShogiEngine` struct if persistent

### Adding New Binary

1. Create `src/bin/new_binary.rs`
2. Add to `Cargo.toml`:
```toml
[[bin]]
name = "new-binary"
path = "src/bin/new_binary.rs"
```
3. Use `clap` for CLI parsing (see `src/bin/tuner.rs` for example)

## Important Patterns

### Error Handling
```rust
// Use thiserror for custom errors
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Something went wrong: {0}")]
    Custom(String),
}
```

### Logging/Debug
```rust
// Use telemetry module
crate::utils::telemetry::debug_log("message");
crate::utils::telemetry::trace_log("CONTEXT", "message");
```

### Thread Safety
```rust
// Engine uses Arc<Mutex<>> for shared state
use std::sync::{Arc, Mutex};
let shared = Arc::new(Mutex::new(data));

// Stop flag for search termination
use std::sync::atomic::{AtomicBool, Ordering};
stop_flag.store(true, Ordering::Relaxed);
```

### Feature Flags
```rust
// Conditional compilation
#[cfg(feature = "simd")]
fn simd_operation() { ... }

#[cfg(not(feature = "simd"))]
fn simd_operation() { /* fallback */ }
```

## Testing Strategy

### Unit Tests
- Located within modules (`#[cfg(test)]` blocks)
- Run with `cargo test --lib`

### Integration Tests
- Located in `tests/` directory
- Run with `cargo test --test <name>`

### Benchmarks
- Located in `benches/` directory
- Most require `--features legacy-tests`
- Run with `cargo bench --features legacy-tests --bench <name>`

### Performance Tests
- Can be skipped with `SHOGI_SKIP_PERFORMANCE_TESTS=1`
- Important for CI environments

## Data Formats

### SFEN (Shogi FEN)
```
lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
^^^^^^^^^                                                   ^ ^ ^
board layout                                       player captured movenum
```

### USI Move Format
```
7g7f    - Move from 7g to 7f
2d2c+   - Move with promotion
P*5e    - Pawn drop to 5e
```

### Piece Characters
| Piece | Black | White |
|-------|-------|-------|
| King | K | k |
| Rook | R | r |
| Bishop | B | b |
| Gold | G | g |
| Silver | S | s |
| Knight | N | n |
| Lance | L | l |
| Pawn | P | p |
| Promoted | +R, +B, +S, +N, +L, +P | +r, +b, +s, +n, +l, +p |

## NNUE Specifics

### Architecture
- Input: ~768 features (piece-square features)
- Hidden 1: 256 neurons (ClippedReLU)
- Hidden 2: 32 neurons (ClippedReLU)
- Output: 1 value (centipawn score)

### Training
```bash
# Run NNUE trainer
cargo run --release --bin nnue_trainer

# Weights saved to:
# - nnue_weights_trained.json (final)
# - nnue_weights_iter_N.json (checkpoints)
```

### Using Trained Weights
Engine automatically loads `nnue_weights_trained.json` on startup if present.

## Gotchas and Tips

1. **SIMD is default** - Use `--no-default-features` for compatibility
2. **Release mode is much faster** - Always benchmark in release
3. **Many NNUE weight files** - Don't commit `nnue_weights_iter_*.json`
4. **LTO enabled** - Release builds are slow but optimal
5. **Clippy is strict** - Run `cargo clippy` before committing
6. **Line width 100** - Check `rustfmt.toml` for style

## Quick Debug Workflow

```bash
# Enable debug output
export RUST_LOG=debug

# Run with backtrace
RUST_BACKTRACE=1 cargo run --bin usi-engine

# Test specific functionality
cargo test test_name -- --nocapture
```

## Getting Help

1. Check `AGENTS.md` in project root for build/test commands
2. Check `docs/` directory for detailed documentation
3. Check `memory/ARCHITECTURE.md` for system design
4. Check `memory/MODULES.md` for module details
5. Check `memory/BINARIES.md` for binary usage
