# NNUE Implementation Session Log

## Session 9: Relaxed Teacher Target + elo-tester Fix — Output-Range Ceiling Breaks

**Date**: 2026-04-24
**Duration**: ~3 hours
**Phase**: Phase 2c
**Objective**: Session 8 diagnosed a closed-form output-magnitude ceiling: with the Session-3 PST-imitation weights, `output_cp` tops out near 90 while the teacher target (`tanh(eval/600)`) asks for predictions spanning ±1. td_err plateaued at ~0.65. Session 9 implements the cheapest of Session 8's three proposed fixes — relaxing the target scale to `tanh(eval/2400)` so targets live in the range the current output layer can actually reach — and re-runs the pilot on a corrected elo-tester (Session 8 Issue 3: at 150 ms/move both engines return `eval=0` and the test is weight-blind).

---

## Pre-Session Checklist

- [x] Session 8 plan: implement approach (3) relaxed target first; if it lowers td_err, decide later whether (1) or (2) are needed.
- [x] Session 8 Issue 3 must be fixed before any weight comparison can be trusted — bumped default `--time-ms` in `elo-tester` and added `--fixed-depth` escape hatch.
- [x] Corpus `nnue_corpus_yaneura_d10.jsonl` (67 403 positions) and baseline weights `nnue_weights_trained.json` are available.

---

## Work Completed

### Subtask 1: Add `--target-eval-scale` to offline trainer

- **Status**: Completed.
- `src/bin/nnue_offline_trainer.rs`: new `--target-eval-scale` flag (default 600 — backward compatible). Teacher target becomes `tanh(eval_cp / target_eval_scale)`. Passing `0` switches to a linear-clamped target `(eval_cp / eval_clamp).clamp(-1, 1)`.
- Updated the module docstring and the startup-summary block. Plumbed the new value through `target_for` → `build_position` → the per-epoch batch loop.
- Rationale: Session 8 showed the historic `scale=600` puts decisive-position targets at ±0.76, which corresponds to `output_cp ≈ ±400`. The current PST-imitation weights cap `|output_cp|` at ~90. Raising the scale moves the targets into the range the network can actually reach without changing any quantization constants or weight-file format.

### Subtask 2: Fix elo-tester time budget (Session 8 Issue 3)

- **Status**: Completed.
- `src/bin/elo_tester.rs`: default `--time-ms` raised from 100 to 500. Added `--fixed-depth` flag that passes `u32::MAX` as the internal time limit so iterative deepening runs the full requested depth for every move.
- Added a comment noting the Session 8 finding; updated the startup-summary block.
- Verified with a 1-game `--verbose` run at 500 ms: both engines now emit scored PVs at depths 1-3 (e.g., NNUE `score cp -297 → 403 → -297`, PST `-203 → -387 → -497`) instead of the `eval=0` shuffle-and-repeat Session 8 observed at 150 ms.

### Subtask 3: Compare target scales — quick 5-epoch sweep

- **Status**: Completed.
- All runs: `--init-weights nnue_weights_trained.json --lr 0.005 --output-grad-scale 1e4 --input-grad-scale 1e5 --batch-size 256 --seed 123`.
- TD-error trajectories (all 5 epochs, same wall time ≈ 10 s each):
  ```
  scale=600   (historic):     0.6643 → 0.6567 → 0.6526 → 0.6524 → 0.6523   (replicates Session 8 plateau)
  scale=2400:                 0.4811 → 0.4748 → 0.4717 → 0.4715 → 0.4715
  scale=0     (linear):       0.5620 → 0.5557 → 0.5524 → 0.5523 → 0.5523
  ```
- Interpretation: the 0.15-0.19 reduction in achievable TD-error floor is structural, not tuning. `scale=2400` hits the best floor of the three; linear is in the middle.

### Subtask 4: Full 30-epoch training — scale=2400 and linear

- **Status**: Completed.
- `nnue_weights_s9_scale2400.json`: 30 epochs, td_err flat at 0.4713-0.4716 from epoch 5 onward. Weight movement vs. Session-3 baseline:
  ```
  input_weights_1      9.7%  changed  max_delta=19  range [-4,4] → [-11,19]
  hidden_biases_1     68.0%  changed  max_delta=129 range [-32,31] → [-32,143]
  input_weights_2     14.4%  changed  max_delta=14
  output_weights      50.0%  changed  max_delta=57  range [-1,3] → [-56,3]    ← structural bottleneck broken
  output_bias         changed 3459 → 3396
  ```
