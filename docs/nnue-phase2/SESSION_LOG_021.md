# NNUE Implementation Session Log

## Session 21: Feature-Space Audit — The Plateau Was the Metric; Perspective Normalisation Lifts Held-Out r From 0.449 to 0.641

**Date**: 2026-09-17
**Phase**: Phase 2o
**Objective**: Audit the NNUE feature spaces and their implementations before resuming training, on
the hypothesis that the Session 15–20 plateau reflected a defect rather than a capacity limit.

> **READ THIS BEFORE SESSIONS 15–20.** Those logs conclude that the 256→32→1 architecture "has
> saturated for this corpus." **That conclusion is refuted.** The diagnostic they were steered by had a
> mathematical ceiling of r = 0.608 and the network had been sitting on it since Session 15. Treat
> every r-based conclusion from Sessions 15–20 as unreliable, and every "reproducible, seed N" claim
> from Sessions 1–20 as false (weight init was never seeded). Full detail:
> `FEATURE_SPEC_AND_AUDIT.md`.

---

## Pre-Session Checklist

- [x] Read `SESSION_LOG_020.md` — Session 21 plan was "extend Bake-off B to 90–120 games".
- [x] **Plan was not executable as written**: every weight file from Sessions 14–20 lived only in
      `/tmp` and had been purged. `weights/archive/` holds 42 files, none from Session 14 onward.
- [x] **Build was broken**: CLT `MacOSX27.0.sdk` declares an `arm64e.x1` architecture the installed
      clang cannot parse. Workaround is
      `export SDKROOT=/Applications/Xcode.app/.../MacOSX.sdk`. Not a code problem.
- [x] Corpus `nnue_corpus_yaneura_d10.jsonl` (67 403 records) present, so retraining was reproducible
      from the logged commands.

---

## Headline Results

1. **The Pearson diagnostic had a ceiling of 0.608** (Finding 1). It scored the network against raw,
   unclamped `eval_cp` while training clamped the target to ±2000. The corpus's mate tail (|cp| >
   20000) is 8.1% of records but carries **97.2% of the variance in raw cp**, and Pearson is
   variance-weighted — so the metric was largely a test of predicting mate scores, which the clamp
   removes from training. `corr(clamp(teacher, ±2000), raw teacher) = 0.608`. Reported values were
   S15 +0.607, S16 +0.614, S19 +0.610, S20 +0.595. **Six sessions of architecture and corpus decisions
   were steered by a number that could not move.**

2. **There was no held-out validation set** (Finding 1b). `validation_pearson` was passed the training
   corpus, which is reshuffled every epoch. Every r since Session 11 is a training-fit number.

3. **No feature space had perspective normalisation** (Finding 2). All three key on absolute player
   and absolute square while the target is to-move-POV, so a position and its 180° colour-mirror —
   identical positions with identical value — shared **zero** active feature indices. On the *starting
   position* the two orientations evaluated to +11447 and −8121. Fixing this was the single largest
   win of the session.

4. **The engine floored every product before accumulating** (Finding 3), while the trainer divided
   exactly, so the optimiser was differentiating a function the engine never computed. Drift on
   identical inputs: 148–673 cp.

5. **Weight init was never seeded.** `NNUEWeights::new_with_features` used `rand::thread_rng()`, so
   `--seed` controlled only corpus shuffling. No fresh-init run in Sessions 1–20 was reproducible.

6. **`--output-weights` saved the last epoch, not the best.** Held-out r peaks around epoch 11–15 then
   declines, so every recorded 30-epoch run ended by saving weights worse than the peak it had already
   reached.

7. **Session 20's Issue 7 root-cause hypothesis is refuted.** It attributed the colour asymmetry to a
   per-side corpus imbalance. Measured, the corpus is balanced within 1% on every axis (50.2% Black
   share; mean |cp| 3449 vs 3419; decisive fraction 53.8% vs 53.6%). The asymmetry was architectural,
   and it shows up as a *generalisation* gap: held-out Black/White 0.525 vs 0.378 while training-set
   Black/White was 0.922 vs 0.940.

---

## Cumulative Result

Held-out `pearson[clamp]`, same recipe and seed (47) throughout, changing one thing at a time:

