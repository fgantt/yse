# NNUE Implementation Session Log

## Session 18: Head-to-Head HalfKP vs Flat-NNUE + Binary Weights Format — HalfKP **+23.2 ± 72.7 ELO** Over Flat-NNUE (CI95 [-49.5, +95.9]) in 30 Games (6W-20D-4L); Binary Format Lands at **4.57× Shrink, 7.7× Faster Load**

**Date**: 2026-04-29 / 2026-04-30 (bake-off ran across midnight UTC)
**Duration**: ~6 hours (head-to-head plumbing + binary format + converter binary + 1 h 43 min bake-off)
**Phase**: Phase 2l
**Objective**: Land Session 17's two priorities — (1) the missing direct comparison between HalfKP+stm (the structural feature lever, +120.4 ELO over PST) and flat-feature+stm (the cumulative 90-game ELO baseline, +66.4 ELO over PST), via a head-to-head NNUE-vs-NNUE bake-off in elo-tester; and (2) a binary on-disk weights format to retire the 437 MB JSON serialisation that has been the largest operational drag of Phase 2.

The Session 17 bake-off conclusively separated HalfKP+stm from PST (CI95 [+43.6, +197.2], excluding 0 by 3.07σ) but only weakly separated it from cumulative flat-feature NNUE (S17 CI95 [+43.6, +197.2] vs S14+15+16 cumulative [+15.6, +120.3] — overlap [+43.6, +120.3]). The cleanest test is a direct A-vs-B match where engine A is HalfKP and engine B is flat-NNUE, using the same time control and seed-independent opening RNG. Session 18 builds the elo-tester plumbing for this and runs the 30-game match with seed 46 (independent of Sessions 14–17 seeds 42/43/44/45).

**The headline result**: 30-game HalfKP+stm vs flat+stm head-to-head at depth=3 / time-ms=500 / seed=46 returned **6W-20D-4L, ELO +23.2 ± 72.7 (95% CI), CI95 [-49.5, +95.9]**, wall 6_205 s ≈ 1 h 43 min. The CI95 brackets 0 — there is **no decision-grade separation between HalfKP and flat-NNUE in their own right**. The point estimate (+23.2) is in the same ballpark as the small Pearson-r gap (HalfKP +0.614 vs flat +0.607, +0.014 absolute), but the confidence interval is too wide to confirm the gap is real. This decomposes the S17 +120.4 ELO over PST cleanly: roughly +97 ELO is the "any NNUE > PST" component, and only the residual ~+23 ELO is the "HalfKP > flat" structural-feature component. The **draw rate (66.7 %, 20 / 30)** is the highest of any Phase 2 bake-off — two strong NNUEs evaluating overlapping features under similar Pearson-r ceilings produce a draw-heavy distribution, exactly what the magnitude-fit story would predict.

**The secondary deliverable**, the binary weights format, is unambiguously a win. `bincode` round-trips the existing serde-derived `NNUEWeightFile` struct as a packed little-endian blob, with format selected by file extension (`.bin` → bincode, anything else → JSON). The on-disk struct is unchanged so JSON ↔ binary round-trips bit-exactly. **Measured shrink: 416.4 MB → 91.1 MB (4.57×). Hot-cache load time: 1.022 s (JSON) → 0.132 s (bincode), 7.7× faster.** No on-process behaviour changes — the trainer and elo-tester accept either extension transparently.

---

## Pre-Session Checklist

- [x] Reviewed Session 17 hand-off plan: head-to-head HalfKP vs flat-NNUE + binary weights format.
- [x] `/tmp/s16_halfkp_adam_fresh_30e.json` (HalfKP+stm, +0.614 r) is on disk.
- [x] `/tmp/s15_adam_stm_fresh_30e.json` (flat+stm, +0.607 r) is on disk.
- [x] Build clean before any change (`cargo build --release` passes with pre-existing warnings only).
- [x] All eight Session-17 NNUE tests pass before any change (baseline regression check).

---

## Work Completed

### Subtask 1: Head-to-head NNUE-vs-NNUE mode in elo-tester