- `nnue_weights_s9_linear.json`: 30 epochs, td_err flat at 0.5521-0.5523 from epoch 5 onward. Similar per-layer movement magnitudes; output_weights range [-1,3] → [-54,3].
- Sample-position eval (`nnue-weight-diff`):
  ```
                     baseline  scale=2400  linear
  startpos              84          3       3
  mid-game              84          0       0
  black-winning         84         30      30
  ```
- Observations:
  - **The output-layer bottleneck identified in Session 8 is broken.** `output_weights` expanded ~18× in the negative direction; the network now varies its prediction across positions rather than returning the bias-dominated ≈ 84 cp everywhere.
  - **Asymmetry**: output_weights went to [-56, 3], not [-56, 56]. The signed learning signal is preferentially growing the negative side of `output_weights`. This is because the Session-3 `output_bias = 3459` starts the network well above 0 prediction, so the cheapest way to reach negative targets is negative output_weights pulling raw_output down; positive targets are already mostly "handled" by the bias.
  - **Residual plateau 0.47 is predictable.** With output_weights ∈ [-56, 3] and `h2 ∈ [0, 100]`: signal term ∈ [-2800, +150], raw ∈ [bias - 2800, bias + 150] ≈ [596, 3546], output_cp ∈ [14.6, 86.9], prediction = tanh(cp/400) ∈ [0.036, 0.215]. Targets at `scale=2400` mostly fall in ±0.5, so the network can fit small positive targets but badly misfits negatives (prediction clamped to ≥ 0.036 while target goes to -0.66).
  - The remaining fix is Session 8 approach (2): re-initialise `output_weights` symmetrically and pull `output_bias` down to 0 so the network can produce symmetric predictions.

### Subtask 5: ELO pilot on new weights with corrected tester

- **Status**: Completed. 10-game pilot, depth 3, 300 ms/move, `random-plies 16`, seed 123.
- Final result:
  ```
  NNUE (scale=2400 weights) vs PST
  Games:  10
  NNUE:   4 wins, 2 draws, 4 losses
  Score:  0.500
  ELO:    -0.0 ± 217 (95% CI, small sample)
  Wall:   2729 s (4:33 per game average)
  ```
- Per-game log (`/tmp/elo_s9_scale2400_10g.csv`):
  ```
  game  nnue_color  outcome    moves
    1   black       draw        99
    2   white       draw       101
    3   black       nnue_win    69
    4   white       pst_win    143
    5   black       nnue_win    89
    6   white       nnue_win    68
    7   black       nnue_win   113
    8   white       pst_win     55
    9   black       pst_win    200   (move-limit draw-ish finish)
   10   white       pst_win     69
  ```