| Configuration | held-out r |
|---|---|
| colour-absolute + stm + hand, epoch 30 (the Session 20 recipe) | +0.449 |
| POV features, epoch 10 | +0.618 |
| POV + single-shift arithmetic, best epoch (11) | **+0.641** |
| *(baseline retrained under current code, for a fair bake-off)* | +0.471 |

And the overfitting picture the new split exposed, on the pre-POV baseline:

| | training set | held-out |
|---|---|---|
| `pearson[clamp]` | +0.928 | +0.449 |
| `spearman[raw]` | +0.939 | +0.383 |

So the plateau was the metric — the network reached 0.928 on the objective it actually optimises —
**but it also overfits badly.** The Session 18 Eval's critique #5 ("the corpus is small and the network
has likely memorised it") was right all along; it could not be demonstrated while the metric measured
training fit against a ceiling. Generalisation, not capacity, is the binding constraint, which inverts
the case for HalfKP-pp and wider layers: they would make it worse.

---

## Work Completed

| # | Change | Commit |
|---|---|---|
| 1 | `validation_pearson` scores against the clamped target, adds Spearman, prints the legacy number beside its own ceiling; game-level 20:1 held-out split | `508b4d8` |
| 2 | `feature_index_pov` / `hand_feature_index_pov`, `refresh_pov`, trainer extractors, `--use-pov-features` | `cfa57bc` |
| 3 | Dual accumulators (one per perspective) so POV works in search; `--use-pov-features[-b]` on `elo-tester` | `8109e02` |
| 4 | Single-shift i64 arithmetic; `--early-stop-patience`; save-best-not-last; seeded weight init | `f6dc97a` |
| 5 | Revised Finding 4 from measurement; documented the duplicated trainer forward/backward | *(this log)* |

**New test suite**: `tests/nnue_semantics.rs` — 10 oracle tests asserting properties the encoding must
satisfy, rather than comparing the implementation against itself. Index injectivity and range for all
three spaces, hand-eligibility rules, trainer↔engine extraction parity (legacy *and* POV),
mirror invariance under POV, a pin on the legacy encoding's non-invariance, and trainer↔engine
arithmetic agreement as relative error.

**New NNUE lib tests**: POV incremental matches full refresh at every ply of a 10-ply sequence; unmake
restores *both* accumulator frames; POV + hand captures match full refresh. 14/14 pass.

---

## Decisions Made

**Decision 1**: Fix the diagnostic before the encoding, reversing the order originally proposed.
- **Rationale**: while the metric was pinned at its ceiling, no training change could be evaluated —
  including the encoding fix. Re-baselining first gave every later change something to be measured
  against.
- **Confidence**: High. This turned out to be load-bearing; the encoding result is only interpretable
  because the metric moved.

**Decision 2**: Dual accumulators rather than full-refresh-per-move for the engine-side POV path.
- **Rationale**: full refresh is what HalfKP already accepts, but it would bias any depth-sensitive
  bake-off against NNUE by refresh cost. Dual accumulators keep each frame's indexing fixed, so both
  stay incrementally updatable and a stm flip only changes which is read. The earlier docs deferred
  this shape ("HalfKP-pp") on ROI grounds; that calculus changed once the eval gain was measured
  rather than speculative.
- **Confidence**: High.

**Decision 3**: Retrain the baseline under current code before the bake-off.
- **Rationale**: the existing baseline was optimised against the old floored arithmetic; pitting it
  against the fixed engine would have quietly handicapped it. Retrained fairly it scores 0.471, up
  from 0.449 — it benefited from the arithmetic fix too.
- **Confidence**: High.

**Decision 4**: Keep depth 3 / 500 ms for the bake-off despite the S18-Eval depth critique.
- **Rationale**: comparability with the 150-game historical vs-PST arm. A depth-5 run is ~4× the wall
  clock and is the right follow-up only if these come back positive but modest.
- **Caveat**: shallow search may *under*-measure NNUE, since better leaf eval compounds up the tree.
- **Confidence**: Medium.

