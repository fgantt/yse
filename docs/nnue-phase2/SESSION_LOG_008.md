# NNUE Implementation Session Log

## Session 8: f32 Shadow Weights — Mechanics Fixed, Reveals Deeper Structural Ceiling

**Date**: 2026-04-24
**Duration**: ~4 hours (≈2 h infrastructure + training, ≈2 h diagnosis)
**Phase**: Phase 2b-finish
**Objective**: Session 7 identified i16 weight quantization as the blocker preventing offline training from moving input-layer weights. Add f32 shadow weights, retrain on the existing 67 K YaneuraOu corpus, re-run the ELO pilot, and determine whether the "NNUE ≈ PST" ceiling finally breaks.

---

## Pre-Session Checklist

- [x] Session 7 diagnosis: i16 per-position updates round to 0 or saturate in one batch — blocks input-layer learning.
- [x] Plan: add `ShadowWeights` (f32 mirror of `NNUEWeights`) owned by `NNUETrainer`, accumulate updates in f32, re-quantize once per batch.
- [x] Corpus from Session 7 (`nnue_corpus_yaneura_d10.jsonl`, 67 403 positions, 93 % decisive) available.

---

## Work Completed

### Subtask 1: Implement f32 shadow weights in `NNUETrainer`
- **Status**: Completed (committed in `b952a59`).
- Added `ShadowWeights { input_weights_1: Vec<Vec<f32>>, hidden_biases_1: Vec<f32>, input_weights_2: Option<Vec<Vec<f32>>>, hidden_biases_2: Option<Vec<f32>>, output_weights: Vec<f32>, output_bias: f32 }` in `src/evaluation/nnue_training.rs`.
- `ShadowWeights::from_weights` copies the canonical i16 weights into f32. `ShadowWeights::quantize_into` renders back via `clamp + round as i16`, returning `(total_change, max_change, change_count)` for stats.
- `NNUETrainer::new` initialises `shadow` at construction time.
- `train_batch_accumulated` was rewritten: the f32 backprop now writes into `self.shadow` (no clamping, no rounding), and a single `quantize_into(&mut self.weights)` call renders the shadow into i16 weights once per batch. This means sub-unit updates that used to round to 0 now accumulate and eventually cross i16 thresholds.
- Added `resync_shadow_from_weights` (for mid-training checkpoint loads) and `get_shadow` (for diagnostics).

### Subtask 2: Add `nnue-weight-diff` binary
- **Status**: Completed (committed in `b952a59`).
- New file `src/bin/nnue_weight_diff.rs` (~136 lines). Reports per-layer n / changed-count / changed-% / L1 / max_delta / min+max ranges for two weight files, plus NNUE evaluations at three sample FENs (startpos, mid-game, black-winning).
- This tool was essential for answering "did training do anything?" — it made the contrast between the Session-7 saturated weights and the Session-8 shadow-trained weights immediately visible.

### Subtask 3: Retrain on YaneuraOu corpus with shadow weights — three variants
- **Status**: Completed.
- All runs: 30 epochs, batch size 256, on `nnue_corpus_yaneura_d10.jsonl`, init from `nnue_weights_trained.json` (Session-3 PST-imitation weights) unless noted.
- Variants produced:
  - `nnue_weights_shadow_trained.json` — `lr=0.005, out_scale=1e4, in_scale=1e3` (Session-7 defaults). Very conservative: 0.1 % of input weights moved, max delta 2, input range preserved [-4, 4].
  - `nnue_weights_shadow_highlr.json` — higher effective rate. 7.8 % input weights moved, max delta 23, input range widened to [-11, 23].
  - `nnue_weights_shadow_fresh.json` — random init (no `--init-weights`). 49.9 % of input weights differ from Session-3 baseline (as expected — different init), but output biases collapse toward 0 and all sample positions evaluate to 1-5 cp.
