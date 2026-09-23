# NNUE Implementation Session Log

## Session 28: 100K Games, and Finding Out What We Were Measuring

**Date**: 2026-09-21 – 2026-09-22
**Phase**: Phase 2v
**Objective**: Test whether the +65 Elo ceiling from Session 26 was a data-volume limit, by
generating 100,000 self-play games instead of 12,000.

> **READ THIS FIRST — three corrections to standing practice.**
>
> 1. **`elo-tester` at fixed depth 2 does not predict play.** It scored this session's best
>    candidate at **+72.2 ± 49.4**; a 1 s/move match on the same pair returned **−58.5 ± 62.3**.
>    Opposite signs, non-overlapping intervals. Every network shipped in Sessions 23–26 was chosen
>    this way, including the one currently deployed.
> 2. **Held-out correlation is anti-correlated with strength** in the region that matters. Across
>    six checkpoints it rose monotonically 0.785 → 0.840 while Elo went −96, +72, +60, −2, −31, −17.
>    It would have selected the checkpoint that plays worse than what we already ship.
> 3. **We are roughly twice as inaccurate per move as YaneuraOu at its match setting**, on neutral
>    ground. Earlier tables suggesting parity were measured on positions from our own games.

---

## The corpus

`corpus_s28/run.sh`: 8 × 12,500 games, played by `engines/usi-engine-s27` (the Session 27 build,
+145 Elo and ~30% faster than the engine that made `corpus_self`), labelled by YaneuraOu at depth 10.

**100,000 games, 12,475,678 positions.** Audit clean: 0 chain breaks in 59,537 consecutive pairs,
and 0 move-list differences from the teacher on 6,000 sampled positions. Combined with everything
prior: **23,742,790 positions**, 58% from our own games (was 12%).

A stronger engine did not produce better-balanced games:

| corpus | ≤500 cp | >2000 cp | mean ply |
|---|---|---|---|
| new (100K games, s27 engine) | 21.9% | 56.9% | 83 |
| old (12K games, weaker engine) | 20.6% | 56.9% | 77 |
| teacher self-play | 37.4% | 38.3% | 70 |

Games last longer; they are no less decided. We multiplied examples of the same skew.

## The dose curve

k9, warm-started from the deployed network, 6 epochs, checkpoint every epoch. 200 paired games per
point against `nnue/default.bin` (`elo-tester`, fixed depth 2):

| epoch | 1 | **2** | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| Elo | −96.2 | **+72.2** | +59.6 | −1.7 | −31.4 | −17.4 |
| held-out r | 0.785 | 0.797 | 0.809 | 0.820 | 0.832 | **0.840** |

The usable dose *did* extend: the peak moved from ~12.7M positions seen (Session 26) to ~45M. But the
**magnitude did not**: +72.2 ± 49.4 against Session 26's +65.0 ± 47.0. Eight times the data, same
answer.

And then the confirmation match killed it. Epoch 2 vs the deployed network, 1 s/move, 120 games:
**48-4-68, −58.5 ± 62.3.** Nothing was deployed; `nnue/default.bin` is unchanged.

## What we had been measuring

Move loss over 150 positions, judged by YaneuraOu at depth 10. The standing set is drawn from *our
own games*. A second set was built from held-out teacher self-play (`game_id % 20 == 0`, never
trained on, neither engine steering):

| test set | ours (depth 4) | YaneuraOu (depth 1) |
|---|---|---|
| our own games | 172 / 82 | 192 / 38 |
| neutral (teacher games) | 272–306 / 108–121 | 257 / 62 |

On our own positions we appear ~20 cp better than the reference; on neutral ground we are worse. The
bias is real but modest — it does not by itself explain losing 21 of 24.

What does explain it is the opponent we actually face. Measured on the neutral set:

| engine | mean | median | >200 cp |
|---|---|---|---|
| **ours, depth 4** | **306** | **121** | 39% |
| YaneuraOu, 50 nodes | 248 | **64** | 32% |
| YaneuraOu, 200 nodes | 245 | 53 | 26% |
| YaneuraOu, depth 1 | 257 | 62 | 31% |

Two things fall out. **Their search budget barely matters** — 50 nodes, 200 nodes and depth 1 all
land within 12 cp of each other, so their *evaluation alone* is picking the moves. And **we make
about twice the error per move** (median 121 vs 64), which compounds exactly the way the game logs
show: below −1000 by ply 19–41.

Note the awkward corollary: the network scores r = 0.810 against the teacher on these very positions
while choosing moves twice as badly. Correlation on position *values* does not deliver correct
*ranking between siblings*, and ranking is what a search consumes.

Metric noise: our depth-4 mean read 272 and 306 on two runs of the same set. Treat ±30 cp mean as the
floor; the medians are stabler.

## Next

1. **Fix the measurement chain before training anything else.** Only 1 s/move matches have survived
   scrutiny this session, at ~2 h per 120 games. Node-limited matches at 20k understate by about half
   (+73 vs a known +145) and at 5k invert (+11.6); fixed-depth-2 `elo-tester` can invert the sign.
   Use fixed-depth only as a cheap screen, and never to ship.
2. **The gap is evaluation quality on positions from strong play.** Not search (the machinery now
   beats plain alpha-beta), not speed (+30% this session), and not volume of our own games (this
   session). Training that targets *move ranking* rather than position-value regression is the
   untested lever.
3. Do not retry 81 king buckets on this premise. Its rationale was that data volume limited the
   Session 24 result; that is now at best half true.
