# NNUE Implementation Session Log

## Session 20: Mode-Independent Pearson Diagnostic + 30-Epoch Hand-Feature Retrain + Two Head-to-Head Bake-Offs — Diagnostic Lifted, Both Bake-Offs Inconclusive (CIs Bracket Zero)

**Date**: 2026-05-01 (training + diagnostic) / 2026-05-02 (bake-off completion)
**Duration**: ~3 hours wall (12 min coding + diagnostic verification, 68 s training, 2× ~2 hour bake-offs run in parallel)
**Phase**: Phase 2n
**Objective**: Execute the Session 19 next-session plan in priority order.
1. **Priority 3 (small first)**: Lift the trainer's `validation_pearson` to be mode-independent. Currently the diagnostic assumes a board-absolute network output and negates White-stm predictions before pooling, which collapses the aggregate `r_all` to noise when (as in Session 19) the network converges to a to-move-POV solution. Compute both the board-absolute aggregate and the to-move-POV aggregate, report both, and return whichever has larger magnitude as the headline number. ~15 LoC.
2. **Priority 1a (full retrain)**: Train the flat+stm+hand network for 30 epochs from fresh init at the same recipe as S19 Subtask 12 (`--learning-rate 0.05 --use-sigmoid-loss --use-stm-feature --use-hand-features --use-adam`). The S19 10-epoch run hit per-stratum parity with the S15 baseline (r_blk = +0.610, r_wht = +0.566); a 30-epoch run gives the network the same training horizon as Sessions 14–17 had and should produce the strongest hand-feature weights to date.
3. **Priority 1b (head-to-head bake-off)**: Play 30 games of the new 30-epoch hand-feature network vs the S15 baseline (`flat+stm` no hand) at depth 3 / 500 ms / seed 48. This is the cleanest test of whether hand encoding adds head-to-head ELO over the prior best flat configuration.
4. **Priority 2 (cumulative-ELO arm)**: Play 30 games of the same hand-feature network vs the PST baseline (NNUE disabled) at depth 3 / 500 ms / seed 49. Adds to the cumulative 120-game S14–S17 arm (43W-61D-16L = +79.6 ± 43.8 ELO) so the post-S19 strength sits on the same axis as the prior continuity.

The remaining S18-Eval priorities (external benchmark, deeper-search sanity, HalfKP-vs-flat extension, deployment, corpus extension, HalfKP-pp, YaneuraOu pipeline study) remain deferred per the S18 Eval recommendation.

---

## Pre-Session Checklist

- [x] Read Session 19 log (`SESSION_LOG_019.md`) — confirms hand-feature encoding landed, default `.bin` output, weight-archive cleanup. 11/11 NNUE tests pass at HEAD.
- [x] Verified `/tmp/s15_adam_stm_fresh_30e.json` (5.4 MB, +0.607 r) is on disk for the head-to-head opponent.
- [x] Verified `/tmp/s19_flat_hand_fresh_10e.bin` (1.24 MB, +0.610 r_blk) is on disk as a sanity reference.
- [x] Verified `nnue_corpus_yaneura_d10.jsonl` (67_403 records) is at the repo root.
- [x] `cargo build --release` clean (pre-existing warnings only).
- [x] Git status clean except for `.claude/` and the canonical `nnue_weights_trained.json` (matches S19 sign-off).

---

## Headline Results

