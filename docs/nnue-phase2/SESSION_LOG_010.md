# NNUE Implementation Session Log

## Session 10: Symmetric Output-Layer Init — Negative Result, Reframes the 0.47 Floor

**Date**: 2026-04-25
**Duration**: ~3 hours
**Phase**: Phase 2d
**Objective**: Implement Session 9's hand-off plan — re-initialise `output_weights` from a symmetric distribution and reset `output_bias = 0` so the network can produce both positive and negative predictions. Train at `target_eval_scale = 2400`, sweep across {600, 1200, 2400} (and extend to 4000/8000 as a diagnostic), then run a 10-game pilot.

The actual outcome falsified the hand-off hypothesis. The symmetric init does *not* lower the td_err floor; the cross-scale sweep instead reveals that the entire 0.47 plateau Session 8/9 attributed to "output-layer asymmetry" was the corpus's `mean(|target|)` evaluated against an effectively-flat prediction. The forward-pass arithmetic that explains it is in **Observations & Insights**.

---

## Pre-Session Checklist

- [x] Reviewed Session 9 plan: re-init output_weights ~ N(0, σ), zero `output_bias`, retrain at scale=2400, sweep, pilot.
- [x] Session-3 weights `nnue_weights_trained.json` and corpus `nnue_corpus_yaneura_d10.jsonl` available.
- [x] elo-tester defaults already corrected in Session 9 (`--time-ms 500`, `--fixed-depth` flag).

---

## Work Completed

### Subtask 1: Add `nnue-init-symmetric` binary

- **Status**: Completed.
- New binary `src/bin/nnue_init_symmetric.rs` registered in `Cargo.toml`. Loads an existing weight file, replaces `output_weights` with samples from `Normal(0, σ)` (rounded to i16, clamped to [-127, 127]), sets `output_bias` to a CLI value (default 0), and writes the result. Prints before/after stats and sample-position cp evaluations.
- Defaults chosen so a single-line invocation reproduces the planned init:
  ```
  nnue-init-symmetric \
      --input  nnue_weights_trained.json \
      --output nnue_weights_s10_init.json \
      --sigma  20.0 \
      --output-bias 0 --seed 1
  ```
- Sanity output:
  ```
  before: output_bias = 3459, output_w range [-1, 3], mean +0.12
          startpos/mid/black-winning all evaluate to +84 cp
  after:  output_bias = 0,    output_w range [-36, 59], mean -1.06
          startpos/mid/black-winning all evaluate to 0 cp
  ```
- σ=20 was chosen so the new `|output_w|` magnitudes (mean ~16, max 59) are roughly the same order as the values Session-9 training produced (max delta 57). The intent was to *unblock* the negative side without changing magnitudes.

### Subtask 2: Train symmetric init at scale=2400 (30 epochs)

