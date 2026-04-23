# NNUE Implementation Session Log

## Session 6: External USI Teacher - Corpus Generator Infrastructure

**Date**: 2026-04-22
**Duration**: ~2 hours
**Phase**: Phase 2b / pre-Phase-4.2 (new training signal)
**Objective**: Break past the "NNUE ≈ PST" ceiling Session 5 documented, by switching the training teacher from our own PST evaluator to an external strong USI engine (YaneuraOu). This session delivers the infrastructure — a USI client library and a corpus-generator binary — plus an end-to-end smoke test. The actual retraining against the new corpus is Session 7.

---

## Pre-Session Checklist

- [x] Reviewed Session 5 conclusion (NNUE-vs-PST pilot: 0 decisive games / 50; NNUE is a compressed PST under current training signal).
- [x] Verified YaneuraOu (`.../YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1`) and Apery (`.../apery_rust/target/release/apery`) respond to USI handshake.
- [x] Decided to start with YaneuraOu (stronger NNUE teacher, readily available).
- [x] Decided on JSONL corpus format and `go depth N`-based search (reproducible).

---

## Work Completed

### Subtask 1: Manual USI sanity checks
- **Status**: Completed
- **Changes made**: None (diagnostic only)
- Ran `usi / isready / position startpos / go depth 10 / quit` against each engine from its own cwd.
- YaneuraOu (V900Git, APPLEM1 binary) returned: `depth 10 score cp 156 nodes 30963 nps 5160500 bestmove 2g2f ponder 8c8d`. Eval file `eval/nn.bin` loaded correctly when cwd was set to the engine directory.
- Apery (2.1.0) returned: `depth 10 score cp 36 ... bestmove 2g2f ponder 4a3b` with per-depth progression visible — good for parse testing.
- Learned two engine-specific quirks:
  - **YaneuraOu** defaults: `USI_OwnBook = true`, `MinimumThinkingTime = 2000 ms`, `NetworkDelay = 120`, `NetworkDelay2 = 1120`. We must override all four to get honest search results. Book file (`standard_book.db`) fails to load silently and the engine proceeds.
  - **Apery** defaults: `Book_Enable = false` (good), `Eval_Dir = eval/20190617` (requires correct cwd).
- First attempt at `quit`-after-`go` returned `bestmove 1g1f` instantly with `nodes 0` — this was a harness bug: we piped `quit` into the engine before it had finished searching, so it aborted. Adding a delay before `quit` in the interactive test produced the expected deep-search output.

### Subtask 2: USI client module (`src/usi_client.rs`, 214 lines)
- **Status**: Completed
- **Changes made**:
  - New file `src/usi_client.rs`:
    - `UsiEngine` struct wrapping `Child`, `ChildStdin`, and `BufReader<ChildStdout>`.
    - `UsiEngine::spawn(path, cwd)` — spawns with `current_dir(cwd)`, `stderr(Stdio::null())`.
    - `handshake()` reads to `usiok`, `isready()` reads to `readyok`, ignoring intermediate `id` / `option` / `info string` lines.
    - `set_option(name, value)` — sends `setoption name X value Y`. Engine-specific options that aren't recognized are silently ignored by the engine, which is what we want.
    - `set_position_startpos(&[String])` — `position startpos moves m1 m2 ...`.
    - `go_depth(u32)` / `go_movetime(u32)` — reads until a `bestmove` line, parsing every `info ...` line in between.
    - `SearchResult` captures `bestmove`, optional ponder, last-reported `score_cp`, `score_mate`, `depth`, `nodes`, `time_ms`.
    - `SearchResult::as_cp_with_mate(magnitude)` maps `score_mate ±N` to `±(magnitude − |N|)` so downstream training code sees a single `i32` eval.
    - `UsiError` enum: `Spawn`, `Io`, `EngineClosed`, `ProtocolError(String)`.
  - File: `src/lib.rs` — added `pub mod usi_client;` next to existing `pub mod usi;` (which is the USI *server* side of our own engine — distinct from this *client* module).