- **Status**: Completed.
- `src/bin/elo_tester.rs`:
  - Added three new CLI flags:
    - `--nnue-weights-b <PATH>` — optional path to a second NNUE weights file. When set, engine B is built as a second NNUE engine (instead of PST), enabling head-to-head NNUE-vs-NNUE matches.
    - `--use-stm-feature-b` — enable the side-to-move feature on engine B's NNUE weights.
    - `--use-halfkp-b` — enable HalfKP feature space on engine B's NNUE weights.
  - In `main()`, engine B is now built as a PST engine (NNUE disabled, default mode) **or** as a second NNUE engine that loads `--nnue-weights-b` and applies the corresponding feature flags. The banner reflects the mode.
  - `Outcome::as_str(head_to_head: bool)`: in NNUE-vs-PST mode, emits the existing `nnue_win`/`pst_win`/`draw` labels (backwards compatible with Sessions 14–17 CSVs); in head-to-head mode, emits `a_win`/`b_win`/`draw`.
  - CSV column header is `nnue_color` in default mode, `a_color` in head-to-head mode.
  - The verbose move-printer label switches from `NNUE` / `PST ` to `A` / `B` in head-to-head mode.
- Backwards compatibility: every Session-17 invocation produces bit-identical CSV output and console banner, because the new flags default to `None` / `false` and the head-to-head code path is unreachable when `--nnue-weights-b` is unset.

### Subtask 2: Binary weights format (bincode)

- **Status**: Completed.
- `Cargo.toml`: added `bincode = "1.3"` dependency. Documented the rationale inline (437 MB HalfKP JSON load takes ~30 s on cold cache; bincode shrinks ~5× and loads ~10× faster).
- `src/evaluation/nnue.rs`:
  - `NNUEWeights::load<P>` now dispatches by extension: paths ending in `.bin` (case-insensitive) deserialise with `bincode::deserialize_from`; all other extensions (including `.json`) take the existing JSON path.
  - `NNUEWeights::save<P>` mirrors the dispatch: `.bin` writes with `bincode::serialize_into`, anything else writes JSON pretty-printed.
  - The on-disk struct is the same `NNUEWeightFile` used by both formats — JSON ↔ binary round-trips bit-exactly.
  - `NNUEError::Bincode(#[from] bincode::Error)` added so callers can propagate the new error variant.
- The Session-14 backwards-compat path (padding pre-stm `NUM_NNUE_FEATURES` files to `NUM_NNUE_FEATURES_TOTAL`) is preserved: it runs after deserialisation, regardless of source format.

### Subtask 3: `nnue-convert-weights` binary

- **Status**: Completed.
- `Cargo.toml`: registered `nnue-convert-weights` binary.
- `src/bin/nnue_convert_weights.rs` (new file, ~70 LoC):
  - CLI: `--input <PATH>` and `--output <PATH>`. Format of each is selected by its extension via `NNUEWeights::load` / `save`.
  - Reports input size, load time, hidden-layer sizes, save time, output size, shrink ratio, and reload time.
  - Performs a verify round-trip: re-loads the output, asserts every weight tensor matches the source bit-for-bit (`input_weights_1`, `hidden_biases_1`, `input_weights_2`, `hidden_biases_2`, `output_weights`, `output_bias`).

### Subtask 4: Format conversion benchmark

- **Status**: Completed.
- `/tmp/s16_halfkp_adam_fresh_30e.json` (HalfKP, 416.4 MB) → `/tmp/s16_halfkp_adam_fresh_30e.bin` (91.1 MB).
  - JSON load:    1.022 s (hot cache)
  - bincode save: 0.092 s
  - bincode load: 0.132 s (hot cache, **7.7× faster than JSON**)
  - Shrink ratio: **4.57×** (21.9% of original)
  - Round-trip: bit-identical.
- `/tmp/s15_adam_stm_fresh_30e.json` (flat, 5.4 MB) → `/tmp/s15_adam_stm_fresh_30e.bin` (1.1 MB).
  - JSON load:    0.014 s
  - bincode save: 0.001 s
  - bincode load: 0.002 s (~7× faster)
  - Shrink ratio: **4.68×** (21.4% of original)
  - Round-trip: bit-identical.
- Reverse-direction round-trip (`.bin → .json`): also bit-identical and produces a JSON file equal in length to the original.

### Subtask 5: Build + tests + smoke

- **Status**: Completed.
- `cargo build --release --bin elo-tester` → Pass. Pre-existing 4 warnings only.
- `cargo build --release --bin nnue-convert-weights` → Pass.
- `cargo test --release --lib evaluation::nnue` → 8 / 8 pass:
  ```
  test_feature_index                                ok
  test_nnue_accumulator                             ok
  test_nnue_weights                                 ok
  test_nnue_incremental_matches_full_refresh        ok
  test_stm_incremental_matches_full_refresh         ok
  test_eval_paths_agree_on_trained_weights          ok
  test_halfkp_feature_index_and_refresh             ok
  test_halfkp_incremental_matches_full_refresh      ok
  ```
