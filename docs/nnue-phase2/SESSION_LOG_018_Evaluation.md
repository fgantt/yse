# Evaluation: NNUE Phase 2 — Critique and Recommendations

## Context

You asked for an honest audit of the NNUE Phase 2 work documented in `docs/nnue-phase2/` (sessions 1–18, run from 2026-04-22 through 2026-04-30) so you can judge whether the current direction is sound and what should come next. This document is the deliverable: a verified snapshot of where the implementation actually stands, what's working, where I think the approach is fragile or under-evidenced, and a prioritized recommendation for sessions 19+.

I cross-checked the session logs against the live code in `src/evaluation/nnue.rs`, `src/bin/nnue_offline_trainer.rs`, `src/bin/elo_tester.rs`, the test suite, and the original Phase 2 reference docs (DEEP_DIVE_ANALYSIS, STOCKFISH_COMPARISON, NNUE_IMPLEMENTATION_PLAN). Code matches the logs — there is no documentation drift to flag.

---

## Verified Current State

**Architecture (`src/evaluation/nnue.rs:32–168`)**
- Two feature spaces compiled in: flat (`NUM_NNUE_FEATURES = 2,268` + 1 stm = 2,269) and HalfKP (`NUM_NNUE_FEATURES_HALFKP = 183,708` + 1 stm = 183,709).
- Topology: 2,269 (or 183,709) → 256 → 32 → 1, i16 weights, i32 accumulators/biases.
- HalfKP is **single-perspective** (own-king-conditioned only). stm flip forces a full HalfKP refresh — `nnue_make_move` sets `needs_refresh = true` (lines 787–791).
- Output scale: `cp = output * 400 / 4080` (`SCALE_FACTOR=400`, `OUTPUT_DIVISOR=4080`).
- Pieces-in-hand are **not** encoded in either feature space.

**Training (`src/bin/nnue_offline_trainer.rs`)**
- Adam (β1=0.9, β2=0.999) added in S15; SGD remains the default flag.
- Two loss options: tanh-MSE (default) and sigmoid-MSE (`--use-sigmoid-loss`, S13). Targets blend teacher eval with game outcome via `--outcome-weight`.
- Corpus: ~67K JSONL positions from YaneuraOu self-play at depth 10 (S6/S7).
- Weights save/load by extension: `.bin` → bincode, anything else → JSON (S18).

**Engine integration**
- NNUE refreshed once at search root, then maintained incrementally through make/unmake (`src/search/search_engine.rs` ~14,525). `current_stm` tracked in evaluator (lines 607–608).
- 8/8 NNUE tests pass, including the load-bearing `test_nnue_incremental_matches_full_refresh`, `test_stm_incremental_matches_full_refresh`, `test_halfkp_incremental_matches_full_refresh`, and `test_eval_paths_agree_on_trained_weights`.

**Measured strength (best results)**
- S17, HalfKP+stm vs PST, n=30: **+120.4 ± 76.8 ELO** (CI95 [+43.6, +197.2]), 11W-18D-1L.
- S18, HalfKP+stm vs flat+stm, n=30: **+23.2 ± 72.7 ELO** (CI95 [-49.5, +95.9]), 6W-20D-4L. Indecisive.
- Decomposition: of the S17 +120 ELO over PST, ~+97 is "any NNUE > PST" and only ~+23 is "HalfKP > flat".
- Pearson r (training-time, vs teacher cp): +0.614 with HalfKP, +0.607 with flat. Saturating.

**Time invested**: ~41 hours of dev across 18 sessions, plus ~25 hours of wall-clock training/bake-off.

---

## What's Working

1. **The negative-result discipline.** S10–S13 ran six gradient/loss/optimizer interventions and reported all negative. That's 11+ hours of disciplined falsification that correctly steered the team off the loss-shape lever and toward feature engineering. Most projects don't have the patience for that.
2. **The S14 stm-feature breakthrough.** Single binary feature lifted Pearson r from a ±0.10 noise floor to +0.59. This is a textbook example of identifying that the input space lacked the sufficient statistic for the target (target is to-move-POV, but features were color-symmetric). Correct diagnosis, correct fix.
3. **The Pearson-r metric (S11).** Adding side-stratified rank correlation as a separate signal from td_err is what unblocked S10–S13 diagnostics. Without it, the team would have been chasing td_err numbers that had nothing to say about play strength.
4. **Engine-side incremental accumulator + tested invariants.** The make/unmake snapshot path with O(1) unmake, plus the four tests that compare incremental against full refresh, is the right shape. This is the part of NNUE most teams get subtly wrong, and the test coverage here is good.
5. **Methodology evolution.** Multi-seed bake-offs (42/43/44/45/46), CI95 reporting, head-to-head plumbing (S18) — the measurement infrastructure is now competent enough to inform decisions.
6. **Binary weights format (S18).** 4.57× shrink and 7.7× load speedup at zero behavior change — the right operational call, well-scoped.