- All variants show healthy weight-file numerics (no saturation, no collapse to degenerate state like Session-7's post-train weights had).

### Subtask 4: Re-run `elo-tester` on shadow-trained variants
- **Status**: Completed.
- Config: `--games 20 --depth 3 --time-ms 150 --random-plies 16 --seed 123`.
- Results:
  ```
  nnue_weights_trained.json         (baseline PST):       0W 20D 0L
  nnue_weights_shadow_trained.json  (conservative train): 0W 20D 0L
  nnue_weights_shadow_highlr.json   (aggressive train):   0W 20D 0L
  Same with random-plies 24, seed 777:                    0W 20D 0L
  ```
- Weight files clearly differ (verified via `nnue-weight-diff`) and produce position-varying evaluations — but all three produce identical gameplay at this test configuration.

### Subtask 5: Investigate why ELO test is weight-insensitive
- **Status**: Completed — not a weight issue, a test-config issue **and** a deeper structural issue.
- Ran `elo-tester --games 1 --verbose` at `time-ms 150`: **both engines return `eval=0` for every move from ply 17 onward** and drift into a shuffle-and-repeat pattern (8h↔8g for one side, 3a↔3b for the other) until 3-fold repetition triggers a draw. Wall time ~1 s for 20 games confirms no real search is happening.
- Re-ran same game at `time-ms 500`: both engines now return meaningful `cp` evals (NNUE 343, PST 397, etc.) and play a very different, actually-progressing game (pawn pushes, piece promotions at depth 2-3).
- **Diagnosis**: at 150 ms the iterative-deepening search bails out of depth 3 before producing a scored PV, and both engines end up returning `eval=0` which ties every legal move in the move-picker. At 150 ms the test is weight-blind by construction. Session 5's and Session 7's 0/20 pilots were measuring the same thing.
- Implication: **the current `elo-tester` + 150 ms setting cannot measure weight quality**. Any ELO comparison needs ≥ 500 ms/move or a fixed-depth search that is guaranteed to complete.

### Subtask 6: Instrument training loss — is the model actually learning?
- **Status**: Completed.
- Added `--verbose` to offline trainer (prints per-20-batch `td_err` + `w_chg`). Ran several short (3-epoch) experiments scanning `input_grad_scale ∈ {1e3, 1e5, 1e6}`:
  ```
  in_scale=1e3:  td_err  0.6644 → 0.6630 → 0.6562   input_w1 changed   0.1 %   max_delta  2
  in_scale=1e5:  td_err  0.6644 → 0.6562 → 0.6527   input_w1 changed   2.5 %   max_delta  24
  in_scale=1e6:  td_err  0.6644 → 0.6528 → 0.6529   input_w1 changed   8.1 %   max_delta  76
  ```
- **Input-layer gradients DO reach the input layer** (at `1e6` the input range widens to [-44, 76]). Shadow weights solved the "gradients rounded to zero" problem.
- **But TD error barely moves regardless of how much input weights are updated** — at all three scales, 3 epochs drives td_err by ≈ 0.01. This rules out "input-layer gradients are too small" as the root cause.

### Subtask 7: Trace the forward pass to identify the real ceiling
- **Status**: Completed.
- NNUE output formula: `raw = output_bias + Σ_j (h2_activated[j] * output_weights[j]) >> 6; output_cp = raw * SCALE_FACTOR / FINAL_DIVISOR` where `SCALE_FACTOR = 400`, `FINAL_DIVISOR = 255 * 64 = 16320`.
- For the Session-3 baseline (`nnue_weights_trained.json`): `output_bias = 3459`, `output_weights ∈ [-1, 3]`, so the `h2 · output_weights >> 6` contribution is at most ±150 or so. Result: `raw ≈ 3459 ± 150`, which maps to `output_cp ≈ 84 ± 4 cp`. The bias alone produces 84 cp; the weighted-activation signal contributes single-digit cp variation.
- Sample-position confirmation (from `nnue-weight-diff`):
  ```
                       startpos   mid-game   black-winning
  baseline trained          84         84              84     (pure bias)
  shadow_trained            22         18              31     (small variation)
  shadow_highlr              8          6              44     (more variation)
  shadow_fresh               4          5               1     (collapsed to 0)
  ```
- Training target for decisive positions is `tanh(eval_cp / 600)` ≈ ±1; prediction is `tanh(output_cp / 400)`. With `output_cp ≈ 30` max, `prediction ≈ 0.075`. For a target of ≈ 0.8 (decisive teacher eval), `|error| ≈ 0.725`. **This is exactly the TD-error floor we observe (~0.65)**.

### Subtask 8: Answer the user's question: tuning or structural?
- **Status**: Completed.
- **Structural.** Three distinct factors compound:
  1. **Session-3 weights are bias-dominated.** `output_weights` range [-1, 3] and `output_bias = 3459` mean the network's learnable output signal is two orders of magnitude smaller than its constant bias term. Any eval ≈ `84 ± (noise)` cp.
  2. **Training target magnitude cannot be reached by the network.** Targets after `tanh(eval/600)` reach ±1 for any decisive position, which would require `|raw_output|` near `16320`. With `output_weights` in the ±3 range and h2 activations in the ±100 range, the achievable `|raw_output|` is ≈ `32 * 100 * 3 / 64 ≈ 150` — two orders of magnitude short of the target scale.
  3. **The gradient chain is highly compressed.** `d_raw = error * (1 - p²) / 16320` plus three `>> 6` (/64) reductions in backprop. Even with `input_grad_scale = 1e6`, a single-unit change in an i16 input weight requires hundreds of batches of consistent gradient direction.
- Tuning `learning_rate` / `input_grad_scale` / `outcome_weight` cannot fix this. Increasing the scales just makes input weights move faster without increasing the achievable output amplitude, because the bottleneck is at the output layer: `h2 · output_weights >> 6` is a dot product of 32 small-magnitude numbers and cannot produce `±16320` unless `output_weights` themselves grow into the ±1000s — which in turn would need hundreds of epochs of output-layer-only learning before the input layer gets meaningful gradients.

---

## Testing & Verification

### Build Check
```
cargo build --release  →  Pass (existing warnings only)
```

### Functional Tests
```
Test 1: Shadow weights initialise from i16 weights on NNUETrainer::new        → Pass
Test 2: train_batch_accumulated writes to shadow, quantizes once per batch    → Pass
Test 3: nnue-weight-diff reports non-zero deltas after training               → Pass
Test 4: shadow-trained variants produce position-varying NNUE evals           → Pass
Test 5: shadow-trained variants beat PST at self-play (20-game pilot)         → FAIL (0/20/0)
Test 6: training TD error decreases materially within 30 epochs               → FAIL (0.66 → 0.65 plateau)
Test 7: elo-tester at 150 ms/move produces decisive games                     → FAIL (search bails at eval=0)
```

### Performance / Metrics
```
Trainer throughput: 67 403 pos / 1.9 s = ~35 000 pos/sec/epoch (matches Session 7).
Input-layer reach (in_scale=1e6, 3 epochs): 8.1 % of 580 608 input weights changed; range widened from [-4, 4] to [-44, 76]. Gradients do reach the input layer.
TD error floor: ≈ 0.65 across all scale settings — matches the predicted ceiling `≈ |target| - |max reachable prediction|`.
```

---

## Observations & Insights

**What went well**:
- The shadow-weights implementation is clean and load-bearing: it genuinely fixes the Session-7 "weights won't move" symptom. Weight-diff confirms updates now reach every layer.
- Adding `nnue-weight-diff` up-front (before retraining) paid off immediately — without it, the shadow code would have looked like another null result; with it, the *mechanics* were visibly healthy even though the strength was not.
- The iterative narrowing of the diagnosis — shadow-mechanics OK → input gradients OK at higher scale → TD error still plateaus → trace the forward pass → arithmetic matches the observed error floor — avoided the Session-7 trap of cycling through hyperparameters without a model of what was bounding the result.

**What was harder than expected**:
- Discovering mid-session that the ELO-tester is *intrinsically* weight-insensitive at 150 ms/move. Both NNUE and PST return `eval=0` from their iterative-deepening search under that time budget, so the pilot compares identical move-pickers against each other regardless of weights. Sessions 5 and 7 measured the same uninformative zero.
- Re-running at 500 ms produced meaningful, *different* moves — but at 500 ms a 20-game pilot at depth 3 would take several minutes, not the 1 s we were getting. The "null pilot" in Sessions 5-7 was fast precisely because the search was collapsing, not because the engines were efficient.
- It took a forward-pass arithmetic trace to realise the TD-error floor isn't a training bug — it's the exact magnitude predicted by the output layer's bias-dominated behaviour.

**Surprises**:
- **The TD-error floor is literally computable from the bias and weight magnitudes.** `output_cp ≈ bias * 400/16320 ≈ 84`, `prediction ≈ tanh(84/400) ≈ 0.21`, target mean |t| ≈ 0.8 for decisive positions → floor ≈ 0.6. This is not a deep-learning-is-hard plateau; it's a "you can't output cp > 30 no matter what you train" ceiling.
- **Aggressive `input_grad_scale` does not help td_err** even though it materially moves input weights. Any learning in the input layer still has to propagate through `output_weights ∈ [-1, 3]` to affect the output, and the bottleneck is the output layer's magnitude, not the input layer's ability to differentiate features.
- **`shadow_fresh` (random init) collapses to near-zero predictions** rather than exploring the output space. The random-init output_bias (~50) is much smaller than Session-3's (3459), so the starting prediction is ~0; once there, the model has no gradient signal that pulls the output_bias up to the target scale, because doing so would require large coordinated `output_weights` × `h2_activations`.

**Insights for future sessions**:
- The ELO-tester needs a fixed time budget that actually lets depth-3 search complete (≥ 300 ms/move empirically), or preferably a fixed-depth/fixed-node mode. Without that, any weight comparison is noise.
- The Session-1 output scaling choice (`* 400 / 16320`) was made for a Stockfish-style architecture with `output_weights` in the ±1000s; it doesn't fit a PST-imitation starting point with `output_weights` in the ±3s. Either the weights need a magnitude bump (not easy in i16 quantised space) or the output scaling needs to be relative to the achievable output range.
- One concrete structural fix: **re-initialise `output_weights` with larger magnitudes and `output_bias = 0`** before teacher-target training, then let the teacher signal drive the output_bias back up. This is effectively what a fresh random-init Stockfish trainer does and what `shadow_fresh` attempted but did not finish — it collapsed rather than converged because the gradient signal was also insufficient to lift the output magnitude from scratch.
- A smaller, deeper question: the training target `tanh(eval/600)` is saturating at ±1 for eval > 1800, pushing the model toward an output range it cannot reach. An alternative target like `eval_cp / 1000` (no tanh) or `tanh(eval/2400)` (less-saturating) may be a better fit for the current architecture's achievable output range.

---

## Decisions Made

**Decision 1**: Answer "is it tuning or structural?" with structural, and stop tuning the existing training loop.
- **Rationale**: The td_err floor ≈ 0.65 is a closed-form prediction from `bias * SCALE/DIVISOR` and the mean target magnitude. No `(lr, out_scale, in_scale, batch_size, epochs)` choice can move it because none of them can grow `output_weights` and `h2_activation` products fast enough to reach the target scale in reasonable time.
- **Confidence**: High (arithmetic matches observation).

**Decision 2**: Do not commit the three shadow-trained weight files or the 67 K corpus.
- **Rationale**: Reproducible from source + a one-line training command. Training artefacts. Same policy as Session 7.
- **Confidence**: High.

**Decision 3**: Session 9 should attack the output-magnitude bottleneck, not add more training data, epochs, or scale-tuning.
- **Rationale**: Generating more corpus won't help a network that structurally can't output values in the target range. Three concrete avenues (below) each address the structural ceiling rather than iterating around it.
- **Confidence**: High.

**Decision 4**: Do not touch the `elo-tester` time budget / search protocol this session.
- **Rationale**: Deserves a dedicated session to settle on "fixed-depth vs bigger time" and rerun the whole Session-5-onward pilot matrix against a now-reliable measurement. Scope-creeping it here would mix training-infra work with testing-infra work and make both harder to judge.
- **Confidence**: High.

---

## Blockers & Issues

### Issue 1 (resolved): i16 weight quantization blocks gradient flow (Session 7 carry-over)
- **Severity**: High
- **Resolution**: f32 shadow weights in `NNUETrainer` — confirmed by weight-diff: gradients reach all layers, no saturation, no rounding to zero.
- **Status**: Resolved.

### Issue 2 (open, new): Output-magnitude ceiling bounds TD error at ~0.65
- **Severity**: High (blocks Phase-4 ELO improvement)
- **Description**: With the Session-3 PST-imitation starting weights, the network output `raw = output_bias + (h2 · output_weights) >> 6` is dominated by the bias term (3459) while the learnable signal term has magnitude ≤ ~150. Mapped through `* 400 / 16320`, this is ~84 cp constant + ~4 cp signal. Training targets span ±1 (decisive positions); predictions can only span ~±0.08. Error floor ≈ 0.6.
- **Root cause**: Mismatch between the Stockfish-style output scaling (`SCALE/DIVISOR = 400/16320`) and the actual weight magnitudes produced by PST-imitation training.
- **Planned resolution paths (Session 9)**: see "Next Session Plan" below — three concrete options.
- **Status**: Open.

### Issue 3 (open, new): `elo-tester` at default 150 ms/move is weight-insensitive
- **Severity**: High (invalidates Phase-4 pilot methodology from Sessions 5-7)
- **Description**: Both engines return `eval=0` and drift into 3-fold repetition via move-picker tiebreaks. Not a bug in any one engine — both hit the time limit before iterative deepening produces a scored bestmove. Wall time ≈ 0.05 s/game confirms no real search.
- **Planned resolution**: Either raise `--time-ms` to ≥ 500 or add a `--fixed-depth` mode that disables the time cutoff. Then rerun Session-5/7/8 pilot configurations for comparability.
- **Status**: Open, deferred to Session 9 or a dedicated testing session.

### Issue 4 (still open): promoted-rook move-gen truncated (Session 6)
- **Severity**: Medium
- **Status**: Not surfaced this session.

### Issue 5 (still open): `nnue_trainer.rs` drop-move bug (Session 7)
- **Severity**: Low (PST training doesn't read hand state).
- **Status**: Tracked; no PST retrain attempted this session.

---

## Next Session Plan (Session 9)

**Primary goal**: close Issue 2 by giving the network the structural capacity to reach teacher-target magnitudes.

**Three candidate approaches** (ordered by cost/risk):

1. **Rescale the output normalization** — change `FINAL_DIVISOR` (or introduce a per-file scaling constant) to match the achievable raw output range with the current weights. E.g., if `output_weights ∈ [-3, 3]` and `h2 ∈ [0, 100]` produce raw ≤ ~150, use `DIVISOR = 150` (giving ≤ 400 cp output). Cheap to try (a single constant + retrain). Risk: changes the meaning of weights file format; existing saved weights become mis-scaled.

2. **Re-initialise `output_weights` with larger magnitude and `output_bias = 0`, then train from there** — rather than starting from Session-3 PST-imitation weights, write a one-off tool that takes Session-3 input/hidden weights but replaces the output layer with a fresh `N(0, σ)` initialisation with σ chosen so expected `|raw|` ≈ 8000 (half of `FINAL_DIVISOR`). Train with teacher targets from that. Medium cost: small new binary. Risk: output layer learning while lower layers are already useful may produce a degenerate intermediate.

3. **Change the training target to a less-saturating function** — replace `tanh(eval/600)` with `tanh(eval/2400)` or a clamped linear `(eval / 2000).clamp(-1, 1)`. Let the network learn a range it can actually reach. Cheap (one line in `nnue_offline_trainer.rs`). Risk: learned network will under-score decisive positions; may or may not still improve over PST in practice.

Recommended order: **(3) first** (cheapest, tells us if the current weights can learn *any* real signal), then **(1)** (if (3) shows learning but output range is still too compressed), then **(2)** (if the first two don't break the ceiling).

**Secondary goals (if time):**
- Fix `elo-tester` time budget (Issue 3) so Session 9's training results can be measured.
- Run Session 3's PST imitation again with `corpus_gen.rs`-style drop-move tracking (Issue 5) — optional, doesn't block NNUE progress.

**Estimated duration**: 3-5 hours.
**Prerequisite**: None.

---

## File Changes Summary

### Files Modified (committed in `b952a59`)
- `src/evaluation/nnue_training.rs` — added `ShadowWeights`, wired into `NNUETrainer::new` / `train_batch_accumulated` / `get_weights_mut` / `resync_shadow_from_weights` / `get_shadow`. ~300 lines added/modified.
- `Cargo.toml` — added `[[bin]] nnue-weight-diff`.

### Files Created (committed in `b952a59`)
- `src/bin/nnue_weight_diff.rs` (~136 lines) — per-layer weight-diff + sample-position eval tool.
- `docs/nnue-phase2/SESSION_LOG_008.md` (this file).

### Files Deleted
- None.

### Artefacts produced (untracked)
- `nnue_weights_shadow_trained.json` + `_epoch_{10,20,30}.json` (conservative train from Session-3 baseline).
- `nnue_weights_shadow_highlr.json` + `_epoch_{5,10,15}.json` (higher scale).
- `nnue_weights_shadow_fresh.json` + `_epoch_{10,20,30}.json` (random init).
- `/tmp/elo_s8_{baseline,shadow_trained,shadow_highlr,highlr_s777}.csv` (four pilot runs — all 0/20/0 draws, confirmed uninformative).
- `/tmp/nnue_diag_in{1e5,1e6}.json` (diagnostic training runs for scale sweep).

---

## Testing Results

### Session 8 success criteria

**Criterion 1**: Shadow-weights mechanics work end-to-end (init, backprop writes to shadow, quantization updates i16 weights).
- [x] Met (weight-diff confirms deltas at every layer; no saturation).

**Criterion 2**: Trainer produces weight files that differ measurably from the Session-3 baseline.
- [x] Met (0.1 %-8.1 % of input weights changed depending on scale; output layer moves meaningfully).

**Criterion 3**: ELO pilot shows NNUE > PST (> 50 % score).
- [ ] **Not met** — 0 / 20 / 0 across three variants.
- Root cause split into two independent issues: (i) elo-tester at 150 ms is weight-blind (Issue 3), (ii) td_err floors at ~0.65 because the network structurally cannot output target-scale values (Issue 2).

**Criterion 4**: Understand *why* the retrained NNUE doesn't improve strength, to level-set Session 9.
- [x] Met. Output-magnitude ceiling is computable from the architecture and matches the observed td_err floor. Session 9 work is not tuning — it's output-range structural work.

---

## Historical Session Reference

| Session | Phase        | Title                                                           | Status    | Date       |
|---------|--------------|-----------------------------------------------------------------|-----------|------------|
| 1       | 1.1-1.2      | Weight Initialization & Output Scaling                          | Completed | 2026-04-22 |
| 2       | 1.3, 2.1-2.3 | Training Verification & Algorithm Fixes                         | Completed | 2026-04-22 |
| 3       | 2.4          | Extended Training + Game Diversity                              | Completed | 2026-04-22 |
| 4       | 3.2, 3.4     | Speed Optimization (Incremental Accumulator)                    | Completed | 2026-04-22 |
| 5       | 4.1          | ELO Validation Infrastructure + First Pilot                     | Completed | 2026-04-22 |
| 6       | 2b-setup     | External USI Teacher — Corpus Generator Infrastructure          | Completed | 2026-04-22 |
| 7       | 2b-execute   | YaneuraOu Corpus + Offline Trainer — Integration Lessons        | Completed | 2026-04-23 |
| 8       | 2b-finish    | f32 Shadow Weights — Mechanics Fixed, Reveals Structural Ceiling| Completed | 2026-04-24 |
| 9       | 2c           | Output-Range Fix (rescale DIVISOR / re-init output / relax target) | Pending   | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — the concrete infrastructure win (f32 shadow weights) is committed and verified; the ELO result is unchanged from Session 7 but now with a *correct* causal explanation rather than a tuning shopping list.
**Ready for next session**: Yes
**Comments**: The session's headline finding is that the training plateau is not a tuning problem, it's an architecture-scale mismatch: the Session-3 PST-imitation init puts the network in a corner of weight space where the output layer cannot produce cp values > ~90, while the training targets reach ~2000 cp. Shadow weights correctly fixed the issue they were designed for (gradients reaching the input layer); they then exposed the real ceiling, which the scale-tuning cycle in Session 7 had hidden. Session 9 needs to attack the output-magnitude ceiling structurally, and also repair the elo-tester measurement protocol so any future training gain is actually observable.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-24
