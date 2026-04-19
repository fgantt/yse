# Agent Guide for Shogi Engine

## Repository Structure

This is a **Rust Shogi engine** implementing the USI (Universal Shogi Interface) protocol. It's a **single-crate workspace** with a library (`lib.rs`) and multiple binaries.

**Key directories:**
- `src/` - Library code: bitboards, evaluation, search, USI protocol
- `src/bin/` - 10 binaries (usi-engine, tuner, analyzer, nnue_trainer, etc.)
- `tests/` - ~100 integration tests
- `benches/` - ~90 criterion benchmarks (most require feature flags)
- `examples/` - Demonstrations, some with NNUE integration
- `config/` - JSON/YAML configs for PST and opening book weights
- `resources/` - Magic tables, benchmark positions, material values
- `docs/` - Extensive documentation including NNUE training guides

## Build Commands

**Standard build** (SIMD enabled by default):
```bash
cargo build --release --bin usi-engine
```

**Build without SIMD** (if compatibility required):
```bash
cargo build --release --bin usi-engine --no-default-features --features hierarchical-tt
```

**All binaries:** `usi-engine`, `tuner`, `analyzer`, `strength-tester`, `move-assessor`, `puzzle-gen`, `generate_magic_tables`, `optimize_magic_numbers`, `pst-tuning-runner`, `nnue_trainer`

## Testing Strategy

**Default features include SIMD** - most tests need it:
```bash
cargo test                          # Standard tests with SIMD
cargo test --all-features           # All features
cargo test --no-default-features    # Minimal features (rarely used)
```

**Most benchmarks require legacy-tests feature:**
```bash
cargo bench --features legacy-tests --bench <name>
```

**SIMD-specific benchmarks:**
```bash
cargo bench --features simd --bench simd_performance_benchmarks
```

**Performance tests can be skipped** with `SHOGI_SKIP_PERFORMANCE_TESTS=1` (CI environments).

## Feature Flags (Cargo.toml:6-28)

**Default:** `["hierarchical-tt", "simd"]`

Critical flags:
- `simd` - Native SIMD optimizations (AVX2/SSE on x86_64, NEON on ARM64). **No WASM support.**
- `hierarchical-tt` - Hierarchical transposition table (Task 9.x)
- `legacy-tests` - Enable 90+ legacy benchmarks and examples that may not track current APIs
- `legacy-examples` - Enable legacy example binaries
- `verbose-debug` - Compile-time debug logging (expensive string formatting)
- `statistics` - Statistics tracking (Task 7.3)
- `tt-prefetch` - TT prefetching hints (Task 4.0)
- `material_fast_loop` - Fast-loop material evaluation (Task 5.0)

## Build Configuration

**Cargo config** (`.cargo/config.toml`): Sets `target-cpu=native` for SIMD optimizations.

**Rustfmt** (`rustfmt.toml`):
- `max_width = 100`
- `edition = "2021"`
- `tab_spaces = 4`

**Clippy** (Cargo.toml:101-116): Pedantic + nursery + cargo warnings enabled, with specific allows for common patterns.

## CI Workflow

**GitHub Actions:** `.github/workflows/simd-performance-check.yml`
- Runs on `ubuntu-latest` and `macos-latest`
- Tests compile with `--features simd --release`
- Runs SIMD performance regression tests (fails on regression)
- Performance validation tests are `continue-on-error: true` (CI machines may be slow)
- Benchmarks only list tests (`--list`), full runs are local-only

## Architecture Notes

**This is a git worktree** - path indicates `yse-worktrees/nnue`, suggesting this is a branch-specific worktree for NNUE development.

**NNUE Integration:**
- Neural network evaluation (256->32->1 architecture by default)
- Training binary: `cargo run --bin nnue_trainer`
- Trained weights: `nnue_weights_trained.json` and iteration snapshots (`nnue_weights_iter_N.json`)
- See `docs/NNUE_TRAINING_GUIDE.md` for training workflow
- NNUE examples: `examples/nnue_*.rs`

**Magic Bitboards:**
- Precomputed tables in `resources/magic_tables/`
- Generation: `cargo run --bin generate_magic_tables`
- Optimization: `cargo run --bin optimize_magic_numbers`

**Evaluation System:**
- PST (piece-square tables) configs in `config/pst/`
- Tapered evaluation (opening/endgame interpolation)
- Opening book with embedded data
- Micro-tablebase for endgames

## Common Workflows

**Run USI engine interactively:**
```bash
./target/release/usi-engine
```

**Test with USI commands:**
```bash
echo -e "usi\nisready\nposition startpos\ngo depth 3\nquit" | ./target/release/usi-engine
```

**Train NNUE weights:**
```bash
cargo run --bin nnue_trainer
```

**Tune evaluation parameters:**
```bash
./target/release/tuner --dataset games.json --output weights.json --method adam --iterations 1000
```

**Analyze position:**
```bash
./target/release/analyzer startpos --depth 6
```

## Key Gotchas

1. **SIMD is default** - If you see build errors on older CPUs, use `--no-default-features --features hierarchical-tt`
2. **Legacy tests** - Most benchmarks won't compile without `--features legacy-tests`
3. **Performance tests** - May fail on CI; set `SHOGI_SKIP_PERFORMANCE_TESTS=1` to skip
4. **Worktree context** - This is likely a feature branch for NNUE; main repo may be elsewhere
5. **Many NNUE weight files** - Root has 70+ JSON files (`nnue_weights_iter_*.json`); don't commit accidentally
6. **Release profile** - Uses `lto = true` and `codegen-units = 1` for max performance (slow builds)
7. **Panic handling** - `main.rs` installs custom panic hooks and signal handlers for robust error reporting
8. **Test count** - ~100 integration tests; full test suite takes significant time

## Code Style

- Pedantic clippy enabled but with practical allows (see Cargo.toml:108-116)
- Module inception, too_many_lines, too_many_arguments, etc. allowed
- Comprehensive error documentation not required (missing_errors_doc allowed)
- Line width 100 chars, comment width 80 chars

## Documentation

**Key docs:**
- `docs/ENGINE_UTILITIES_GUIDE.md` - All binaries and their usage
- `docs/NNUE_TRAINING_GUIDE.md` - NNUE training workflow
- `docs/UTILITIES_QUICK_REFERENCE.md` - Quick command reference
- `docs/performance/` - Telemetry, profiling, benchmarking
- `docs/tuning/` - PST and material value tuning
- `docs/design/` - Architecture decisions and task tracking

## Verification Commands

**Check everything compiles:**
```bash
cargo build --all-targets --all-features
```

**Run core tests:**
```bash
cargo test --lib
cargo test --test usi_e2e_tests
cargo test --test simd_performance_validation_tests --features simd
```

**Format and lint:**
```bash
cargo fmt
cargo clippy --all-targets --all-features
```
