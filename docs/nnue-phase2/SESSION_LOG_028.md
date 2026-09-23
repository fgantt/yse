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

---

## Addendum: the handicap ladder, and a failed hypothesis

### Even-material matches against YaneuraOu carry no information

Ours at 20,000 nodes against YaneuraOu at 1, 5, 20 and 50 nodes: 3, 5, 2 and 1 wins of 24. The win
rate is **flat across a 50× change in its search budget** — its evaluation alone decides the games,
and our own improvements cannot register. Every "1 in 24" result in these logs says the same thing
regardless of how much we improved.

### The ladder (commit `8e816bb`)

`usi_match --start-sfen` plays from a handicap position. Counting only our games on the side holding
the odds, against YaneuraOu at 50 nodes:

| handicap | record | score |
|---|---|---|
| four-piece | 2W-1D-9L | 21% |
| **six-piece** | **5W-1D-6L** | **46%** |
| eight-piece | 8W-2D-2L | 75% |

**Six-piece odds is the sparring partner.** At 46% the win rate responds to real changes; at even
material nothing does.

Two traps, both of which invalidated a first attempt: the handicap belongs to one colour while the
harness alternates colours, so the aggregate is meaningless and only the per-colour split counts; and
`--random-plies 8` squanders a material advantage asymmetrically, because the side with more pieces
has more to hang. Use 2 or fewer.

### Failed: widening the training target

56.9% of our self-play positions sit past the ±2000 cp clamp, so more than half the corpus carried an
identical target. The inference — that the network is therefore blind where games are decided, and
that this is why it cannot convert — was wrong.

Retrained with `--eval-clamp 6000 --sigmoid-eval-scale 1200` (both are needed; at scale 410 a 2000 cp
position already targets 0.992, so raising the clamp alone changes nothing). Conversion at four-piece
odds:

| network | record |
|---|---|
| deployed | 2W-1D-9L |
| wide target, epoch 2 | 1W-2D-9L |
| wide target, epoch 4 | 0W-0D-12L |

Worse, and monotonically worse with training. The cost of widening — losing resolution near equality,
where a 500 cp edge drops from target 0.772 to 0.603 — evidently dominates. Scale 410 is standard
practice for good reason. Nothing was deployed.

Held-out r rose 0.880 → 0.895 across the run, which is *not* evidence: it is computed against the
clamp, and the clamp changed.

### Why conversion fails, without needing a bug

Our median per-move loss on neutral ground is 121 cp against YaneuraOu's 64 — a deficit of ~57 cp per
move. A four-piece handicap is worth roughly 2,400 cp, which at that rate is consumed in about 42
moves; these games run past 100. Losing from four-piece odds is the arithmetic of the per-move gap,
not a separate defect in how won positions are played. The earlier framing of this as "structurally
wrong" over-read it.