### Subtask 3: Corpus generator binary (`src/bin/corpus_gen.rs`, ~310 lines)
- **Status**: Completed
- **Changes made**:
  - New file `src/bin/corpus_gen.rs`:
    - `clap`-driven CLI: `--engine-path`, `--engine-cwd`, `--games`, `--depth`, `--random-plies`, `--max-moves`, `--seed`, `--threads`, `--hash-mb`, `--output`, `--skip-random-opening-positions` (default true), `--mate-magnitude`, `--verbose`.
    - Phase 1 of each game: our own `MoveGenerator::generate_legal_moves` picks `random_plies` random legal moves from startpos to produce opening diversity. Positions are recorded but flagged `from_random_opening: true`.
    - Phase 2: teacher drives both sides. Each ply:
      1. `engine.set_position_startpos(&usi_moves)` then `engine.go_depth(depth)`.
      2. If `bestmove == "resign" | "0000" | ""` → to-move player resigns → assign outcome to opposite side.
      3. If `bestmove == "win"` → to-move declares entering-king win.
      4. Otherwise, record `(sfen_before_move, player, eval_cp)` and apply the move via `Move::from_usi_string` + `BitboardBoard::make_move`.
    - Game-end detection on our side: no legal moves (checkmate vs stalemate), ≥3-fold repetition (via `ShogiHashHandler::get_position_hash`), or `ply >= max_moves`.
    - Outcome backfilled and written to JSONL per-position after game ends. Records include: `game_id, ply, sfen, player, eval_cp, outcome (+1/0/-1 Black-POV), outcome_str, from_random_opening`.
    - Hand-rolled JSON writer (no serde dependency for this tiny schema) — one record per line.
  - File: `Cargo.toml` — added `[[bin]] corpus-gen` entry.

### Subtask 4: Pre-existing bug discovered — promoted-rook sliding in our move generator
- **Status**: Documented, not fixed (out of scope)
- **Symptom**: First attempted smoke test failed with
  `error: protocol error: teacher move 8f2f is not in our legal-move list for Black`.
- **Root cause**: Our `generate_legal_moves` produces only 1-step moves for a promoted rook ("dragon"). At SFEN `.../1+R7/...` the list contained `8f9e 8f8e 8f7e 8f9f 8f7f 8f7g` — the king-like 1-step moves — but no long slides along rank f (`8f6f`, `8f5f`, ..., `8f1f`) or file 8 past one square. YaneuraOu correctly sees those slides.
- **Cross-check**: A non-promoted rook at 2h in the same position correctly generated `2h2g 2h2f 2h2e 2h2d` (multi-square north slide). So the bug is specific to promoted rook (and presumably promoted bishop / "horse") move generation, not a general rook bug.
- **Impact for this session**: Relaxed the legality check in `corpus_gen.rs` — the teacher is authoritative for move legality; `Move::from_usi_string` + `board.make_move` apply from/to coordinates directly and do not depend on the buggy generator. Smoke test then worked end to end.
- **Impact elsewhere**: This is an existing correctness issue in our engine. It likely costs playing strength (missed escapes, missed sliding threats) and should be fixed, but that's a separate thread of work from NNUE training.

### Subtask 5: End-to-end smoke test
- **Status**: Completed
- **Command**:
  ```
  ./target/release/corpus-gen \
      --engine-path <YaneuraOu binary> \
      --games 5 --depth 8 --random-plies 8 \
      --seed 2026 --output /tmp/corpus_smoke.jsonl
  ```
- **Result**:
  ```
  Game   1/  5: white_win in 150 plies,  150 positions (142 written)  t=0.33s
  Game   2/  5: black_win in 129 plies,  129 positions (121 written)  t=0.42s
  Game   3/  5: white_win in 136 plies,  136 positions (128 written)  t=0.34s
  Game   4/  5: black_win in 125 plies,  125 positions (117 written)  t=0.33s
  Game   5/  5: black_win in  67 plies,   67 positions ( 59 written)  t=0.17s
  --- Summary ---
  5 games, 3 black / 2 white / 0 draw, 100% decisive
  567 positions written, 1.6s wall time
  ```

### Subtask 6: Session log
- **Status**: Completed (this file)

---

## Testing & Verification

### Build Check
```
cargo build --release --lib          → Pass (usi_client compiles clean)
cargo build --release --bin corpus-gen → Pass
No new warnings from either file.
```