---

## Critiques

### 1. The PST baseline is the wrong yardstick for "is NNUE working"

Every ELO measurement in Phase 2 is `<this engine with NNUE>` vs `<this engine with PST>`. This tells you NNUE is better than your own PST evaluator (which it is — by ~+97 ELO of "any NNUE > PST", per the S18 decomposition). It does **not** tell you whether the engine is competitive in absolute terms, and it does not tell you whether your NNUE ceiling is ~1500 ELO or ~2500 ELO.

The reference docs (NNUE_RESEARCH_STOCKFISH_COMPARISON, NNUE_IMPLEMENTATION_PLAN) projected +150–350 ELO based on Stockfish's NNUE adoption gain — but Stockfish's baseline was already ~3300 ELO. Beating an internal PST that is plausibly sub-2000 ELO by +120 doesn't validate the same trajectory.

There is no measurement on file against an external Shogi engine (GnuShogi, fixed-strength YaneuraOu, dlshogi, GIKOU). Until that exists, you cannot answer "is the NNUE actually strong?" — you can only answer "is it stronger than the thing I ship today?".

### 2. n=30 at 500 ms / depth 3 is measuring search noise, not eval quality

S18 explicitly came in indecisive (CI95 ±73 ELO) at n=30. The S19 plan correctly proposes extending to 90–120 games to get to ±35–40, which is a reasonable next step. But there are two deeper concerns:

- **Sample size**: Stockfish's framework (fishtest) uses thousands of games per A/B because ELO standard errors at single-engine-version differences are tight. The cumulative 120-game window across S14–S17 still has ±43 ELO. You're making structural-architecture decisions with 1.5σ noise.
- **Time control / depth**: 500 ms / depth 3 keeps games inside the eval-saturated regime where the search barely sees past the immediate horizon. NNUE's value compounds at deeper search because better leaf eval propagates up the minimax tree. You may be **under-measuring** the true eval gap. A depth-6 or 1000-node bake-off would likely separate HalfKP from flat more clearly than depth-3 / 500 ms.

The bake-off protocol is reproducible and the methodology is sound for this depth — but the conclusions are bounded by the depth chosen, and that bound is not currently acknowledged.

### 3. The next-priority list (S19 P1: pieces-in-hand) is correct, but the ones after it are over-ambitious

S18 closes with five priorities: (1) pieces-in-hand, (2) extend bake-off, (3) trainer `.bin` output, (4) dual-perspective HalfKP-pp, (5) cleanup, (6) corpus extension.

P1 (pieces-in-hand) is the highest-EV unfinished item — the teacher eval includes drop value, the network can't see it, the LoC budget is small (~80 LoC engine + ~50 LoC trainer per S18's own estimate), and the upside (+20–50 ELO) is plausible. **Keep this as P1.**

P4 (dual-perspective HalfKP-pp, ~250 LoC engine + ~100 LoC trainer) is much harder to justify after S18's own decomposition. The S17 +120 ELO over PST is mostly the "any NNUE > PST" component; HalfKP-over-flat is +23 ELO with CI95 brackets 0. Spending 350 LoC and a multi-day rebuild on a dual-perspective variant of a feature space whose marginal value over flat is ~+23 ELO is a poor ROI **unless** justified by inference throughput (which S18 correctly notes — "probably worth doing for inference throughput, not for ELO"). Make sure the inference-throughput case is real before committing: at depth 3 / 500 ms, the bottleneck is the search, not the eval, so the throughput dividend is currently latent.

### 4. Single-perspective HalfKP is an architectural dead-end

The current HalfKP requires a full feature-space refresh on every stm flip (every move). That's 81 squares × ~14 piece-types × 2 players of feature additions, per move, at every search node. The make/unmake snapshot pattern hides this in benchmarks at depth 3, but at depth 6+ the cost compounds. The fact that HalfKP bake-off games ended faster than flat-NNUE games (S18, 47% faster wall time) is partly because the engines reach repetition draws faster, but it also reflects shallower search per second.

Stockfish's HalfKP is dual-perspective precisely so that stm flip costs nothing. If you stay on HalfKP long-term, you will eventually need HalfKP-pp. The question is whether the right move is "build HalfKP-pp now" or "go back to flat-NNUE and add other levers (pieces-in-hand, larger network, deeper layers) to it instead."

### 5. The corpus is small and the network has likely memorized it