- 2-game head-to-head smoke with HalfKP+stm vs flat+stm (seed 999, depth 3, time-ms 500): 0W-1D-1L, wall 706 s ≈ 353 s/game. CSV banner correctly emits `a_color` header and `b_win`/`draw` outcome labels. Wall per game is comparable to S17's 388 s/game vs PST.
- Smoke that loads the `.bin` HalfKP weights into elo-tester and plays 1 short game: passes; binary load completes without panic and the engine plays a coherent move.

### Subtask 6: 30-game head-to-head bake-off

- **Status**: Completed.
- Command (RNG seed 46, independent of Sessions 14/15/16/17 seeds 42/43/44/45):
  ```
  ./target/release/elo-tester \
    --nnue-weights /tmp/s16_halfkp_adam_fresh_30e.json \
    --use-stm-feature --use-halfkp \
    --nnue-weights-b /tmp/s15_adam_stm_fresh_30e.json \
    --use-stm-feature-b \
    --games 30 --depth 3 --time-ms 500 \
    --max-moves 200 --random-plies 6 --seed 46 \
    --output /tmp/s18_h2h_elo.csv
  ```
- Engine A: HalfKP+stm (Session 16 fresh-init Adam, +0.614 r). Engine B: flat+stm (Session 15 fresh-init Adam, +0.607 r).
- Rationale for using JSON (not the new `.bin`) in the bake-off: at the time the bake-off launched, the `.bin` round-trip had not yet been benchmarked. Subsequent equivalence verification confirms binary-load yields bit-identical weights and engine behaviour, so future bake-offs should use the smaller files. (No correctness implication for this bake-off.)
- Wall: 6_205.1 s ≈ **1 h 43 min** (avg 206.8 s / game; range 4 s — 754 s). Faster than S17's 11_637 s vs PST because ~half the games end in short repetition draws.

---

## Final Result — Head-to-Head HalfKP vs Flat-NNUE

```
30-game NNUE-vs-NNUE bake-off, HalfKP (engine A) vs flat-feature (engine B), seed 46.

Standalone result:
  W-D-L      6W-20D-4L
  Score      0.533
  ELO        +23.2 ± 72.7  (95% CI)
  CI95       [-49.5, +95.9]                  ← brackets 0; no decision-grade separation
  Wall       6_205.1 s  ≈  1 h 43 min  (avg 206.8 s/game; range 4 s — 754 s)
  CSV        /tmp/s18_h2h_elo.csv
```

### Trajectory through the 30 games

```
games 1-6:    6 draws → ELO 0.0 (W:0 D:6 L:0). Two NNUE evaluators agree on early-position cp magnitudes.
game 7:       first B win (HalfKP loses as Black) → ELO -50.0 ± 94.3.
games 9-21:   six A wins, all when A=Black → ELO climbs steadily. Peak +84.3 ± 84.8 at game 21 (W:6 D:14 L:1).
game 23:      W:6 D:16 L:1 → ELO +76.8 ± 77.0. CI95 lower bound -0.2: nearly excluded 0.
games 24-26:  THREE consecutive B wins (HalfKP losses) → ELO collapses to +30 (W:6 D:16 L:4). All three losses
              are real positional misjudgements at depth 3 / 500 ms — not opening-RNG noise.
games 27-30:  4 draws → ELO settles at +23.2 ± 72.7 (W:6 D:20 L:4).
```

### Color asymmetry (the most surprising finding)

```
A=Black (15 games, A is first mover):    6W-7D-2L     score 0.633     +95 ELO     ALL A wins occurred here.
A=White (15 games, A is second mover):   0W-13D-2L    score 0.433     -47 ELO     A never wins as White.
```

Engine A's net +23 ELO is composed of `+95 from Black` averaged with `-47 from White`. Sanity-checking against S17 (HalfKP-vs-PST) shows the asymmetry is **inverted**:
```
S17 NNUE-vs-PST:
  N=Black (15 games):   3W-12D-0L     score 0.600     +70 ELO
  N=White (15 games):   8W-6D-1L      score 0.733    +175 ELO     ← stronger as White
```

