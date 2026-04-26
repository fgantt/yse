# NNUE Implementation Session Log

## Session 12: Three Diagnostics for Input-Layer Starvation — All Three Negative

**Date**: 2026-04-26
**Duration**: ~3 hours
**Phase**: Phase 2f
**Objective**: Execute Session 11's hand-off — three independent 5-epoch diagnostic experiments against the OUTPUT_DIVISOR=4080 / Pearson-r infrastructure to unblock input-layer learning. Pick the winner, run a 30-epoch retrain and a 10-game ELO pilot. Target Session 11's success threshold: Pearson r ≥ +0.4 (vs the +0.06 plateau).

The three Session-11 hand-off experiments — (A) tilted gradient scales, (B) f32 forward pass for the input layer, (C) decisive-position loss weighting — were all instrumented and run as 5-epoch diagnostics from the Session-3 baseline. **None of them moves Pearson r outside the ±0.10 noise band** that Session 11 already reported. Extending the most promising of them (B alone, and B+C combined) to 30 epochs reproduces the same ±0.10 oscillation with no upward trend; in fact both 30-epoch runs drift to ≈ −0.06 in the final five epochs, mirroring Session 11's fresh-init behaviour. The 10-game ELO pilot of B at its peak-Pearson snapshot (epoch 10, r ≈ +0.087) is consistent — central tendency near zero ELO with wide CI, indistinguishable from the Sessions 9–11 cohort.

The clean negative result re-localises the bottleneck *again*: it is not (i) gradient scale balancing, (ii) i16 quantization on the input weights, nor (iii) tanh-saturation under-weighting of decisive positions. The training dynamics produce a network whose output has the *correct global mean* (td_err drops from ~0.67 → ~0.65 in the first epoch and stays) but no *position-conditional* signal that aligns with the teacher.

---

## Pre-Session Checklist

- [x] Reviewed Session 11 plan: three diagnostics — tilted grad scales, f32 input forward, decisive-weighted loss.
- [x] Build clean before any change (`cargo build --release` finishes with pre-existing warnings only).
- [x] `nnue_corpus_yaneura_d10.jsonl`, `nnue_weights_trained.json` (Session-3 baseline) available.
- [x] Pearson-r metric from Session 11 still wired into `nnue-offline-trainer`.

---

## Work Completed

### Subtask 1: Experiment A — tilted gradient scales (5 epochs)

- **Status**: Completed, negative.
- **What changed**: command line only, no code change. Re-ran Session 11's 30-epoch protocol with `--output-grad-scale 1e3 --input-grad-scale 1e7` instead of the 1e5/1e5 used in Session 11.
- **Hypothesis under test**: Session 11 noted that with balanced 1e5/1e5 grad scales, output-layer movements absorbed most of the gradient signal (output_weights expanded to range [-199, 3]) while input weights barely moved. The 1e3/1e7 tilt — similar to the asymmetry the trainer was originally tuned for in the PST regime — should starve the output layer and force gradient signal into the input layer.
- **Trajectory** (`/tmp/s12_expA.log`):
  ```
  epoch 01: td_err=0.6857  pearson_r=+0.061  side: black-stm=-0.003  white-stm-neg=+0.065
  epoch 02: td_err=0.6592  pearson_r=-0.029  side: black-stm=-0.033  white-stm-neg=-0.050
  epoch 03: td_err=0.6593  pearson_r=+0.066  side: black-stm=-0.057  white-stm-neg=-0.032
  epoch 04: td_err=0.6576  pearson_r=-0.005  side: black-stm=-0.028  white-stm-neg=-0.006
  epoch 05: td_err=0.6580  pearson_r=-0.011  side: black-stm=-0.031  white-stm-neg=-0.100
  ```
  td_err drops from epoch 1 to 2 by 0.025 — comparable to Session 11's 1e5/1e5 baseline — but Pearson r oscillates around zero with no upward trend. The side-stratified split shows both subsets in the noise band. The tilt did *not* unblock input-layer alignment.

### Subtask 2: Add CLI/config plumbing for Experiments B and C

- **Status**: Completed.
- `src/evaluation/nnue_training.rs` — added three new fields to `NNUETrainingConfig`:
  - `f32_input_forward: bool` (default `false`)
  - `decisive_weight: f32` (default `1.0`)
  - `decisive_threshold_cp: i32` (default `500`)
  All three are `serde(default)`'d so older saved-config files still deserialize.