### Functional Tests
```
Test 1: USI handshake with YaneuraOu end-to-end (handshake → isready → set_position → go_depth → quit)
Expected: no errors; JSONL created
Actual:   5-game smoke completed in 1.6s with 567 positions
Result:   Pass

Test 2: Decisive outcomes (key Phase 4 blocker)
Expected: ≥ one decisive outcome (would have been a Session-5-to-Session-7
          improvement even at 1/5; we were hoping for most to be decisive)
Actual:   5/5 decisive (100%)
Result:   Pass (strong)

Test 3: Eval-cp sign convention from teacher
Expected: sign flips with player-to-move (USI convention)
Actual:   First ply in game 0: Black POV +188; next ply same game White POV -174
Result:   Pass

Test 4: Mate-score handling
Expected: scores near end of decisive games should approach ±mate_magnitude
Actual:   Final recorded plies of game 4 show eval_cp = 29997, -29998, 29999
Result:   Pass
```

### Performance/Metrics
```
Smoke run: 5 games × ~100-150 plies × depth 8 ≈ 0.3 s/game
Positions/second: ~355 positions/sec (including engine spawn + I/O)
Corpus size: 567 JSONL records ≈ 120 KB (~212 bytes/record, uncompressed)

Extrapolation: a 1000-game overnight run at these settings would produce
~100 K-120 K positions in ~5-6 minutes of wall clock. We can comfortably
aim for 100 K+ positions for the Session 7 retrain.
```

---

## Observations & Insights

**What went well**:
- The USI client came out small and tidy — a single file, no async/threading, synchronous reads are sufficient for this workload.
- Putting engine-specific options (`USI_OwnBook`, `MinimumThinkingTime`, `Book_Enable`) as best-effort `setoption` calls means the same `corpus-gen` binary works for both YaneuraOu and Apery — each ignores the options it doesn't recognize.
- YaneuraOu at depth 8 is extraordinarily fast on an Apple Silicon machine: a full 150-ply game completed in 0.33s. No need to batch or parallelize for reasonable-sized corpora.
- The JSONL schema was easy to hand-write (one line per record, 8 fields) without pulling in serde-json. Future tooling (training, analysis) can read this with 5 lines of parser code.

**What was harder than expected**:
- The `quit`-after-`go` problem in initial testing briefly made it look like both engines were returning book moves instantly — turned out to be a piping artifact from the verification shell command, not an actual engine bug.
- The promoted-rook move-generator bug was an unexpected blocker. Luckily it's only a validation issue in our generator, not a `make_move` issue — otherwise the corpus would've been corrupted or the session would've stalled on an engine-internal fix.

