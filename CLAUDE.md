# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Yggdrasil Shogi Engine — a Rust Shogi (Japanese chess) engine implementing the USI (Universal Shogi
Interface) protocol. Single-crate workspace: `lib.rs` (library) + many binaries under `src/bin/`. This
worktree (`nnue` branch/worktree at `yse-worktrees/nnue`) is dedicated to NNUE evaluation development;
the main repo checkout may be elsewhere.

## Build

```bash
# Standard release build (SIMD enabled by default — this is what you almost always want)
cargo build --release --bin usi-engine

# Build without SIMD (compatibility fallback for older CPUs)
cargo build --release --bin usi-engine --no-default-features --features hierarchical-tt

# Build everything (all binaries, tests, examples)
cargo build --all-targets --all-features
```

Release builds use `lto = true` and `codegen-units = 1` — they are slow to compile but required for
real performance/ELO work; debug builds are fine for correctness-only iteration.
`.cargo/config.toml` sets `target-cpu=native`.

## Test

```bash
cargo test                          # default features (includes SIMD) — most tests need this
cargo test --lib                    # unit tests only
cargo test --test <file_stem>       # one integration test file, e.g. usi_e2e_tests
cargo test <test_name> -- --nocapture   # single test by name, with stdout
cargo test --all-features
```

- `SHOGI_SKIP_PERFORMANCE_TESTS=1 cargo test` skips performance-sensitive tests (use in CI-like/slow
  environments).
- Most of `benches/` (~90 files) needs `--features legacy-tests` to even compile:
  `cargo bench --features legacy-tests --bench <name>`.
- SIMD-specific benches: `cargo bench --features simd --bench simd_performance_benchmarks`.

## Lint / Format

```bash
cargo fmt
cargo clippy --all-targets --all-features
```

`rustfmt.toml`: `max_width = 100`, edition 2021, `tab_spaces = 4`. Clippy runs pedantic + nursery +
cargo lint groups (see `Cargo.toml` `[lints.clippy]`) with targeted allows for things like
`module_inception`, `too_many_lines`, `too_many_arguments`, `missing_errors_doc` — don't fight those,
they're intentional.

## Feature Flags

Default: `["hierarchical-tt", "simd"]` (`Cargo.toml`).

| Flag | Purpose |
|---|---|
| `simd` | Native SIMD (AVX2/SSE on x86_64, NEON on ARM64) for bitboard ops. No WASM support. |
| `hierarchical-tt` | Hierarchical transposition table. |
| `legacy-tests` | Enables ~90 legacy benchmarks/examples that may not track current APIs. |
| `legacy-examples` | Enables legacy example binaries. |
| `verbose-debug` | Compile-time-gated debug logging (string formatting fully compiled out when off). |
| `statistics` | Statistics tracking. |
| `tt-prefetch` | TT prefetching hints. |
| `material_fast_loop` | Fast-loop material evaluation. |

## Architecture

### Engine core

`ShogiEngine` (`src/lib.rs`) is the top-level struct owning `BitboardBoard`, `CapturedPieces`,
`OpeningBook`, `MicroTablebase`, and an `Arc<Mutex<SearchEngine>>`. USI I/O lives in `src/usi.rs`
(protocol parsing) with handlers back in `src/lib.rs` (`handle_setoption`, position/go handling).
`src/main.rs` is the `usi-engine` binary entry point and installs custom panic hooks/signal handlers
so the engine reports errors robustly instead of silently dying mid-search.

### Search (`src/search/`, ~64K LoC)

Iterative deepening (`iterative_deepening.rs`) driving Principal Variation Search (`pvs.rs`), with
quiescence search, null-move pruning, late-move reductions (`reductions.rs`), and move ordering
(`move_ordering/` — history heuristic, counter moves, capture ordering, caching). Transposition
tables have multiple implementations layered by feature flag: `transposition_table.rs` (base),
`hierarchical_transposition_table.rs`, `multi_level_transposition_table.rs`,
`compressed_transposition_table.rs`, `thread_safe_table.rs` — pick the right one for a given task by
checking which feature flags gate it. `parallel_search.rs` handles multi-threaded search.

### Evaluation (`src/evaluation/`, ~42K LoC)