67K positions at depth 10 is meaningful for early-stage validation but small for production training. Stockfish's NNUE was trained on 10B+ positions; YaneuraOu uses similar scales. Pearson r saturating at +0.61 at this corpus size strongly suggests the network has fit the corpus's noise as well as its signal. Adding HalfKP's 80× more parameters into 67K positions is a high parameters-to-examples ratio and the +23 ELO marginal gain is consistent with capacity overshooting available signal.

The S19 P6 ("corpus extension to 1M positions") is rightly listed but is buried under five higher-priority items. I'd argue it should move up — particularly before any further capacity-increasing architecture change (HalfKP-pp, deeper layers).

### 6. The deployment story is missing

The 41 untracked `nnue_weights_*.json` files in the repo root, plus the lack of any default-on configuration for NNUE in user-facing entry points, suggests the project hasn't crossed the rubicon of "is this on by default in shipped builds?". S18 correctly identifies cleanup as P5 but doesn't address the larger question: **what does shipping look like?** Specifically:
- Which weights file is the canonical "NNUE on by default" file?
- Where does it live on disk for end users?
- Is the binary embedded or downloaded?
- What's the fallback when the file is missing?

Without a deployment story, the team will continue to accumulate weights files indefinitely and never realize the +97 "any NNUE > PST" ELO in production.

### 7. No comparison to existing Shogi NNUEs

YaneuraOu, dlshogi, and several other open-source Shogi engines have well-tuned NNUE pipelines. The Phase 2 plan was written using Stockfish (chess) as the reference, which the docs themselves flag as imperfect: "Shogi has piece drops, more complex piece values, and different endgame patterns than chess." Yet there's no record of investigating YaneuraOu's feature-space choices or training recipes for adaptation. The team is reinventing what already exists in the Shogi NNUE ecosystem.

### 8. Search-eval coupling has not been audited under the new cp scale

S1 fixed weight init and output scaling so NNUE outputs sane cp values. But search heuristics — aspiration windows, futility margins, null-move-pruning thresholds, late-move-reduction depth — are tuned to the *PST* eval's typical cp magnitude distribution. After swapping in NNUE, the cp distribution changes (different mean, different variance, different ratio of large-magnitude positions). If those heuristics aren't retuned, NNUE's improved leaf eval is being partially squandered by search heuristics that mis-prune.

This is consistent with the S18 finding that HalfKP-vs-flat shows a high draw rate (66.7%): when both engines run the same search heuristics with similar-magnitude eval outputs, the search converges to the same lines. It's also consistent with the surprisingly modest +23 ELO between two architecturally different evaluators.

### 9. The cumulative weight-file sprawl is operationally fragile

41 untracked `nnue_weights_*.json` files are sitting in the repo root, ranging from `nnue_weights_iter_10.json` through experimental epoch checkpoints (`nnue_weights_shadow_highlr_epoch_15.json`). There's no naming convention, no manifest, no archived/active distinction. Reproducing any prior bake-off requires knowing which file was used. This is technical debt that will compound — and the binary format from S18 only makes it worse if checkpoints get emitted as `.bin` without organization.

---

## Recommended Direction

I'd reorder S19 priorities to put falsifiable, high-EV experiments first and defer the architectural rebuilds.

### Priority 1: Pieces-in-hand encoding (keep as S19 P1)
The team's own analysis is correct. ~80 LoC engine + ~50 LoC trainer, retrain on existing corpus, bake-off. Plausible +30–50 ELO. Single biggest unaddressed Shogi-specific gap. **Verify with**: bake-off vs both PST (to confirm cumulative gain) and vs HalfKP+stm without hand features (to isolate hand-encoding's contribution).

### Priority 2: External absolute-strength benchmark (NEW)
Run a 50–100 game match against an external Shogi engine — fixed-strength YaneuraOu (e.g., depth-1 or depth-3) is the easiest because the corpus generator already speaks USI. The goal is to anchor the +120 ELO over PST in absolute terms. If this engine is ~1500 absolute, +120 puts us at ~1620 — a much harder pitch for shipping than if we're already at ~2200. This single measurement should drive whether the next session is "ship what we have and move on" or "keep iterating on architecture."

### Priority 3: Deeper-search bake-off sanity check (NEW, small)
Re-run S17's HalfKP-vs-PST 30-game bake-off at depth 5 / 2000 ms (or 5000 nodes). If the ELO gap is meaningfully larger than +120 at the deeper TC, that confirms NNUE's value compounds with depth and changes the calculus on pursuing HalfKP-pp (for inference throughput at deeper search). If the gap is roughly the same, NNUE's value at this stage is already saturated by search shallowness, and the bottleneck is elsewhere (search heuristics, corpus quality, feature richness).