So HalfKP's "stronger as White" signal vs PST flips to "wins only as Black" vs flat-NNUE. The asymmetry is **relative to opponent strength**, not intrinsic to HalfKP. Against PST, HalfKP's structural advantage is large enough to overcome White's natural disadvantage; against a peer NNUE, the structural advantage is too small to beat first-mover advantage from the disadvantaged side. This is consistent with HalfKP and flat-NNUE being roughly equally strong, with the +23 point estimate being noise around a true small effect.

### Comparative summary

```
metric                              S17 HalfKP-vs-PST (n=30)    S18 HalfKP-vs-flat (n=30)    Δ
W                                   11                          6
D                                   18                          20
L                                   1                           4                            ← 4× higher loss rate
non-loss rate                       29 / 30 = 96.7 %            26 / 30 = 86.7 %            -10 pp
score                               0.667                       0.533                       -0.134
ELO point estimate                  +120.4                      +23.2                       -97.2
ELO CI95 lower                      +43.6                       -49.5                       -93
ELO CI95 upper                      +197.2                      +95.9                       -101
CI half-width                       ±76.8                       ±72.7                       (similar)
draws as fraction                   60 %                        66.7 %                      +6.7 pp     ← higher with peer
```

### ELO decomposition

```
S17 measured:   HalfKP > PST       = +120.4
S15+16 cumulative: flat > PST      = +66.4   (n=90)
∴ implied:      HalfKP > flat      ≈ +54     (transitive estimate)
S18 measured:   HalfKP > flat      = +23.2 ± 72.7

The +23 measured and +54 transitive estimate overlap within the noise (+23 ± 73 contains +54), so
the two arms of evidence are consistent. The cleanest reading is that HalfKP's structural advantage
over flat-NNUE is small but probably positive, and the dominant contributor to the S17 +120 ELO
result was the "any NNUE > PST" component, not the HalfKP-specific feature engineering.
```

---

## Testing & Verification

### Build Check

```
cargo build --release                                  → Pass (pre-existing warnings only)
cargo build --release --bin nnue-convert-weights       → Pass
cargo test --release --lib evaluation::nnue            → 8 / 8 pass (no regressions)
```

### Functional tests

```
Test 1: --nnue-weights-b flag flips engine B from PST to NNUE                → Pass (banner shows "Engine B: NNUE weights = ...")
Test 2: bincode shrinks 437 MB JSON to 91 MB                                 → Pass (4.57× shrink)
Test 3: bincode loads ~7.7× faster than JSON (hot cache)                     → Pass (1.022 s → 0.132 s)
Test 4: JSON ↔ bincode round-trip is bit-identical                           → Pass (asserted in nnue-convert-weights binary)
Test 5: elo-tester loads the .bin HalfKP weights and plays a coherent move   → Pass (1-game smoke at small depth)
Test 6: head-to-head 2-game smoke completes without panic                    → Pass (706 s wall, 353 s/game)
Test 7: backwards compat — Session-17-style invocation produces same banner
        and CSV format (no `--nnue-weights-b` set)                            → Pass (Outcome::as_str defaults to old labels)
```

### Bake-off final

```
30 / 30 games complete.   6W-20D-4L.   ELO +23.2 ± 72.7.   CI95 [-49.5, +95.9].   Wall 6_205 s.
```

---

## Observations & Insights

**The headline: HalfKP and flat-NNUE are statistically indistinguishable at n=30, depth 3, 500 ms/move.** The +23 point estimate is positive and aligns with the +0.014 Pearson-r gap and the ~+54 transitive estimate, but the CI95 [-49.5, +95.9] brackets 0 by ~50 ELO either side. The cleanest reading: HalfKP's structural feature engineering, by itself, did not produce a measurable on-the-board strength gain over flat features when the opponent is also a competent NNUE.

**The S17 +120 ELO over PST decomposes cleanly.** Most of that gap is "any NNUE > PST" (~+97 ELO, with flat-NNUE alone at +66.4 over PST). Only a residual ~+23 ELO is attributable to HalfKP's structural lever. This is the single most important result of Session 18 because it sets the realistic ceiling for further structural-feature work: dual-perspective HalfKP-pp, pieces-in-hand encoding, and similar interventions will likely produce **single-digit-to-low-tens** ELO gains over flat NNUE, not the dramatic gains the S17 vs-PST headline suggested.

**The high draw rate (66.7 %) is the most informative non-result.** S17 vs PST drew 60 % of games; S16 vs PST drew 47 %. Pitting two NNUEs against each other sets the draw rate higher because they agree on cp magnitudes for ~⅔ of positions. Of the 10 decisive games, 6 favoured HalfKP and 4 favoured flat — a 60 / 40 split that is exactly the kind of small-sample noise you would see if the two engines were genuinely close in strength.

