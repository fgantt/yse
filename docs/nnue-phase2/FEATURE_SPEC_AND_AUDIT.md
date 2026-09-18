# NNUE Feature Specification and Implementation Audit

**Date**: 2026-09-17
**Scope**: All NNUE feature spaces, the trainer/engine arithmetic paths, the corpus, and the
`validation_pearson` diagnostic.
**Motivation**: Training had plateaued at Pearson r ≈ 0.58–0.61 since Session 15 and head-to-head
bake-offs kept returning non-results. Sessions 18–20 attributed this to architectural saturation and
corpus size. This audit tests that explanation against the code and the data.

**Headline**: the plateau is not architectural. The diagnostic that Sessions 15–20 were steered by
has a mathematical ceiling of **r = 0.608**, and the network has been sitting on it since Session 15.
Separately, the feature encoding has no perspective normalisation, which forces the network to learn
each colour as an independent problem.

---

## Part 1 — Feature Specification

Three composable feature spaces, all defined in `src/evaluation/nnue.rs`. Extraction is implemented
**twice**: `nnue.rs` (engine, accumulator-based) and `nnue_training.rs` (trainer, index-list based).
The two agree — verified by `tests/nnue_semantics.rs::trainer_feature_extraction_matches_engine_refresh`.

### 1.1 Flat piece-square space

```
feature_index(player, piece_type, square)
    = player_idx * 14 * 81  +  piece_type.as_index() * 81  +  square
```

| Component | Range | Encoding |
|---|---|---|
| `player_idx` | 0–1 | Black = 0, White = 1 — **absolute**, not relative to side to move |
| `piece_type.as_index()` | 0–13 | 8 base (Pawn…King) + 6 promoted; bijective |
| `square` | 0–80 | `Position::to_u8() = row * 9 + col` — **absolute**, never mirrored |

Total `NUM_NNUE_FEATURES = 2 × 14 × 81 = 2268`. Injective and fully covering — verified.

### 1.2 Side-to-move bit

```
STM_FEATURE_INDEX = NUM_NNUE_FEATURES = 2268
```

Active **iff** the side to move is Black. `NUM_NNUE_FEATURES_TOTAL = 2269`.

Applied in `refresh_with_stm` as a pure **additive** row into `hidden_1`. It shifts the
representation; it cannot rescale or sign-flip it.

### 1.3 HalfKP space

```
feature_index_halfkp(own_king_sq, player, piece_type, square)
    = own_king_sq * 2268  +  feature_index(player, piece_type, square)
```

`own_king_sq` is the **side-to-move's** king. Single-perspective (own-king only), so every
side-to-move flip re-indexes the entire feature set and forces a full accumulator refresh
(`needs_refresh = true`). `NUM_NNUE_FEATURES_HALFKP = 81 × 2268 = 183 708`;
`STM_FEATURE_INDEX_HALFKP = 183 708`; total 183 709.

King-less positions return an empty feature vector and the caller skips the record.

### 1.4 Pieces-in-hand thermometer

```
hand_feature_index(base, player, piece_type, k)
    = base  +  player_idx * 38  +  HAND_TYPE_OFFSET[type]  +  (k - 1)
```

Feature `(player, type, k)` is active when that player holds **≥ k** of that type.

| Type | Pawn | Lance | Knight | Silver | Gold | Bishop | Rook |
|---|---|---|---|---|---|---|---|
| Max count | 18 | 4 | 4 | 4 | 4 | 2 | 2 |
| Offset | 0 | 18 | 22 | 26 | 30 | 34 | 36 |

38 per side, 76 total. King and promoted types are not hand-eligible (promoted pieces revert on
capture) and correctly return `None`.

Bases: `HAND_FEATURE_BASE_FLAT = 2269`, `HAND_FEATURE_BASE_HALFKP = 183 709`. Hand features are
**not** king-conditioned even under HalfKP. `player` is again **absolute**.

### 1.5 Network and quantisation

