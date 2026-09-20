# NNUE Implementation Session Log

## Session 26: Training on Our Own Games — +65 Elo Against Ourselves, Nothing Against the Teacher

**Date**: 2026-09-19 – 2026-09-20
**Phase**: Phase 2t
**Objective**: Session 25 found the evaluation, not the search, to be the binding constraint, and that
the network is weaker on positions from our own games (r 0.698) than on its training distribution
(0.80). Generate a corpus from our engine's own games, retrain on it, and measure.

> **READ THIS FIRST.** Two standing rules come out of this session.
> 1. **Held-out correlation is not a selection metric.** It ranked four checkpoints in exactly the
>    reverse of their match results, twice. Screen with it if you like; decide with a bake-off.
> 2. **Self-play data has a dose.** 12,000 games support 3–6 passes. Past that, memorisation reverses
>    the gain: +65 Elo at 3 passes, −7 at 15, −125 at 60.

---

## The corpus (`corpus_self/`)

`corpus-gen --player-engine` (Session 25) has our engine play both sides at 3,000 nodes/move while
YaneuraOu labels every position at depth 10. 8 processes × 1,500 games, half with
`--random-move-prob 0.05`, game ids from 500,000.

**12,000 games, 1,372,266 positions.** `corpus_audit`: 0 chain breaks in 1,360,266 consecutive pairs,
and **0 move-list differences from the teacher on 137,227 sampled positions** — our move generator and
YaneuraOu agree exactly.

Our games are far more lopsided than the teacher's self-play:

| \|eval\| | our games | teacher corpus |
|---|---|---|
| ≤ 500 | 20.6% | 37.4% |
| 501–2000 | 22.5% | 24.2% |
| 2001–10000 | 42.7% | 29.5% |
| > 10000 (mate) | 14.2% | 8.8% |

By ply 30–39 only 30% of our positions are still inside ±500. Games decide themselves early, which is
the same story Session 25's move-loss analysis told.

## Attempt 1: filter to the positions that carry gradient — **wrong**

57% of our positions sit past the trainer's ±2000 clamp, where the target saturates. Filtering to the
576,777 within it and oversampling 6× gave a 13.36M mix (26% self-play), 12 epochs.

| checkpoint | r on real-game positions | paired Elo vs `default.bin` |
|---|---|---|
| epoch 1 | 0.767 | **+50.7 ± 47.5** |
| epoch 10 | **0.804** | **−125.0 ± 51.4** |

The best-correlating network lost by 125 Elo. Implied material fell (Rook 447→429, Gold 361→321,
Knight 199→169), which looked like the cause until epoch 1 — whose material is *lower still* — won by
50. **Material erosion was a symptom; memorisation of 12,000 games was the cause.**

## A trainer bug this exposed (commit `f0bc25e`)

`validation_pearson` scored the **first** `--validate-sample` records of the validation split, in corpus
order. Combined corpora are concatenated in blocks, one per generation run, so the sample was always one
block. This session put self-play first, so every validation number came from self-play alone — all
inside the clamp by construction, reported as `clamped=0.0%` against a corpus that is 45% clamped. Early
stopping therefore chose epoch 1 and saved it as the final weights; only the `--checkpoint-every 10`
default preserved the epoch-10 network at all. Earlier sessions' held-out numbers were computed on the
oldest block of each corpus for the same reason. Now strided across the whole validation set.

## Attempt 2: keep everything, 3× — **right**

Unfiltered self-play ×3 + the 9.89M teacher corpus = **14.0M positions**, 5 epochs, checkpoint every
epoch. Held-out r 0.806 → 0.812, stable (and meaningful for the first time: 45.3% clamped).

| epoch | passes over self-play | r on real-game positions | **paired Elo vs `default.bin`** |
|---|---|---|---|
| **1** | 3 | 0.768 | **+65.0 ± 47.0** (39 pairs won, 16 lost) |
| 2 | 6 | 0.783 | +61.4 ± 45.6 |
| 3 | 9 | 0.787 | +45.4 ± 48.2 |
| 5 | 15 | 0.799 | −6.9 ± 47.7 |

Correlation rises monotonically; strength falls monotonically. Dropping the filter was worth ~+14 Elo
over attempt 1. **`k9_full3x_epoch_1.bin` is deployed as `nnue/default.bin`**; the previous network is
`weights/session26/k9.bin` (md5-verified before the overwrite).

## Against the actual opponent: no change

24 games, our engine 1 s/move, YaneuraOu 50 nodes/move, paired openings: **1W–1D–22L**. The previous
network scored 1–3 in 24. Longest think 892 ms on a 1 s budget — nothing lost to the clock.

Move loss (150 positions from our games, judged by YaneuraOu depth 10; Session 25 figures in brackets):

| | mean | median | >200 cp |
|---|---|---|---|
| ours depth 1 | 249 (258) | 79 (104) | 33% |
| ours depth 2 | 265 (280) | 88 (120) | 35% |
| ours depth 3 | 263 (267) | 106 (104) | 34% |
| ours depth 4 | 332 (326) | 170 (150) | 47% |
| YaneuraOu depth 1 | 192 (211) | 38 (52) | 23% |

The YaneuraOu depth-1 row moved by 19 cp mean / 14 median between two runs of the same deterministic
measurement, so treat ±20 mean as this metric's noise floor; our small gains are inside it.

**What is outside the noise: move loss grows with depth, and depth 4 is much worse than depth 1** (332
vs 249 mean, 170 vs 79 median), reproduced in both sessions' runs. Searching deeper picks worse moves.
That is a correctness signature, not a training-scale problem, and it makes every evaluation improvement
worth less than it should be.

## Next

1. **Why is depth 4 worse than depth 1?** The cheapest high-value question open. Compare against
   `search_oracle` at the same depths, check odd/even parity of the returned scores, and check what
   quiescence returns at the leaves. Fix this before spending more on evaluation.
2. **More self-play games.** The dose curve above is limited by game count, not epochs. 100K games
   (days of generation) would widen the usable window; 12K supports only 3–6 passes.
3. Perspective on scale: at 1 s/move we search ~20,000 nodes and lose 22 of 24 to an opponent using
   **50**. The remaining gap is not tuning-sized, and incremental data work is returning +50–75 Elo per
   multi-hour cycle against ourselves.