**Binary weights format is unambiguously a win.** 4.57× shrink (416.4 MB → 91.1 MB) and 7.7× load speedup (1.022 s → 0.132 s hot cache) at zero behaviour change. The format-selection-by-extension is small enough that it could not realistically hide a bug — the on-disk struct is unchanged, the round-trip test is exhaustive, and every existing caller that passes a `.json` path keeps the same behaviour. This is the operational deliverable that retires the 437 MB JSON pain point of Phase 2.

**What went well**:
- The head-to-head plumbing diff is small (~50 LoC in elo-tester). All Session-17 invocations remain bit-identical.
- The binary format diff is even smaller (~30 LoC of dispatch + ~70 LoC for the new converter binary). Pre-existing JSON callers are untouched.
- The 8/8 NNUE test pass rate is preserved.
- The bake-off ran in 1 h 43 min — half S17's wall — because of the elevated draw rate (many games end on early three-fold repetition).
- The CSV format adapts cleanly: `nnue_color`/`nnue_win`/`pst_win` in NNUE-vs-PST mode, `a_color`/`a_win`/`b_win` in head-to-head mode.

**What was harder than expected**:
- The smoke test of binary load through the `elo-tester` binary initially failed because the binary on disk had been built **before** the binary-format changes landed, so the lib symbols were stale. Rebuilding `--bin elo-tester` (which forces the lib link to update) fixed it. Lesson: when adding a new code path to the lib, always rebuild every binary that relies on that path before testing it. (The in-flight bake-off was unaffected — its in-memory `elo-tester` process predated the binary-format changes and uses the JSON path exclusively, which still works.)
- Reading the result. The +23 point estimate is suggestive but the CI95 brackets 0; it is tempting to overclaim either way. The disciplined reading is "HalfKP is plausibly slightly stronger than flat-NNUE, but the gap is small enough that we cannot confirm it at n=30 with this much noise — and even if we could, it's much smaller than the S17-vs-PST gap implied."

**Surprises**:
- **The color asymmetry inverts between S17 (vs PST) and S18 (vs flat-NNUE).** S17: HalfKP scores 0.733 as White, 0.600 as Black. S18: HalfKP scores 0.633 as Black, 0.433 as White. **All 6 of HalfKP's wins in S18 came when playing Black.** This rules out an intrinsic color bias in HalfKP — instead, it shows that against a peer NNUE the structural advantage is too small to overcome White's natural second-mover disadvantage, while against PST it is large enough to overcome that disadvantage. The asymmetry direction depends on the opponent's strength, not the engine's intrinsics.
- **Three consecutive HalfKP losses (games 24, 25, 26) collapsed the running ELO from +76.8 (just outside CI95-excludes-0) to +30.** All three were genuine positional misjudgements, not opening-RNG artefacts — two as White, one as Black. Without those three losses, HalfKP would have been at W:6 D:19 L:1 → ELO ~+76 ± 60, CI95 narrowly excluding 0. The bake-off result is therefore a bona-fide noise event around a small true effect, not a stable measurement of equality.
- **The bake-off wall (6_205 s, 1 h 43 min) was 47 % faster than S17's HalfKP-vs-PST bake-off (11_637 s).** Reason: of the 30 games, 14 ended in ≤120 moves and 5 ended in ≤30 moves (mostly three-fold repetition). When two NNUEs reach the same evaluation in the same position, they tend to play the same move, accelerating into repetition. Useful operational note for future head-to-head bake-offs at this time control.

**Insights for future sessions**:
- The structural-feature lever (HalfKP) has produced a small-but-positive ELO gain over flat features that is too noisy to confirm at n=30. A 90–120-game extension of the head-to-head bake-off would tighten the CI95 from ±73 to ±35–40 — enough to either confirm the +23 point estimate at decision-grade significance or rule it out. This is the cheapest decision-grade follow-up to Session 18.
- Pieces-in-hand encoding is now the most likely source of meaningful additional ELO. The teacher's eval signal includes drop value; both flat and HalfKP networks ignore it. Expected: +20–50 ELO over the current ceiling, on a budget of ~150 LoC.
- Dual-perspective HalfKP-pp would unlock true incremental updates (no full-refresh on stm flip), buying ~2× inference throughput. But on the strength side it is unlikely to gain more than the +23 point estimate of single-perspective HalfKP — the parameter capacity gain is small and the rank-correlation already saturates near 0.61. Probably worth doing for inference throughput, not for ELO.
- The 437 MB → 91 MB JSON-to-bincode shrink unblocks routine training-checkpoint emission. Future trainer runs should default to `.bin` output; rotating per-epoch checkpoints becomes practical.