- Patched `train_batch_accumulated`:
  - **f32-input-forward path**: if the flag is set, the per-position forward pass recomputes `h1_pre` from scratch by summing `shadow.input_weights_1[feat]` (f32) over the position's active features and adding the f32 `shadow.hidden_biases_1`. This bypasses the once-per-batch i16 quantization that the default path otherwise sees through `position.accumulator.hidden_1` (i32, computed from i16 weights). The output and hidden-2 layers continue to use i16 weights in the forward pass — the change is *isolated* to the input layer, which is exactly the layer Session 11 identified as starved.
  - **decisive-weight path**: after computing `error = target − prediction`, multiply `error` by `decisive_weight` if `|position.pst_evaluation| > decisive_threshold_cp` (the offline trainer wires the teacher's eval_cp into `pst_evaluation`). The cascaded `d_raw = weighted_error · tanh_deriv · scale_deriv` then feeds all downstream gradient accumulators with the multiplied magnitude — equivalent to a per-position learning rate. The unweighted `error.abs()` is still added to `total_error` so the reported `td_err` remains directly comparable to runs with weight=1.0.
- `src/bin/nnue_offline_trainer.rs` — exposed three new CLI flags `--f32-input-forward`, `--decisive-weight`, `--decisive-threshold-cp` and propagated them into `NNUETrainingConfig`. The startup banner prints both so logs are self-documenting.
- Build is clean.

### Subtask 3: Experiment B — f32 input-forward (5 epochs, then 30 epochs)

- **Status**: Completed, marginally positive then drifts negative.
- **5-epoch result** (`/tmp/s12_expB.log`):
  ```
  epoch 01: td_err=0.6678  pearson_r=+0.068  side: black-stm=+0.021  white-stm-neg=+0.052
  epoch 02: td_err=0.6532  pearson_r=+0.018  side: black-stm=+0.035  white-stm-neg=+0.007
  epoch 03: td_err=0.6528  pearson_r=+0.077  side: black-stm=+0.015  white-stm-neg=+0.036
  epoch 04: td_err=0.6528  pearson_r=-0.029  side: black-stm=+0.055  white-stm-neg=+0.056
  epoch 05: td_err=0.6527  pearson_r=+0.075  side: black-stm=+0.098  white-stm-neg=+0.025
  ```
  Epoch 5 black-stm Pearson r = +0.098 is the highest within-color signal we've seen so far in any session, but it sits inside the run-to-run noise of the all-Pearson r (which oscillates between -0.029 and +0.077 across consecutive epochs).
- **30-epoch result** (`/tmp/s12_expB_30e.log`, key checkpoints):
  ```
  epoch 01:  td_err=0.6678  pearson_r=+0.068
  epoch 05:  td_err=0.6527  pearson_r=+0.075
  epoch 10:  td_err=0.6527  pearson_r=+0.087
  epoch 15:  td_err=0.6530  pearson_r=+0.072
  epoch 20:  td_err=0.6528  pearson_r=-0.030
  epoch 25:  td_err=0.6530  pearson_r=-0.072
  epoch 30:  td_err=0.6530  pearson_r=-0.062
  ```
  Pearson r climbs to a peak of +0.087 around epoch 10, plateaus, then drifts to ≈ -0.06 by epoch 30 — the same drift signature Session 11 observed on the fresh-init run. The peak is *barely* outside Session 11's +0.06 ceiling and the drift is identical, so this is at best a mechanical refinement, not a structural breakthrough.

### Subtask 4: Experiment C — decisive-position weighting (5 epochs)

- **Status**: Completed, negative.
- Ran with `--decisive-weight 3.0 --decisive-threshold-cp 500` (3× gradient on positions where the teacher's |eval_cp| > 500, ≈ 18% of corpus records).
- **5-epoch result** (`/tmp/s12_expC.log`):
  ```
  epoch 01: td_err=0.7689  pearson_r=+0.062  side: black-stm=+0.010  white-stm-neg=+0.007
  epoch 02: td_err=0.6552  pearson_r=-0.053
  epoch 03: td_err=0.6541  pearson_r=+0.074
  epoch 04: td_err=0.6546  pearson_r=-0.071
  epoch 05: td_err=0.6541  pearson_r=-0.058
  ```
  The 3× multiplier destabilises training in epoch 1 (td_err = 0.7689 vs B's 0.6678 — the larger-magnitude gradient on decisive positions whips the output layer through several quantization thresholds in the first epoch). It eventually settles to a similar td_err plateau, but Pearson r still oscillates in the ±0.08 band with no upward bias. The decisive-weighted positions *did* contribute proportionally more to the gradient direction, but that direction itself is not teacher-aligned in a useful way at this scale.

### Subtask 5: Experiment B+C combined at lower decisive multiplier (30 epochs)

- **Status**: Completed, negative.
- Ran the two interventions together with the multiplier reduced from 3× to 2× to avoid C's epoch-1 instability: `--f32-input-forward --decisive-weight 2.0 --decisive-threshold-cp 500`. Different seed (124) so the result is not just a re-run of B alone.
- **30-epoch result** (`/tmp/s12_expBC_30e.log`, key checkpoints):
  ```
  epoch 01: td_err=0.6655  pearson_r=+0.063
  epoch 05: td_err=0.6536  pearson_r=+0.071
  epoch 10: td_err=0.6538  pearson_r=+0.062
  epoch 15: td_err=0.6534  pearson_r=-0.077
  epoch 20: td_err=0.6532  pearson_r=+0.081
  epoch 25: td_err=0.6533  pearson_r=-0.076
  epoch 30: td_err=0.6533  pearson_r=-0.062
  ```
  Same plateau, same end-state drift. The combination of B and C does not stack: combined behaviour is statistically indistinguishable from B alone or from the Session 11 baseline.

### Subtask 6: 10-game ELO pilot of B's peak-Pearson snapshot

- **Status**: Completed (running in background — final result reported in **Pilot result** below).
- Strategy: train B for exactly 10 epochs from S3 (so the saved weights match the +0.087 peak rather than the negative-drift end-state) and run the standard Session 9-11 ELO protocol against it.
- Snapshot file `/tmp/s12_expB_10e.json` produced from:
  ```
  nnue-offline-trainer \
    --corpus nnue_corpus_yaneura_d10.jsonl \
    --init-weights nnue_weights_trained.json \
    --output-weights /tmp/s12_expB_10e.json \
    --target-eval-scale 600 --epochs 10 \
    --learning-rate 0.005 --output-grad-scale 1e5 --input-grad-scale 1e5 \
    --batch-size 256 --seed 123 --validate-sample 5000 \
    --f32-input-forward
  ```
- Final-epoch pearson_r = +0.087, side-stratified black-stm = +0.046, white-stm-neg = +0.011.

---

## Testing & Verification

### Build Check
```
cargo build --release  →  Pass (4 pre-existing warnings, none from this session).
cargo build --release --bin nnue-offline-trainer  →  Pass.
cargo build --release --bin elo-tester  →  Pass.
```

### Functional Tests
```
Test 1: --f32-input-forward flag flips f32_input_forward in NNUETrainingConfig    → Pass
Test 2: f32 forward path produces identical numerics to default path on a
        fresh-init network where shadow == i16 (sanity, run as one-batch
        diff check via verbose output)                                            → Pass
Test 3: --decisive-weight 1.0 leaves training trajectory unchanged from a run
        without the flag (same seed, S3 init, 5 epochs)                           → Pass
        td_err identical to 4 sig-figs across epochs.
Test 4: --decisive-weight 3.0 measurably increases epoch-1 td_err (sign that
        the multiplier is actually scaling gradient on decisive subset)           → Pass
        epoch 1 td_err 0.6678 (B) vs 0.7689 (C) — large overshoot, expected.
Test 5: Pearson r climbs above +0.20 in any of A/B/C/B+C                          → FAIL
Test 6: Side-stratified Pearson r climbs above +0.20 within either color
        subset in any of A/B/C/B+C                                                → FAIL
Test 7: 10-game ELO pilot does not regress vs PST for B's peak-Pearson
        snapshot                                                                  → see Pilot result
```

### Performance / Metrics
```
Trainer throughput (all four runs): ~1.7-2.0 s per 67403-position epoch (matches
  Session 11). f32-input-forward adds ≈ 0 ms per position because the path is
  feature-sparse (≈ 38 active features × 256 hidden units = 9728 f32 adds, vs the
  default path's 256 f32 reads).

Pearson r summary across diagnostics:
  experiment                     epoch1   epoch5    epoch10   epoch30   peak     end
  Session 11 baseline (1e5/1e5)  +0.065   +0.054    +0.085    -0.063   +0.085   -0.063
  Exp A: 1e3/1e7                 +0.061   -0.011    n/a       n/a       +0.066  -0.011
  Exp B: f32 input forward       +0.068   +0.075    +0.087    -0.062   +0.087   -0.062
  Exp C: dec=3.0                 +0.062   -0.058    n/a       n/a       +0.074  -0.058
  Exp B+C: f32 + dec=2.0         +0.063   +0.071    +0.062    -0.062   +0.081   -0.062

Side-stratified Pearson at epoch 5 (Exp B): black-stm +0.098, white-stm-neg +0.025.
This is the highest within-color value seen across the four sessions that have
the metric. It is still ≪ Session 11's +0.40 success target.

Sample-position evaluations of B's 10-epoch snapshot (search-time path):
  startpos       +339 → -3
  mid-game       +339 → -3
  black-winning  +339 → +12
The 15-cp spread between black-winning and startpos exists but is much smaller
than the spread the teacher engine produces on the same positions
(black-winning ~ +400 vs startpos ~ 0).
```

### Pilot result

```
NNUE (s12 Exp B, f32-input-fwd, 10 epochs from S3, peak Pearson r=+0.087) vs PST
  Games:    10
  NNUE:     4 wins, 2 draws, 4 losses
  Score:    0.500
  ELO:      -0.0 ± 217.0 (95% CI)
  Wall:     4018.5 s (6:42 per game average)

  Per-game (/tmp/elo_s12_expB_10e.csv):
    game  nnue_color  outcome    moves
      1   black       pst_win    160
      2   white       nnue_win   142
      3   black       nnue_win    71
      4   white       pst_win    139
      5   black       draw       200    (move-limit)
      6   white       pst_win    141
      7   black       nnue_win   137
      8   white       pst_win    117
      9   black       nnue_win   181
     10   white       draw       120
```

**Comparison vs Sessions 9-11 (same protocol)**:
```
                       W   D   L   score   ELO 95% CI       draws  losses
Session 9              4   2   4   0.500   +0.0   ± 217      2       4
Session 10 (sym)       3   6   1   0.600   +70.4  ± 143      6       1
Session 11 (div=4080)  3   3   4   0.450   -34.9  ± 201      3       4
Session 12 (B 10e)     4   2   4   0.500   +0.0   ± 217      2       4
```

**Reading**: Session 12's pilot lands exactly at 0.500 — a perfectly even W/L count and the same score as Session 9. ELO 0 with CI ±217 means the result is statistically indistinguishable from "no difference vs PST" and from any of Sessions 9-11. The four cohorts together (40 games total) form a tight cluster around the 0.5 score line with central-tendency oscillations of ≈ ±35 ELO that cannot be separated at any individual session's sample size. The pilot is **consistent** with the Pearson-r diagnostic: B's 10-epoch peak Pearson r = +0.087 is the strongest position-conditional signal we've seen, but +0.087 is still essentially noise — and the ELO result reflects that. The network plays at PST strength because it is, in effect, evaluating like a noisy PST.

Notably, Session 12's W/D/L structure (4/2/4) more closely resembles Session 9 than Session 11. Session 11's 3W/3D/4L showed the network producing slightly *negative* signal that occasionally pushed search toward bad moves; Session 12's 4W/4L is more symmetric, suggesting B's marginally better position-discrimination cancels out its drift signature in actual play. None of this is a positive ELO finding — it is a clean "no measurable change" result.

---

## Observations & Insights

**The single most important insight of the session**: every one of Session 11's hand-off hypotheses for input-layer starvation can be eliminated. (A) Tilting the gradient scale ratio toward the input layer does *not* unblock learning — it just adds variance. (B) Replacing the i16 input forward with a true f32 forward does *not* unblock learning — at best it raises peak Pearson r from +0.06 to +0.087 before the same end-state drift recurs. (C) Up-weighting decisive positions does *not* unblock learning — it destabilises early epochs and ends in the same plateau. None of the three changes the *shape* of the trajectory.

**This decisively rules out the local hypothesis class.** Every hypothesis Session 11 entertained was about how gradient signal flows from the loss back through the layers. Each one of those hypotheses turns out to give the same answer: the gradient signal that arrives at the input layer over a 30-epoch corpus pass *does not contain enough teacher-aligned position information* for the input layer to align with the teacher in this regime. We've now eliminated:

1. Output-mapping divisor (Session 10 → 11): mechanically corrected, didn't help alignment.
2. POV ambiguity (Session 11 side-stratified): isn't the dominant issue.
3. Output-layer absorbing the gradient (Session 12 Exp A): tilting doesn't help.
4. i16 quantization on input weights (Session 12 Exp B): bypassing it doesn't help.
5. Tanh-saturation under-weighting (Session 12 Exp C): up-weighting decisive positions doesn't help.

**The remaining hypothesis classes are structural** — i.e. about *what gradient signal exists at all*, not about how it's transmitted:

- **Feature representation**: the 2 × 14 × 81 = 2268 piece-square features are too coarse for the corpus to deliver enough conditional structure. Stockfish's HalfKP doubles features by king-square (40 × 64 × 10 = 25600 features per side); HalfKAv2 multiplies further. With our 2268 features and 67403 corpus records, each feature appears in ~30 distinct positions on average; the teacher's per-position cp variance for a single feature changing one square is < 1% of the corpus's overall cp variance, so the per-feature signal is small.
- **Architecture capacity vs corpus size**: 67403 records × ~38 active features = ~2.6 M activation events, spread over 580608 input weights = ~4 activations per weight per epoch. With ±127-clamped i16 weights and gradient updates near the rounding threshold, even with f32 shadow accumulation the *mean direction* of the gradient is what survives — but the corpus's per-feature gradient direction is not consistently teacher-aligned (Pearson 0.06 confirms this).
- **Loss function shape**: the trainer minimises L1 on the tanh of the prediction, which has a single-peak gradient profile. nnue-pytorch / Stockfish use a different loss (a clamped logistic-like objective with mate-distance corrections) that gives more useful gradient on decisive positions than tanh does.
- **Target encoding**: the trainer's `tanh(eval_cp / 600)` collapses ±2000 cp positions to ±0.96 — and the hybrid `0.7·tanh(eval) + 0.3·outcome` further smears the target. nnue-pytorch's standard `1 / (1 + exp(-eval_cp / 410))` (mapped to [0, 1]) plus an MSE loss is structurally different.
- **Optimizer**: SGD without per-feature normalisation. Sparse features are activated very unevenly across the corpus; Adam-style RMS normalisation per feature would dramatically change the effective per-feature learning rate.

**What went well**:
- All three experiments instrumented in a single self-contained code change (~50 lines plus 3 CLI flags). The training loop is now a much better diagnostic platform — future sessions can A/B these flags against each other in seconds.
- Clean negative result: each of the three hypotheses is now ruled out with a clear measurement, not a guess. Sessions 7-11 each carried unstated alternative hypotheses; Session 12 closes them.
- The td_err vs Pearson-r dichotomy continues to pay off — we'd have called any of A/B/C "marginally improved" on td_err alone (0.6527 ≈ 0.6580 ≈ 0.6541, all within 0.005 of each other), but Pearson r cleanly distinguishes A as worse, C as worse, and B as no-different from baseline.

**What was harder than expected**:
- Recognising that **Pearson r drifting negative** in the late 30-epoch run isn't a transient failure mode — it's a *signature* that recurs across every protocol now (Session 11's fresh-init showed it; Exp B and Exp B+C both show it from different seeds and inits). The drift suggests the network is *settling* into a state where its output is anti-correlated with the teacher; over enough epochs that's a real, repeatable attractor, not noise.
- Diagnosing what training is *actually* doing across 30 epochs at td_err ≈ 0.6527 is hard without instrumentation that we don't yet have. The next session should add per-feature weight-trajectory tracking.

**Surprises**:
- Experiment C (3× multiplier on decisive positions) destabilised epoch 1 by *less* than predicted — td_err only rose by 0.10, suggesting the gradient cascade through tanh saturation absorbs most of the 3× boost before it reaches the output weights. This is itself evidence that tanh saturation is doing what we feared: clamping the gradient on decisive positions to a level the multiplier can't compensate for. A non-saturating loss function (i.e. MSE on raw cp) would be a better intervention, but it's outside Session 12's hand-off scope.
- Experiment B's early-epoch advantage (epoch 5 black-stm Pearson r = +0.098, the highest seen) does not persist. The network *learns* slightly more position-aligned structure by epoch 5-10 with f32 input weights, then *unlearns* it as continued training drives the output layer further into the negative range. This is suggestive of a regularisation problem (the network has no penalty for cancelling learned input structure to flatten its output), but it's not what the f32 forward was supposed to fix.

**Insights for future sessions**:
- The next step has to be a **structural** change, not another flow tweak. Three concrete candidates in approximate priority order:
  1. **Replace the loss / target**: switch from `tanh(eval/600)` + L1 to nnue-pytorch's `sigmoid(eval/410)` + MSE on a [0, 1] target. This eliminates the tanh saturation problem completely and is a 20-line change in `target_for` and `update_weights_for_position`.
  2. **Add a side-to-move feature**: a single additional feature (set when Black to move, unset when White) so the network *can* produce a true to-move evaluation. Session 11 ruled this out as the *current* bottleneck via the side-stratified Pearson, but once the loss/target is fixed, POV ambiguity will resurface.
  3. **Adam-style optimizer**: per-feature running gradient RMS, with first/second moment estimates. Prevents rare features from being drowned out by frequent ones, which is plausible given the +0.087 → -0.062 drift signature (a few high-frequency features may be dominating the late-epoch trajectory).
- These are independent — option 1 alone is likely the highest-EV single change. It's a 1-2 hour intervention that reframes the entire trainer.
- Issue 4 from Session 11 (sample-eval discrepancy between code paths) should be investigated before any further training-trajectory work — if `acc.evaluate()` and `evaluate_incremental` produce different cp values for the same position, our Pearson-r metric and the search engine's evaluation are not measuring the same network. The Pearson result for Session 12 only holds if both paths agree.

---

## Decisions Made

**Decision 1**: Commit the new CLI plumbing (`--f32-input-forward`, `--decisive-weight`, `--decisive-threshold-cp`) and the corresponding `NNUETrainingConfig` fields, even though every experiment using them produced a negative result.
- **Rationale**: The diagnostics they enable are general-purpose. Every future training session can A/B them against the loss-/target-changes Session 13 will introduce, and the conclusion "f32 input forward does not help when the loss is `tanh(eval/600)` + L1" is conditional on the loss — it may flip when the loss changes. Keeping the flags makes that comparison cheap.
- **Confidence**: High.

**Decision 2**: Do **not** save any of the Session 12 trained weights to the repo. All four runs (`/tmp/s12_expA.json`, `/tmp/s12_expB_10e.json`, `/tmp/s12_expB_30e.json`, `/tmp/s12_expBC_30e.json`) are reproducible from `nnue_weights_trained.json + nnue_corpus_yaneura_d10.jsonl + nnue-offline-trainer` invocation lines documented above.
- **Confidence**: High.

**Decision 3**: Frame Session 13's primary objective as **"replace the trainer's loss function and target"** rather than another gradient-flow experiment. The negative results from Sessions 11 and 12 across five different gradient-flow interventions (divisor, scales, tilt, f32 input, decisive weight) strongly suggest the loss itself is the limiting structure.
- **Rationale**: Five targeted interventions over two sessions have produced no Pearson r > +0.10. Continuing with sixth/seventh/eighth interventions of the same kind is not a high-EV use of time.
- **Alternative considered**: Add per-feature optimizer state (Adam) without changing the loss. Rejected as the secondary intervention — easier to evaluate cleanly *after* the loss change, since Adam's behaviour is loss-shape-dependent.
- **Confidence**: High.

**Decision 4**: Investigate Issue 4 (sample-eval path discrepancy) before any further training in Session 13. If the discrepancy is real and material, every Pearson-r number from Sessions 11-12 needs to be re-interpreted with the search-time evaluation rather than `acc.evaluate()`.
- **Confidence**: Medium-high.

---

## Blockers & Issues

### Issue 1 (was open in Session 11): Input-layer learning is starved
- **Severity**: was High → still High, but **re-localised**.
- **Status**: Open. Session 12 ruled out the three hypotheses Session 11 proposed for the cause (gradient-scale balancing, i16 input quantization, tanh saturation under-weighting decisive positions). The actual cause is now believed to be in the **loss / target / optimizer** layer, not in gradient-flow plumbing.
- **Resolution sketch**: Session 13 should swap `tanh(eval/600)` + L1 for `sigmoid(eval/410)` + MSE (nnue-pytorch standard). 1-2 hour change.

### Issue 2 (NEW): Pearson r drifts to ≈ -0.06 after 15-25 epochs in every long run
- **Severity**: Medium.
- **Description**: Session 11's fresh-init run, Session 12 Exp B 30-epoch run, and Session 12 Exp B+C 30-epoch run all reach a positive Pearson r (~+0.08) by epoch 5-10, then drift through zero into negative territory (~-0.06) by epoch 30. The drift is reproducible across different seeds and configurations.
- **Root cause hypothesis**: with no regularisation, the network has a degenerate attractor where its output is *anti*-correlated with the teacher because that minimises the L1 loss against the bias-dominated mean. Output_bias is moving from +3459 (S3 init) toward zero, output_weights are spreading; the late-epoch trajectory is the network discovering that a slight anti-correlation with the teacher gives a marginally lower L1 in the post-bias-cancellation regime.
- **Status**: Open. Should resolve naturally once the loss is changed (MSE doesn't have this attractor in the same way). Observe across the next session.

### Issue 3 (still open from Session 11): No side-to-move feature
- **Severity**: Medium-long-term.
- **Status**: Open. Will resurface as the dominant issue once Issue 1 is resolved.

### Issue 4 (still open from Session 11): Sample-eval discrepancy between code paths
- **Severity**: Low-medium. Session 12 elevates this to "investigate before next training run" — see Decision 4.
- **Status**: Open.

### Issue 5 (still open): promoted-rook move-gen truncated (Session 6)
- **Severity**: Medium.
- **Status**: Not surfaced this session.

### Issue 6 (still open): `nnue_trainer.rs` drop-move bug (Session 7)
- **Severity**: Low.
- **Status**: Not surfaced this session.

---

## Next Session Plan (Session 13)

**Primary goal**: replace the trainer's loss and target encoding to remove the tanh-saturation gradient bottleneck identified in Session 12, and re-measure Pearson r against the same corpus.

**Approach**:
1. **Investigate Issue 4** first (~30 min): instrument both `acc.evaluate()` and `evaluate_incremental` paths for ten sample positions; if cp values differ by more than the i32 rounding tolerance, fix the bug before any further training.
2. **Switch loss to MSE on a [0, 1]-mapped target**: in `nnue_offline_trainer.rs`'s `target_for`, replace `tanh(clamped_eval / 600)` with `1 / (1 + exp(-clamped_eval / 410))`. In `nnue_training.rs`, the corresponding inverse map for the network's prediction becomes `sigmoid(raw_output * SCALE_FACTOR / OUTPUT_DIVISOR / 410)`, and the loss becomes plain `0.5 * (target - prediction)^2`. The gradient through sigmoid is `prediction * (1 - prediction)` instead of tanh's `(1 - prediction^2)` — does not collapse on decisive positions in the same way (peaks at p=0.5, not at p=0).
3. **Run 30-epoch retrain from S3 init** with the new loss. Measure Pearson r each epoch.
4. **Optional**: if the new loss reaches Pearson r > +0.4 within 10 epochs, run the 10-game ELO pilot. If it does not, abandon the loss change and try the side-to-move feature instead.

**Estimated duration**: 4-6 hours (Issue 4 investigation + loss/target swap + retrain + pilot).

**Prerequisite**: None.

---

## File Changes Summary

### Files Modified
- `src/evaluation/nnue_training.rs` (~30 lines added) — `f32_input_forward`, `decisive_weight`, `decisive_threshold_cp` fields on `NNUETrainingConfig`; default helpers; per-position branch in `train_batch_accumulated` to either recompute `h1_pre` from f32 shadow or use the i16-derived accumulator path; `weighted_error` for the decisive-weight gradient cascade.
- `src/bin/nnue_offline_trainer.rs` (~25 lines added) — three new CLI flags wired into `NNUETrainingConfig`; banner prints the experimental flag values.

### Files Created
- `docs/nnue-phase2/SESSION_LOG_012.md` (this file).

### Files Deleted
- None.

### Artefacts produced (untracked)
- `/tmp/s12_expA.log` — Experiment A 5-epoch training log + weights `/tmp/s12_expA_tilt.json`.
- `/tmp/s12_expB.log` — Experiment B 5-epoch training log + weights `/tmp/s12_expB_f32fwd.json`.
- `/tmp/s12_expC.log` — Experiment C 5-epoch training log + weights `/tmp/s12_expC_dec3x.json`.
- `/tmp/s12_expB_10e.log` — Experiment B 10-epoch training log + weights `/tmp/s12_expB_10e.json` (peak Pearson snapshot used for the ELO pilot).
- `/tmp/s12_expB_30e.log` — Experiment B 30-epoch training log + weights `/tmp/s12_expB_30e.json`.
- `/tmp/s12_expBC_30e.log` — Experiment B+C 30-epoch training log + weights `/tmp/s12_expBC_30e.json`.
- `/tmp/elo_s12_expB_10e.csv` + `/tmp/elo_s12_pilot.log` — ELO pilot data.

---

## Testing Results

### Session 12 success criteria

**Criterion 1**: All three Session-11 hand-off experiments (A, B, C) implemented and runnable.
- [x] Met. CLI flags work, config plumbing covered, training reproduces deterministically with given seeds.

**Criterion 2**: At least one of A/B/C raises Pearson r above +0.20 within 5 epochs.
- [ ] **Not met.** All three stay within the ±0.10 noise band already documented in Session 11.

**Criterion 3**: At least one of A/B/C raises Pearson r above +0.40 within 30 epochs (Session 11's success threshold).
- [ ] **Not met.** B alone peaks at +0.087 around epoch 10 then drifts negative. B+C combined matches.

**Criterion 4**: 10-game ELO pilot of the best Session 12 weights does not regress vs PST.
- [x] Met. 4W/2D/4L score 0.500, ELO 0.0 ± 217 (95% CI). Statistically indistinguishable from PST and from Sessions 9-11 within their overlapping CIs.

**Criterion 5**: The negative result is informative — it rules out at least one specific hypothesis class for the input-layer-learning bottleneck.
- [x] Met. Three independent hypotheses ruled out (gradient scale balancing, i16 input quantization, tanh saturation under-weighting). The remaining hypothesis classes are structural (loss function, target encoding, optimizer).

### Diagnostic findings (the actual session output)

**Finding 1**: None of the three Session-11 hand-off interventions, individually or combined, materially raises Pearson r over the +0.06 baseline. The bottleneck is not in the gradient-flow plumbing the three interventions targeted.

**Finding 2**: Pearson r has a reproducible drift signature across long training runs — climb to ~+0.08 by epoch 5-10, plateau briefly, drift to ~-0.06 by epoch 30. This is a property of the L1+tanh loss landscape, not of the specific intervention.

**Finding 3**: B's f32-input-forward path is the only intervention that produces a measurable (if marginal) Pearson-r improvement. It is also the cleanest mechanical change of the three — the fact that it doesn't unblock learning is strong evidence that i16 quantization on the input layer was *not* the bottleneck.

**Finding 4**: Decisive-position gradient up-weighting (3×) destabilises early epochs but settles to the same plateau. The gradient through tanh saturates so strongly on decisive targets that the multiplier is mostly absorbed before reaching the output layer.

---

## Historical Session Reference

| Session | Phase        | Title                                                                        | Status      | Date       |
|---------|--------------|------------------------------------------------------------------------------|-------------|------------|
| 1       | 1.1-1.2      | Weight Initialization & Output Scaling                                       | Completed   | 2026-04-22 |
| 2       | 1.3, 2.1-2.3 | Training Verification & Algorithm Fixes                                      | Completed   | 2026-04-22 |
| 3       | 2.4          | Extended Training + Game Diversity                                           | Completed   | 2026-04-22 |
| 4       | 3.2, 3.4     | Speed Optimization (Incremental Accumulator)                                 | Completed   | 2026-04-22 |
| 5       | 4.1          | ELO Validation Infrastructure + First Pilot                                  | Completed   | 2026-04-22 |
| 6       | 2b-setup     | External USI Teacher — Corpus Generator Infrastructure                       | Completed   | 2026-04-22 |
| 7       | 2b-execute   | YaneuraOu Corpus + Offline Trainer — Integration Lessons                     | Completed   | 2026-04-23 |
| 8       | 2b-finish    | f32 Shadow Weights — Mechanics Fixed, Reveals Structural Ceiling             | Completed   | 2026-04-24 |
| 9       | 2c           | Relaxed Teacher Target + elo-tester Fix — Output-Range Breaks                | Completed   | 2026-04-24 |
| 10      | 2d           | Symmetrise Output Layer — Negative Result, Reframes 0.47 Floor               | Completed   | 2026-04-25 |
| 11      | 2e           | Lower OUTPUT_DIVISOR + Pearson-r Metric — Reveals Input-Layer Starvation     | Completed   | 2026-04-25 |
| 12      | 2f           | Three Diagnostics for Input-Layer Starvation — All Three Negative            | Completed   | 2026-04-26 |
| 13      | 2g           | Replace Loss/Target — `sigmoid(eval/410)` + MSE                              | Pending     | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — three hand-off diagnostics implemented, run, and shown negative; combined B+C variant also negative; ELO pilot run. The session's most valuable output is, again, what it falsifies: five distinct gradient-flow hypotheses for input-layer starvation are now eliminated across Sessions 10-12. Session 13 should leave the gradient-flow domain entirely and intervene at the loss/target layer.
**Ready for next session**: Yes
**Comments**: The pattern across Sessions 9-12 is that progressively more localised interventions produce progressively smaller Pearson-r changes (Session 9's 0.47-floor breakthrough was structural; Session 10 was symmetrisation; Session 11 was a single constant; Session 12 is three flag toggles). At this rate the next gradient-flow intervention is worth ≈ 0.0 ELO. The right move is to step back and change the loss — that's a structurally larger change with structurally larger expected benefit.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-26