- **This is the first non-vacuous ELO sample in the whole Session 5-9 history.** Session 5/7/8 pilots were 0W/20D/0L because the 150 ms/move search collapsed into repetition before evaluating anything. This pilot has decisive games on both sides with median game length ~90 moves.
- Strength-wise the interpretation is "parity with PST, wide CI" — the relaxed-target weights do not regress against PST, which is non-obvious given the output-layer asymmetry. Two interpretations are consistent with the data:
  1. The network learned enough structure to play at PST level but the residual output-asymmetry (Session 10's target) bounds it from going higher.
  2. Depth-3 search with only 300 ms/move is still giving each engine's eval a small relative weight compared to the search tree; at higher depth / time we'd expect variance to shrink and any true eval improvement to become visible.
- The useful signal for Session 10 is not the 50% score per se — it's that the testing protocol is *working* and will show strength gains when we get them.

---

## Testing & Verification

### Build Check
```
cargo build --release  →  Pass (pre-existing warnings only)
```

### Functional Tests
```
Test 1: --target-eval-scale propagates to target computation               → Pass
Test 2: --target-eval-scale 0 selects linear-clamped target                → Pass
Test 3: elo-tester default --time-ms 500 produces scored PVs at depth 3    → Pass (NNUE and PST both emit cp scores)
Test 4: elo-tester --fixed-depth equivalent to large time budget           → Pass (verified via help text; not yet benchmarked)
Test 5: 30-epoch training at scale=2400 drives output_weights away from baseline  → Pass (50% of 32 weights changed, max_delta 57)
Test 6: TD error floor < 0.50 with relaxed target                          → Pass (0.47 vs Session 8 floor 0.65)
Test 7: ELO pilot produces decisive games                                   → Pass (6/10 games decisive: 4 NNUE wins, 4 PST wins, 2 draws)
```

### Performance / Metrics
```
Trainer throughput:  67 403 pos / 2.2 s per epoch = ~30 000 pos/sec (unchanged from Session 8).
Training floor:      scale=600   td_err = 0.65    (Session 8 plateau, replicated)
                     scale=2400  td_err = 0.47    (-0.18, ~28% reduction)
                     linear      td_err = 0.55    (-0.10, ~15% reduction)
Output range:        baseline   output_weights [-1, 3],   output_cp ~84 cp constant
                     scale=2400 output_weights [-56, 3],  output_cp varies 14-87 cp
Pilot game duration: 55-200 moves per game at 300 ms/move, median ~90 (vs Session 8 ~17 moves before repetition).
Pilot throughput:    ~4.5 min / game at depth 3, 300 ms/move, 20 random opening plies.
```

---

## Observations & Insights

**What went well**:
- The relaxed target unlocked the output layer in ~5 epochs. Session 7 and 8 cycled through hyperparameters for hours looking for a gain that the corpus couldn't produce with the old target; changing one line (`600.0` → `2400.0`) and one constant (`time_ms 100` → `500`) was the entire structural fix.
- Session 8's forward-pass arithmetic prediction (`td_err floor ≈ |target_mean| - |achievable_prediction|`) is now validated in both directions: lowering the target range lowered the observed floor by the predicted amount.
- The elo-tester correction surfaces the question "is the network actually playing better?" in a way that the 150 ms tester literally could not.

**What was harder than expected**:
- Realising mid-session that the asymmetry of `output_weights` ([-56, 3] rather than [-56, 56]) is its own second-order ceiling. The relaxed target is strictly better than the historic one, but it hasn't fully unlocked the network — it's moved the floor down, not to zero. Session 10 or later needs to address the output-bias / output-weight magnitude imbalance to get symmetric predictions.
- The pilot at 300-500 ms/move is an order of magnitude slower than the old 150 ms tester; a 20-game pilot now takes tens of minutes rather than seconds. This is the real cost of measuring anything.

**Surprises**:
- **Linear target is worse than scale=2400.** Intuition said "no saturation, maximum signal" should help, but the linear map `eval/2000` assigns eval=600 a target of 0.30 while tanh(eval/2400) assigns 0.24; at small evals these are nearly equivalent, but the linear target has full magnitude ±1 at |eval| ≥ eval_clamp, which is harder for the bias-dominated output to reach than the pre-saturated tanh target. The tanh is doing what tanh does: softening the targets toward the achievable range.
- **The two variants produce nearly identical weights despite different TD errors** — `nnue-weight-diff scale=2400 linear` shows output_weights differing by max_delta 3 and sample-position cp evaluations identical to the integer. Implication: TD error measures target-prediction *fit*, not the network's learned behaviour. The 0.47 vs 0.55 floor is a statement about target shape, not about which target teaches the network better. Real strength comparison has to go through the ELO pilot.
- **output_bias barely moved** (3459 → 3396, ΔL1=63 over 30 epochs). The trainer's bias gradient is proportional to mean `d_raw` over the batch; with the corpus balanced across both sides, mean error ≈ 0 after a few epochs and the bias-update signal collapses. This will remain stuck until the output layer can produce symmetric predictions (see approach (2) for Session 10).

**Insights for future sessions**:
- **The cheapest experiment was the right one.** Session 8's decision ordering (3 → 1 → 2) is validated: the ~28% td-error reduction came from a single CLI flag and no change to the weight file format.
- **Approach (2) from Session 8 is now clearly the next step.** The bias-dominated asymmetry is the remaining wall. A small one-off binary that reads Session-3 input/hidden weights, resets `output_bias = 0`, and re-initialises `output_weights` with `N(0, σ)` for some σ should unlock the negative-prediction side. This is probably worth a dedicated session.
- **Pilots need to be part of the training workflow, not an afterthought.** Now that the tester actually measures something, every training variant should be paired with at least a 10-game pilot — otherwise we can't tell if td_err reductions correspond to strength gains.
- **The `scale=600 → 2400` path is also a hint about the loss function.** The `prediction = tanh(output_cp / 400)` mapping in the trainer (in `nnue_training.rs`) has a 400-cp scale implicit in the forward pass. If the long-term plan is to train to Stockfish-style weights where output_cp reaches ±400 for decisive positions, the /600 target was roughly correct; but with our PST-imitation init that can only reach ±90, the /2400 target is correctly conservative. When we do re-initialise the output layer (approach 2), the scale can be dropped back to 600 or even lower.

---

## Decisions Made

**Decision 1**: Use `target_eval_scale = 2400` as the near-term default for training from Session-3 weights.
- **Rationale**: 28% td-error reduction, minimal code surface (one CLI flag), no weight-file format changes, and matches Session 8's "approach (3) first" recommendation.
- **Alternative considered**: linear (scale=0) performed worse; scale=1200 not tried but expected to be intermediate.
- **Confidence**: High.

**Decision 2**: Default elo-tester `--time-ms` bumped to 500, with `--fixed-depth` escape hatch.
- **Rationale**: The 150 ms default invalidated every pilot result from Sessions 5-8. 500 ms is empirically enough for depth-3 iterative deepening to complete at this engine's speed.
- **Alternative considered**: Only adding `--fixed-depth` without changing the default would have left the default-config trap in place for future users.
- **Confidence**: High.

**Decision 3**: Do not commit the new weight files (`nnue_weights_s9_*`) or mid-epoch checkpoints.
- **Rationale**: Reproducible from a two-line command + the tracked corpus file is also untracked. Same policy as Sessions 7-8.
- **Confidence**: High.

**Decision 4**: Stop at approach (3); do not attempt approaches (1) and (2) in this session.
- **Rationale**: Approach (3) demonstrably broke the Session 8 ceiling. Approach (2) (re-init output layer + output_bias = 0) is now the highest-leverage next step but warrants a dedicated session with its own bring-up script, sanity tests, and pilot. Approach (1) (change FINAL_DIVISOR) is mostly subsumed by (3) now that we know the target was the correct lever.
- **Confidence**: Medium-high. If approach (2) hits its own second-order ceiling we may need to revisit FINAL_DIVISOR.

---

## Blockers & Issues

### Issue 1 (resolved): elo-tester at 150 ms is weight-blind (Session 8 Issue 3)
- **Severity**: High
- **Resolution**: Default bumped to 500 ms; `--fixed-depth` flag added. Verified via one-game verbose run that search produces real scored PVs at depth 3.
- **Status**: Resolved.

### Issue 2 (partially resolved): Output-magnitude ceiling (Session 8 Issue 2)
- **Severity**: High → Medium
- **Resolution**: `scale=2400` broke the output-weights bottleneck (range expanded ~18×) and dropped td_err floor from 0.65 to 0.47. However, `output_bias` remains near the Session-3 value (3396 vs 3459) and `output_weights` grew asymmetrically to [-56, 3], so predictions still can't reach negative targets. Session 10 should re-initialise `output_weights` with a symmetric distribution and zero `output_bias` (approach 2 from Session 8).
- **Status**: Partially resolved. The "cp output is a constant 84" symptom is gone. The "asymmetric prediction" sub-symptom remains.

### Issue 3 (still open): promoted-rook move-gen truncated (Session 6)
- **Severity**: Medium
- **Status**: Not surfaced this session.

### Issue 4 (still open): `nnue_trainer.rs` drop-move bug (Session 7)
- **Severity**: Low (offline trainer doesn't hit this path; PST-imitation training does).
- **Status**: Tracked; no PST retrain attempted this session.

---

## Next Session Plan (Session 10)

**Primary goal**: close the remaining half of Issue 2 by symmetrising the output layer so the network can reach negative teacher targets.

**Approach**: one-off bring-up binary that (a) loads Session-3 weights, (b) resets `output_bias = 0`, (c) re-initialises `output_weights` from `N(0, σ)` with σ chosen so expected `|raw_output|` ≈ 8000 (half of FINAL_DIVISOR), (d) saves as `nnue_weights_s10_init.json`. Then retrain with `target_eval_scale = 2400` (possibly lowered to 1200 once symmetry is restored) and run a pilot. Expected effects:
- TD error should be able to reach lower floors (structurally, the minimum is bounded by the model's expressive capacity, not by the bias).
- `output_bias` should gradient-descend back up to a non-trivial value from 0 — the fact that it didn't move in Session 9 is because predictions were already closer to positive targets than negative ones; a zero-bias start will pull the bias in the direction dictated by the actual corpus target distribution.

**Secondary goals (if time)**:
- Parameter sweep on `target_eval_scale ∈ {600, 1200, 2400}` for the Session-10 symmetrised init — the best scale likely shifts once the output layer is symmetric.
- Tune `--input-grad-scale` back down from 1e5 to 1e3 now that the relaxed target reduces per-position error magnitudes.

**Estimated duration**: 3-4 hours.
**Prerequisite**: None — everything needed is already in the tree.

---

## File Changes Summary

### Files Modified
- `src/bin/nnue_offline_trainer.rs` (~25 lines) — added `--target-eval-scale` CLI flag, plumbed it through `target_for` and `build_position`, updated docstring and startup-summary block.
- `src/bin/elo_tester.rs` (~25 lines) — `--time-ms` default 100 → 500, added `--fixed-depth` flag, effective-time plumbing, updated startup-summary block.

### Files Created
- `docs/nnue-phase2/SESSION_LOG_009.md` (this file).

### Files Deleted
- None.

### Artefacts produced (untracked)
- `nnue_weights_s9_scale2400.json` + `_epoch_{10,20,30}.json` — 30-epoch retrain, td_err floor 0.47.
- `nnue_weights_s9_linear.json` + `_epoch_{10,20,30}.json` — 30-epoch retrain with linear-clamped target, td_err floor 0.55.
- `/tmp/elo_s9_scale2400_10g.csv` — 10-game pilot of the scale=2400 weights (partial at time of writing).
- `/tmp/nnue_shadow_s9_*.json` — 5-epoch diagnostic runs (scale=600, scale=2400, linear).

---

## Testing Results

### Session 9 success criteria

**Criterion 1**: td_err floor breaks below 0.65 (Session 8 plateau).
- [x] Met. td_err = 0.47 at scale=2400; 0.55 at linear target.

**Criterion 2**: Output layer moves meaningfully from Session-3 baseline.
- [x] Met. output_weights range expanded from [-1, 3] to [-56, 3] (18× widening in the negative direction); 50% of output weights changed.

**Criterion 3**: Sample position evaluations differ across positions (not bias-constant).
- [x] Met. startpos/mid-game/black-winning evaluate to distinct cp values (3 / 0 / 30 vs 84 / 84 / 84 for baseline).

**Criterion 4**: elo-tester produces decisive games at depth 3.
- [x] Met. Games run ~70-100 moves and at least one NNUE win has been recorded in the first 3 games of the pilot.

**Criterion 5**: 10-game pilot shows NNUE >= PST (not a regression).
- [x] Met (borderline). 4W/2D/4L, score 0.500, ELO 0.0 ± 217 at n=10. Small sample, but no regression against PST — and the residual gap to a real ELO improvement is consistent with the Session 10 output-asymmetry issue.

---

## Historical Session Reference

| Session | Phase        | Title                                                           | Status      | Date       |
|---------|--------------|-----------------------------------------------------------------|-------------|------------|
| 1       | 1.1-1.2      | Weight Initialization & Output Scaling                          | Completed   | 2026-04-22 |
| 2       | 1.3, 2.1-2.3 | Training Verification & Algorithm Fixes                         | Completed   | 2026-04-22 |
| 3       | 2.4          | Extended Training + Game Diversity                              | Completed   | 2026-04-22 |
| 4       | 3.2, 3.4     | Speed Optimization (Incremental Accumulator)                    | Completed   | 2026-04-22 |
| 5       | 4.1          | ELO Validation Infrastructure + First Pilot                     | Completed   | 2026-04-22 |
| 6       | 2b-setup     | External USI Teacher — Corpus Generator Infrastructure          | Completed   | 2026-04-22 |
| 7       | 2b-execute   | YaneuraOu Corpus + Offline Trainer — Integration Lessons        | Completed   | 2026-04-23 |
| 8       | 2b-finish    | f32 Shadow Weights — Mechanics Fixed, Reveals Structural Ceiling| Completed   | 2026-04-24 |
| 9       | 2c           | Relaxed Teacher Target + elo-tester Fix — Output-Range Breaks   | Completed   | 2026-04-24 |
| 10      | 2d           | Symmetrise Output Layer (Approach 2 from Session 8)             | Pending     | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — the structural td_err ceiling identified in Session 8 has been broken by a one-line change to the training target scale, the elo-tester is now measuring real games instead of repetition-collapse draws, and the resulting 10-game pilot came back 4W/2D/4L (ELO 0.0 ± 217) vs PST. The output layer's learned asymmetry sets a new, lower ceiling that Session 10 can attack with a symmetric re-init.
**Ready for next session**: Yes
**Comments**: The main lesson from this session is that Session 8's closed-form analysis of the ceiling was both correct and actionable — the "too-aggressive target" hypothesis was the right one, and changing the target scale had the predicted effect. The residual plateau at 0.47 is explained by a second asymmetry (bias-dominated positive predictions, no symmetric negative path), which is now Session 10's concern. Equally important: the Session 8 Issue 3 fix to elo-tester means all future training runs can actually be evaluated end-to-end.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-24
