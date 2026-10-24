# Repository layout and setup

How this repository is organised: what is in git, what lives only on the development machine,
and how to set up another machine. Written 2026-10-17, when the NNUE line of work became `main`.

## Branches

| ref | what it is |
|---|---|
| `main` | The engine going forward: NNUE evaluation, lean search, USI. Was the `nnue` branch until 2026-10-17. |
| `legacy-main` (branch) and `pre-nnue-main` (tag) | The old classical (pre-neural-network) engine, i.e. the `main` before 2026-10-17. Historical only; nothing is merged from it. |

On 2026-10-17 `main` was pointed at the `nnue` branch's tip (a force-push; nobody else had a
clone), the old `main` was kept as `legacy-main` / `pre-nnue-main`, and `nnue` was deleted. The
history of the NNUE work (Sessions 1-35, `docs/nnue-phase2/SESSION_LOG_*.md`) is `main`'s
history.

On the development machine the working copy is `shogi-game/yse-worktrees/nnue` (a git worktree,
now on `main`; the directory name is historical and the user's GUI points at it). The original
clone at `shogi-game/yse` has `legacy-main` checked out.

## What is where

| path | in git | notes |
|---|---|---|
| source, tests, docs, `scripts/` | yes | |
| `nnue/default.bin` (shipped network, ~100 MB) | **Git LFS** | Loaded by the engine by default (USI `NNUEFile`, `src/lib.rs` `DEFAULT_NNUE_FILE`). Without it the engine falls back to the much weaker classical evaluation and says so on `isready`. |
| `nnue/default-q9.bin` | Git LFS | The previous default network (Session 36 q9, 187M positions), for comparisons. |
| `nnue/default-p9.bin` | Git LFS | An older default network (Session 36 p9, 125M positions), for comparisons. |
| `nnue/default-k81.bin` | Git LFS | An older default network (Session 35, 81 king buckets), for comparisons. |
| `nnue/default-s26.bin` | Git LFS | An older default network (Session 26), for comparisons. |
| `matches/user_games/`, `matches/s35/losses/` | yes | The user's GUI games and the test set built from them (`scripts/loss_set.py`). |
| `eval_gap/` | yes | Eval-gap positions and results (`scripts/eval_gap.py`). |
| `weights/archive/` | yes | Old weight snapshots. |
| `corpus_*/` | no (ignored) | Training corpora, ~40 GB. `corpus_s37/pool_d8.jsonl` (57 GB, 249M positions labelled by YaneuraOu; `corpus_s36b/pool_d8.jsonl` plus `corpus_s37/gen_d8`) is what the current network was trained on; each ~62M-position batch costs ~11 h of generation and labelling to rebuild. **Lives only on the development machine for now.** |
| `weights/session*/` | no (ignored) | Training runs and checkpoints. `weights/session37/r9.bin` is the current network. |
| `engines/` | no (ignored) | Local builds kept for matches; `engines/usi-engine-stable` is what the GUI runs. Rebuild from a commit; binaries are machine-specific. |
| `matches/*` other than the above | no (ignored) | Match logs and CSVs. Commit the few a session log cites. |
| `resources/magic_tables/magic_table.bin` | no (ignored) | Generated (86 MB). |
| `book/user_book1.db` | no (ignored) | Opening book (USI `BookFile book/user_book1.db`; off by default). YaneuraOu team's New Peta-shock book, 2.25M positions, 493 MB, MIT: `new_petabook_20250505c.7z` from github.com/yaneurao/YaneuraOu/releases/tag/new_petabook233, extracted with `7zz x`. |

Ignored files are still deleted by `git clean -x`. Never run `git clean -fdx` in a working copy
that holds corpora or weights.

## Setting up a machine

1. Install Git LFS **before** cloning (`brew install git-lfs`, or your package manager), then
   `git lfs install` once. A clone made without LFS has a ~130-byte pointer file at
   `nnue/default.bin`; run `git lfs pull` to fetch the real network.
2. `git clone https://github.com/fgantt/yse.git` and `cargo build --release --bin usi-engine`.
   On macOS, if build scripts fail to link with `unknown architecture arm64e.x1` /
   `tapi error: malformed file`, the Command Line Tools SDK is newer than clang can read; use
   Xcode's SDK: `export SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk`.
3. Magic tables: `cargo run --release --bin generate_magic_tables` writes
   `resources/magic_tables/magic_table.bin`. Without it the engine generates the tables on every
   start (~60 s) and runs searches slower.
4. Analysis and training scripts use YaneuraOu as teacher and judge. They find it through
   `scripts/paths.py`: `$YSE_YANE` if set, otherwise a YaneuraOu build in a `yaneuraou-*`
   directory beside the repository or one of its parents (the development machine has
   `shogi-game/yaneuraou-V900-git-mac-all`). `python3 scripts/paths.py` prints what it found.
   `$YSE_REPO` overrides the repository root.
5. Training needs the corpus, which is not distributed yet: copy `corpus_s35/` from the
   development machine.

## Changing the shipped network

Copy the new file over `nnue/default.bin` and commit it; LFS stores it. Keep the previous one
for comparisons (`nnue/default-<name>.bin` is also tracked by LFS, `.gitattributes`). Each
version stays in LFS storage, so commit a network only when it ships. GitHub's free LFS quota
is limited (storage and monthly bandwidth, check current figures); at ~100 MB per network
that allows a few dozen versions.

## Distributions (planned: macOS, Windows, Linux)

Not built yet. Constraints the build has to respect:

- `.cargo/config.toml` sets `target-cpu=native`: binaries built this way may crash on other CPUs.
  Distribution builds need an explicit target level instead (e.g. `x86-64-v3` for AVX2 plus a
  `--no-default-features --features hierarchical-tt` fallback without SIMD; Apple Silicon and
  ARM64 Linux get NEON as standard).
- The engine looks for `nnue/default.bin` in the working directory, then beside the executable
  and up to three parent directories (`resolve_nnue_path`), so a package should ship
  `nnue/default.bin` next to the binary. The magic table is found the same way
  (`resources/magic_tables/`, or `$SHOGI_MAGIC_TABLE_PATH`); ship it or accept slow starts.
- Preferences go to the OS config directory (`dirs::config_dir()`, `$SHOGI_PREFS_DIR` overrides).
- Windows and Linux builds have not been tested.
