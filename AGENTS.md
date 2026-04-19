# Agent Instructions for Yggdrasil Shogi Engine

## Build & Test

**Standard build** (SIMD enabled by default):
```bash
cargo build --release --bin usi-engine
```

**Disable SIMD** if needed (e.g., for compatibility):
```bash
cargo build --release --bin usi-engine --no-default-features --features hierarchical-tt
```

**Test order matters**:
1. Format check: `cargo fmt --check`
2. Clippy: `cargo clippy --all-targets --all-features`
3. Unit tests: `cargo test --lib`
4. Integration tests: `cargo test --test "*"`
5. Feature-gated tests: `cargo test --features legacy-tests`

**Benchmarks require features**:
```bash
cargo bench --features legacy-tests,simd
```
(Do NOT run full benches locally without `--features legacy-tests,simd`)

## Features & Build Options

| Feature | When to Use | Default |
|---------|------------|---------|
| `simd` | x86_64 with AVX2/SSE or ARM64 with NEON (2-4x speedup) | ✓ Yes |
| `hierarchical-tt` | Multi-level transposition table optimization | ✓ Yes |
| `legacy-tests` | Old/long-tail test suites that may not track current APIs | No |
| `legacy-examples` | Outdated examples | No |
| `verbose-debug` | Compile-time debug logging with string formatting | No |
| `statistics` | Statistics tracking (disabled compiles out code) | No |
| `tt-prefetch` | Transposition table prefetching hints | No |

**Default feature set** (`cargo test` with no flags) uses `hierarchical-tt` + `simd`. Most tests pass without legacy flags.

## Architecture Notes

**Shogi-specific coordinates**: 9×9 board. Row 0 = rank 9 (top), Row 8 = rank 1 (bottom). Columns 0–8 for files a–i.

**Entry points**:
- Main: `src/main.rs` → `src/usi.rs` (USI protocol loop) → `src/lib.rs` (`ShogiEngine` facade)
- Search: `src/search/search_engine.rs` (alpha-beta core)
- Evaluation: `src/evaluation/evaluation.rs`
- Board: `src/bitboards.rs` (bitboard-based representation)

**Key modules**:
- `src/search/` — iterative deepening, transposition table, parallel search, quiescence, LMR, IID
- `src/evaluation/` — material, piece-square tables (PST), king safety, patterns, caching
- `src/types/` — `Player`, `PieceType`, `Position`, `Piece`, `Move`, `Board`
- `src/moves.rs` — legal/pseudo-legal move generation
- `src/opening_book.rs` — JSON/binary book loading
- `src/tablebase/` — endgame micro-tablebase

## Test Organization

**Unit tests**: Live in source files as `#[cfg(test)] mod tests { ... }` or as `src/**/*_tests.rs`.

**Integration tests**: `tests/` directory, 140+ test files covering:
- Aspiration windows
- LMR and IID optimization  
- Opening books
- Transposition tables
- SIMD performance validation
- Parallel search
- Evaluation components
- Quiescence and null move pruning

**Benchmarks**: `benches/` directory, use Criterion, many require `--features legacy-tests` or `--features simd`.

**SIMD tests**: `tests/simd_*_tests.rs`. Compile-checked in CI. Validate `target-cpu=native` optimizations. Skip on slow CI machines.

## Common Workflows

**Run a single test**:
```bash
cargo test --lib <test_name>
```

**Run integration tests for a feature**:
```bash
cargo test --test evaluation_integration_tests
cargo test --features simd --test simd_nps_validation
```

**Focused test for debugging**:
```bash
cargo test -- --nocapture --test-threads=1 <test_name>
```

**Check a specific benchmark compiles** (lists tests, doesn't run):
```bash
cargo bench --bench simd_performance_benchmarks --features simd -- --list
```

**Build with all feature combinations** (for CI-like verification):
```bash
cargo build
cargo build --no-default-features
cargo build --features simd,hierarchical-tt,statistics,verbose-debug,tt-prefetch
```

## Performance Tuning Gotchas

**Clippy pedantic lints are enabled** but with allowances for:
- `too_many_lines`, `too_many_arguments`, `cognitive_complexity` (engine code is complex)
- `similar_names`, `type_complexity`, `missing_errors_doc`, `missing_panics_doc`

Individual `#[allow(...)]` comments in code are expected.

**Release profile** uses:
- `opt-level = 3` (aggressive)
- `lto = true` (link-time optimization)
- `codegen-units = 1` (single unit, slower compile, faster runtime)
- `debug = true` (symbols retained for profiling)

**Magic number tables** are generated at build time. Changes to `src/bitboards/magic/magic_table.rs` may require running:
```bash
cargo run --bin generate_magic_tables --release
cargo run --bin optimize_magic_numbers --release
```

## CI & Workflows

**Single workflow**: `.github/workflows/simd-performance-check.yml`
- Runs on push to `main` and pull requests
- Builds with SIMD feature
- Runs `simd_performance_regression_tests` (required)
- Runs `simd_performance_validation_tests` (can be skipped if `SHOGI_SKIP_PERFORMANCE_TESTS` is set; may fail on slow CI machines)
- Benchmarks compile-checked only (full benches skipped, too slow for CI)

## Configuration & Files

**Platform tuning**: `.cargo/config.toml` sets `rustflags = ["-C", "target-cpu=native"]` (enables AVX2/SSE/NEON).

**Linting**: Clippy pedantic/nursery/cargo lints in `Cargo.toml`.

**Search/evaluation settings**: Defaults in `src/lib.rs` (`ShogiEngine` struct).

**Opening book**: JSON config in `config/opening_book/` and `resources/material/` for piece values.

**PST tuning**: Piece-square tables in `config/pst/default.json`.

## Memory & Documentation

The `memory/` directory contains agent-friendly notes:
- `PROJECT.md` — High-level overview, entry points, binaries
- `MODULES.md` — Module responsibilities and public APIs
- `CONTEXT.md` — Key types, board representation, coordinate system
- `SEARCH.md` — Search algorithm details
- `ENTRY_POINTS.md` — How code flows from USI input to move output

Prefer reading these before exploring source for the first time.

## Existing Instruction Sources

See also:
- `docs/` — Design documents, implementation guides, performance analysis
- README.md — Build/run instructions
- Inline code comments and doc-comments in source

If contradictions arise between `AGENTS.md`, config, and source, trust the executable (code/config) first.