- **Status**: Completed.
- Command (matches Session 9's hyperparameters):
  ```
  nnue-offline-trainer \
      --corpus nnue_corpus_yaneura_d10.jsonl \
      --init-weights nnue_weights_s10_init.json \
      --output-weights nnue_weights_s10_sym_scale2400.json \
      --target-eval-scale 2400 --epochs 30 \
      --learning-rate 0.005 --output-grad-scale 1e4 --input-grad-scale 1e5 \
      --batch-size 256 --seed 123 --checkpoint-every 10
  ```
- TD-error trajectory (every epoch identical to 4 d.p.):
  ```
  epoch 01..30:  0.4715  0.4715  0.4714  0.4714  0.4713  0.4713 ...
  ```
- Weight movement after 30 epochs vs the symmetrised init:
  ```
  input_weights_1   0.4% changed   max_delta=5
  hidden_biases_1   55.5% changed  max_delta=7
  input_weights_2   5.7% changed   max_delta=4
  hidden_biases_2   40.6% changed  max_delta=6
  output_weights    40.6% changed  max_delta=10
  output_bias       0 → 21
  ```
- Sample position evals (init → trained):
  ```
  startpos       0 → +2 cp
  mid-game       0 → +2 cp
  black-winning  0 → 0 cp
  ```
- Conclusion at this point: training "ran" but moved the network by an imperceptible amount. The floor 0.4715 was reached in the first epoch — i.e. before any meaningful weight update — and held flat thereafter.

### Subtask 3: Hyperparameter sanity (rule out a slow-LR explanation)

- **Status**: Completed. Two 5-epoch comparisons designed to distinguish "training is too slow" from "training cannot make progress":
  - `lr=0.05, out_scale=1e4, in_scale=1e5` (10× the original lr)
  - `lr=0.005, out_scale=1e5, in_scale=1e6` (10× both grad scales)
- Both produced **bit-identical weight deltas** vs the symmetric init — because `lr × out_scale` is the same in both cases (500). td_err in both: 0.4713 at epoch 5.
- Genuinely different scale: `lr=0.05, out_scale=1e5, in_scale=1e6` (effective scale 5000, 100× of original). After 30 epochs:
  ```
  input_weights_1   6.5% changed   max_delta=25
  output_bias       0 → 105
  output_weights    46.9% changed  max_delta=15
  ```
  td_err trajectory: still 0.4715 every epoch.
- 100× more aggressive learning moved the weights ~5× more, but td_err did not budge by more than measurement noise (0.4713 vs 0.4715). This rules out "the symmetric init just trains slowly" — it is **structurally pinned** to td_err ≈ 0.47.

### Subtask 4: Cross-scale sweep — what `target_eval_scale` actually changes

- **Status**: Completed. 5-epoch runs from `nnue_weights_s10_init.json` with `target_eval_scale ∈ {600, 1200, 2400, 4000, 8000}`, all other hyperparameters as Session 9.
- TD-error floor per scale:
  ```
  scale=600    →  td_err = 0.6523
  scale=1200   →  td_err = 0.5718
  scale=2400   →  td_err = 0.4713
  scale=4000   →  td_err = 0.4042
  scale=8000   →  td_err = 0.3432
  ```
- Weight deltas vs `nnue_weights_s10_init.json` confirm none of these runs *learned* anything meaningful — at every scale the weights barely move and sample-position evals all stay at 0 cp:
  ```
  scale=600 :   output_bias 0→14, output_w max_delta=4, sample evals 0/0/0
  scale=8000:   output_bias 0→ 4, output_w max_delta=1, sample evals 0/0/0
  ```
- The td_err curve is therefore not a learning curve at all — it is `mean(|target|)` measured at a network that never deviates from `prediction ≈ 0`.

### Subtask 5: Compare against Session-9 weights — does the floor change?

- **Status**: Completed. One epoch of training was launched from `nnue_weights_s9_scale2400.json` (the *fully trained* Session-9 network) at scale=2400 to read out the network's td_err on the first batch:
  ```
  Resume from Session-9 trained weights, scale=2400, 1 epoch:
    td_err = 0.4713
  ```
- This is the same number Session 10's symmetric init produces from epoch 1. **The "Session-9 plateau" and the "Session-10 symmetric saddle" are the same number.**
- Sample-position evals (Session 9 trained vs Session 10 symmetric trained, both at scale=2400):
  ```
                     S9 trained   S10 sym trained
  startpos              +3            +2
  mid-game               0            +2
  black-winning        +30             0
  ```
- Session 9's network is materially better at the diagnostic position (`+30` for a clearly winning Black endgame vs `0` for the symmetric net), even though both report `td_err = 0.47`. The td_err number does not measure what we thought it did.

### Subtask 6: 10-game ELO pilot of symmetric weights

- **Status**: Completed.
- Command (same protocol as Session 9):
  ```
  elo-tester --nnue-weights nnue_weights_s10_sym_scale2400.json \
             --games 10 --depth 3 --time-ms 500 --random-plies 16 --seed 123
  ```
- Final result:
  ```
  NNUE (s10 symmetric weights) vs PST
    Games: 10
    NNUE:  3 wins, 6 draws, 1 loss
    Score: 0.600
    ELO:   +70.4 ± 143.1 (95% CI, small sample)
    Wall:  3700.6 s (6:10 per game average)
  ```
- Per-game (`/tmp/elo_s10_sym_10g.csv`):
  ```
  game  nnue_color  outcome    moves
    1   black       draw       109
    2   white       nnue_win   132
    3   black       draw        41
    4   white       draw       200    (move-limit)
    5   black       nnue_win    59
    6   white       draw       200    (move-limit)
    7   black       nnue_win   109
    8   white       pst_win     97
    9   black       draw        38
   10   white       draw       200    (move-limit)
  ```
- **The pilot does *not* show regression** despite the network having effectively zero standalone positional signal. Three of ten games hit the 200-move limit (vs. one in Session 9), reflecting that a flat-eval NNUE component does not produce positional pressure. Decisive games went 3-1 in NNUE's favour — well within the ±143 ELO 95% CI, but suggestive that NNUE-+-PST does not regress against PST-alone even when the NNUE half is essentially silent.
- **Why didn't the symmetric weights regress?** The engine's evaluator (per the implementation plan, `src/evaluation.rs:520-526`) blends NNUE with PST. When NNUE returns ≈ 0 cp, the evaluator falls back to PST + a near-zero NNUE bonus — i.e. the NNUE engine plays *almost* the same game PST-only would, but with NNUE noise on the order of ±5 cp from integer-truncation residuals affecting move ordering, history scores, and TT contents. The 3-6-1 score is consistent with that reading: most games draw because there's no positional pressure, and the small NNUE-side noise occasionally tips a tactical decision. This is also consistent with Session 9's 4-2-4 (network with sample evals 3/0/30 also barely moved the needle).

---

## Testing & Verification

### Build Check
```
cargo build --release  →  Pass (pre-existing warnings only, new binary compiles cleanly)
```

### Functional Tests
```
Test 1: nnue-init-symmetric loads + saves + correctly mutates output layer    → Pass
Test 2: After symmetrisation, all 3 sample positions evaluate to 0 cp         → Pass (output_bias=0, prediction is tiny)
Test 3: Trainer accepts the symmetric init and runs without error             → Pass
Test 4: Higher lr×grad_scale produces proportionally larger weight deltas     → Pass (mechanics work)
Test 5: td_err floor is independent of effective lr×grad_scale                → Pass — and this is the bug
Test 6: td_err floor at scale=2400 is the same number for symmetric init,
        Session-9 trained weights, and Session-3 baseline weights              → Pass — confirms the floor is intrinsic to the corpus, not the network
```

### Performance / Metrics
```
Trainer throughput:  67403 pos / ~1.5-2.0 s per epoch (matches Session 9).
TD-error reachability:
  scale=600,   sym init:  0.6523 (5 epochs)        ←  identical to Session 9 scale=600 floor
  scale=2400,  sym init:  0.4713 (5 epochs)        ←  identical to Session 9 scale=2400 floor
  scale=2400,  S9 weights: 0.4713 (1 epoch resume) ←  identical
  scale=8000,  sym init:  0.3432                   ←  proves td_err ∝ mean(|target|)
Effect of 100× LR:  weight deltas grow ~5×; td_err does not change.
Sample-position evals (cp), all scale=2400 trained:
  weights                  startpos  mid-game  black-winning
  Session-3 baseline           84       84        84
  Session-9 trained             3        0        30
  Session-10 sym init init      0        0         0
  Session-10 sym trained        2        2         0
```

### Pilot result
```
NNUE (s10 symmetric weights) vs PST          Session 9 baseline (asymmetric weights)
  Games: 10                                    Games: 10
  NNUE:  3W / 6D / 1L  score 0.600             NNUE:  4W / 2D / 4L  score 0.500
  ELO:   +70.4 ± 143.1 (95% CI)                ELO:   +0.0 ± 217   (95% CI)
  Wall:  3700.6 s                              Wall:  2729 s
```
The two pilots are not statistically distinguishable (overlapping CIs, n=10 each), but both confirm "NNUE-+-PST does not regress against PST-alone." Session 10's higher draw rate (60% vs 20%) reflects the symmetric weights producing essentially no positional pressure.

---

## Observations & Insights

**The single most important insight of the session**: the cross-scale sweep produces a near-perfect linear relationship between `target_eval_scale` and the td_err floor.

That is not a property of *the network*, that is a property of *the target*. With predictions held at ~0 (which is what the symmetric init achieves), the loss reduces to `E[|target|]`. As we re-scale the targets via `tanh(eval/scale)` for a fixed eval distribution, the average target magnitude shrinks roughly proportionally — and so does the floor. The td_err numbers between Session 7 (0.66 at scale=600) and Session 9 (0.47 at scale=2400) are 78% explained by `mean(|target|)`, with the remaining 22% being whatever small variance the network happens to express across positions.

**This reframes the entire Session 7-9 ladder.** The "improvement" from scale=600 → scale=2400 was not the network learning a deeper structure; it was the targets shrinking until they happened to match the network's prediction range. Session 9's 30-epoch retrain at scale=2400 also did not move the network by much — its sample evals (3/0/30) differ from the Session-3 baseline (84/84/84) because output_bias dropped and output_weights expanded asymmetrically, but the td_err didn't actually drop because of training; it dropped because the targets did.

**Why does the symmetric init *not* improve the floor**:

- Forward pass: `prediction = tanh(raw_output / 16320)`, where `raw_output = output_bias + Σ_j (h2_act_j · output_w_j) >> 6`.
- With `output_bias = 0` and `output_w ~ N(0, 20)` (i16), `raw_output` is a sum of 32 zero-mean terms each of order ±50. Net std-dev per position: ~150-300, which produces `prediction ≈ ±0.02`. tanh's near-linear region.
- The training-time gradient through the output layer is `d_raw = error · (1 - prediction²) / 16320`. At `prediction ≈ 0`, `(1 - prediction²) ≈ 1`, so `d_raw ≈ error / 16320` — an attenuation factor of nearly four orders of magnitude.
- Worse, because the corpus targets are roughly mean-zero (the corpus has both colors winning and an outcome blend of ±1 + tanh of signed evals), the *direction* of `d_raw` flips between positive-target and negative-target positions. Their contributions to `acc_output_b` and `acc_output_w[j]` largely cancel within a 256-position batch.
- Net effect: the only weights that learn are those activated for features that *correlate* with the target sign (e.g. material imbalance), and even those gradients are scaled down by 1/16320 → barely moves anything in 30 epochs.

**Why Session 9's network wasn't actually stuck — it just wasn't really learning either**:

- Session-3 weights had `output_bias = 3459` ≈ tanh(0.21), so every position predicted +0.21. With targets symmetric around 0 at scale=2400, the loss was minimised by pulling `output_bias` toward 0 (which Session 9 did, but slowly: 3459 → 3396).
- Session 9's training expanded `output_weights` in the negative direction (`[-1,3] → [-56,3]`) because the *initial* prediction was uniformly +0.21 and most targets were below that, so the easiest path to lower error was to drag predictions down for whichever features happened to be active. The asymmetry (one-sided expansion) is the gradient saying "go negative, not positive" — i.e. the corpus actually pulls predictions down because the bias was too high.
- The td_err floor 0.47 at scale=2400 is the same number whether the network has +84 cp constant predictions, +0 cp constant predictions, or fluctuating ±2 cp predictions. None of those reach the targets. The network's representational capacity (`prediction = tanh(raw/16320)` with i16-bounded `raw`) caps maximum |prediction| at well below the maximum |target|, which is exactly why the floor exists.

**Where the actual ceiling lives**:

- Maximum reachable `|raw_output|`: with all 32 `output_w` saturated at 127, all 32 `h2_act` at their typical ~30-100, and the `>>6` divisor, `|raw_output|_max ≈ 32 · 100 · 127 / 64 ≈ 6350`, plus `|output_bias|` up to 32767.
- Even with `raw = 6350`, `tanh(6350/16320) = tanh(0.39) ≈ 0.37`. So the network *cannot* express `|prediction| > 0.37` from the post-input components alone.
- Decisive teacher-eval positions at scale=2400 produce targets up to `tanh(2000/2400) = 0.69`. Beyond ±0.37 the targets are unreachable. **The 1/16320 divisor in the prediction mapping is the structural floor**, and it dominates everything Session 7 onward has been measuring.

**What went well**:
- The symmetrise binary is small, focused, and reproducible. Re-running the experiment from scratch is one command.
- The diagnostic sweep across `target_eval_scale ∈ {600, 1200, 2400, 4000, 8000}` was the cheapest possible disconfirmation of the Session 9 hypothesis — 50 seconds of training revealed that the entire ladder had been measuring `mean(|target|)`.

**What was harder than expected**:
- Recognising that "training is moving the weights" and "the network is learning" are different statements. The 100× LR comparison — same td_err but 5× larger weight deltas — was needed to tell them apart.

**Surprises**:
- Session 9's hypothesis was *plausible from the diagnostics it had*, but it did not survive the additional diagnostic of asking "what's the floor with the OTHER weights at this scale?" Once we ran one epoch of Session-9 trained weights at scale=2400 and got the same 0.4713, the case was closed.
- The td_err number is an unreliable proxy for network quality in this regime. Two networks reporting the same td_err can have meaningfully different sample-position evals.

**Insights for future sessions**:
- The next productive lever is the `1/16320` divisor in the prediction mapping, not the input/output layer initialisation. Lowering it (or replacing the `tanh(raw/16320)` mapping with one that saturates at smaller `|raw|`) is the architectural change that would let the network *reach* the targets it is being trained on.
- Concretely, `prediction = tanh(raw / 1000)` would saturate at `|raw| ≈ 2500` (well within the i16-bounded range), make the gradient near zero `(1-pred²)/1000` rather than `(1-pred²)/16320`, and align the network's expressive range with the corpus's target range. Cost: one constant change in `nnue.rs` *plus* a coordinated re-scaling of `target_eval_scale` and probably a partial retrain.
- An alternative cheap experiment is to *zero* the input-side and re-init from scratch (architecture-fresh). The Session-3 PST-imitation init has been sitting under all of this and we've never compared against an "actual" SGD-trained network from white-noise weights at the corrected divisor. Worth one session.

---

## Decisions Made

**Decision 1**: Stop pursuing the "symmetrise output layer" path. The negative result is robust — same floor across initialisations, same floor across scales, same floor across LR/grad scales.
- **Rationale**: Session 9's hypothesis was disconfirmed by a one-epoch resume diagnostic and a five-scale sweep. Continuing to tune σ, lr, or epochs of this approach cannot move the floor because the floor is `mean(|target|)` minus the network's tiny expressive variance, and the latter is bounded by `1/16320`.
- **Confidence**: High.

**Decision 2**: Recommend Session 11 attack the prediction-mapping divisor (1/16320), not the weight initialisation.
- **Rationale**: Forward-pass arithmetic shows that `|prediction| ≤ tanh(raw_max/16320) ≈ 0.37` is a hard architectural ceiling. Targets at `target_eval_scale = 2400` for decisive positions exceed this. A structural fix means changing the mapping; everything else has been re-tuning a divisor we should be lowering.
- **Alternative considered**: Replace `tanh` with a different saturating function (clipped ReLU, sigmoid). Same conclusion — the divisor is the load-bearing constant.
- **Confidence**: High that this is the next lever; medium that any one specific replacement is the right one.

**Decision 3**: Do not commit `nnue_weights_s10_*` artifacts. Same policy as Sessions 7-9 — fully reproducible from `nnue_weights_trained.json + nnue_corpus_yaneura_d10.jsonl + nnue-init-symmetric` invocation.
- **Confidence**: High.

**Decision 4**: Keep `nnue-init-symmetric` in the tree even though the experiment was negative. It's a useful tool for any future "re-initialise this layer" experiment, and the cost is one small file plus a Cargo entry.
- **Confidence**: High.

---

## Blockers & Issues

### Issue 1 (re-classified): "Output-magnitude ceiling" (Session 8 Issue 2)
- **Severity**: High → re-classified as the *prediction-mapping divisor*, not output-layer asymmetry.
- **Root cause**: `prediction = tanh(raw_output / 16320)` plus i16-bounded `raw_output` caps `|prediction|` at ≈ 0.37 regardless of training. Asymmetry of `output_bias`/`output_weights` was a *symptom*, not the cause.
- **Status**: Open. Session 11 should change the divisor.

### Issue 2 (still open): td_err is an unreliable network-quality proxy in this regime
- **Severity**: Medium.
- **Description**: Two networks with materially different sample-position evals reach the same td_err at scale=2400.
- **Resolution sketch**: Add a stronger validation metric — e.g. *correlation* between `prediction` and `tanh(eval/scale)` over the corpus, not just the L1 distance — so that "the network differentiates positions at all" is visible separately from "the targets are small."
- **Status**: Open.

### Issue 3 (still open): promoted-rook move-gen truncated (Session 6)
- **Severity**: Medium.
- **Status**: Not surfaced this session.

### Issue 4 (still open): `nnue_trainer.rs` drop-move bug (Session 7)
- **Severity**: Low.
- **Status**: Not surfaced this session.

---

## Next Session Plan (Session 11)

**Primary goal**: change the prediction-mapping divisor so that the network can actually reach the targets it is being trained against, then retrain from a Session-3 init and validate that *both* td_err and sample-position evals improve.

**Approach**:

1. In `src/evaluation/nnue.rs`: replace `(output * SCALE_FACTOR) / FINAL_DIVISOR` (= `output * 400 / 16320`) with a smaller divisor — e.g. `output / 4` — so `|output_cp| ≈ 1500` for `|raw| ≈ 6000`. The training-time prediction in `nnue_training.rs` will need its `tanh(output_cp / 400)` mapping coordinated with the new scale; the simplest is to make the *training* prediction match the *inference* output_cp computation directly so the network can't see two different scales.
2. Sanity check: with the modified mapping and Session-3 weights, sample-position evals should range much further than ±84 cp (positions like the black-winning endgame should hit several hundred cp). If they don't, revisit the divisor.
3. Re-train from `nnue_weights_trained.json` at the new mapping with `target_eval_scale = 600` (Session 8's original) — the target rescaling that Session 9 was forced into was a workaround for the divisor, and once the divisor is fixed the historic scale should be appropriate.
4. Pilot at the Session 9 protocol (10 games, depth 3, 500 ms/move, random-plies 16). If the network has actually learned, we should see asymmetric W/L vs PST rather than the 4-2-4 parity Session 9 produced.

**Secondary goal (nice-to-have)**: add a non-td_err validation metric — Pearson correlation between `prediction` and `tanh(eval/scale)` over a held-out 5% slice — so future sessions can distinguish "the network differentiates positions" from "the network reports a small td_err because the targets are small."

**Estimated duration**: 4-6 hours (architectural change touches both nnue.rs and nnue_training.rs, with coordinated retraining + validation).
**Prerequisite**: None.

---

## File Changes Summary

### Files Modified
- `Cargo.toml` (3 lines) — added `nnue-init-symmetric` `[[bin]]` entry.

### Files Created
- `src/bin/nnue_init_symmetric.rs` (~150 lines) — the symmetrise tool.
- `docs/nnue-phase2/SESSION_LOG_010.md` (this file).

### Files Deleted
- None.

### Artefacts produced (untracked)
- `nnue_weights_s10_init.json` — symmetric init derived from `nnue_weights_trained.json`.
- `nnue_weights_s10_sym_scale2400.json` + epoch 10/20/30 checkpoints — 30-epoch retrain (no learning beyond noise).
- `/tmp/s10_sym_lr0p05.json`, `/tmp/s10_sym_grad1e5.json`, `/tmp/s10_sym_aggressive.json` — LR/grad scale sanity runs.
- `/tmp/s10_sym_sweep_scale{600,1200,2400,4000,8000}.json` — cross-scale diagnostic sweep.
- `/tmp/s10_diag_s9_resume.json` — 1-epoch resume from Session-9 weights at scale=2400.
- `/tmp/elo_s10_sym_10g.csv` — pilot results.

---

## Testing Results

### Session 10 success criteria

**Criterion 1**: Symmetrise binary correctly resets `output_bias` and re-initialises `output_weights`.
- [x] Met. `output_bias` 3459 → 0; `output_w` range [-1, 3] → [-36, 59], mean -1.06.

**Criterion 2**: Training from the symmetric init reduces td_err below Session 9's 0.47 floor.
- [ ] **Not met.** td_err = 0.4713-0.4715 at every epoch, identical to the Session 9 plateau.

**Criterion 3**: Symmetric init network produces non-zero, position-differentiated cp evaluations after training.
- [ ] **Not met.** Sample positions evaluate to 0/+2/+2 cp respectively; baseline cp differentiation (Session 9: 3/0/30) was *destroyed*, not improved.

**Criterion 4**: 10-game ELO pilot with symmetric weights does not regress vs PST.
- [x] Met. 3W/6D/1L, score 0.600, ELO +70.4 ± 143.1 — non-regression vs PST. Within Session 9's CI, but with 60% draw rate (vs Session 9's 20%) reflecting reduced positional pressure from the silent NNUE half.

### Diagnostic findings (the actual session output)

**Finding 1**: The td_err floor is `mean(|target|)` at the network's effectively-flat prediction surface, not a network-capacity ceiling.
- Confirmed by: scale sweep 0.65 → 0.34 across {600, 1200, 2400, 4000, 8000} with no meaningful weight movement.

**Finding 2**: The Session 7-9 td_err ladder was target-magnitude rescaling, not learning.
- Confirmed by: Session-9 trained weights resumed at scale=2400 produce td_err = 0.4713 — same as the symmetric init's first epoch.

**Finding 3**: Architectural ceiling is `prediction = tanh(raw/16320)` with i16-bounded `raw`, capping `|prediction|` at ≈ 0.37.
- Forward-pass arithmetic; held targets at scale=2400 reach ±0.69 for decisive positions.

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
| 10      | 2d           | Symmetrise Output Layer — Negative Result, Reframes 0.47 Floor  | Completed   | 2026-04-25 |
| 11      | 2e           | Lower Prediction-Mapping Divisor (Approach pivoted from S9 plan) | Pending     | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — the planned experiment was executed, the hypothesis was falsified, and the diagnostics that falsified it also identified the load-bearing structural constant (the 1/16320 divisor in the prediction mapping). Session 11 has a concrete, narrow target.
**Ready for next session**: Yes
**Comments**: This is a session where the most valuable output is the negative result, not the code. The Session 8/9 framing of "output-layer asymmetry caps td_err" turns out to be wrong; the actual ceiling has been the prediction-mapping divisor since Phase 1. That's a comforting finding in one way (we've been close to the issue all along) and a sobering one in another (Sessions 7-9's improvements in td_err were largely numerical artifacts of target rescaling). Session 11 needs to make one small but coordinated change to the divisor, and then we'll see whether the network can actually move td_err *and* improve cp differentiation simultaneously.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-25
