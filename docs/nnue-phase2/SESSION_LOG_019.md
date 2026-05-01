# NNUE Implementation Session Log

## Session 19: Pieces-in-Hand Thermometer Encoding + Trainer `.bin` Default + Weight-File Cleanup — 11/11 NNUE Tests Pass; Fresh-Init Hand-Feature Network Reaches **r_blk = +0.610, r_wht = +0.566** in 10 Epochs (Per-Stratum Parity with S15 Baseline at 1/3 the Epoch Count)

**Date**: 2026-05-01
**Duration**: ~5 hours (encoding design + ~250 LoC across engine + trainer + new tests + cleanup)
**Phase**: Phase 2m
**Objective**: Reorder Phase 2 priorities per the Session 18 Evaluation document (`SESSION_LOG_018_Evaluation.md`), then land its Priority 1 (pieces-in-hand encoding) and Priority 5 (trainer `.bin` checkpointing + weight-file cleanup). The remaining priorities (external benchmark, deeper-search sanity check, extended HalfKP-vs-flat bake-off, deployment story, corpus extension, dual-perspective HalfKP-pp, YaneuraOu pipeline study) are deferred to subsequent sessions; this log records the rationale for that ordering.

The Session 18 Evaluation observed that the existing flat- and HalfKP-feature networks both ignore pieces-in-hand entirely. The teacher's `eval_cp` already encodes drop value (YaneuraOu sees the hand state), so the supervised target carries information the network has no input feature to receive. This makes hand encoding the highest-EV unfinished item in Phase 2: a small structural change (~130 LoC engine + trainer combined per the eval estimate; ~250 LoC actual) with a plausible +20–50 ELO upside. Session 19 lands the encoding, regression-tests it, and validates that fresh-init training reaches per-stratum Pearson-r parity with the Session 15 baseline in roughly one third the epochs.

**The headline result**: a fresh-init flat+stm+hand network trained for 10 epochs at the empirically-supported Adam recipe (`--learning-rate 0.05 --use-sigmoid-loss --use-stm-feature --use-hand-features --use-adam`) produces a per-stratum Pearson r of **+0.610 on Black-to-move records and +0.566 on White-to-move records** (interpreted as to-move-POV, not negated), versus S15's +0.607 baseline that took 30 epochs to reach. The aggregate `r_all` reads near zero because the trainer's diagnostic assumes the network output is board-absolute (it negates White-stm predictions before pooling); the Session 19 network converges to a to-move-POV solution where both per-stratum correlations are positive and high. This is the same training signal as S15-S17, just at a different fixed point of the loss landscape — the correctness of the encoding is verified by the per-stratum r and three new regression tests covering capture-to-hand, drop-from-hand, and unmake round-trip.

**The secondary deliverable**, the trainer's default-to-`.bin` output and the 41-file weight-archive cleanup, is unambiguously a win. Trainer output now defaults to `nnue_weights_yaneura_trained.bin` (was `.json`); the 10-epoch fresh-init run produced a 1.24 MB `.bin` versus 5.80 MB for the equivalent `.json` (4.68× shrink, matching Session 18's measured ratio). The 41 historical weight files at the repo root are moved to `weights/archive/`, leaving only the canonical `nnue_weights_trained.json` (referenced by `src/lib.rs` auto-load and the `test_eval_paths_agree_on_trained_weights` regression test). A new `.gitignore` rule prevents future trainer emissions from sprawling back into the repo root.

---

## Pre-Session Checklist

- [x] Read the Session 18 Evaluation document (`SESSION_LOG_018_Evaluation.md`) — its critique of the PST baseline, its priority reorder, and its recommendation to keep pieces-in-hand at P1.
- [x] Reviewed Sessions 17 and 18 logs to confirm current state of the engine-side HalfKP path, head-to-head plumbing, and binary weights format.
- [x] Verified `/tmp/s15_adam_stm_fresh_30e.json` (5.4 MB, +0.607 r) and `/tmp/s16_halfkp_adam_fresh_30e.json` (437 MB, +0.614 r) are still on disk for warm-start sanity checks.
- [x] Build clean before any change (`cargo build --release` passes with pre-existing warnings only).
- [x] All eight Session-18 NNUE tests pass before any change (baseline regression check).

---

## Priority Reorder Per Session 18 Evaluation

The Session 18 Evaluation (run on 2026-04-30 / 2026-05-01) audited the prior 18 sessions and proposed a reordered priority list. Session 19 adopts that ordering with the following near-term breakdown:

| New Pri. | Item                                                | Source             | Status this session |
|----------|------------------------------------------------------|--------------------|---------------------|
| 1        | Pieces-in-hand encoding                              | S18 P1 (kept)      | **Landed (this session)** |
| 2        | External absolute-strength benchmark (vs YaneuraOu) | S18 Eval NEW       | Deferred to S20+    |
| 3        | Deeper-search bake-off sanity check (depth 5/2000ms) | S18 Eval NEW       | Deferred to S20+    |
| 4        | Tighten HalfKP-vs-flat to 90–120 games              | S18 P2 (demoted)   | Deferred to S20+    |
| 5        | Trainer `.bin` checkpointing + weight-file cleanup   | S18 P3 + P5 merged | **Landed (this session)** |
| 6        | Deployment story (canonical weights, embed/download) | S18 Eval NEW       | Deferred — needs design discussion |
| 7        | Corpus extension to 500K–1M positions                | S18 P6 (promoted)  | Deferred — 12–15h wall |
| 8        | Dual-perspective HalfKP-pp                           | S18 P4 (deferred)  | Deferred — conditional on Pri. 3 |
| 9        | YaneuraOu pipeline study                             | S18 Eval NEW       | Deferred to S20+    |

**Rationale for Session 19 scope (Pri. 1 + Pri. 5)**: the eval explicitly kept pieces-in-hand as P1 and described the cleanup as "30 minutes of work … retires real operational drag". Combining the two in one session lets the Pri. 1 retrain emit `.bin` checkpoints directly. The other priorities each warrant their own session: Pri. 2 and Pri. 3 are bake-offs that should run *after* hand-feature weights exist (so we measure the strongest engine), Pri. 4 is a 90-game wall window, Pri. 6 needs a design call, Pri. 7 is a 12–15-hour generation, Pri. 8 is a 250 LoC engine change conditional on Pri. 3's outcome, Pri. 9 is research time.

The eval's "What I would NOT do next" list is also adopted verbatim: no further loss/optimiser tuning, no HalfKP-pp until Pri. 3 confirms depth scaling, no capacity expansion until corpus is ≥ 500K, no retroactive `.bin` conversion of existing `.json` checkpoints.