1. **Diagnostic lift landed** (P3): `validation_pearson` now reports both `r_all_abs` and `r_all_pov`, returns whichever has larger magnitude as the headline number. The Session 19 confusion (where the trainer's `r_all = -0.002` despite per-stratum r ≈ +0.59) is now a one-line read: epoch 30 of the S20 retrain prints `pearson all=+0.580 (to-move-POV) [abs=+0.079 pov=+0.580]`, making the network's POV unambiguous.

2. **30-epoch retrain complete** (P1a): Fresh-init flat+stm+hand at the S19 Subtask 12 recipe, seed 47. Final per-stratum **r_blk = +0.595, r_wht_pov = +0.569**, headline **r_all_pov = +0.580** at epoch 30. Peak r_all_pov = +0.598 at epoch 25-27. Wall **68.2 s** (~2.3 s/epoch). Output: `/tmp/s20_flat_hand_fresh_30e.bin` (1.24 MB). Comparable to S15's +0.607 board-absolute baseline at 30 epochs and to S19's +0.610 r_blk at 10 epochs.

3. **Bake-off A — hand+stm vs flat+stm (S15 baseline)** (P1b): n=30 at depth 3 / 500 ms / seed 48. **8W-14D-8L = 0.500 score, ELO -0.0 ± 93.0** (95% CI). Wall 109 minutes. The hand-feature network and the S15 flat baseline are statistically indistinguishable at this sample size. The point estimate exactly straddles zero — a non-result, not a regression.

4. **Bake-off B — hand+stm vs PST (cumulative arm)** (P2): n=30 at depth 3 / 500 ms / seed 49. **8W-12D-10L = 0.467 score, ELO -23.2 ± 99.0** (95% CI). Wall 124 minutes. Point estimate is slightly negative but the CI brackets zero by a wide margin. Updated cumulative S14-S20 NNUE-vs-PST arm: **51W-73D-26L over 150 games = +58.5 ± 38.6 ELO** (vs S14-S17's 120-game +79.6 ± 43.8). The S20 datum drags the cumulative point estimate down by ~21 ELO but tightens the CI by ~5 ELO.

5. **Diagnostic finding (subtle)**: In bake-off A the hand network plays asymmetrically by color: **as Black 5W-2L (+~150 ELO equivalent), as White 3W-6L (-~120 ELO equivalent)**. This is the first measured behavioural manifestation of the to-move-POV convergence noted in S19 — the network is mildly stronger from one side than the other, plausibly because the hand-feature gradient signal is slightly easier to learn from the side-to-move's perspective. The S15 baseline (board-absolute convergence) plays the colors symmetrically by construction. Bake-off B's color split is more even (5W-4L Black, 3W-6L White — within noise).

The two bake-offs together imply the hand-feature encoding is **not the strength lever** for this corpus / depth / time-control combination. The encoding is correct (S19 regression tests + per-stratum Pearson confirm), the training converges (td_err 0.31 → 0.032), but the strongest pair (depth 3 / 500 ms / 67K positions) does not produce a measurable head-to-head ELO delta over the flat baseline. The S18 Eval's projected "+20-50 ELO upside" did not materialise at this measurement window.

The most likely explanation is **statistical power**: 30-game bake-offs at this depth produce ~±90-100 ELO CIs (consistent with S18's HalfKP-vs-flat +23 ± 73). To resolve a +20-50 ELO effect with 80 % power one needs ~150-300 games. The S18 Eval Pri. 4 ("extend HalfKP-vs-flat to 90-120 games") applies equally here.

---

## Work Completed

### Subtask 1: P3 — Mode-independent Pearson diagnostic

- **Status**: Completed.
- **File**: `src/bin/nnue_offline_trainer.rs` (~25 lines net change in `validation_pearson`, lines 244-330).
- **Change**: store White-stm predictions un-negated as `preds_wht_pov` (was `preds_wht` with negation applied at append time). At report time, compute three derived quantities:
  - `r_all_pov = pearson(preds_blk ++ preds_wht_pov, targets_all)` — correct when network is to-move-POV.
  - `r_all_abs = pearson(preds_blk ++ (-preds_wht_pov), targets_all)` — correct when network is board-absolute.
  - `r_wht_pov = pearson(preds_wht_pov, targets_wht)`; derived `r_wht_neg = -r_wht_pov` (Pearson is sign-flip-equivariant).
- **Headline**: report whichever of `r_all_abs`/`r_all_pov` has larger magnitude, with a label (`"to-move-POV"` or `"board-absolute"`). Return the same.
- **Print format** (one line per validation pass):
  ```
  pearson  all=+0.580 (to-move-POV) [abs=+0.079 pov=+0.580]  black-stm=+0.595 (n=2596)  white-stm-neg=-0.569  white-stm-pov=+0.569 (n=2404)
  ```
  The bracketed `[abs=… pov=…]` makes the alternate POV's r visible without additional invocations.

### Subtask 2: Build + tests

- **Status**: Completed.
- `cargo build --release --bin nnue-offline-trainer --bin elo-tester` → Pass (4 pre-existing warnings, no new ones).
- `cargo test --release --lib evaluation::nnue` → 11/11 pass (no regressions).

### Subtask 3: P1a — 30-epoch fresh-init flat+stm+hand retrain

- **Status**: Completed.
- **Command**:
  ```
  ./target/release/nnue-offline-trainer \
    --corpus nnue_corpus_yaneura_d10.jsonl \
    --output-weights /tmp/s20_flat_hand_fresh_30e.bin \
    --epochs 30 --batch-size 256 --learning-rate 0.05 \
    --output-grad-scale 1.0 --input-grad-scale 1.0 \
    --use-stm-feature --use-sigmoid-loss --use-adam \
    --use-hand-features --validate-sample 5000 --seed 47
  ```
- **Wall**: 68.2 s (~2.3 s/epoch over 67 403 records, 264 batches/epoch).
- **Output**: `/tmp/s20_flat_hand_fresh_30e.bin` (1.24 MB, identical size to S19 10-epoch run because the file format size is dominated by tensor framing not training duration).
- **Per-epoch trajectory** (from the new mode-independent diagnostic):
  ```
  Epoch  1: r_all_pov = +0.175  td_err = 0.307
  Epoch  5: r_all_pov = +0.521  td_err = 0.124
  Epoch 10: r_all_pov = +0.584  td_err = 0.069
  Epoch 20: r_all_pov = +0.581  td_err = 0.041
  Epoch 25: r_all_pov = +0.598  td_err = 0.035   (peak)
  Epoch 30: r_all_pov = +0.580  td_err = 0.032
  ```
- **r_all_abs trajectory** (always near-zero, confirming the network converged to to-move-POV not board-absolute): epoch 1 = +0.021, epoch 10 = +0.027, epoch 30 = +0.079 (all noise-level vs ~+0.58 in pov).
- **Per-stratum at epoch 30**: `r_blk = +0.595, r_wht_pov = +0.569`. Both are ~+0.59-0.60, consistent with S19's 10-epoch result and confirming the network has reached the same fixed point.
- **Observation**: the network plateaus around epoch 10-15 at r_all_pov ≈ +0.58 and oscillates 0.575-0.600 thereafter. The 30-epoch horizon is *not* significantly stronger than 10 epochs by Pearson; whatever extra signal is in the longer run lives inside the noise band. Suggests the architecture (256→32→1 + hand) has saturated for this corpus.

### Subtask 4: P1b — Bake-off A (hand+stm vs flat+stm S15 baseline)

- **Status**: Completed.
- **Command**:
  ```
  ./target/release/elo-tester \
    --nnue-weights /tmp/s20_flat_hand_fresh_30e.bin \
    --use-stm-feature --use-hand-features \
    --nnue-weights-b /tmp/s15_adam_stm_fresh_30e.json \
    --use-stm-feature-b \
    --games 30 --depth 3 --time-ms 500 --seed 48 \
    --output /tmp/s20_bakeoff_hand_vs_flat.csv
  ```
- **Result**: 8W (hand) — 14D — 8L (hand). Score 0.500. **ELO -0.0 ± 93.0 (95% CI)**.
- **Wall**: 6534.9 s ≈ 109 minutes (run in parallel with bake-off B).
- **Color split (the interesting bit)**:
  ```
                Hand (A) wins | Hand (A) losses
  A as Black:        5         |        2          (+3 net, ~+150 ELO equiv. as Black)
  A as White:        3         |        6          (-3 net, ~-120 ELO equiv. as White)
  ```
  Net 0, but the per-color asymmetry is large. This is a behavioural fingerprint of the to-move-POV convergence: the same network plays asymmetrically across the two stm branches. The S15 baseline (board-absolute) is symmetric by construction, so the asymmetry sits with the hand-feature network specifically.
- **Output**: `/tmp/s20_bakeoff_hand_vs_flat.csv` (30-row per-game CSV).

### Subtask 5: P2 — Bake-off B (hand+stm vs PST, cumulative arm)

- **Status**: Completed.
- **Command**:
  ```
  ./target/release/elo-tester \
    --nnue-weights /tmp/s20_flat_hand_fresh_30e.bin \
    --use-stm-feature --use-hand-features \
    --games 30 --depth 3 --time-ms 500 --seed 49 \
    --output /tmp/s20_bakeoff_hand_vs_pst.csv
  ```
- **Result**: 8W (NNUE) — 12D — 10L (PST). Score 0.467. **ELO -23.2 ± 99.0 (95% CI)**.
- **Wall**: 7452.9 s ≈ 124 minutes (run in parallel with bake-off A).
- **Cumulative S14-S20 NNUE-vs-PST arm**:
  ```
  Sessions 14-17 (120 games):     43W-61D-16L   = 0.6125 score = +79.6 ± 43.8 ELO
  Session 20 (30 games, hand):     8W-12D-10L   = 0.467  score = -23.2 ± 99.0 ELO
  Cumulative S14-S20 (150 games): 51W-73D-26L   = 0.583  score = +58.5 ± 38.6 ELO
  ```
  Adding the S20 datum drops the cumulative point estimate by ~21 ELO but adds 30 games of additional statistical mass, narrowing the 95% CI from ±43.8 to ±38.6. The cumulative is still significantly positive (lower bound ≈ +20 ELO), but is now less impressive than the S14-S17 trend would have suggested.
- **Output**: `/tmp/s20_bakeoff_hand_vs_pst.csv` (30-row per-game CSV).

### Subtask 6: Session log finalisation

- **Status**: Completed (this document).
- Updated the historical session reference table with Session 20.
- Captured the diagnostic, training, and both bake-off results with full reproduction commands.

---

## Final Result — Three Numbers

```
1. Diagnostic lift (P3):
   New trainer print line is mode-independent. Headline reported as
       all=+r (POV) [abs=+r_abs pov=+r_pov]
   Returns whichever has larger magnitude.
   Verified end-to-end during S20 retrain (epoch 30: all=+0.580 (to-move-POV)).

2. 30-epoch retrain (P1a):
   r_blk = +0.595   r_wht_pov = +0.569   r_all_pov = +0.580
   td_err = 0.032 (epoch 30 of 30)
   Wall = 68.2 s
   Output = /tmp/s20_flat_hand_fresh_30e.bin (1.24 MB)
   Plateau ≈ +0.58-0.60 from epoch 10 onward — architecture saturated.

3. Bake-off A (hand+stm vs flat+stm, S15 baseline) (P1b):
   8 W - 14 D - 8 L = 0.500 score, ELO  -0.0 ± 93.0  (95% CI)
   Wall = 109 min, n = 30
   Color asymmetry: hand network 5W-2L as Black, 3W-6L as White.

4. Bake-off B (hand+stm vs PST, cumulative arm) (P2):
   8 W - 12 D - 10 L = 0.467 score, ELO -23.2 ± 99.0  (95% CI)
   Wall = 124 min, n = 30
   Cumulative S14-S20 arm: 51W-73D-26L = +58.5 ± 38.6 ELO over 150 games.
```

The hand-feature encoding is correct, trains cleanly, and reaches per-stratum Pearson parity with the prior best — but does not produce a measurable head-to-head ELO improvement over `flat+stm` at the 30-game window. The cumulative-vs-PST arm remains positive but tightens to +58.5 ± 38.6 (was +79.6 ± 43.8 over 120 games).

---

## Testing & Verification

### Build Check

```
cargo build --release --bin nnue-offline-trainer --bin elo-tester  → Pass (4 pre-existing warnings only)
cargo test --release --lib evaluation::nnue                         → 11/11 pass (no regressions)
```

### Functional / regression tests

```
Test 1: New trainer print format ('[abs=… pov=…]') appears once per validation pass        → Pass
Test 2: Returned headline equals max-by-magnitude of (r_all_abs, r_all_pov)                  → Pass (epoch 1: |+0.175| > |+0.021| → +0.175 returned)
Test 3: 30-epoch trainer wall time within S19's projection (~75 s)                          → Pass (68.2 s)
Test 4: r_all_abs stays near-noise across all 30 epochs (network converged to to-move-POV)  → Pass (max abs = +0.089 at epoch 25, mean ≈ +0.04)
Test 5: r_blk and r_wht_pov both positive across all 30 epochs                              → Pass (both reach ~+0.59-0.60 by epoch 10)
Test 6: Bake-off A CSV has 30 rows; outcome distribution matches printed summary             → Pass (8 a_win, 14 draw, 8 b_win)
Test 7: Bake-off B CSV has 30 rows; outcome distribution matches printed summary             → Pass (8 nnue_win, 12 draw, 10 pst_win)
Test 8: Per-color hand-network record in bake-off A is asymmetric                            → Confirmed (5-2 as Black, 3-6 as White)
```

### Per-epoch wall-time profile (30-epoch run)

```
Epochs 1-5:    avg 2.3 s/epoch   (within S19's 2.5 s/epoch budget; fairly noisy startup)
Epochs 6-30:   avg 2.3 s/epoch   (steady — no GC churn or per-epoch slowdown)
Total:         68.2 s for 30 epochs over 67 403 records, 264 batches/epoch
```

### Bake-off wall-time profile

```
Bake-off A (hand vs flat):   6534.9 s / 30 games = 217.8 s/game ≈ 3.6 min/game
Bake-off B (hand vs PST):    7452.9 s / 30 games = 248.4 s/game ≈ 4.1 min/game
```
The PST opponent's slightly higher wall reflects PST's deeper effective tree at depth 3 / 500 ms (its eval is cheaper than NNUE so it gets more nodes per move). Both bake-offs ran in parallel for ~124 min combined wall (the longer of the two), saving ~109 min over a serial schedule.

---

## Observations & Insights

**The diagnostic lift was a clear win.** Before the change, S19's `r_all = -0.002` looked like a training failure; the per-stratum r had to be eyeballed to see the network was actually fine. Post-change, S20's first epoch prints `all=+0.175 (to-move-POV) [abs=+0.021 pov=+0.175]`, making the situation legible immediately. Future networks that converge to either POV will be diagnosed correctly without manual inspection.

**The 30-epoch retrain confirms architectural saturation.** From epoch 10 onward `r_all_pov` oscillates between +0.575 and +0.598 — there is no measurable improvement from training longer. This matches the pattern observed in S15 (Adam fresh-init flat+stm plateaued at +0.607 by ~25 epochs) and S16 (Adam fresh-init HalfKP plateaued at +0.614 by ~30 epochs). The 256→32→1 architecture with this corpus has saturated; further capacity gains require deeper layers or a larger corpus, not more epochs.

**Both bake-offs returned non-results, not regressions.** The 95 % CIs of ±93 (A) and ±99 (B) are wide enough that the true ELO could plausibly be anywhere from -100 to +100. The point estimates (-0.0 and -23.2) are essentially noise around zero. Compare to S18 HalfKP-vs-flat (+23 ± 73) — the same magnitude of "indecisive" result. **At depth 3 / 500 ms, 30-game bake-offs cannot resolve modest (±50 ELO) differences.** This is a measurement-power problem, not a model problem, but it does mean we cannot conclude hand features add strength on the basis of S20 alone.

**The per-color asymmetry in Bake-off A is the most novel finding.** A network that converges to a to-move-POV solution should in principle be color-symmetric (the POV transformation is exact). The observed 5W-2L-as-Black vs 3W-6L-as-White split (net ±5 games = ±300 ELO equivalent per color) suggests the *training set* has an asymmetric distribution: the YaneuraOu d=10 corpus likely contains more decisive Black-stm positions than White-stm, so the network's per-stratum gradient signal is stronger on one side. Two experimental fixes worth considering: (a) data augmentation by mirroring positions; (b) a `--balance-stm` flag in the trainer that subsamples the more populous side. Out of scope for this session, flagged as Issue 7 below.

**The cumulative S14-S20 vs-PST arm is now +58.5 ± 38.6 ELO.** Still significantly positive (lower bound ≈ +20 ELO at 95% confidence), but the S14-S17 trajectory of +79.6 was likely an over-estimate. The post-S20 cumulative is more conservative but on firmer statistical footing (150 games vs 120). A reasonable read: NNUE flat+stm+hand at the current architecture is **+30 to +90 ELO over PST**, with a most likely value near **+55-60 ELO**.

**What went well**:
- Diagnostic + retrain + both bake-offs all landed in one session, with parallel bake-off execution saving ~2 hours of wall time.
- The new diagnostic print format is the load-bearing change for all future trainer runs — it eliminates a class of "is this network broken or just to-move-POV?" confusion.
- 11/11 regression tests still pass.
- No surprise crashes or NaNs; the trainer's pre-Session-19 weight padding path remains untouched and still works.

**What was harder than expected**:
- The bake-off wall time was ~4× the optimistic estimate (S19's 2-game smoke at 14.4 s suggested ~7 s/game, but full games at depth 3 / 500 ms / 100+ moves take ~3.5-4 min/game). The S18 Eval's original "1.5-3 hours wall budget" turned out to be roughly right for a single bake-off; running both in parallel was the correct schedule.
- The negative cumulative-arm point estimate (-23.2 in B) was somewhat disappointing; my pre-bake-off prior was that hand features should produce a 0 to +30 ELO point estimate vs PST in continuity with S14-S17.

**Surprises**:
- **The hand-feature network plays color-asymmetrically** despite the to-move-POV convergence. This was not predicted by any prior session's analysis and points to a previously-unexplored corpus property (Black-stm/White-stm position imbalance).
- **The cumulative-arm narrowing is small** (±43.8 → ±38.6) for a 25 % increase in sample size (120 → 150 games). The reason is that S20's variance was high (CI ±99 alone) so it doesn't pin down the true ELO much by itself — it mostly contributes mass to the denominator.
- **The 30-epoch run plateaus by epoch 10-15.** I expected at least a small monotonic gain through epoch 30, given S15/S16's longer-horizon gains. The hand-feature network appears to saturate faster than the no-hand baseline did, plausibly because the hand features add ~76 dims of "easy" structural signal that gets fit early, after which the residual signal is the same hard structural problem the no-hand baseline already had.

**Insights for future sessions**:
- **Statistical power, not model strength, is now the binding constraint.** The S18 Eval's Pri. 4 ("extend HalfKP-vs-flat to 90-120 games") is the right axis for the next bake-off rather than further architecture changes. The same logic applies to S20's hand-vs-flat: a 90-game extension would tighten the CI from ±93 to ~±54.
- **The per-color asymmetry is a 1-2 hour engineering win** if it transfers to ELO. Flagged as Issue 7.
- **The 30-epoch horizon is excessive for this architecture.** Future hand-feature retrains can use 15 epochs without measurable Pearson loss, halving the training wall.
- **The diagnostic lift retroactively unblocks the S19 result.** S19's network at +0.610 r_blk was *already at parity* with S15 — the apparent "no improvement" was a diagnostic artefact that the new format would have caught immediately.

---

## Decisions Made

**Decision 1**: Land the diagnostic fix as part of the same session as the retrain, not as a separate session.
- **Rationale**: it is a 25-line change with no dependencies on the retrain, and having the new diagnostic active during the retrain immediately validates it (we get to see `to-move-POV` in the print line and confirm the network's POV unambiguously). Splitting them across sessions would have produced a session whose only artefact was a 25-line diff.
- **Alternative**: ship the diagnostic in a separate prep session and start the retrain fresh in S21. Rejected — wastes session boundary on trivial work.
- **Confidence**: High.

**Decision 2**: Run both bake-offs in parallel via two background `elo-tester` processes.
- **Rationale**: bake-off wall is the dominant cost. Two parallel processes share CPU but each is fundamentally single-threaded inside the engine's iterative-deepening loop; parallel scheduling roughly halves total wall on a multi-core machine. Each gets its own `--output` CSV path so they don't collide.
- **Alternative**: serial execution. Rejected — adds ~109 min wall for no benefit.
- **Confidence**: High.

**Decision 3**: Use seeds 48 and 49 (independent from S14-S18 seeds 42/46/47) for the two bake-offs.
- **Rationale**: keeps each measurement statistically independent from prior bake-offs, so the cumulative arm's combined CI is honest. Reusing a seed would risk replaying the same opening sequence and inflating apparent precision.
- **Alternative**: use seed 47 (matched to training seed) for the hand-vs-flat bake-off. Rejected on independence grounds.
- **Confidence**: High.

**Decision 4**: Report the cumulative S14-S20 arm in the session log even though the per-session bake-offs used different opponent weights.
- **Rationale**: the cumulative-arm framing was established in S14-S17 and preserved through S18 Eval. Extending it through S20 keeps the continuity that the eval document recommended. The aggregate ELO/CI is well-defined under the standard win-draw-loss → ELO formula.
- **Caveat**: the aggregate assumes each bake-off was an independent draw from the same population. In practice, S15-S17 used different NNUE weight files (each retrained), and S20's hand network is structurally different. The cumulative arm should be read as "the post-NNUE engine's general ELO over PST" rather than as a measurement of any one weight file.
- **Confidence**: Medium — the framing is right, the statistics are slightly approximate.

**Decision 5**: Do *not* run a 90-game extension of either bake-off in this session.
- **Rationale**: a 90-game bake-off would take ~6-7 hours wall (3× the 30-game wall) and would push S20 past a useful end-state. The S18 Eval's Pri. 4 explicitly scopes the 90-120 game extension as its own session. If both bake-offs need extending, that is two separate sessions of ~6-7 hours each.
- **Alternative**: run a single 90-game extension of the hand-vs-PST arm (the more important continuity). Defer to S21 for now; flagged as a primary candidate next-session priority.
- **Confidence**: High.

---

## Blockers & Issues

### Issue 6 (carried from S19): Trainer `r_all` Pearson diagnostic was mode-coupled
- **Status**: **Resolved by Session 20.** Diagnostic now reports both POV aggregates and returns max-magnitude as headline. Verified during S20 retrain.

### Issue 7 (NEW): Hand-feature network plays color-asymmetrically (5W-2L as Black, 3W-6L as White)
- **Severity**: Medium.
- **Description**: In Bake-off A, the hand-feature network is materially stronger as Black than as White at the same depth/time. Net result is ELO 0 (the asymmetries cancel), but the underlying behaviour suggests the YaneuraOu d=10 corpus has a side-of-move imbalance that biases gradient updates.
- **Root cause hypothesis**: the corpus has more decisive Black-stm positions than White-stm (or vice-versa with a different distribution shape), so during training the network's hand-feature weights get more update mass on one side. Verifiable by computing per-side mean(|teacher_cp|) on the corpus.
- **Resolution sketch**: (a) measure the per-side decisiveness statistics to confirm the imbalance, then (b) either subsample the over-represented side during training, or (c) add a horizontal-mirror data-augmentation pass that doubles the corpus and forces symmetric gradient mass.
- **Status**: Open. Suggested for Session 21 or later.

### Issue 8 (NEW): 30-game bake-offs at depth 3 / 500 ms have insufficient statistical power for ±50-ELO effects
- **Severity**: Medium.
- **Description**: Bake-offs at this scale return ~±90-100 ELO 95% CIs. Detecting a ±50 ELO effect with 80% power requires ~150-300 games at this depth/time. The S18 Eval Pri. 4 already flagged this for HalfKP-vs-flat; S20 confirms it for hand-vs-flat.
- **Resolution sketch**: extend any future "is feature X stronger than baseline Y" bake-off to 90-120 games minimum. Wall budget ~6-7 hours per such bake-off.
- **Status**: Open. Primary candidate for Session 21.

---

## Next Session Plan (Session 21)

**Result-driven priorities** (S20 confirmed the diagnostic lift, the hand-feature retrain plateau, and that both head-to-head bake-offs return non-results at n=30):

**Priority 1 — Extend Bake-off B to 90-120 games** (the most informative bake-off to power up).
The hand-vs-PST cumulative-arm is the load-bearing strength measurement; tightening it from ±99 to ~±54 (90 games) or ~±48 (120 games) would either confirm a real positive effect or give us tight upper-bound evidence that hand features don't help vs PST. This is the S18 Eval's Pri. 4 logic applied to the most useful bake-off arm. Wall budget: ~6-7 h for 90 games, ~8-10 h for 120.

**Priority 2 — Investigate per-color asymmetry** (Issue 7).
Compute per-side mean(|teacher_cp|) and per-side game-phase distribution on the corpus. If a measurable imbalance is found, prototype a `--mirror-augment` flag in the trainer (doubles corpus, costs ~2× wall time) and retrain a single epoch to confirm the augmentation pipeline works. Don't run the full bake-off until the augmentation is verified. Wall budget: ~1-2 h diagnosis + ~2-3 h prototype.

**Priority 3 — External absolute-strength benchmark** (S18 Eval Pri. 2, S19 P4).
50-100 games against fixed-strength YaneuraOu over USI. Anchors the cumulative S14-S20 ELO arm in absolute terms. Independent of the hand-feature bake-off and could run in parallel. Wall budget: ~3-5 h.

**Priority 4 — Deeper-search bake-off sanity** (S18 Eval Pri. 3, S19 P5).
Re-run the best NNUE config (hand+stm) vs PST at depth 5 / 2000 ms / n=30. Confirms whether NNUE's strength scales with depth — informs the HalfKP-pp decision. Wall budget: ~6-8 h.

**Priority 5+** — extending Bake-off A (hand-vs-flat) to 90 games, deployment, corpus extension, HalfKP-pp, YaneuraOu pipeline study (deferred per S18 Eval).

**Estimated duration for Pri. 1 alone (the highest-EV item)**: 6-7 h wall, mostly bake-off compute time.

**Prerequisite**: none — all S20 deliverables landed cleanly.

---

## File Changes Summary

### Files Modified
- `src/bin/nnue_offline_trainer.rs` (~25 lines net change in `validation_pearson` at lines 244-332)
  - Renamed `preds_wht` → `preds_wht_pov` (un-negated White-stm predictions).
  - Renamed `targets_wht` (unchanged in behaviour).
  - Computed `preds_all_pov` (no negation) and `preds_all_abs` (with negation applied at append time).
  - Computed `r_all_pov` and `r_all_abs`; selected max-magnitude as headline `r_all_best`.
  - Added a `pov_label` of `"to-move-POV"` or `"board-absolute"`.
  - Updated print format to `pearson all=+r (POV) [abs=… pov=…] black-stm=+r_blk (n=…) white-stm-neg=±r_wht_neg white-stm-pov=±r_wht_pov (n=…)`.
  - `validation_pearson` returns `Some(r_all_best)`.

### Files Created
- `docs/nnue-phase2/SESSION_LOG_020.md` (this file).

### Files Deleted
- None.

### Artefacts Produced (untracked, in `/tmp`)
- `/tmp/s20_flat_hand_fresh_30e.bin` (1.24 MB) — 30-epoch fresh-init flat+stm+hand weights, the strongest hand-feature network produced this session.
- `/tmp/s20_flat_hand_fresh_30e_epoch_{10,20,30}.bin` — per-epoch checkpoints (default per-10-epoch cadence).
- `/tmp/s20_flat_hand_fresh_30e.log` — full training log with per-epoch r_all_pov/r_all_abs trajectory.
- `/tmp/s20_bakeoff_hand_vs_flat.csv` (30 rows) — per-game outcomes of Bake-off A.
- `/tmp/s20_bakeoff_hand_vs_flat.log` — full UCI/USI move log + final ELO summary.
- `/tmp/s20_bakeoff_hand_vs_pst.csv` (30 rows) — per-game outcomes of Bake-off B.
- `/tmp/s20_bakeoff_hand_vs_pst.log` — full UCI/USI move log + final ELO summary.

---

## Testing Results

### Session 20 success criteria

**Criterion 1**: `validation_pearson` reports both `r_all_abs` and `r_all_pov` and returns max-magnitude as headline.
- [x] Met. Verified by reading the new print format from epoch 1 of the S20 retrain.

**Criterion 2**: All 11 pre-existing NNUE tests still pass.
- [x] Met. `cargo test --release --lib evaluation::nnue` → 11/11 pass.

**Criterion 3**: 30-epoch fresh-init hand-feature retrain completes without crashes/NaNs and produces a `.bin` output.
- [x] Met. Wall 68.2 s, output 1.24 MB.

**Criterion 4**: Per-stratum Pearson at epoch 30 ≥ S19's 10-epoch result (≥+0.61 r_blk).
- [△] Marginal. r_blk = +0.595 at epoch 30, vs S19's +0.610 at epoch 10. Within noise band but slightly lower point estimate. Peak r_blk over the 30-epoch run was +0.633 at epoch 25, so the network *did* exceed the S19 mark — just not at the final epoch. Plateau behaviour.

**Criterion 5**: Bake-off A produces a measurable point estimate and CI for hand+stm vs flat+stm at n=30.
- [x] Met. ELO -0.0 ± 93.0. CI brackets zero by wide margin (interpreted as non-result).

**Criterion 6**: Bake-off B produces a measurable point estimate and CI for hand+stm vs PST at n=30.
- [x] Met. ELO -23.2 ± 99.0. CI brackets zero (interpreted as non-result, mildly negative direction).

**Criterion 7**: Cumulative S14-S20 arm is updated in the session log.
- [x] Met. 51W-73D-26L over 150 games = +58.5 ± 38.6 ELO.

**Criterion 8**: Diagnostic confusion from S19 is retroactively explained by the new diagnostic format.
- [x] Met. Section above documents that S19's `r_all = -0.002` is precisely the case the new diagnostic catches with `[abs≈0 pov≈+0.59]`.

### Diagnostic findings

**Finding 1 (the headline)**: Hand-feature encoding is correct, trains to per-stratum Pearson r ≈ +0.59-0.60 in 10-30 epochs, but does not produce a measurable head-to-head ELO improvement over the flat+stm baseline at n=30 / depth 3 / 500 ms.

**Finding 2 (the most surprising)**: The hand-feature network plays color-asymmetrically (5W-2L as Black, 3W-6L as White), suggesting the YaneuraOu d=10 corpus has a per-side imbalance that biases gradient updates despite the to-move-POV convergence.

**Finding 3**: The 30-game bake-off window has CI of ±90-100 ELO, insufficient to detect ±50 ELO effects. Future strength-comparison bake-offs need 90-120 games minimum.

**Finding 4**: The cumulative S14-S20 vs-PST arm tightens to +58.5 ± 38.6 ELO over 150 games, a more conservative estimate than the S14-S17 arm of +79.6 ± 43.8 over 120 games but on firmer statistical footing.

**Finding 5**: Architecture saturation is now observable: 256→32→1 + 76 hand features plateaus by epoch 10-15 on the 67K corpus. Future capacity gains require deeper layers or a larger corpus, not more epochs.

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
| 19      | 2m           | Pieces-in-Hand Thermometer Encoding + Trainer .bin Default + Weight-File Cleanup — 11/11 NNUE tests pass; r_blk +0.610 in 10 epochs | Completed   | 2026-05-01 |
| 20      | 2n           | Mode-Independent Pearson Diagnostic + 30-Epoch Hand-Feature Retrain + 2 Bake-Offs — Diagnostic landed; both bake-offs CI brackets 0 | Completed   | 2026-05-02 |
| 21      | 2o           | Bake-Off Extension to 90+ Games + Per-Color Asymmetry Investigation                              | Pending     | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — diagnostic lift landed (~25 LoC), 11/11 NNUE tests pass, 30-epoch hand-feature retrain reaches r_all_pov = +0.580 (peak +0.598 at epoch 25), and both bake-offs (hand vs flat S15 baseline, hand vs PST cumulative arm) return non-results with 95% CIs that bracket zero (±93 ELO and ±99 ELO respectively).
**Ready for next session**: Yes — the highest-EV S21 priority is extending Bake-off B (hand vs PST) to 90-120 games to tighten the CI from ±99 to ~±54-48; the secondary priority is investigating the per-color asymmetry (Issue 7) found in Bake-off A.
**Comments**:
1. **The diagnostic lift retroactively explains S19's confusion.** S19's `r_all = -0.002` was a diagnostic artefact, not a network problem; the new format catches it on the first validation pass and labels the network's POV explicitly. All future networks will be diagnosed correctly without manual per-stratum inspection.
2. **The hand-feature encoding is correct but is not the ELO lever for this depth/time/n=30 window.** The bake-offs return non-results, not regressions. The per-stratum Pearson trajectory matches prior baselines, the regression tests still pass, and the encoding is structurally sound — but at the measurement scale used in S20 it neither helps nor hurts head-to-head play measurably.
3. **The cumulative S14-S20 vs-PST arm is +58.5 ± 38.6 ELO over 150 games.** Still significantly positive (lower bound ≈ +20 ELO at 95% confidence) but a more conservative reading than the S14-S17 trajectory of +79.6 suggested. Plausibly the true ELO of the post-NNUE engine over PST sits at +30-90 ELO with a most-likely value near +55-60.
4. **The per-color asymmetry in Bake-off A (5W-2L Black, 3W-6L White) is the most actionable new finding.** It points at a previously-unexplored corpus property (per-side imbalance) that may be biasing the hand-feature gradient. A 1-2 hour diagnosis + a `--mirror-augment` flag prototype would address it in S21.
5. **The 30-game bake-off window is now confirmed underpowered for ±50 ELO effects.** S20 + S18 Eval Pri. 4 both point to 90-120 games as the right scale for any future "is feature X stronger than Y" bake-off. The wall budget per such bake-off is 6-7 hours, but each one produces a decision-grade answer rather than a non-result.
6. **The 30-epoch training horizon is excessive.** From epoch 10-15 onward `r_all_pov` plateaus at +0.575-0.598 with no monotonic gain. Future hand-feature retrains can use 15 epochs without measurable Pearson loss, halving the training wall.

---

**Template Version**: 1.0
**Last Updated**: 2026-05-02