```
hidden_1 = ReLU(accumulator + b1)                      i32, NO upper clamp
hidden_2[j] = ReLU( sum_i (h1[i] * W2[i][j]) >> 6  + b2[j] )
output      = b_out + sum_j (h2[j] * Wout[j]) >> 6
cp          = output * 400 / 4080
```

| Tensor | Quantised range | Set in |
|---|---|---|
| `input_weights_1` | **±127** | `nnue_training.rs:279` |
| `input_weights_2` | ±32767 | `nnue_training.rs:310` |
| `output_weights` | ±32767 | `nnue_training.rs:342` |

Training keeps f32 shadow weights and re-quantises with clamp + round-to-nearest.

### 1.6 Target convention

`eval_cp` in the corpus is the teacher's score **from the side-to-move's perspective** (USI `score
cp` convention; `corpus_gen.rs` does not negate it). `target_for` keeps that convention for the
teacher term and converts the board-absolute `outcome` to match via `player_mult`. The convention is
consistent end to end — confirmed empirically: mean `eval_cp` is +727 on Black-to-move plies and
−457 on White-to-move plies, i.e. both say "Black is better on average", matching the +0.14 mean
outcome.

---

## Part 2 — Audit Findings

### Finding 1 — The Pearson diagnostic has a ceiling of 0.608 (critical)

`validation_pearson` (`nnue_offline_trainer.rs:285`) correlates the network's output against **raw,
unclamped** `eval_cp`. But `target_for` (`:478`) **clamps the training target to ±2000** before
applying the sigmoid. The network is therefore trained to reproduce `clamp(teacher, ±2000)` and then
scored against something else.

The corpus makes this fatal:

```
|eval_cp| percentiles:  p50=592   p75=2036   p90=3624   p95=29994   p99=35281
|cp| > 2000 : 25.4% of corpus  (fully clamped — all rank information destroyed)
|cp| > 20000:  8.1% of corpus  — and this 8.1% carries 97.2% of the total variance
```

Pearson is variance-weighted, so the metric is overwhelmingly a test of "can you predict mate
scores" — precisely what the clamp removes from training.

**The ceiling for a perfect network:**

```
corr(clamp(teacher, ±2000), raw teacher) = 0.608
  per-stratum:  black 0.606    white 0.607

  clamp   600 -> 0.482      clamp  4000 -> 0.737
  clamp  1000 -> 0.528      clamp  8000 -> 0.918
  clamp  2000 -> 0.608      clamp 20000 -> 0.994
```

Compare against the reported history:

| Session | Reported r | Ceiling | Interpretation at the time |
|---|---|---|---|
| 15 | +0.607 | 0.608 | "Adam fresh-init hits +0.61" |
| 16 | +0.614 | 0.608 | "HalfKP best yet" |
| 19 | +0.610 (blk) | 0.606 | "parity with baseline" |
| 20 | +0.595 (blk), peak +0.633 | 0.606 | "architecture saturated" |

**Sessions 15–20 were at the ceiling of the metric.** The "plateau" is the metric, not the model.
S20's conclusion that "the 256→32→1 architecture has saturated for this corpus" is not supported;
neither is the S18-Eval inference that the network has memorised the corpus. Both were read off a
number that could not move.

Note S14's jump from r≈0.10 to +0.59 was genuine — 0.10 is far below the ceiling, so the
side-to-move feature really did fix something. Everything after S15 was noise in the third decimal.

**Fix**: score against the same target the network is trained on (clamped), and/or exclude mate
scores from the diagnostic sample, and/or report Spearman rank correlation, which is not
outlier-dominated. Also reconsider training on mate positions at all — standard practice is to drop
them or give them a separate bounded target rather than collapsing them into ±2000.

### Finding 1b — There was no held-out validation set (critical)

`validation_pearson` was called with the **training** corpus, and `records.shuffle(&mut rng)` runs at
the top of every epoch (`:835`), so the "validation" sample was a rotating slice of data the network
had just trained on. Every r reported in Sessions 11–20 is a training-fit number.