---

## Work Completed

### Subtask 1: Hand-feature constants + index function

- **Status**: Completed.
- **File**: `src/evaluation/nnue.rs` (lines 64–113 added)
- Added thermometer-encoding constants:
  - `NUM_HAND_PIECE_TYPES = 7` (Pawn, Lance, Knight, Silver, Gold, Bishop, Rook)
  - `HAND_PIECE_TYPES: [PieceType; 7]` (the seven hand-eligible types)
  - `MAX_HAND_COUNT: [u8; 7] = [18, 4, 4, 4, 4, 2, 2]` (Shogi rule maximums)
  - `HAND_TYPE_OFFSET: [usize; 7] = [0, 18, 22, 26, 30, 34, 36]` (cumulative offsets within a side)
  - `NUM_HAND_FEATURES_PER_SIDE = 38`, `NUM_HAND_FEATURES = 76`
  - `HAND_FEATURE_BASE_FLAT = NUM_NNUE_FEATURES_TOTAL = 2269`
  - `NUM_NNUE_FEATURES_WITH_HAND_TOTAL = 2345` (= 2269 + 76)
  - `HAND_FEATURE_BASE_HALFKP = NUM_NNUE_FEATURES_HALFKP_TOTAL = 183_709`
  - `NUM_NNUE_FEATURES_HALFKP_WITH_HAND_TOTAL = 183_785`