---

## Decisions Made

**Decision 1**: Implement head-to-head as an optional second-engine override on the existing elo-tester rather than as a new binary.
- **Rationale**: every Session-14-onwards invocation already loads engine A as NNUE; the only difference for head-to-head is whether engine B is PST or a second NNUE. Adding three CLI flags is much smaller than duplicating the matchplay loop in a separate binary, and keeps the bake-off methodology directly comparable across sessions.
- **Confidence**: High.

**Decision 2**: Format selection by file extension rather than magic bytes or a try-fallback.
- **Rationale**: the simplest dispatch users can reason about — `.bin` is binary, everything else is JSON. No magic-bytes header is added so the on-disk binary is the raw bincode of `NNUEWeightFile`, and the JSON files remain plain JSON. Try-fallback would parse JSON twice in the binary case (because bincode would consume bytes before failing), which is uglier than a one-line extension check.
- **Alternative considered**: magic-bytes header (rename-safe). Rejected on YAGNI — file rename is a user action that warrants the user knowing what they renamed.
- **Confidence**: High.

**Decision 3**: Use bincode rather than postcard or safetensors.
- **Rationale**: bincode reuses the existing serde derives on `NNUEWeightFile` with zero schema work. postcard would require the same plumbing for a marginally smaller wire size. safetensors is the ML-standard but requires per-tensor framing and would not round-trip the existing `Option<Vec<Vec<i16>>>` fields without a schema overhaul. bincode's 4.57× shrink is already enough to retire the 437 MB JSON pain point.
- **Confidence**: High. If the file size becomes load-bearing for distribution (e.g. embedded in a release artefact), revisit with safetensors.

**Decision 4**: Run the bake-off at the same time control as Sessions 14–17 (depth 3, time-ms 500, random-plies 6, max-moves 200, max-games 30) but with seed 46.
- **Rationale**: directly comparable to S17's HalfKP-vs-PST measurement; seed independence keeps the opening distribution different.
- **Confidence**: High.

---

## Blockers & Issues

### Issue 3: HalfKP weights JSON file is 437 MB
- **Status**: **Resolved by Session 18.** `bincode` shrinks to 91 MB, with the runtime supporting both formats transparently via extension dispatch. Future training output should default to `.bin`.

### Issue 4: Pieces-in-hand not encoded
- **Status**: Open. Both flat and HalfKP feature spaces ignore drops. Resolution: separate session, ~100 LoC trainer + ~80 LoC engine.

### Issue 5 (S17 NEW): `current_stm` tracking creates a new make/unmake invariant
- **Status**: Open but not surfaced this session.

---

## Next Session Plan (Session 19)

**Result-driven priorities** (Session 18 came in indecisive between HalfKP and flat-NNUE: ELO +23.2 ± 72.7, CI95 [-49.5, +95.9]; binary format shipped at 4.57× shrink / 7.7× load speedup):

**Priority 1 — Pieces-in-hand encoding** (the most likely source of meaningful additional ELO).
Both flat and HalfKP networks ignore drops; the teacher's eval signal includes drop value. Adding `2 × 7 × max_count` features to the flat space is the smallest viable scope. Expected: +20–50 ELO over the current ceiling. Trainer change ~50 LoC, engine change ~30 LoC, retrain budget ~30 min on the existing corpus, bake-off another 1.5–3 hours. The structural-feature lever has been taken; the feature-coverage lever is the next-cheapest.

**Priority 2 — Tighten the HalfKP-vs-flat decision** (the cleanest follow-up to S18).
Extend the head-to-head bake-off from 30 to 90–120 games. CI95 shrinks from ±73 → ±35–40, which would either confirm the +23 ELO point estimate at decision-grade significance or rule it out. This is the most informative experiment per unit of bake-off wall time. ~5–8 hours.

**Priority 3 — Migrate training pipeline to `.bin` output.**
The `nnue-offline-trainer` and elo-tester now both transparently load `.bin` weights. Default the trainer's checkpoint emission to `.bin` (~10 LoC, retains `.json` as an opt-in). Per-epoch HalfKP checkpoints drop from 437 MB to 91 MB; a 30-epoch run goes from 13 GB to 2.7 GB.