Two coexisting evaluation strategies, selected/blended via `PositionEvaluator`:
- **Classical**: piece-square tables (`piece_square_tables.rs`, configs in `config/pst/`), tapered
  opening/endgame interpolation (`tapered_eval.rs`), king safety, castle recognition (`castles.rs`,
  `castle_geometry.rs`, `castle_fixtures.rs`), tactical/positional pattern detectors, material
  (`material.rs`), all coordinated through `integration.rs` / `component_coordinator.rs`.
- **NNUE** (`nnue.rs`, `nnue_training.rs`) — see below; this is the active development focus in this
  worktree.

### NNUE (this worktree's focus)

- Architecture: 256→32→1 (`DEFAULT_HIDDEN_SIZE_1`/`DEFAULT_HIDDEN_SIZE_2` in `nnue.rs`).
- Three composable feature spaces, all defined in `src/evaluation/nnue.rs`:
  - **Flat** piece-square features (`NUM_NNUE_FEATURES`).
  - **HalfKP**-style king-conditioned features (`NUM_NNUE_FEATURES_HALFKP`) — much larger, toggled
    per-`NNUEEvaluator` instance (not a compile-time feature).
  - Optional **side-to-move** bit (`STM_FEATURE_INDEX*`) and **pieces-in-hand** thermometer encoding
    (`NUM_HAND_FEATURES`, 76 dims total, NOT king-conditioned even under HalfKP).
- `NNUEAccumulator` supports incremental (make/unmake-move) updates for search-speed; `NNUEEvaluator`
  wraps weights + accumulator and exposes the toggles above.
- **Two trainers exist** — know which one you mean:
  - `nnue_trainer` (auto-discovered binary from `src/bin/nnue_trainer.rs`) — the original *online*
    self-play + TD(λ) trainer described in `docs/NNUE_TRAINING_GUIDE.md`. Largely superseded.
  - `nnue-offline-trainer` (`src/bin/nnue_offline_trainer.rs`, explicit `[[bin]]`) — trains from a
    static corpus (`nnue_corpus_yaneura_d10.jsonl`, ~67K YaneuraOu depth-10 positions) and is the
    trainer used by every session in `docs/nnue-phase2/SESSION_LOG_*.md`. **This is the one in active
    use**; default output is the `.bin` (bincode) format, not JSON.
- Weight files: the canonical `nnue_weights_trained.json` at repo root is auto-loaded by the engine
  and is the one file intentionally *not* gitignored (`.gitignore` excludes all other
  `nnue_weights_*.{json,bin}` at the root — the trainer writes checkpoints there by default, so clean
  up or redirect `--output-weights` to avoid sprawl). Older/superseded weight snapshots live in
  `weights/archive/`.
- Strength measurement tool: `elo-tester` (`src/bin/elo_tester.rs`) plays head-to-head bake-offs
  between two configs (e.g. NNUE vs PST, or two NNUE variants via `--nnue-weights`/
  `--nnue-weights-b`) and reports win/draw/loss + ELO with a 95% CI.
- **Read `docs/nnue-phase2/SESSION_LOG_032.md` (or the latest `SESSION_LOG_*.md`) before starting new
  work** -- its READ THIS FIRST block is the current state. Standing findings as of Session 32:
  - **Search had real bugs; check correctness before tuning.** Session 32 found the parallel
    search running on one thread, LMR and null-move verification re-searches scoring fake 0
    draws, colour-blind Zobrist keys, and perpetual check scored as a draw -- together several
    hundred Elo of self-play. Trace any exact 0 or other suspicious score to its source.
  - **The remaining weakness is evaluation of king danger**, not search depth: our score follows
    material and swings 500-800 cp with the side to move in mutual attacks. But two attempts to
    move the evaluation toward YaneuraOu's numbers lost 110-180 Elo. Eval-gap error is a
    diagnostic, not a target; judge evaluation changes only by games.
  - **Measure before believing, and check the denominator.** Several confident diagnoses were
    wrong in sign or magnitude (Sessions 29 and 32). An SPRT stopped on 14-2 after 8 pairs; the
    minimum-pairs rule exists because of it.
  - **Benchmark the engine you ship.** `ParallelEnable value false` measurements describe a
    different engine; profile per thread, not only per function.
  - **Self-play Elo overstates real gains.** Track the YaneuraOu six-piece / 2,000-node ladder rung
    (Session 32: 78%) for progress that transfers.