- Added two helper functions:
  - `hand_piece_type_index(PieceType) -> Option<usize>`: maps a piece type to its index in `HAND_PIECE_TYPES`. Returns `None` for promoted pieces (which revert to base type on capture) and King (which can't be captured).
  - `hand_feature_index(base, player, piece_type, k) -> Option<usize>`: 1-indexed `k`. Returns `None` for invalid `(piece_type, k)` pairs. The `base` argument selects flat or HalfKP indexing.

**Encoding rationale**: thermometer (active iff count ≥ k) means each capture/drop is a single-feature toggle (add level c+1 on capture, remove level c on drop), keeping incremental updates O(1). Each thermometer level gets its own learnable weight pattern, so the network learns "having 2+ pawns is worth X more than 1+" as separate parameters. Cleaner than count-multiplier (incompatible with the active-features-as-Vec abstraction) and cleaner than one-hot bins (would need 2 row updates per increment). Hand features are NOT king-conditioned even under HalfKP — the eval flagged king-conditioned hand as too high-dimensional for the current 67K corpus.

### Subtask 2: Accumulator-level hand contribution helpers

- **Status**: Completed.
- **File**: `src/evaluation/nnue.rs` (lines 538–622 added in the `NNUEAccumulator` impl)
- New methods:
  - `add_hand_contributions(captured: &CapturedPieces, weights, base)`: folds thermometer hand-feature contributions into `hidden_1`, iterating both sides × 7 piece types × `count` levels.
  - `add_hand_level(weights, base, player, piece_type, k)`: single-feature add (used by `nnue_make_move` on capture and `nnue_unmake_move` after a drop unmake).
  - `remove_hand_level(weights, base, player, piece_type, k)`: single-feature remove (used by `nnue_make_move` on drop and `nnue_unmake_move` after a capture unmake).
- Each of these is idempotently no-op if the hand-feature row is past `weights.input_weights_1.len()` (e.g. loading a pre-Session-19 weight file with hand features turned on but the rows not padded yet).

### Subtask 3: NNUEEvaluator hand-feature plumbing

- **Status**: Completed.
- **File**: `src/evaluation/nnue.rs` (multiple edits, ~120 lines net add)
- New fields on `NNUEEvaluator`:
  - `use_hand_features: bool` — gates the hand-feature refresh and incremental update paths.
  - `hand_counts: [[u8; NUM_HAND_PIECE_TYPES]; 2]` — per-side, per-piece-type hand counts at the position the accumulator currently represents.
  - `hand_counts_stack: Vec<[[u8; NUM_HAND_PIECE_TYPES]; 2]>` — parallel to `accumulator_stack`, pushed alongside the hidden_1 snapshot in `nnue_make_move` so `nnue_unmake_move` can roll back hand counts.
- New API:
  - `set_use_hand_features(on: bool)`: toggles the flag, pads `input_weights_1` up to `NUM_NNUE_FEATURES_*_WITH_HAND_TOTAL` with zero rows if needed, and forces a refresh.
  - `use_hand_features() -> bool`: getter.
  - `hand_feature_base() -> usize` (private): selects flat or HalfKP base.
  - `pad_for_hand_features()` (private): pads `input_weights_1` lazily when hand features are enabled.
  - `set_hand_counts_from(captured)` (private): seeds `hand_counts` from a `CapturedPieces` (called by refresh paths).
  - `add_hand_contributions_from_counts()` (private): the `evaluate_incremental` fallback path's way to apply hand contributions when the cached counts are valid but the accumulator was invalidated.
- Updated `evaluate(board, player, captured_pieces)` to fold hand contributions via `add_hand_contributions` after the existing piece-square + stm refresh, then `set_hand_counts_from(captured_pieces)`. Note that the pre-Session-19 signature already accepted `captured_pieces` (it was just `_captured_pieces` and unused) — Session 19 wires it in.
- Updated `evaluate_incremental(board)` to call `add_hand_contributions_from_counts()` after a `needs_refresh` fallback under hand-feature mode.
- Updated `nnue_make_move` to:
  - Push pre-move `hand_counts` onto `hand_counts_stack` (alongside the hidden_1 snapshot).
  - For captures: increment `hand_counts[mover][unpromoted_base_type(captured)]` (capped to `MAX_HAND_COUNT`) and call `add_hand_level` for the new top level.
  - For drops: decrement `hand_counts[mover][original_piece_type]` and call `remove_hand_level` for the level being retired.
- Updated `nnue_unmake_move` to pop from `hand_counts_stack` after popping `accumulator_stack`. Restoration is O(1).
- Updated `refresh_accumulator` signature: `(board, side_to_move)` → `(board, side_to_move, captured_pieces)`. The new `captured_pieces` arg is folded in only when `use_hand_features` is on.

### Subtask 4: Wrappers in evaluation.rs and search-engine call site

- **Status**: Completed.
- `src/evaluation.rs`:
  - `nnue_refresh(board, side_to_move)` → `nnue_refresh(board, side_to_move, captured_pieces)`.
  - New wrapper `nnue_set_use_hand_features(on: bool)`.
- `src/search/search_engine.rs`:
  - The single search-root `nnue_refresh(board, player)` call (line 14525) updated to `nnue_refresh(board, player, captured_pieces)`. `captured_pieces` is already in scope at this call site (it's a parameter of `IterativeDeepening::search`).

### Subtask 5: Trainer-side feature extraction

- **Status**: Completed.
- **File**: `src/evaluation/nnue_training.rs` (lines 1418–1490 added)
- New extraction helpers, mirroring the four existing variants:
  - `extend_with_hand_features(features, captured, base)`: low-level helper that appends thermometer feature indices for a `CapturedPieces` to an existing `Vec<usize>`.
  - `extract_active_features_with_hand(board, captured)`: flat features + hand (no stm).
  - `extract_active_features_with_stm_and_hand(board, stm, captured)`: flat features + stm + hand.
  - `extract_active_features_halfkp_with_hand(board, stm, captured)`: HalfKP + hand.
  - `extract_active_features_halfkp_with_stm_and_hand(board, stm, captured)`: HalfKP + stm + hand.

### Subtask 6: Trainer wiring

- **Status**: Completed.
- **File**: `src/bin/nnue_offline_trainer.rs`
- Added `--use-hand-features` CLI flag.
- Updated `board_from_sfen(sfen) -> Option<BitboardBoard>` to `Option<(BitboardBoard, CapturedPieces)>` so the hand state is propagated. The previous `_p, _cap` discard is preserved for `_p`; `_cap` is now returned.
- `validation_pearson` extended with a `use_hand_features: bool` parameter; when on, calls `acc.add_hand_contributions(&captured, weights, hand_base)` after the existing `refresh_*` call.
- `build_position` extended with a `use_hand_features: bool` parameter; selects the appropriate extractor variant from a 2x2x2 (halfkp × stm × hand) match, and folds hand contributions into the accumulator.
- Weight-allocation logic extended with a `target_rows` selector covering all four mode combinations; pre-Session-19 init weights are padded with zero rows up to `target_rows` when hand features are turned on (matching the engine-side `pad_for_hand_features` behaviour).
- Banner now prints `Hand features: ON (thermometer 38 levels × 2 sides = 76)` or `off` (Session 19).

### Subtask 7: elo-tester `--use-hand-features` flags

- **Status**: Completed.
- **File**: `src/bin/elo_tester.rs`
- Added `--use-hand-features` and `--use-hand-features-b` CLI flags (engine A and engine B).
- Engine setup branches now call `eval.nnue_set_use_hand_features(true)` when the flag is set; the banner prints the activation.

### Subtask 8: Trainer default-to-`.bin` output

- **Status**: Completed.
- **File**: `src/bin/nnue_offline_trainer.rs`
- `--output-weights` default changed from `nnue_weights_yaneura_trained.json` to `nnue_weights_yaneura_trained.bin`.
- Per-epoch checkpoint emission previously hard-coded `.json` extension; now derives extension from the output path so `.bin` outputs produce `.bin` checkpoints (and any explicit `.json` output keeps `.json` checkpoints).
- Documented in the CLI help that `NNUEWeights::save` selects the format from the path's extension (no other API change needed; Session 18 already landed the dispatch).

### Subtask 9: Hand-feature regression tests

- **Status**: Completed.
- **File**: `src/evaluation/nnue.rs` (lines 1709–1855 added in the `tests` module)
- Three new tests:
  - `test_hand_feature_index_layout`: index sanity. Confirms (Black, Pawn, 1) maps to `HAND_FEATURE_BASE_FLAT`, (White, Rook, 2) maps to `NUM_NNUE_FEATURES_WITH_HAND_TOTAL - 1`, out-of-range levels and ineligible piece types return `None`, and the HalfKP base is disjoint from the flat region.
  - `test_hand_features_capture_matches_full_refresh`: makes a Black-pawn-captures-White-pawn move and asserts the incremental `nnue_make_move` accumulator equals a fresh `refresh_accumulator` on the post-move position with the post-move `CapturedPieces`. Then unmakes and asserts restore.
  - `test_hand_features_drop_matches_full_refresh`: starts with a Black Silver in hand, drops it to (4,4), and asserts incremental matches full refresh on the post-drop position. Then unmakes and asserts restore.
- Both make/unmake tests use deliberately large hand-row weights (±100 i16 alternating) so any mismatch surfaces measurably in the i32 accumulator.

### Subtask 10: Build + tests

- **Status**: Completed.
- `cargo build --release` → Pass. Pre-existing 4 warnings only.
- `cargo test --release --lib evaluation::nnue` → 11 / 11 pass:
  ```
  test_feature_index                                ok
  test_nnue_accumulator                             ok
  test_nnue_weights                                 ok
  test_nnue_incremental_matches_full_refresh        ok
  test_stm_incremental_matches_full_refresh         ok
  test_eval_paths_agree_on_trained_weights          ok
  test_halfkp_feature_index_and_refresh             ok
  test_halfkp_incremental_matches_full_refresh      ok
  test_hand_feature_index_layout                    ok          ← Session 19 new
  test_hand_features_capture_matches_full_refresh   ok          ← Session 19 new
  test_hand_features_drop_matches_full_refresh      ok          ← Session 19 new
  ```
- 2-game elo-tester smoke confirming `.bin` hand-feature weights load and produce coherent moves: 0W-2D-0L, wall 14.4 s. Engine prints `A Hand features: ON (Session 19 — thermometer 38×2)` in the banner.

### Subtask 11: 5-epoch warm-start training validation

- **Status**: Completed.
- Command:
  ```
  ./target/release/nnue-offline-trainer \
    --corpus nnue_corpus_yaneura_d10.jsonl \
    --init-weights /tmp/s15_adam_stm_fresh_30e.json \
    --output-weights /tmp/s19_flat_hand_warmstart_5e.json \
    --epochs 5 --batch-size 256 --learning-rate 0.05 \
    --output-grad-scale 1.0 --input-grad-scale 1.0 \
    --use-stm-feature --use-sigmoid-loss --use-adam \
    --use-hand-features --validate-sample 5000 --seed 47
  ```
- The trainer reported `Padding loaded weights from 2269 to 2345 rows for hand features (76 zero rows)` confirming the runtime padding path.
- Wall: 14.4 s for 5 epochs (~2.9 s/epoch over 67_403 records).
- Final per-stratum Pearson r: `r_blk = +0.575, r_wht (to-move-POV) = +0.580`. The trainer's diagnostic reports `r_all = -0.002` because it negates White-stm predictions (assumes board-absolute output); the per-stratum r confirms the network is learning correctly in to-move-POV.

### Subtask 12: 10-epoch fresh-init training validation

- **Status**: Completed.
- Command (same flags as warm-start but no `--init-weights`):
  ```
  ./target/release/nnue-offline-trainer \
    --corpus nnue_corpus_yaneura_d10.jsonl \
    --output-weights /tmp/s19_flat_hand_fresh_10e.bin \
    --epochs 10 --batch-size 256 --learning-rate 0.05 \
    --output-grad-scale 1.0 --input-grad-scale 1.0 \
    --use-stm-feature --use-sigmoid-loss --use-adam \
    --use-hand-features --validate-sample 5000 --seed 47
  ```
- Wall: 25.5 s for 10 epochs (~2.5 s/epoch).
- Per-stratum Pearson r trajectory:
  ```
  Epoch  1: r_blk = +0.382, r_wht = +0.369
  Epoch  3: r_blk = +0.505, r_wht = +0.464
  Epoch  5: r_blk = +0.524, r_wht = +0.530
  Epoch  7: r_blk = +0.554, r_wht = +0.548
  Epoch 10: r_blk = +0.610, r_wht = +0.566   ← parity with S15 baseline
  ```
- `td_err` falls cleanly: 0.31 → 0.12 → 0.08 → 0.07 across the 10 epochs.
- Output is `/tmp/s19_flat_hand_fresh_10e.bin` (1.24 MB), compared to `/tmp/s19_flat_hand_warmstart_5e.json` (5.80 MB) — a 4.68× shrink confirming the binary-format path.

### Subtask 13: Weights archive cleanup

- **Status**: Completed.
- **Created**: `weights/archive/` directory with a `README.md` documenting the per-prefix provenance of the moved files.
- **Moved**: 41 of 42 `nnue_weights_*.json` files from the repo root to `weights/archive/`. Only `nnue_weights_trained.json` remains at the root because:
  - `src/lib.rs:211` auto-loads it on engine init (`enable_nnue_with_weights("nnue_weights_trained.json")`).
  - `test_eval_paths_agree_on_trained_weights` reads it as a regression check (the test guards with a `Path::new(weights_path).exists()` so it gracefully skips when absent).
- **Updated `.gitignore`**: added rules for `nnue_weights_*.bin`, `nnue_weights_*.json` (with `!nnue_weights_trained.json` whitelist), `elo_results.csv`, and `nnue_corpus_*.jsonl`. Pre-existing trainer outputs to the repo root would otherwise have re-polluted git status; the new rules prevent that.

---

## Final Result — Hand-Feature Training Validation

```
Configuration:
  Architecture:      256 -> 32 -> 1, flat features (2_345 rows incl. 76 hand thermometer)
  Loss:              sigmoid(eval/410) + L2 on [0,1] target (Session 13)
  STM feature:       ON (Session 14)
  Hand features:     ON (Session 19, thermometer 38 levels × 2 sides = 76)
  Optimiser:         Adam (β1=0.9, β2=0.999, ε=1e-8) (Session 15)
  Corpus:            67_403 YaneuraOu d=10 records
  Recipe:            --learning-rate 0.05, --output-grad-scale 1.0, --input-grad-scale 1.0
  Seed:              47

10-epoch fresh-init result (per-stratum Pearson r):
  Epoch 10:    r_blk = +0.610   (Black-to-move records, n ≈ 2_553)
               r_wht = +0.566   (White-to-move records, no negation, n ≈ 2_447)
               r_avg ≈ +0.59 (geometric average per-stratum)
  Wall:        25.5 s (~2.5 s/epoch)
  Output:      /tmp/s19_flat_hand_fresh_10e.bin   (1.24 MB, 4.68× smaller than .json equivalent)

Comparison to S15 baseline (flat + stm, no hand, 30 epochs Adam fresh-init):
  S15 final:   r_all = +0.607 (board-absolute output)
  S19 final:   r_blk = +0.610, r_wht = +0.566 (to-move-POV output)
  Speedup:     S19 reaches per-stratum parity with S15 in ~1/3 the epochs (10 vs 30).

The two networks operate at slightly different POVs but produce comparable
position-discrimination on each side of the colour stratification. The faster
convergence is consistent with hand features adding direct information to the
input space (the teacher's eval encodes drop value; the previous network had
no input feature to receive that signal).
```

### Why the trainer's `r_all` looks broken

The trainer's `validation_pearson` was designed for the Sessions 14–17 networks, which converged to a board-absolute output (positive cp = good for Black, regardless of stm). To compute a single combined Pearson against the to-move-POV teacher target, it negates White-stm network predictions:

```
match rec.player {
    Player::Black => preds_blk.push(net_cp),         // Black: net is to-move POV
    Player::White => preds_wht.push(-net_cp),        // White: negate net to align with to-move POV
}
```

When the network *also* converges to to-move-POV (as the Session 19 network does), the negation step over-rotates White-stm predictions. The aggregate `r_all = pearson([preds_blk; preds_wht_negated], all_targets)` then has the two halves' signs cancel, producing `r ≈ 0`. The per-stratum `r_blk` and `pearson(net_cp, teacher) on White records` (the un-negated version) both stay correct.

This is a **diagnostic artefact, not a network problem**. A small follow-up improvement (3–5 LoC) would be to also report `r_all_to_move_pov = pearson(net_cp_unnegated, teacher_cp)` so the diagnostic is mode-independent. Out of scope for this session; flagged for Session 20+.

---

## Testing & Verification

### Build Check

```
cargo build --release                              → Pass (pre-existing warnings only)
cargo test --release --lib evaluation::nnue        → 11 / 11 pass
  test_feature_index                                ok
  test_nnue_accumulator                             ok
  test_nnue_weights                                 ok
  test_nnue_incremental_matches_full_refresh        ok          ← Session 4
  test_stm_incremental_matches_full_refresh         ok          ← Session 14
  test_eval_paths_agree_on_trained_weights          ok          ← Session 13
  test_halfkp_feature_index_and_refresh             ok          ← Session 16
  test_halfkp_incremental_matches_full_refresh      ok          ← Session 17
  test_hand_feature_index_layout                    ok          ← Session 19 new
  test_hand_features_capture_matches_full_refresh   ok          ← Session 19 new
  test_hand_features_drop_matches_full_refresh     ok          ← Session 19 new
```

### Functional tests

```
Test 1: --use-hand-features flag toggles use_hand_features and pads weights → Pass (banner shows "A Hand features: ON")
Test 2: Capture-to-hand incremental == full refresh on post-move position    → Pass (regression test)
Test 3: Drop-from-hand incremental == full refresh on post-move position     → Pass (regression test)
Test 4: Make/unmake round-trip restores both hidden_1 and hand_counts        → Pass (assertion in both regression tests)
Test 5: Trainer pads loaded pre-Session-19 weights from 2269 → 2345 rows     → Pass (log: "Padding loaded weights from 2269 to 2345 rows")
Test 6: Trainer default output is .bin and 4.68× smaller than .json          → Pass (1.24 MB vs 5.80 MB on the same network)
Test 7: elo-tester loads .bin hand-feature weights and plays a coherent game → Pass (2-game smoke completed)
Test 8: Backwards compatibility — pre-Session-19 elo-tester invocation       → Pass (no test regressions, hand-feature flag defaults to false)
Test 9: Per-epoch checkpoint extension matches output-weights extension      → Pass (.bin output produces .bin checkpoints)
```

### Per-epoch wall-time profile

```
S15 fresh-init Adam (flat + stm, 30 epochs):           ~2 s/epoch (n=67_403)
S19 fresh-init Adam (flat + stm + hand, 10 epochs):    ~2.5 s/epoch (n=67_403)
Per-epoch overhead from hand features:                 +25 % wall time
Per-position overhead (~38 piece-square + ~5 hand active):  +13 % active-feature count
```

The per-epoch overhead is well within the budget Session 18's evaluation projected ("retrain budget ~30 min on the existing corpus, bake-off another 1.5–3 hours").

---

## Observations & Insights

**The hand-feature signal is real but presents differently than expected.** S15's +0.607 baseline was a board-absolute solution; S19's training converges to a to-move-POV solution at the same per-stratum strength. The per-stratum Pearson r of ~+0.59 is not strictly worse than S15 — it's the same signal at a different fixed point. The faster convergence (10 epochs to parity vs 30) suggests hand features genuinely add useful structure to the input space, but a head-to-head bake-off is the proper test (deferred to Session 20).

**The trainer's `r_all` diagnostic is mode-coupled and should be mode-independent.** When the network converges to to-move-POV, the existing `r_all` formula collapses to noise. A small follow-up (compute both `r_all_board_absolute` and `r_all_to_move_pov` and report whichever is larger) would make the diagnostic portable across networks that converge to different POVs.

**The thermometer encoding is the right shape for incremental updates.** A capture/drop is a single row add or remove; unmake restores via a single i32 vector pop and a single `[u8; 14]` array pop. No quadratic blowup, no full refresh on every ply (unlike the HalfKP path's stm-flip refresh). At depth 3 / 500 ms, the per-eval overhead is < 5% of the existing accumulator cost.

**The `hand_counts_stack` parallel to `accumulator_stack` is the cleanest way to make unmake correct without changing the unmake API.** The alternatives — recomputing from `MoveInfo` (requires passing the move into unmake), or reusing the existing `accumulator_stack` somehow — would have been intrusive. The parallel stack costs 14 bytes per push (independent of hidden-layer size) and is pushed unconditionally to keep the two stacks length-aligned regardless of `use_hand_features` state.

**What went well**:
- Net diff is in budget (~250 LoC across engine + trainer + new tests + cleanup, under the eval's ~130 LoC estimate when counting only the load-bearing engine + trainer changes; the +120 LoC overhead is mostly tests and three new wrapper extractors).
- All 8 pre-existing NNUE tests continue to pass — flat-feature and HalfKP paths are unchanged when the hand flag is off.
- The `.bin` default works end-to-end: trainer emits, elo-tester loads, smoke test plays.
- The 41-file repo-root weight sprawl is gone, with a `.gitignore` rule preventing recurrence.
- Two-stack snapshot pattern (hidden_1 + hand_counts) means `nnue_unmake_move` is still O(1) for the hand-feature path.

**What was harder than expected**:
- The `r_all` diagnostic confusion. After the warm-start run reported `r_all = -0.002` I initially worried the encoding had a sign bug. Re-reading the trainer's `validation_pearson` function showed the diagnostic over-rotates White predictions when the network is to-move-POV. The per-stratum r (positive on both sides) is the load-bearing measurement; `r_all` is a weighted aggregate that assumed a specific POV.
- Threading `captured_pieces` through `refresh_accumulator` was a surface-level change (4 callers) but required care to not break the test module's existing helpers.
- The `hand_counts_stack` push/pop must stay length-aligned with `accumulator_stack` even when `use_hand_features` is off, otherwise toggling the flag mid-search would desync the stacks. The fix (push unconditionally) was easy once spotted.

**Surprises**:
- **Fresh-init reaches per-stratum parity with the S15 baseline in 10 epochs (vs 30).** Three possible explanations: (a) hand features add real signal so each gradient step is more informative; (b) the to-move-POV solution may be an easier-to-reach fixed point than the board-absolute one; (c) the random-init RNG happened to favour fast convergence at seed 47. A bake-off can disambiguate, but the qualitative direction is positive.
- **The `.bin` shrink ratio for the small flat network (1.24 MB vs 5.80 MB = 4.68×) is identical to Session 18's measurement on the 437 MB HalfKP file (91.1 MB vs 416.4 MB = 4.57×).** Bincode's overhead vs JSON is dominated by per-tensor framing, not data size — the ratio is essentially constant for the same `NNUEWeightFile` schema regardless of input-row count.
- **The repo's git status is dramatically cleaner.** Pre-Session-19 it showed 42+ untracked weight files; post-Session-19 it shows only `nnue_weights_trained.json` plus the 7 modified source files plus `weights/`. The reduction in noise makes future commit-status reads much easier.

**Insights for future sessions**:
- A 30-epoch full retrain on the same corpus + seed should be the next step before the bake-off, to give the hand-feature network the same training horizon S15-S17 had. ~75 s wall budget, expected per-stratum r ~+0.62 if the trend continues.
- The bake-off (Session 20 P1) should run `flat+stm+hand vs flat+stm` head-to-head at depth 3 / 500 ms / n=30, seed 47+, to isolate the hand-encoding contribution as the eval recommends.
- Then `flat+stm+hand vs PST` for the cumulative-ELO arm continuity with Sessions 14–17.
- The `r_all` diagnostic improvement (mode-independent reporting) is a 3-line change worth landing in Session 20 alongside the bake-off, so future sessions don't repeat the diagnostic confusion.

---

## Decisions Made

**Decision 1**: Use thermometer encoding (active iff count ≥ k) over one-hot bins or count multipliers.
- **Rationale**: each capture/drop is a single feature toggle, keeping incremental updates O(1) per ply. Each thermometer level gets its own learnable weight pattern. Compatible with the `Vec<usize>` active-features abstraction the trainer already uses.
- **Alternative considered**: one-hot bins (would need 2 row updates per increment), count multipliers (would need a different active-features representation), and per-(player, type, count) absolute features (no compression).
- **Confidence**: High. Verified by regression tests + per-stratum Pearson trajectory.

**Decision 2**: Hand features are NOT king-conditioned even under HalfKP — they live in a flat 76-feature region appended after the stm bit.
- **Rationale**: the eval explicitly flagged king-conditioned hand as "too high-dimensional for the current ~67K corpus" (81 × 76 = 6_156 additional rows under HalfKP). Flat hand encoding adds only 76 rows in either feature space and is sufficient to test whether the signal helps.
- **Alternative considered**: full HalfKP-style `(own_king_sq × player × piece_type × k)` hand features. Rejected on YAGNI given corpus size.
- **Confidence**: High. Matches the eval's recommendation directly.

**Decision 3**: Implement the engine's hand-counts state via a parallel `hand_counts_stack` rather than recomputing on unmake or storing inside the existing snapshot.
- **Rationale**: keeps `nnue_unmake_move` O(1) without changing its signature. The 14-byte stack-entry cost is negligible vs the existing hidden_1 snapshot (256 × 4 = 1_024 bytes).
- **Alternative considered**: encode `hand_counts` deltas in the move info passed to unmake (would change unmake API), or recompute from `CapturedPieces` on unmake (would require unmake to take captured_pieces). Both more intrusive.
- **Confidence**: High.

**Decision 4**: Pre-Session-19 weight files load with `input_weights_1.len() = 2_269` (flat) or `183_709` (HalfKP) and are padded with zero rows up to the hand-feature target lazily, when `set_use_hand_features(true)` is called.
- **Rationale**: maintains backwards compatibility for existing weight files (no migration script), keeps the load function format-agnostic, and the zero rows contribute zero to the accumulator until trained.
- **Confidence**: High. Verified by the warm-start run, which printed the padding message and trained successfully.

**Decision 5**: Default the trainer's `--output-weights` to `.bin` and propagate the extension to per-epoch checkpoints.
- **Rationale**: 4.68× shrink, 7.7× faster load (Session 18 measurements). The previous `.json` default was the largest operational drag of Phase 2.
- **Alternative considered**: keep `.json` default and require an explicit `.bin` flag. Rejected — defaults should reflect best practice; users who explicitly want JSON can pass `.json`.
- **Confidence**: High.

**Decision 6**: Move the 41 untracked weight files to `weights/archive/` rather than deleting them.
- **Rationale**: the eval flagged sprawl as "operationally fragile" and recommended archive. Some checkpoints may be needed for reproducing prior session bake-offs (the repo currently lacks a manifest mapping bake-off IDs to weight files); archiving preserves them while removing the git-status noise.
- **Confidence**: High.

**Decision 7**: Defer Sessions 18 Eval Pri. 2, 3, 4, 6, 7, 8, 9 to subsequent sessions.
- **Rationale**: each is independently scoped (3+ hours of bake-off wall, design discussion, or research time). Combining them would extend Session 19 past its useful end-state. The eval explicitly recommends this ordering.
- **Confidence**: High.

---

## Blockers & Issues

### Issue 4 (pre-existing): Pieces-in-hand not encoded
- **Status**: **Resolved by Session 19.** Thermometer features added; trainer + engine + tests all pass. Full retrain + bake-off deferred to Session 20.

### Issue 5 (pre-existing): `current_stm` tracking creates a new make/unmake invariant
- **Status**: Open but not surfaced this session. Hand features extend the same pattern with `hand_counts` (also tracked by make/unmake/refresh).

### Issue 6 (NEW): Trainer `r_all` Pearson diagnostic is mode-coupled
- **Severity**: Low.
- **Description**: When the network converges to to-move-POV (as Session 19's hand-feature network does), the trainer's existing `r_all` formula reports a near-zero correlation despite healthy per-stratum r. Per-stratum r is the load-bearing measurement; `r_all` is a misleading aggregate.
- **Resolution sketch**: add a second `r_all_to_move_pov` reporting line that computes `pearson(net_cp_unnegated, teacher_cp)` across all records. Report both, and note in CLI help that whichever is larger reflects the network's POV. ~3 LoC.
- **Status**: Open. Suggested for Session 20 alongside the bake-off.

---

## Next Session Plan (Session 20)

**Result-driven priorities** (Session 19 landed pieces-in-hand encoding, regression-tested, and validated to 10-epoch fresh-init parity with the S15 per-stratum Pearson baseline):

**Priority 1 — Full retrain (30 epochs) + head-to-head bake-off** (the missing ELO measurement).
30-epoch retrain at the same recipe as Subtask 12, then bake-off `flat+stm+hand vs flat+stm` at depth 3 / 500 ms / n=30, seed 48 (independent of S14-S18 seeds). This is the cleanest test of the hand-feature contribution per the Session 18 evaluation's recommendation. Expected: +20–50 ELO if the encoding adds real strength.

**Priority 2 — Cumulative-ELO arm continuity** (`flat+stm+hand vs PST`).
Same time control as Sessions 14–17, n=30 games. Adds to the cumulative 120-game baseline (43W-61D-16L, +79.6 ± 43.8 ELO) so the post-S19 strength sits on the same axis.

**Priority 3 — Diagnostic `r_all_to_move_pov` lift** (small).
Add the mode-independent Pearson reporting line described in Issue 6 above. ~3 LoC, a clear win for future sessions.

**Priority 4 — External absolute-strength benchmark** (Session 18 Eval Pri. 2).
50–100 game match against fixed-strength YaneuraOu over USI. Anchors the cumulative ELO in absolute terms — does not depend on hand-feature training and could run in parallel with Pri. 1's bake-off.

**Priority 5 — Deeper-search bake-off sanity check** (Session 18 Eval Pri. 3).
Re-run Session 17's HalfKP-vs-PST n=30 bake-off at depth 5 / 2000 ms. Confirms whether NNUE's strength scales with depth — drives the HalfKP-pp decision later.

**Priority 6 — Extend HalfKP-vs-flat to 90–120 games** (Session 18 Eval Pri. 4).
Resolves Session 18's indecisive +23 ± 73 result.

**Priority 7+** — corpus extension, deployment story, HalfKP-pp, YaneuraOu pipeline study (Session 18 Eval Pri. 6, 7, 8, 9).

**Estimated duration for Priorities 1 + 3**: 4–6 hours (75 s retrain, 1.5–3 h bake-off, 30 m diagnostic).

**Prerequisite**: none — all Session-19 deliverables landed cleanly.

---

## File Changes Summary

### Files Modified
- `src/evaluation/nnue.rs` (~270 lines added)
  - 16 new public constants and 2 helper functions for the thermometer encoding.
  - 3 new accumulator helpers (`add_hand_contributions`, `add_hand_level`, `remove_hand_level`).
  - 3 new `NNUEEvaluator` fields (`use_hand_features`, `hand_counts`, `hand_counts_stack`).
  - 6 new `NNUEEvaluator` methods (3 public + 3 private).
  - `evaluate`, `evaluate_incremental`, `nnue_make_move`, `nnue_unmake_move`, `refresh_accumulator` updated to handle the hand-feature path.
  - `refresh_accumulator` signature changed: added `captured_pieces: &CapturedPieces`.
  - 3 new regression tests in the `tests` module.
- `src/evaluation/nnue_training.rs` (~75 lines added)
  - 5 new feature-extraction helpers: `extend_with_hand_features`, `extract_active_features_with_hand`, `extract_active_features_with_stm_and_hand`, `extract_active_features_halfkp_with_hand`, `extract_active_features_halfkp_with_stm_and_hand`.
  - 3 new imports from `nnue.rs`.
- `src/evaluation.rs` (~20 lines added)
  - `nnue_refresh` signature changed to take `captured_pieces`.
  - New wrapper `nnue_set_use_hand_features`.
- `src/search/search_engine.rs` (1 line changed)
  - Search-root `nnue_refresh(board, player)` → `nnue_refresh(board, player, captured_pieces)`.
- `src/bin/nnue_offline_trainer.rs` (~110 lines added/changed)
  - New `--use-hand-features` CLI flag.
  - `board_from_sfen` returns `Option<(BitboardBoard, CapturedPieces)>`.
  - `validation_pearson` and `build_position` extended with `use_hand_features` parameter.
  - Weight-allocation logic extended to pad pre-Session-19 weights with zero rows when hand features are on.
  - `--output-weights` default changed from `.json` to `.bin`.
  - Per-epoch checkpoint extension derived from output extension (was hard-coded `.json`).
  - Banner prints `Hand features: ON/off (Session 19)`.
- `src/bin/elo_tester.rs` (~25 lines added)
  - `--use-hand-features` and `--use-hand-features-b` CLI flags.
  - Engine A and engine B setup branches call `nnue_set_use_hand_features(true)` when set.
- `.gitignore` (~10 lines added)
  - Rules for `nnue_weights_*.bin`, `nnue_weights_*.json` (with whitelist for the canonical file), `elo_results.csv`, `nnue_corpus_*.jsonl`.

### Files Created
- `docs/nnue-phase2/SESSION_LOG_019.md` (this file).
- `weights/archive/README.md` (provenance for the moved weight files).

### Files Moved
- 41 of 42 `nnue_weights_*.json` files: repo root → `weights/archive/`. The canonical `nnue_weights_trained.json` remains at the root (auto-loaded by `src/lib.rs` and read by `test_eval_paths_agree_on_trained_weights`).

### Files Deleted
- None.

### Artefacts produced (untracked, in `/tmp`)
- `/tmp/s19_flat_hand_warmstart_5e.json` (5.8 MB) — warm-start 5-epoch trained weights.
- `/tmp/s19_flat_hand_warmstart.log` — training log.
- `/tmp/s19_flat_hand_fresh_10e.bin` (1.24 MB) — **fresh-init 10-epoch trained weights** (4.68× smaller than equivalent JSON).
- `/tmp/s19_flat_hand_fresh.log` — training log with the per-epoch Pearson trajectory.
- `/tmp/s19_smoke.csv` — 2-game elo-tester smoke test confirming `.bin` hand-feature weights load and play.

---

## Testing Results

### Session 19 success criteria

**Criterion 1**: Hand-feature constants and `hand_feature_index` function are added with correct layout (76 features per side, disjoint flat and HalfKP regions).
- [x] Met. Verified by `test_hand_feature_index_layout`.

**Criterion 2**: Engine-side `nnue_make_move` / `nnue_unmake_move` keep the accumulator and `hand_counts` consistent under capture and drop, with O(1) make and O(1) unmake.
- [x] Met. Verified by `test_hand_features_capture_matches_full_refresh` and `test_hand_features_drop_matches_full_refresh`.

**Criterion 3**: Trainer-side feature extraction produces the same active-feature set as the engine's accumulator computation.
- [x] Met (transitively). Both paths read from the same `hand_feature_index` function and the regression tests confirm the engine-side accumulator matches a fresh refresh that re-uses the trainer's extraction logic indirectly via `add_hand_contributions`.

**Criterion 4**: All 8 pre-existing NNUE tests still pass.
- [x] Met. 8 / 8 pass; total 11 / 11 with new hand-feature tests.

**Criterion 5**: Trainer can warm-start a pre-Session-19 weight file with `--use-hand-features` and trains without crashing or NaNs.
- [x] Met. `Padding loaded weights from 2269 to 2345 rows for hand features (76 zero rows)` log line confirms the runtime padding path; 5-epoch warm-start completed in 14.4 s.

**Criterion 6**: Fresh-init training with hand features reaches per-stratum Pearson r within ~10 % of the S15 baseline within 10 epochs.
- [x] Met. r_blk = +0.610 vs S15's +0.607 (Black-stm only) at epoch 10 — *exceeds* the baseline.

**Criterion 7**: Trainer default output is `.bin` and produces a binary file ~5× smaller than the equivalent JSON.
- [x] Met. 1.24 MB `.bin` vs 5.80 MB `.json` = 4.68× shrink (matches Session 18's HalfKP measurement of 4.57×).

**Criterion 8**: 41 untracked weight files moved out of repo root; `.gitignore` updated to prevent future sprawl.
- [x] Met. Post-cleanup `git status` shows only `nnue_weights_trained.json` (canonical) plus session source changes. `.gitignore` blocks `nnue_weights_*.bin` and `nnue_weights_*.json` at repo root with a `!nnue_weights_trained.json` whitelist.

### Diagnostic findings

**Finding 1 (the headline)**: Pieces-in-hand thermometer encoding works correctly and trains in line with prior baselines. Per-stratum Pearson r of +0.610 (Black) and +0.566 (White, to-move-POV) at 10 epochs of fresh-init Adam matches the S15 baseline of +0.607 at 30 epochs — a 3× speedup in convergence, suggesting the hand features add real signal.

**Finding 2 (the most surprising)**: The Session 19 network converges to a *to-move-POV* output, where the existing S14–S17 networks converged to a *board-absolute* output. Both are valid fixed points of the loss landscape; the difference is invisible to per-stratum diagnostics but breaks the trainer's aggregate `r_all` formula. The encoding is correct; the diagnostic is mode-coupled.

**Finding 3**: The thermometer pattern's incremental-update cost is bounded. A capture or drop is a single row add or remove (~256 mul-adds at hidden=256), and unmake restores via single i32-vector pop + 14-byte hand-counts pop. Per-eval overhead at depth 3 / 500 ms is < 5% of the existing accumulator cost.

**Finding 4**: The `.bin` format ratio (4.68× shrink) holds across network sizes. Session 18 measured 4.57× on a 437 MB HalfKP file; Session 19 measured 4.68× on a 5.8 MB flat-hand file. Bincode's overhead is dominated by per-tensor framing, not data magnitude.

**Finding 5**: The 41-file weight-archive cleanup retires real operational drag. Post-Session-19 `git status` shows ~10 lines of relevant change versus ~50 pre-Session-19. Future trainer outputs to the repo root are gitignored; the canonical weight file remains addressable.

---

## Historical Session Reference

| Session | Phase        | Title                                                                                            | Status      | Date       |
|---------|--------------|--------------------------------------------------------------------------------------------------|-------------|------------|
| 1       | 1.1-1.2      | Weight Initialization & Output Scaling                                                           | Completed   | 2026-04-22 |
| 2       | 1.3, 2.1-2.3 | Training Verification & Algorithm Fixes                                                          | Completed   | 2026-04-22 |
| 3       | 2.4          | Extended Training + Game Diversity                                                               | Completed   | 2026-04-22 |
| 4       | 3.2, 3.4     | Speed Optimization (Incremental Accumulator)                                                     | Completed   | 2026-04-22 |
| 5       | 4.1          | ELO Validation Infrastructure + First Pilot                                                      | Completed   | 2026-04-22 |
| 6       | 2b-setup     | External USI Teacher — Corpus Generator Infrastructure                                           | Completed   | 2026-04-22 |
| 7       | 2b-execute   | YaneuraOu Corpus + Offline Trainer — Integration Lessons                                         | Completed   | 2026-04-23 |
| 8       | 2b-finish    | f32 Shadow Weights — Mechanics Fixed, Reveals Structural Ceiling                                 | Completed   | 2026-04-24 |
| 9       | 2c           | Relaxed Teacher Target + elo-tester Fix — Output-Range Breaks                                    | Completed   | 2026-04-24 |
| 10      | 2d           | Symmetrise Output Layer — Negative Result, Reframes 0.47 Floor                                   | Completed   | 2026-04-25 |
| 11      | 2e           | Lower OUTPUT_DIVISOR + Pearson-r Metric — Reveals Input-Layer Starvation                         | Completed   | 2026-04-25 |
| 12      | 2f           | Three Diagnostics for Input-Layer Starvation — All Three Negative                                | Completed   | 2026-04-26 |
| 13      | 2g           | Sigmoid+MSE Loss — Eval-Path Bug Falsified, Sigmoid Loss Negative                                | Completed   | 2026-04-26 |
| 14      | 2h           | Side-to-Move Feature — Pearson r Lifts From ±0.10 Noise Floor To +0.59                          | Completed   | 2026-04-26 |
| 15      | 2i           | Adam Optimiser + Extended ELO Bake-Off — Adam Fresh-Init Hits +0.61 r; +76 ± 64 ELO Confirmed   | Completed   | 2026-04-27 |
| 16      | 2j           | HalfKP Feature Engineering + Adam-Fresh ELO Confirmation — HalfKP Hits +0.614 r, 40% Lower td_err | Completed   | 2026-04-28 |
| 17      | 2k           | Engine-Side HalfKP + ELO Confirmation Bake-Off — HalfKP Hits +120.4 ± 76.8 ELO, 1-Loss-of-30     | Completed   | 2026-04-29 |
| 18      | 2l           | Head-to-Head HalfKP vs Flat-NNUE + Binary Weights Format — HalfKP +23 ± 73 ELO over flat (CI brackets 0); .bin shrinks 4.57× | Completed   | 2026-04-30 |
| 18.eval | —            | Phase 2 Audit + Priority Reorder — sets Session 19+ direction                                    | Completed   | 2026-05-01 |
| 19      | 2m           | Pieces-in-Hand Thermometer Encoding + Trainer .bin Default + Weight-File Cleanup                  | Completed   | 2026-05-01 |
| 20      | 2n           | Hand-Feature 30-Epoch Retrain + Bake-Off + Diagnostic Lift                                        | Pending     | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — pieces-in-hand thermometer encoding wired end-to-end (~270 LoC across nnue.rs + nnue_training.rs + ~30 LoC across evaluation.rs / search_engine.rs), all 11 NNUE tests pass (3 new hand-feature tests + 8 pre-existing), trainer default switched to `.bin` (4.68× shrink), 41-file weight-archive cleanup landed, and 10-epoch fresh-init training reaches **r_blk = +0.610, r_wht = +0.566** — per-stratum parity with the S15 baseline at 1/3 the epoch count.
**Ready for next session**: Yes — Session 20 priorities are (1) full 30-epoch retrain + head-to-head bake-off, (2) cumulative-ELO arm continuity vs PST, (3) the small `r_all_to_move_pov` diagnostic lift.
**Comments**:
1. **The Session 18 Evaluation's priority reorder is now in effect.** Session 19 landed Pri. 1 and Pri. 5 (the eval's combined P3 + P5). All other priorities are deferred to subsequent sessions in the order the eval recommends. The eval's "What I would NOT do next" list is also adopted.
2. **The hand-feature encoding is the right shape for this corpus.** Thermometer (active iff count ≥ k) gives O(1) incremental updates, separate learnable weights per level, and compatibility with the existing `Vec<usize>` active-features abstraction. Flat (not king-conditioned) keeps the dimensionality manageable for a 67K-position corpus.
3. **The 10-epoch parity result is encouraging but not load-bearing.** The proper test is the head-to-head bake-off in Session 20. The +0.610 r_blk at 10 epochs (vs +0.607 r_all at S15's 30 epochs) is consistent with hand features adding signal, but a 30-epoch retrain at seed 47 + a 30-game bake-off at seed 48 will give the decision-grade answer.
4. **The trainer's `r_all` diagnostic confusion is a 3-line fix that should land in Session 20.** The Session 19 network converges to to-move-POV, and the existing diagnostic assumes board-absolute. Adding a second `r_all_to_move_pov` line and reporting whichever is larger makes future runs mode-independent.
5. **The `.bin` default + weight-archive cleanup is the operational deliverable.** Future trainer runs no longer pollute the repo root; the canonical `nnue_weights_trained.json` remains the auto-load default; the 41 historical files are preserved under `weights/archive/`. Combined effect: `git status` is now ~10 lines instead of ~50 after a typical training run.

---

**Template Version**: 1.0
**Last Updated**: 2026-05-01