Fixed by holding out every 20th game (`VALIDATION_GAME_STRIDE`). The split is by **game**, not by
record: adjacent plies of one game are near-identical positions, so a record-level split leaks
training positions into validation. 64 230 train / 3 173 held-out.

The gap this exposed is large — see Part 4.

### Finding 2 — No perspective normalisation in any feature space (critical)

Every space keys on **absolute** player and **absolute** square (§1.1, §1.3, §1.4). There is no
board mirroring and no colour swapping anywhere in the feature path.

But the target is to-move-POV (§1.6). So a position `P` and its 180° colour-mirror `P′` — the same
position seen from the other side, with identical to-move-POV value — produce feature vectors that
share **zero** active indices. The network must learn identical shogi knowledge twice, in two
disjoint parameter regions, from half the corpus each.

The single stm bit cannot bridge this: it is an additive offset into `hidden_1`, not a
transformation of the representation.

Demonstrated by `tests/nnue_semantics.rs::mirrored_position_evaluates_identically`. On the **starting
position** — the most symmetric position in shogi — the two orientations evaluate to **+11447 and
−8121**.

This explains the Session 20 Issue 7 colour asymmetry (network 5W-2L as Black, 3W-6L as White).
S20 hypothesised a per-side corpus imbalance. **The data refutes that**: the corpus is balanced to
within 1% on every axis measured.

```
side      n      mean_cp   mean|cp|   median|cp|   >500cp
black   33840     +726.6     3448.6        595.0    53.8%
white   33563     -456.5     3419.2        588.0    53.6%
                                    Black share of corpus: 50.2%
```

The asymmetry is architectural, not distributional.

**Fix**: normalise features to the side-to-move's perspective — when White is to move, mirror the
square (`square → 80 - square`) and swap the piece's colour. Then `x(P) == x(P′)` exactly, every
training example teaches both colours, and colour symmetry holds by construction rather than being
something the network must spend capacity learning. This also makes the stm bit redundant for its
original purpose.

### Finding 3 — The trainer differentiates a function the engine does not compute (moderate)

| | Hidden-2 accumulation | Output accumulation |
|---|---|---|
| Trainer (`nnue_training.rs:614`) | `h2[j] += act * w as f32 / 64.0` | `raw += v * w as f32 / 64.0` |
| Engine (`nnue.rs:705`) | `h2[j] += (act * w) >> 6` | `out += (a * w) >> 6` |

The engine floors **every individual term** before accumulating; the trainer divides exactly. Over
256 hidden-1 terms and 32 output terms this is a systematic distortion, not rounding noise. The
optimiser computes gradients for the f32 function; the engine — and actual play — uses the integer
one.

Measured by `tests/nnue_semantics.rs::trainer_forward_matches_engine_evaluate`: drift of **148 to
673 cp** on identical weights and inputs.

Note `validation_pearson` calls the engine path, so the reported r already includes this skew; it is
the *gradient* that is computed on the wrong function.

**Fix**: accumulate full precision and shift once at the end, in both paths; or have the trainer
replicate the engine's integer arithmetic exactly in its forward pass.

### Finding 4 — Quantisation scheme is internally inconsistent (moderate)

`input_weights_1` is clamped to **±127** — the Stockfish i8 convention, which is sound *because*
Stockfish pairs it with a **clipped** ReLU that bounds activations to [0, 127]. This implementation
uses a plain unclipped ReLU (`nnue.rs:687`, `:712` — `.max(0)` with no `.min()`), and then uses
±32767 for the later layers, compensating with the per-term `>> 6` shifts of Finding 3.

The i8 input range was adopted without the activation clamp that justifies it. Activations reach
~5000 (40 active features × 127), so the `>> 6` shifts are load-bearing for overflow safety — which
is why they are applied per-term rather than once, which is what causes Finding 3.