**Decision 5**: Accept that the bake-off confounds four changes (POV, arithmetic, save-best, early
stopping).
- **Rationale**: separating them costs ~6 h per arm. The aggregate question ("did the audit work
  convert to ELO?") is the one worth answering first; attribution matters only if the aggregate is
  surprising.
- **Confidence**: Medium.

---

## Blockers & Issues

### Issue 7 (from S20): colour asymmetry — **root cause corrected, fix landed**
The corpus-imbalance hypothesis is refuted by measurement. Cause was the missing perspective
normalisation; mirror invariance is now exact by construction.

### Issue 8 (from S20): 30-game bake-offs underpowered — **addressed**
This session's bake-offs are 90 games each, targeting ~±54 ELO.

### Issue 9 (NEW): all weight tensors use a small fraction of their quantisation range
`output_weights` is 32 numbers drawn from **11 distinct values**, max magnitude 42 against a ±32767
range; `input_weights_1` has 99% of values ≤16 with nothing at the ±127 clamp. Effective precision is
~5 bits where 8–15 are available. This is a scale-allocation problem — the split of scale between
layers is a free reparameterisation and the `>> 6` divisors appear chosen rather than derived.
- **Status**: Open, with a proposed fix and an explicit caution (the `/64` factor appears in ~15
  places across two implementations; introduce a shared constant and a finite-difference gradient
  check first). See Finding 4.

### Issue 10 (NEW): the trainer has two forward/backward implementations, neither marked as live
`update_weights_for_position` (online TD(λ)) and `train_batch_accumulated` (the only path the offline
trainer reaches). The Finding 3 fix initially landed in the unused one. See Finding 6.
- **Status**: Open (both consistent and cross-referenced, but not deduplicated).

### Issue 11 (NEW): `cargo test --lib evaluation::` has 6 failures plus a stack overflow
All reproduce unchanged at `e264e22` and are outside NNUE (`advanced_interpolation`, `castles`,
`config`, `endgame_patterns`). Every session log runs only the `evaluation::nnue` subset, so these went
unnoticed. Same blind spot as Finding 5, one level up.
- **Status**: Open, pre-existing, untouched by this session.

---

## Next Session Plan (Session 22)

**Prerequisite**: read the two 90-game bake-off results (`weights/session21/bakeoff_*.csv`), launched
at the end of this session — POV vs the retrained baseline (seed 50) and POV vs PST (seed 51).

**Priority 1 — Attack overfitting.** Now the binding constraint and, for the first time, measurable.
Cheap levers first, since training is ~45 s: L2 / weight decay, dropout. Note POV already delivered a
2× effective-data increase, so re-measure before assuming corpus expansion is needed — its payoff can
now be verified rather than argued.

**Priority 2 — Issue 9 (quantisation scale).** Introduce a shared shift constant, add a
finite-difference gradient check, then try `>> 3`. Plausible upside; needs the care noted above.

**Priority 3 — Deeper-search bake-off.** Depth 5 / 2000 ms, now that dual accumulators removed the
per-move refresh penalty that would have confounded it.

**Priority 4 — Deployment.** `usi-engine` has no `--use-pov-features` equivalent, so shipped builds
cannot use the best network. Also still unanswered from S18-Eval: which weights file ships, where it
lives, and what happens when it is missing.

**Priority 5 — HalfKP under POV.** Deliberately last: HalfKP multiplies the feature space 81× while
overfitting is the binding constraint, so it is the wrong direction until Priority 1 lands.

**Do not**: expand the corpus or add capacity before Priority 1 reports. Both were argued for in
S18-Eval from the saturated metric.

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Audit complete; four fixes landed across `508b4d8`, `cfa57bc`, `8109e02`, `f6dc97a`.
Held-out `pearson[clamp]` 0.449 → 0.641. Two 90-game bake-offs running at sign-off.
**Ready for next session**: Yes, once the bake-offs report.
**Comments**:
1. **Every gain this session is held-out eval quality, not measured ELO.** The bake-off is the
   unconverted step. Do not describe 0.641 as a strength result.
2. **The metric is now the asset.** `pearson[clamp]` on held-out games moves, has a ceiling of 1.0,
   and measures generalisation. Most of the value of this session is that future experiments can be
   evaluated at all.
3. **Two independent process failures let the defects persist for 20 sessions**: a test suite that only
   compared the implementation against itself (Finding 5), and a habit of running only the
   `evaluation::nnue` subset (Issue 11). Both are worth more attention than any single bug found here.
4. **Reproducibility is new, not restored.** Runs before `f6dc97a` cannot be reproduced even given
   their logged seeds, so their third-decimal comparisons were within init noise (±0.01) of each other.

---

**Template Version**: 1.0
**Last Updated**: 2026-09-17