- **Trustworthy measurements**, in order of cost: `search-diag` (tree shape, cutoff index,
  LMR rejection reasons, per-ply node ratio); neutral-set move loss at fixed depth or a time control
  (150 held-out teacher positions, ±30 cp on the mean, medians stabler); the six-piece handicap
  ladder against YaneuraOu at 2,000 nodes (Session 32 stable 78%; the 50-node rungs are now too easy
  to separate engines; only our games as Black, holding the odds, count); 1 s/move head-to-head over 120 games. **`elo-tester` at fixed depth 2 can invert the
  sign** and must never be a ship gate.
- **Ship gate: SPRT against `engines/usi-engine-stable`**, not a fixed game count. A 120-game match
  resolves only ±60 Elo, so nearly everything read "neutral". Run
  `usi_match --engine-a NEW --engine-b STABLE --byoyomi 1000 --games 600 --sprt-elo0 0 --sprt-elo1 40`
  (private copies of both binaries; stops on PASS/FAIL, `--games` is the cap). Bounds [0, +40]:
  a neutral change passes ~6% of the time in a median ~314 games, a +60 change passes in ~176,
  a −40 change fails in ~122. `scripts/sprt.py check <csv>` tests any match CSV (or several
  pooled); `scripts/sprt.py plan elo0 elo1` simulates the cost of other bounds. Changes that leave
  the single-threaded `search_bench --fingerprint` byte-identical cannot lose strength: stack them
  and gate the stack.

### Other subsystems

- `src/bitboards/` — board representation, magic bitboards for sliding pieces
  (`bitboards/magic/`, generated via `generate_magic_tables` / tuned via `optimize_magic_numbers`,
  tables cached in `resources/magic_tables/`), SIMD-accelerated ops.
- `src/tablebase/` — micro-tablebase for endgame positions.
- `src/opening_book.rs` + `src/ai/openingBook.json` — embedded opening book data.
- `src/tuning/` — parameter tuning infra used by the `tuner` binary (Adam/LBFGS/Genetic methods).
- `src/types/` — core domain types (pieces, players, moves, positions).

### Binaries (`src/bin/`, see `docs/ENGINE_UTILITIES_GUIDE.md` for full usage/examples)

`usi-engine`, `tuner`, `analyzer`, `strength-tester`, `move-assessor`, `puzzle-gen`,
`generate_magic_tables`, `optimize_magic_numbers`, `pst-tuning-runner`, `elo-tester`, `corpus-gen`,
`nnue-offline-trainer`, `nnue-weight-diff`, `nnue-init-symmetric`, `nnue-convert-weights`, plus
auto-discovered `nnue_trainer` and `profiler` (no explicit `[[bin]]` entry, but still buildable/runnable
by filename since `src/bin/*.rs` is auto-discovered by Cargo).

## Data formats

**SFEN** (Shogi FEN): `lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1` — board / side
to move / captured pieces / move number.

**USI move notation**: `7g7f` (normal move), `2d2c+` (move with promotion), `P*5e` (drop).

## Gotchas

1. SIMD is default — build errors on unusual/older CPUs mean drop to
   `--no-default-features --features hierarchical-tt`.
2. Most benchmarks and some examples require `--features legacy-tests` to compile at all.
3. Don't accidentally commit NNUE weight checkpoints — `.gitignore` already excludes
   `nnue_weights_*.{json,bin}` except the canonical `nnue_weights_trained.json`; also gitignored:
   `elo_results.csv`, `nnue_corpus_*.jsonl`.
4. `memory/` in the repo root holds structured docs for AI agents (`AGENT_QUICKSTART.md`,
   `ARCHITECTURE.md`, `MODULES.md`, `BINARIES.md`) — check there before re-deriving architecture facts.
5. Bake-off / ELO measurements are wall-clock heavy (minutes per game at depth 3); when running
   `elo-tester` at scale, expect multi-hour wall times and consider running two configs in parallel
   background processes with distinct `--output`/`--seed` values.