### Priority 4: Tighten the HalfKP-vs-flat decision (S19 P2, demoted)
Extend S18's bake-off to 90–120 games. Worth doing, but lower priority than the above three because it answers a relatively narrow question (is the +23 ELO real?). The decomposition already tells us most of the gain is "any NNUE", so the strategic value of confirming HalfKP > flat is modest.

### Priority 5: Trainer `.bin` checkpointing + weight-file cleanup (combine S19 P3 + P5)
Move the 41 weights files into `weights/archive/` with a manifest noting which produced which session's bake-off result. Default trainer to `.bin`. Add `.gitignore` for `nnue_weights_*.json` and `nnue_weights_*.bin` at repo root. ~30 minutes of work, retires real operational drag.

### Priority 6: Deployment story (NEW)
Decide where NNUE weights live for shipped builds. Add a `nnue/default.bin` (or similar) committed via Git LFS or downloaded on first run. Wire the engine to load it by default with a clean fallback to PST. Even a stub answer to this question now will save weeks of fragmentation later.

### Priority 7: Corpus extension (S19 P6, promoted)
Bring corpus from 67K to 500K–1M positions before any further capacity-increasing architecture work (HalfKP-pp, larger hidden layers). The combination of "small corpus + high-capacity feature space" is the most likely cause of the saturating Pearson r and the modest HalfKP-vs-flat gap. Wall budget ~12–15 hours of YaneuraOu generation.

### Priority 8 (deferred): Dual-perspective HalfKP-pp
Push to a later session, conditional on Priority 3 showing that NNUE benefit grows with depth. If depth scaling is flat, HalfKP-pp's inference-throughput dividend doesn't matter and you'd be spending 350 LoC for no measurable ELO. If it grows, schedule HalfKP-pp after pieces-in-hand and corpus extension are landed.

### Strategic question worth asking (Priority 9)
Look at YaneuraOu's NNUE feature-space and training pipeline (`learn` directory in the YaneuraOu repo). Specifically: how do they encode pieces in hand? Do they use HalfKP, HalfKAv2, or something Shogi-specific? Is their corpus generation publicly available? An afternoon spent here may save weeks of independent rediscovery and may also provide a strong external NNUE to bench against.

---

## What I Would NOT Do Next

- **Don't pursue further loss/optimizer tuning.** S10–S13 ruled this out in 11+ hours of work. If a future session feels tempted to revisit, re-read those logs first.
- **Don't commit to dual-perspective HalfKP-pp until Priority 3 confirms depth scaling.** It's a 250 LoC engine change with no ELO upside if NNUE is already saturated by shallow search.
- **Don't expand the network capacity (more hidden units, deeper layers) until corpus is >500K positions.** You'll just memorize harder.
- **Don't ship the binary format change retroactively to existing `.json` checkpoints.** The S18 dispatch handles both transparently; converting historical files is unnecessary churn.

---

## Critical Files for Reference

If acting on these recommendations, the relevant code locations are:

- `src/evaluation/nnue.rs:32–168` — feature space constants, weight struct (where pieces-in-hand features would be added)
- `src/evaluation/nnue.rs:766–856` — incremental accumulator (where any feature-space addition needs matching make/unmake support)
- `src/evaluation/nnue.rs:965–1345` — test module (must add new tests for any new feature)
- `src/bin/nnue_offline_trainer.rs:22–216` — trainer entry point (where pieces-in-hand encoding, larger corpus loading, `.bin` checkpoint emission go)
- `src/bin/elo_tester.rs` — bake-off harness (where deeper time-control flags would be added; head-to-head plumbing is already in place from S18)
- `src/search/search_engine.rs:14,046–14,078` and ~14,525 — search-eval boundary (where any search heuristic retuning for NNUE cp scale would land)

## Verification

The recommendations above are testable, in this order:

1. **Pieces-in-hand**: bake-off 30 games HalfKP+stm+hand vs HalfKP+stm at the same time control as S17. Expect CI95 to clearly exclude 0 if the +30–50 ELO estimate is right.
2. **External benchmark**: 50-game match HalfKP+stm vs fixed-strength YaneuraOu via USI. Outcome anchors absolute strength.
3. **Deeper-search sanity check**: re-run S17 protocol at depth 5 / 2000 ms. Compare ELO point estimate to the depth-3 / 500 ms baseline. >+50 ELO delta = NNUE value scales with depth, justifies HalfKP-pp later.
4. **Larger corpus**: re-train HalfKP+stm on 500K+ positions. Pearson r should lift toward +0.65+ if the corpus was the binding constraint.

Each verification is independently scoped (1–6 hours wall) and produces a falsifiable outcome.