**Fix**: adopt clipped ReLU with a documented activation scale, then derive the shift amounts from
it rather than choosing them to avoid overflow.

### Finding 5 — The test suite cannot detect any of the above (process)

Of the 11 pre-existing NNUE tests, 7 are `*_matches_full_refresh` or `*_paths_agree_*`. They compare
the implementation **against itself**. A uniformly wrong encoding passes all of them — which is
exactly what happened for 20 sessions.

Two further weaknesses in `test_eval_paths_agree_on_trained_weights`:

- It **silently returns** if `nnue_weights_trained.json` is absent (`nnue.rs:1674`). That file is
  untracked in git, so on a fresh checkout the test passes vacuously.
- Despite the name, it compares two **engine** paths. It does not test trainer-vs-engine agreement,
  and it exercises only `acc.refresh` — the flat, no-stm, no-hand path.

**Fix**: `tests/nnue_semantics.rs` (added by this audit) covers index injectivity and range for all
three spaces, hand-eligibility rules, trainer/engine extraction parity, mirror invariance, and
trainer/engine arithmetic agreement. Two of its eight tests fail today — deliberately; they are
Findings 2 and 3.

---

## Part 3 — Re-baseline Results (2026-09-17)

Recipe held identical to Session 20 (`--epochs 30 --batch-size 256 --learning-rate 0.05
--output-grad-scale 1.0 --input-grad-scale 1.0 --use-stm-feature --use-sigmoid-loss --use-adam
--use-hand-features --seed 47`). Only the diagnostic changed.

| Metric | Training set | Held-out games |
|---|---|---|
| `pearson[clamp]` (the training objective) | **+0.928** | **+0.449** |
| `spearman[raw]` (ranking quality) | +0.939 | +0.383 |
| `pearson[raw]` (legacy S14–20 metric) | +0.582 | +0.351 |
| ceiling on the legacy metric | 0.597 | 0.625 |
| per-stratum `pearson[clamp]`, Black / White | 0.922 / 0.940 | **0.525 / 0.378** |

Three conclusions:

**1. The plateau was the metric.** On the objective it actually optimises, the network reaches 0.928
on training data — not the 0.58 that six sessions read as saturation. The legacy number sat at
0.582 against a ceiling of 0.597. It was pinned, and no training change could have moved it.

**2. But the network overfits badly.** 0.928 training vs 0.449 held-out. The S18-Eval critique #5
("the corpus is small and the network has likely memorised it") was right — it just could not be
demonstrated while the metric was measuring training fit against a ceiling. Held-out r plateaus by
epoch 10 (spearman +0.378 at epoch 10, +0.383 at epoch 30) while training fit keeps climbing: the
textbook overfitting signature. **Generalisation, not capacity, is the binding constraint.**

**3. The colour asymmetry is a generalisation asymmetry.** On training data Black and White are
learned equally well (0.922 / 0.940). On held-out games they diverge sharply: **0.525 vs 0.378**.
This is much stronger evidence for Finding 2 than the Session 20 bake-off colour split was. Two
independently-learned half-problems memorise equally but generalise differently — exactly what an
un-normalised perspective predicts.

---

## Part 3b — Perspective Normalisation Result (2026-09-17)

`--use-pov-features` added to the trainer: when White is to move the board is rotated 180° and piece
colours swapped, so a position and its mirror produce identical features. It supersedes
`--use-stm-feature` (the bit is constant once normalised, and a colour-conditional bit would undo the
symmetry). Flat space only; rejected together with `--use-halfkp`, whose `own_king_sq` would also need
mirroring.

Same recipe and seed as the Part 3 baseline, substituting `--use-pov-features` for
`--use-stm-feature`. Held-out games:

| Held-out metric | Baseline (colour-absolute + stm) | POV normalised | |
|---|---|---|---|
| `pearson[clamp]` best | +0.449 (epoch 30) | **+0.618 (epoch 10)** | **+38%** |
| `pearson[clamp]` at epoch 30 | +0.449 | +0.603 | +34% |
| `spearman[raw]` at epoch 30 | +0.383 | +0.580 | +51% |
| legacy `pearson[raw]` | +0.351 | +0.490 | |