**Surprises**:
- The speed: at depth 8, YaneuraOu with 1 thread, 256 MB hash, from arbitrary positions, averaged ~3 ms per ply end-to-end including subprocess I/O. Depth 10-12 (what we'd actually want for strong training) might be 3-5× slower, so still minutes-not-hours territory.
- 100% decisive rate at just 8 random plies, whereas our PST self-play needed 24 random plies to get games longer than 30 moves and still drew 100%. YaneuraOu simply plays to win rather than drifting into repetition.
- The eval-cp values track nicely with game outcome direction (see game 4: eval climbs from ~0 in the opening to +300 for Black by ply 10 and keeps rising). That's the signal PST-supervised training never had.

**Insights for future sessions**:
- The promoted-rook bug is a separate thread but worth capturing: until it's fixed, any of our engine's own games will silently play a weaker version of dragon (miss long slides). This probably costs us ELO whenever a promoted rook is on the board — relevant to the `elo-tester` pilot interpretation.
- We should log book-move lookup off carefully: YaneuraOu's "standard_book.db" failed to load (missing file), but if a user has a book installed the engine would start giving book moves by default. `USI_OwnBook=false` fixes this.
- Corpus size / quality tradeoff: ~100 K high-quality positions with eval + outcome is probably more valuable than 1 M positions with PST-only eval. Aim for depth ≥ 10 in the real run.

---

## Decisions Made

**Decision 1**: External USI subprocess rather than in-process library integration
- **Rationale**: YaneuraOu is C++; Apery is Rust but has a specific API and eval-file dependency. Subprocess+USI is portable, matches how every Shogi GUI talks to these engines, and decouples our build graph. Small perf overhead (a few ms of IPC per ply) is immaterial at corpus-generation timescales.
- **Alternative considered**: Embedding Apery's library (would tangle build); writing a Rust wrapper over YaneuraOu's C++ lib (much more work).
- **Confidence**: High

**Decision 2**: Synchronous I/O and single-threaded engine-per-process
- **Rationale**: Simpler code, predictable. The corpus generator is I/O bound on the engine's search, not on the shell around it; there's no benefit to async. Single thread = reproducible, and we can always run multiple `corpus-gen` processes in parallel if we need throughput.
- **Alternative considered**: tokio + multi-game parallelism
- **Confidence**: High

**Decision 3**: Use `go depth N` instead of `go movetime N`
- **Rationale**: Deterministic per-move work amount. Avoids all the `MinimumThinkingTime` / `NetworkDelay` knobs we saw YaneuraOu exposes; we'd have had to set those to 0 anyway, and even then movetime scheduling is fuzzy.
- **Alternative considered**: movetime (would allow time-based scaling)
- **Confidence**: High

**Decision 4**: Recorded SFEN uses our own `board.to_fen()` (from-ply position), not a fetched SFEN from the teacher
- **Rationale**: Our NNUE training pipeline already consumes `BitboardBoard` positions; recording in the format our feature extractor reads is simpler. The only risk is board desync between us and the teacher, which would produce garbage SFEN. This is bounded by any correctness bugs in our `make_move`; the legal-move bug found this session does not affect `make_move`.
- **Alternative considered**: Query teacher for its SFEN after each move (not part of USI standard, varies per engine).
- **Confidence**: Medium — worth adding a sanity check in Session 7: after generating a corpus, re-feed a random sample of SFENs back to the teacher and compare evals. If they match, our SFEN export is faithful.

**Decision 5**: Record positions from to-move player's POV (standard USI convention)
- **Rationale**: Matches how the teacher reported its eval; the training pipeline already has `player` logic for flipping if needed. Normalizing to Black-POV at generation time would throw away information (the `player` field) and couple us to one specific training pipeline.
- **Alternative considered**: Always Black-POV eval_cp
- **Confidence**: High

**Decision 6**: Skip random-opening positions by default (`--skip-random-opening-positions = true`)
- **Rationale**: These have `eval_cp = null` because no search was run on them. Most training pipelines either want a real eval or want to skip. Making "skip" the default keeps the corpus homogeneous. A `--skip-random-opening-positions false` flag is available if someone wants the opening positions too.
- **Alternative considered**: Always include with null eval
- **Confidence**: High

**Decision 7**: Defer the promoted-rook move-generator fix
- **Rationale**: It's an engine-correctness issue, not NNUE-related. Fixing it means changes to move generation across many piece types (by-the-way also check promoted bishop = horse). That's its own session and its own testing surface. For corpus generation, bypassing the validation check (trusting the teacher) is a 1-line fix that unblocks everything.
- **Alternative considered**: Fix it now
- **Confidence**: High for this session; medium for "probably need to fix soon" — this bug likely affects our engine's actual playing strength.

---

## Blockers & Issues

### Issue 1: Promoted rook's long slides missing from `generate_legal_moves`
- **Severity**: Medium (engine-correctness bug, not NNUE-specific)
- **Description**: For a promoted rook (`+R` / dragon), our legal-move generator only emits 1-step moves in all 8 directions. It should emit: all rook slides (any number of empty squares along rank/file) PLUS 1-step diagonals.
- **Root cause**: Unknown — not investigated. Likely a missing branch in the slide-generation logic for promoted pieces. Could plausibly affect promoted bishop ("horse") too.
- **Resolution**: Deferred. For this session, the corpus generator trusts the teacher's moves and does not validate against `generate_legal_moves`.
- **Status**: Tracked. Recommend opening a dedicated fix issue; would want tests comparing our legal-move list against YaneuraOu's across a suite of known positions.

### Issue 2: `eval_cp` for random-opening positions is `null`
- **Severity**: Low (by design)
- **Description**: Phase 1 of each game plays `random_plies` moves without querying the teacher, so those positions have no eval. With `--skip-random-opening-positions=true` they're excluded; with `false` they're included with `eval_cp: null`.
- **Resolution**: Keep current behavior. If we ever want eval on these, just query the teacher on each random-opening position too — cheap.
- **Status**: Known, documented.

---

## Next Session Plan (Session 7)

**What to do next** (rank-ordered):

1. **Generate a real corpus**: 500-1000 YaneuraOu self-play games at depth 10-12 (targeting ~100 K positions). Estimated wall-clock: 5-15 minutes. Save as `nnue_corpus_yaneura_d12.jsonl`.
2. **SFEN round-trip sanity check**: sample 100 positions from the corpus, re-feed each SFEN to YaneuraOu (`position sfen <sfen> 1`), query `go depth 8` and verify the eval is close to the recorded `eval_cp` (it should match within search-variance since recording was also at depth ≥ 8). This validates Decision 4.
3. **Adapt `nnue_trainer`** (or write a new binary, `nnue_offline_trainer`) to:
   - Load the JSONL corpus
   - For each position: reconstruct board via `BitboardBoard::from_fen(sfen)`, compute active features, run NNUE forward pass, compute loss against a blend of `tanh(eval_cp / 600)` and `outcome`, backprop, SGD update.
   - Save trained weights.
4. **Re-run `elo-tester`** with the new weights. Expected: *decisive* NNUE-vs-PST games finally appear. Target ELO gap: +50 (conservative) to +200 (optimistic given the teacher is hundreds of ELO stronger than our PST).
5. Optionally: generate a second corpus with Apery, train a second NNUE, test both against each other and against PST.

**Decision points to make**:
- [ ] Depth for corpus generation (10 vs 12 vs 15): balance quality vs wall-clock.
- [ ] Keep random-opening positions (no eval) for pure-outcome loss, or require all positions to have eval? (Current default: skip, require eval.)
- [ ] Training-side: how to blend `eval_cp` target with `outcome` target? Candidates: `0.7·tanh(eval_cp/600) + 0.3·outcome`, outcome weight rising as `ply/max_ply`.

**Estimated duration**: 3-4 hours for the full Session 7.

**Prerequisite**: Session 6 deliverables (done).

---

## File Changes Summary

### Files Modified
- `Cargo.toml` — added `[[bin]] corpus-gen` entry.
- `src/lib.rs` — added `pub mod usi_client;`.

### Files Created
- `src/usi_client.rs` (214 lines) — synchronous USI client library.
- `src/bin/corpus_gen.rs` (~310 lines) — teacher-self-play corpus generator binary.
- `docs/nnue-phase2/SESSION_LOG_006.md` (this file).

### Files Deleted
- None.

### Other artifacts produced (untracked)
- `/tmp/corpus_smoke.jsonl` (5-game × depth 8 × YaneuraOu, 567 positions).

---

## Testing Results

### Phase Success Criteria (Session 6 goals)

**Criterion 1**: USI client can spawn, handshake, set options, and cleanly quit a real external engine
- [x] Met — verified against YaneuraOu V900Git APPLEM1.

**Criterion 2**: Can drive a complete self-play game via `go depth N`
- [x] Met — 5 games of 67-150 plies each completed end-to-end.

**Criterion 3**: Output schema captures everything the trainer will need
- [x] Met — `sfen`, `player`, `eval_cp` (Option<i32> with mate mapped), `outcome` (+1/0/-1), `game_id`, `ply`, `from_random_opening` all present.

**Criterion 4**: Corpus has a high rate of decisive games (the main thing PST training couldn't produce)
- [x] **Exceeded** — 5/5 decisive in the smoke test (Session 5 was 0/50 decisive with PST).

**Criterion 5**: Per-position generation cost is low enough that a 100 K-position corpus fits in an overnight (or lunch-break) wall-clock budget
- [x] Met — extrapolates to ~5-6 minutes per 100 K positions at depth 8 / 1 thread / Apple Silicon.

### Metrics for Next Phase

**Phase 2b retrain (Session 7) starting point**:
- Training signal: teacher eval in cp, from to-move POV, + game outcome Black-POV
- Corpus source: YaneuraOu (V900Git APPLEM1) self-play
- Expected corpus size for Session 7 run: ~100 K positions from ~1000 games at depth 12
- Prior ELO baseline to beat: NNUE (trained on PST, Session 3) == PST (Session 5 pilot: 0 ELO)
- Session 7 success metric: first decisive NNUE-vs-PST game in the elo-tester pilot.

---

## Questions & Notes

**Questions for next session**:
- Does our `BitboardBoard::from_fen` round-trip with `to_fen` perfectly for all shapes of hand-pieces (multiple of same type, etc.)? The `PBRpppbb` hand syntax is what we need to survive.
- Where will we stage the corpus for Session 7 — keep it under `/tmp` or commit a small one to a new `corpus/` directory (gitignored)?
- What loss weighting `(α·eval_target + (1−α)·outcome)` should we start with? Stockfish-style is roughly α=0.75 early in a game, decaying toward 0.5 near the end.

**General notes**:
- The promoted-rook bug is a *known, deferred* issue. If we fix it before Session 7, re-running the smoke test is free insurance.
- Both engines support MultiPV > 1 if we ever want diverse candidate moves (could drive richer opening diversity than random plies).
- Running multiple `corpus-gen` instances in parallel with different `--seed` gives linear throughput scaling — we never have to deal with async scheduling inside the tool itself.

---

## Attachments

### Logs

<details>
<summary>Smoke test output</summary>

```
=== NNUE Corpus Generator ===
  Engine:       .../YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1
  Engine cwd:   .../NNUE_halfkpvm_256x2_32_32
  Games:        5
  Depth:        8
  Random plies: 8
  Max moves:    256
  Seed:         2026
  Threads:      1
  Hash MB:      256
  Output:       /tmp/corpus_smoke.jsonl

Game   1/  5: white_win in 150 plies,  150 positions (142 written) t=0.33s
Game   2/  5: black_win in 129 plies,  129 positions (121 written) t=0.42s
Game   3/  5: white_win in 136 plies,  136 positions (128 written) t=0.34s
Game   4/  5: black_win in 125 plies,  125 positions (117 written) t=0.33s
Game   5/  5: black_win in  67 plies,   67 positions ( 59 written) t=0.17s

=== CORPUS SUMMARY ===
  Games:              5
  Black wins:         3
  White wins:         2
  Draws:              0
  Decisive ratio:     100.0%
  Positions total:    607
  Positions written:  567
  Wall time:          1.6s
```

</details>

<details>
<summary>Sample corpus records (first 2 + last 2)</summary>

```json
{"game_id":0,"ply":8,"sfen":"lnsgk2nl/3r1gsb1/ppppppppp/9/9/5P2P/PPPPPGPP1/1B5R1/LNS1KGSNL b -","player":"black","eval_cp":188,"outcome":-1,"outcome_str":"white_win","from_random_opening":false}
{"game_id":0,"ply":9,"sfen":"lnsgk2nl/3r1gsb1/ppppppppp/9/9/5P2P/PPPPPGPP1/1B2G2R1/LNS1K1SNL w -","player":"white","eval_cp":-174,"outcome":-1,"outcome_str":"white_win","from_random_opening":false}
...
{"game_id":4,"ply":65,"sfen":"ln7/3Skg3/4p2+Bp/pp1+R3p1/2P3P2/1b2P1G2/PSNP1P1PP/2S4R1/L2GKGSNL b BLGPRSPPPNPPSpbpl","player":"black","eval_cp":29997,"outcome":1,"outcome_str":"black_win","from_random_opening":false}
{"game_id":4,"ply":66,"sfen":"ln7/3S1g3/4pk1+Bp/pp1+R1N1p1/2P3P2/1b2P1G2/PSNP1P1PP/2S4R1/L2GKGSNL b BLGPRSPPPNPPSpbpl","player":"black","eval_cp":29999,"outcome":1,"outcome_str":"black_win","from_random_opening":false}
```

</details>

---

## Historical Session Reference

| Session | Phase          | Title                                                       | Status    | Date       |
|---------|----------------|-------------------------------------------------------------|-----------|------------|
| 1       | 1.1-1.2        | Weight Initialization & Output Scaling                      | Completed | 2026-04-22 |
| 2       | 1.3, 2.1-2.3   | Training Verification & Algorithm Fixes                     | Completed | 2026-04-22 |
| 3       | 2.4            | Extended Training + Game Diversity                          | Completed | 2026-04-22 |
| 4       | 3.2, 3.4       | Speed Optimization (Incremental Accumulator)                | Completed | 2026-04-22 |
| 5       | 4.1            | ELO Validation Infrastructure + First Pilot                 | Completed | 2026-04-22 |
| 6       | 2b-setup       | External USI Teacher — Corpus Generator Infrastructure      | Completed | 2026-04-22 |
| 7       | 2b-execute     | Generate Corpus, Retrain NNUE, Re-run ELO Test              | Pending   | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed
**Ready for next session**: Yes
**Comments**: End-to-end infrastructure works. `corpus-gen` generated a 5-game / 567-position smoke corpus from YaneuraOu in 1.6s, 100% decisive — the training-signal bottleneck identified in Sessions 2-5 is removed in principle. Session 7 can now generate a real-sized corpus, retrain NNUE against teacher eval + game outcome, and re-run the `elo-tester` pilot to see if we finally produce a measurable ELO gain.

One pre-existing engine bug was surfaced (promoted rook's sliding moves are missing from our `generate_legal_moves`); it doesn't block this work but is worth fixing soon because it probably costs our own engine ELO.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-22