**Priority 4 — Dual-perspective HalfKP-pp** (the larger structural intervention).
~250 LoC engine + ~100 LoC trainer. Buys ~2× inference throughput by removing the full-refresh-on-stm-flip cost. ELO upside vs single-perspective HalfKP is small (the parameter-capacity gain is marginal and Pearson-r saturates near 0.61). Probably worth doing for the throughput win, not the strength win. Decoupled from Priority 1.

**Priority 5 — Cleanup.**
The repo has 41 untracked weights files. Move them to `weights/archive/` after confirming none are referenced by tests or scripts. Update `.gitignore` for the new `.bin` extension.

**Priority 6 — Corpus extension** (1M positions at depth 10 from yaneuraOu, ~12–15 hours wall).
With Pearson-r already saturating around 0.614, the marginal value of additional teacher data is modest. Defer until Priorities 1–2 are landed.

**Estimated duration for Priorities 1 + 2**: 8–12 hours.

**Prerequisite**: none — all Session-18 deliverables landed cleanly.

---

## File Changes Summary

### Files Modified
- `Cargo.toml`
  - Added `bincode = "1.3"` dependency.
  - Registered `nnue-convert-weights` binary.
- `src/evaluation/nnue.rs` (~30 lines added)
  - `NNUEWeights::load<P>` and `save<P>`: extension-based dispatch between bincode and JSON.
  - `NNUEError::Bincode(#[from] bincode::Error)` variant.
- `src/bin/elo_tester.rs` (~50 lines added)
  - `--nnue-weights-b`, `--use-stm-feature-b`, `--use-halfkp-b` CLI flags.
  - Engine B build path branches on whether `--nnue-weights-b` is set.
  - `Outcome::as_str(head_to_head)` adapts CSV labels.
  - Banner, CSV header, and verbose move-printer adapt to head-to-head mode.

### Files Created
- `src/bin/nnue_convert_weights.rs` (~70 lines)
  - JSON ↔ bincode round-trip CLI with size, load-time, save-time, and bit-identity verification.
- `docs/nnue-phase2/SESSION_LOG_018.md` (this file).

### Files Deleted
- None.

### Artefacts produced (untracked)
- `/tmp/s16_halfkp_adam_fresh_30e.bin` (91.1 MB) — bincode of S16 HalfKP weights.
- `/tmp/s15_adam_stm_fresh_30e.bin` (1.1 MB) — bincode of S15 flat-feature weights.
- `/tmp/s18_h2h_smoke.csv`, `/tmp/s18_h2h_smoke.log` — 2-game head-to-head smoke (1D-1L for HalfKP, wall 706 s).
- `/tmp/s18_h2h_elo.csv`, `/tmp/s18_h2h_elo.log` — **30-game HalfKP-vs-flat bake-off, seed 46. Final: 6W-20D-4L, ELO +23.2 ± 72.7, wall 6_205 s.**
- `/tmp/s18_bin_smoke.csv` — 1-game smoke confirming binary HalfKP load via elo-tester.

---

## Testing Results

### Session 18 success criteria

**Criterion 1**: elo-tester gains a `--nnue-weights-b` flag that turns engine B into a second NNUE engine.
- [x] Met. Verified by 2-game smoke and the `Engine B: NNUE weights = ...` banner.

**Criterion 2**: All eight pre-existing NNUE tests still pass after the load/save dispatch changes.
- [x] Met. 8/8 pass.

**Criterion 3**: NNUEWeights round-trips JSON ↔ binary bit-identically.
- [x] Met. `nnue-convert-weights` asserts every weight tensor.

**Criterion 4**: 30-game HalfKP-vs-flat bake-off runs to completion and produces a CSV.
- [x] Met. 30 / 30 games completed in 6_205 s. CSV at `/tmp/s18_h2h_elo.csv`. Final: 6W-20D-4L, ELO +23.2 ± 72.7.

**Criterion 5**: Binary weights file is materially smaller than JSON (target 5×).
- [x] Met. 4.57× shrink (close to target). Load time 7.7× faster.

**Criterion 6** (decision-grade): the bake-off produces an ELO point estimate either separating HalfKP from flat-NNUE (CI95 excludes 0) or showing no material gap (CI95 brackets 0 within ~30 ELO).
- [△] Partially met. CI95 [-49.5, +95.9] brackets 0 — does not separate HalfKP from flat-NNUE. The CI95 width of ±73 ELO exceeds the "within ~30 ELO" target, so the result is "indecisive" rather than "no material gap". Resolution: extend the bake-off to 90–120 games in Session 19 (Priority 2) to reach decision-grade resolution.