Per-epoch, the gap is stark:

```
epoch      1      2      3      5     10     15     20     25     30
baseline  0.102  0.238  0.331  0.366  0.436  0.438  0.443  0.446  0.449
POV       0.437  0.532  0.577  0.608  0.618  0.614  0.608  0.609  0.603
```

**POV reaches at epoch 1 what the baseline needs 30 epochs to reach.** It peaks at epoch 10 and then
declines slightly — so 30 epochs is not merely wasteful here, it is mildly harmful. Early stopping at
~10 epochs is the right default, which also cuts training wall to ~27 s.

Mirror invariance is now exact:
`tests/nnue_semantics.rs::mirrored_position_evaluates_identically_under_pov` passes, and
`colour_absolute_encoding_is_not_mirror_invariant` pins the legacy behaviour so the cost stays visible.

One caution on reading the per-stratum split. It moved from 0.525/0.378 (Black/White) to
0.540/0.664, i.e. it flipped sign rather than collapsing to zero. This is **not** residual network
asymmetry — under POV the network applies one identical function to both, which the mirror test
proves exactly. The two strata are different *sets of positions*, and the corpus's own to-move-POV
eval distribution is asymmetric (mean +727 on Black-to-move plies vs −457 on White-to-move). The
per-stratum numbers therefore reflect distributional difficulty, not encoding asymmetry. The mirror
test, not the stratum split, is the symmetry check from here on.

Best weights: `weights/session21/nnue_weights_s21_pov_epoch_10.bin`.

---

## Part 4 — Recommended Sequence

**Done** — Findings 1 and 1b are fixed and the re-baseline in Part 3 is the new reference point.
`pearson[clamp]` on held-out games is now the metric of record: it moves, it has a ceiling of 1.0,
and it measures generalisation.

Remaining, in priority order:

1. ~~**Add perspective normalisation**~~ — **done**, see Part 3b. Held-out `pearson[clamp]` 0.449 →
   0.618. The engine-side path is done too: **dual accumulators** (one per perspective, each with fixed
   indexing so both stay incrementally updatable; a stm flip only changes which is read). This is the
   "HalfKP-pp" shape the earlier docs kept deferring on ROI grounds, and it avoids the per-move full
   refresh HalfKP accepts — so a depth-sensitive bake-off will not be biased against NNUE by refresh
   cost. `--use-pov-features[-b]` is wired through `elo-tester`. Verified by four tests including
   per-ply incremental-vs-full-refresh agreement and trainer-vs-engine POV extraction parity.

   Not yet done on this axis: **HalfKP under POV** (its `own_king_sq` also needs mirroring), and
   `usi-engine` exposure for shipped builds.
2. **Attack overfitting** (Part 3, conclusion 2). 0.928 training vs 0.449 held-out is the binding
   constraint, and it is now measurable. Cheap levers first, since training is ~80 s: L2 /
   weight decay, early stopping on held-out r (it plateaus at epoch 10, so 30 epochs is 3× wasted
   wall), and dropout. Corpus expansion is the expensive lever — but unlike before, its payoff can now
   be verified, and note that perspective normalisation is itself a 2× effective-data increase, so do
   it first and re-measure.
3. **Align the arithmetic** (Findings 3 and 4) — clipped ReLU, single end-of-accumulation shift,
   trainer forward matching engine forward exactly.
4. **Retrain and bake off.** Only after 1–3, and only with ≥90 games per the Session 20 power
   analysis. Do not spend bake-off wall time on a config whose held-out r has not improved.

Note the capacity-related S18-Eval recommendations (HalfKP-pp, larger hidden layers) remain
correctly deprioritised, but for a different reason than that document gave: not because capacity is
already saturated, but because the network is over-fitting the data it has. Adding capacity would
make that worse.
