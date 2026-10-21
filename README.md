# Yggdrasil Shogi Engine

A shogi (Japanese chess) engine in Rust. It speaks the USI protocol, so any USI GUI can use it.
Its evaluation is an NNUE network, and its search is an alpha-beta search with Lazy SMP threads.

- **Evaluation:** a 256 -> 32 -> 1 NNUE with king-relative features (9 king buckets),
  pieces-in-hand and side-to-move inputs, updated incrementally. Trained on 187M positions from
  YaneuraOu self-play, each labelled at depth 8. A classical evaluation (piece-square tables,
  king safety, castles) remains as a fallback.
- **Search:** iterative deepening, principal variation search, aspiration windows, null move,
  late move reductions and pruning, futility and SEE pruning, check extensions, quiescence
  search, df-pn mate search at the root. Move ordering uses side-specific, continuation and
  capture history.
- **Threads:** Lazy SMP over a shared, generation-aged transposition table (256 MB default).

## Strength

Even games against YaneuraOu V9.00 with its own network on one thread at fixed nodes, while we
use 4 threads at 500 ms per move (120 games each, October 2026):

| YaneuraOu nodes per move | our score |
|---|---|
| 3,000 | 78% |
| 10,000 | 27.5% |

In other words, at these settings we are roughly as strong as YaneuraOu searching about 6,000
nodes per move. The main known weakness: in handicap games the network misreads a slow drop
attack building against its own king. Measurements and their history are in
[`docs/nnue-phase2/`](docs/nnue-phase2/README.md).

## Getting started

```bash
brew install git-lfs && git lfs install   # once; the network is stored with Git LFS
git clone https://github.com/fgantt/yse.git && cd yse
cargo build --release --bin usi-engine
cargo run --release --bin generate_magic_tables   # once; faster start-up and search
```

Point your USI GUI at `target/release/usi-engine`. The engine loads its network from
`nnue/default.bin`, looking in the working directory, then next to the executable and up to
three parent directories. If it cannot find a network it falls back to the classical evaluation
and says so after `isready`.

The release build targets the CPU it is built on (`.cargo/config.toml` sets `target-cpu=native`)
and uses SIMD by default. On an older CPU, build with
`--no-default-features --features hierarchical-tt`.

### Main USI options

| option | default | |
|---|---|---|
| `USI_Threads` | CPU count | Search threads (a value set by the GUI persists between runs) |
| `USI_Hash` | 256 | Transposition table, MB |
| `NNUEFile` | `nnue/default.bin` | Network file |
| `UseNNUE` | true | Off: classical evaluation |
| `SearchAlgorithm` | Lean | `Classic` selects the older search |
| `ThreadAlgorithm` | Lazy | `Split` selects the root/PV split search |
| `MoveOverhead` | 100 | ms reserved per move for GUI latency |

## Repository

| path | |
|---|---|
| `src/` | Engine library and binaries: `usi-engine`, plus the training, match and diagnostic tools in `src/bin/` |
| `nnue/` | Shipped network (Git LFS) |
| `scripts/` | Python tools: match statistics (`sprt.py`), labelling with YaneuraOu, test sets from games |
| `tests/`, `benches/` | Integration tests, benchmarks (most benchmarks need `--features legacy-tests`) |
| `docs/` | Documentation: [index](docs/README.md) |

Training corpora, network checkpoints and match logs are not in git. See
[`docs/REPOSITORY.md`](docs/REPOSITORY.md) for what lives where and how to set up a development
machine. Contributor and agent notes are in [`CLAUDE.md`](CLAUDE.md).

## History

The engine began as a classical evaluation engine; that version is preserved as the
`legacy-main` branch (tag `pre-nnue-main`). The NNUE work, from the first network in April 2026
to the current engine, became `main` in October 2026.