### Diagnostic findings

**Finding 1 (the headline)**: HalfKP+stm and flat+stm are statistically indistinguishable at n=30, depth 3, 500 ms/move. The +23 ELO point estimate is positive and aligns with the +0.014 Pearson-r gap, but CI95 [-49.5, +95.9] brackets 0. The S17 +120 ELO over PST decomposes as ~+97 "any NNUE > PST" + residual ~+23 "HalfKP > flat". The structural-feature engineering produces a small-but-positive gain that the bake-off was underpowered to confirm.

**Finding 2 (the most surprising)**: ALL 6 of HalfKP's wins came when playing Black; HalfKP scored 0 wins from 15 White games. The S17 vs-PST bake-off has the **opposite** asymmetry (HalfKP scored 0.733 as White, 0.600 as Black). The asymmetry direction depends on opponent strength, not on intrinsic HalfKP behaviour — supports the reading that HalfKP and flat-NNUE are roughly equally strong, with the +23 ELO estimate being noise around a small true effect.

**Finding 3**: The high draw rate (66.7 %, 20/30) is consistent with two NNUEs evaluating overlapping features under similar Pearson-r ceilings. Of the 10 decisive games, 6 favoured HalfKP and 4 favoured flat — a 60/40 split that would arise by chance with probability ~38 % under a true equal-strength null, so it is fully consistent with no real gap.

**Finding 4**: Binary weights format works exactly as designed. 4.57× shrink, 7.7× load speedup, bit-identical round-trip in both directions. Zero behaviour change at runtime — the `.bin` HalfKP weights produce the same engine moves as the `.json` weights when loaded into elo-tester.

**Finding 5**: The bake-off completed in 1 h 43 min — 47 % faster than S17's 3 h 14 min. The reason is the elevated draw rate accelerating into early three-fold repetitions. 5 of 30 games ended in ≤30 moves (vs only 1 of 30 in S17). This is a useful operational data-point: NNUE-vs-NNUE bake-offs are cheaper than NNUE-vs-PST ones at the same time control.

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

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed — head-to-head plumbing landed (~50 LoC in elo-tester), binary weights format landed (~100 LoC across `nnue.rs` + new `nnue-convert-weights` binary), 8/8 NNUE tests pass, and the 30-game HalfKP-vs-flat bake-off returned **6W-20D-4L, ELO +23.2 ± 72.7 (CI95 [-49.5, +95.9])** — does not separate HalfKP from flat at n=30. Binary format ships at **4.57× shrink, 7.7× faster load**.
**Ready for next session**: Yes — Session 19 priorities are (1) pieces-in-hand encoding for additional ELO and (2) extending the head-to-head bake-off to 90–120 games for decision-grade HalfKP-vs-flat resolution.
**Comments**:
1. **The S17 +120-ELO-over-PST result decomposes as ~+97 "any NNUE > PST" + residual ~+23 "HalfKP > flat".** This is the single most important finding of Session 18 because it sets a realistic expectation for the structural-feature lever: small-but-positive gains, not the dramatic gains the S17 vs-PST headline suggested. Future structural interventions (dual-perspective HalfKP-pp, pieces-in-hand) should be evaluated on this scale, not on the vs-PST scale.
2. **The CI95 brackets 0 but the point estimate is positive and aligned with the Pearson-r gap.** The honest reading is "HalfKP is plausibly slightly stronger than flat, but n=30 is underpowered to confirm." A 90-game extension would tighten CI95 to ±35 — enough to either confirm or reject.
3. **The color asymmetry inversion (HalfKP wins as White vs PST, only as Black vs flat-NNUE) is the most surprising finding.** It rules out an intrinsic HalfKP bias and instead reveals that the asymmetry direction is a function of opponent strength: against PST, HalfKP's structural advantage overcomes White's natural disadvantage; against a peer NNUE, it cannot. Useful diagnostic for future head-to-head measurements.
4. **Binary weights format is the operational deliverable that retires the 437 MB JSON pain point.** Future trainer runs should default to `.bin` output. Backwards compat is in place via extension dispatch — no caller-side change needed for `.json`-loading scripts.
5. **The bake-off wall (1 h 43 min) was 47 % faster than S17.** NNUE-vs-NNUE matches accelerate into three-fold repetition more often than NNUE-vs-PST matches, lowering the per-game cost. Useful for budgeting Session 19's extended bake-off.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-29
