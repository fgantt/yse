# NNUE Implementation Session Log

## Session 29: Training on Move Ranking

**Date**: 2026-09-23
**Phase**: Phase 2w
**Objective**: Test Session 28's untried lever — train the network to *order* a position's
legal moves the way the teacher does, rather than only to predict what positions are worth.

> **READ THIS FIRST.**
>
> 1. **The obvious ranking objective does not work.** Softmax cross-entropy over sibling moves
>    can be minimised by lowering confidence instead of reordering. Its loss fell convincingly
>    against a control where it rose, while top-1 agreement with the teacher stayed flat and
>    move loss did not improve. See *The softmax trap*.
> 2. **A pairwise hinge does work**, on the 1-ply proxy. It closes the shortcut because
>    shrinking the score spread *increases* the loss.
>    Every conclusion below was reached at *fixed depth*, which is the one search mode the defect
>    in item 4 does not touch.
> 3. **A fixed seed does not make a training run reproducible** when the king factoriser is on.
> 4. **The engine searches ~5% of the node budget it is given.** Found at the end of this session,
>    from a real game. It does not affect the fixed-depth measurements here, but it does affect
>    every `nodes`-limited match in Session 28 — including the handicap-ladder calibration — and it
>    is a far larger effect than anything this session's training produced. See *Postscript*.

---

## The data

`rank-gen` (commit `acac6e1`) samples positions, asks YaneuraOu for its top-K moves with scores
via `MultiPV`, replays each move through our own move generator, and records the resulting child
position alongside the teacher's score for that move.

Child SFENs are written out rather than re-derived at training time, so a move our generator
cannot reproduce is dropped once, at generation, instead of failing quietly inside an epoch.

**509,773 groups, mean 7.83 siblings** (~4M ranked positions). Audit clean: 0 malformed, 0
repeated parent positions, every child carrying the opposite side to move, median top-to-last
score spread 560 cp. Two sources: positions our own engine reaches, and teacher self-play with
`game_id % 20 == 0` **excluded** — that is the Session 28 neutral test set, and training on it
would have voided the only move-loss measurement we trust.

Cost: ~30 minutes on 10 parallel workers. MultiPV is cheap because the lines share one search —
depth 8 with 8 lines runs in ~12 ms.

## The softmax trap

The natural objective is softmax cross-entropy: the network's score for move `i` is
`-output_cp(child_i)`, the target is a softmax over the teacher's scores, and the gradient
reaching each child is `(p_i - t_i)/T`. Since `sum_i (p_i - t_i) = 0` it is pure contrast between
siblings and cannot move the network's overall scale, which is why the value term can stay
alongside it.

It trained. Over 3 epochs `|p - t|` fell 0.1259 → 0.1208, against a control where the same number
*rose*. By its own loss it was working.

It changed nothing about move choice:

| objective | top-1 agreement | one-ply move loss |
|---|---|---|
| control (no ranking) | 0.294 → 0.290 | 238 → 248 cp |
| softmax CE | 0.296 → 0.294 | 245 → 253 cp |

A paired neutral-set measurement agreed independently: 126 cp median move loss for both the
ranking network and its value-only control.

**Why.** Our network spreads sibling scores much wider than the teacher does. At T = 100 cp its
softmax is near one-hot while the teacher's is not, so the cheapest way to cut the loss is to
become less confident — which reorders nothing. The loss was measuring calibration and we were
reading it as ordering.

This is the same failure as Sessions 23–26 (shipping on `elo-tester`) and Session 28 (held-out
Pearson r): **a metric that improves while play does not**. It was caught here only because
`measure_rank` reports top-1 agreement and one-ply move loss next to the loss's own units.

## The pairwise hinge

For every ordered pair the teacher separates, ask our own gap to be at least as large, capped:

```
loss_ij = max(0, min(s_i - s_j, cap) - (q_i - q_j))
```

Zero once a pair is correctly ordered by the margin, and *increasing* if the spread shrinks — so
the confidence shortcut is closed. The cap (default 400 cp) keeps a pair the teacher separates by
2000 cp from turning the term back into value regression.

## Results on the 1-ply proxy

Held out: 25,488 ranking groups, never trained on. SE on top-1 is 0.0029, and the comparison is
paired on identical positions.

| network | top-1 | one-ply move loss |
|---|---|---|
| deployed (baseline, measured at lr 0) | 0.302 | 239 cp |
| control, epoch 1 / 2 | 0.307 / 0.306 | 239 / 240 cp |
| pairwise, epoch 1 / 2 | 0.324 / 0.328 | 231 / 225 cp |
| pairwise 3x ranking share, epoch 1 | 0.329 | 226 cp |

All five arms, best epoch (`td` is the value loss, lower is better):

| arm | LR | ranking | td_err | top-1 | move loss |
|---|---|---|---|---|---|
| deployed baseline | — | — | — | 0.302 | 239 cp |
| control | 0.005 | none | **0.1299** | 0.303–0.308 | 238 cp |
| control | 0.001 | none | 0.1350 | 0.305–0.307 | 239 cp |
| pairwise | 0.001 | w=1, 32/batch | 0.1409 | 0.326 | 229 cp |
| pairwise | 0.005 | w=1, 32/batch | 0.1419 | 0.328 | 225 cp |
| pairwise | 0.005 | w=1, **96/batch** | 0.1458 | **0.330** | **223 cp** |

Both controls sit at 0.307 whether the learning rate is 0.005 or 0.001, so value training is
neutral on ranking at this scale — it neither helps nor hurts.

The trade is visible in the table and it is monotone: every arm that improves ranking gives up
value fit, and the ordering is exact. The control's td_err *falls* over its four epochs
(0.1377 → 0.1299) while every pairwise arm's stays flat near 0.142 — the hinge does not merely
cost accuracy, it stops the value objective from converging at all. More ranking share buys more
ranking (0.330) at more value cost (0.1458). That is the trade this objective actually offers, and
the next section is what it bought.

**A claim from the pilot that did not survive.** On a 64K-position corpus the control *degraded*
(238 → 248 cp over 3 epochs) and this log initially read that as "the network buys value accuracy
with sibling ordering". At 1.88M positions the control is flat. The degradation was overfitting a
small corpus, not a tension between the two objectives. The honest statement is the narrower one:
value training does not *improve* ranking.

## The result: it does not transfer

Depth-4 move loss on the neutral set (150 held-out teacher-self-play positions, all three networks
measured in one process against one judge, paired position by position):

| network | mean | median | >200 cp |
|---|---|---|---|
| deployed | **274** | **90** | 33% |
| control, epoch 2 | 290 | 100 | 35% |
| pairwise, epoch 2 | 291 | **126** | 39% |

The ranking network is not better than its control at depth 4 (mean 291 vs 290) and is worse on
the median. Both are worse than the network already deployed.

So the 1-ply gain — decisive at ~7 SE, dose-responsive, replicated across five arms — **does not
reach the engine's move choice**. On these same networks the two metrics point in opposite
directions:

| | 1-ply top-1 | depth-4 median loss |
|---|---|---|
| deployed | 0.302 | 90 cp |
| control | 0.306 | 100 cp |
| pairwise | **0.328** | **126 cp** |

This is the softmax trap again, one level up. The metric was fixed, the objective moved it
convincingly, and play still did not follow.

### The gain is real, and it inverts with depth

Median move loss on the neutral set, same three networks, by search depth:

| network | d1 | d2 | d4 |
|---|---|---|---|
| deployed | 138 | 120 | **90** |
| control, epoch 2 | 135 | 122 | 112 |
| pairwise, epoch 2 | **111** | **108** | 130 |

The hinge *did* improve move choice — clearly, at depth 1 and 2, which is exactly what it was
trained to do. The advantage reverses between depth 2 and depth 4.

### Why, most likely

**Calibration.** The depth curve rules out "the engine never reads what was trained" — it plainly
does read it, at depth 1 and 2. What changes with depth is how much the search leans on *absolute*
scores: futility margins, LMR, null-move verification and aspiration windows all key off them. The
pairwise arm's value fit is measurably worse (td_err 0.1419 against the control's 0.1299) and, worse
than that, flat — the control's value loss converges over its epochs while every pairwise arm's
stalls. The ranking term is scale-free by construction, which is why it cannot disturb the overall
scale, and equally why nothing in it *defends* the calibration deeper search depends on.

So the objective bought better ordering and paid for it in calibration, and the exchange rate turns
against us somewhere between depth 2 and depth 4.

### What this rules out

Training sibling *order* on parent-child pairs, as a bolt-on to value regression, does not improve
play at the depths a real search reaches — even though it decisively improves sibling order and
helps at depth 1-2. Session 28 named "training that targets move ranking rather than
position-value regression" as the untested lever; it is now tested in this form, and what it
actually offers is a trade of calibration for ordering that deeper search does not want.

A variant worth trying later is a ranking term that is *not* scale-free — one that preserves the
absolute score of the best sibling while reordering the rest — so ordering is bought without
spending calibration.

The premise behind it survives: we still make roughly twice the teacher's per-move error, and that
is still evaluation quality rather than search. What fails is this way of attacking it.

## Status

Not deployed. `nnue/default.bin` is unchanged.

No game-based measurement was run: with the cheap honest test showing no transfer and value fit
measurably worse, a two-hour head-to-head could not have been justified. The handicap ladder and
1 s/move match remain the gate for any future candidate that does pass a neutral-set test.

## Method notes

- **A fixed seed does not give a reproducible run** when `--king-factorizer` is on. `acc_shared`
  sums per-row gradients by iterating a `HashMap`, and Rust randomizes hash order per process, so
  float addition lands in a different order each run. Two runs of the *same* binary gave blk
  Pearson +0.738 and +0.739. Negligible in size, but it is the floor on how finely two runs can be
  compared.
- **`measure_rank` is a separate method, not `train_batch_mixed` with `rank_weight = 0`.** The
  latter is not inert: under Adam a zero gradient still decays the moment estimates and steps from
  them, so "measuring" that way would quietly train the network.
- **`--rank-weight 0` is a measured control, not a skipped branch.** The ranking metrics are
  computed whenever groups are supplied and only *applied* when weighted, so the control arm
  reports the same numbers a weighted run does.
- **Judge by per-epoch checkpoints, not the trainer's "best" file.** That file is selected on
  held-out Pearson r, which Session 28 showed rises monotonically while Elo moves the other way.


---

## Postscript: the engine was searching 5% of its budget

A user-played 8-piece handicap game, lost from a position worth +3400 cp, prompted a replay of the
losing moves through both engines. The engine had dropped a gold onto a defended square, then fed
pawns one at a time into the same capture for ten moves.

It was searching at **depth 2**.

| requested | nodes actually searched | depth |
|---|---|---|
| `nodes 20000`, handicap middlegame | **852** | 2 |
| `nodes 20000`, startpos | **1,160** | 3 |
| YaneuraOu, `nodes 20000` | 20,017 | 10–13 |

**Cause.** `TimeManager::should_stop` increments `nodes_this_search` itself, and three of its five
call sites sit *inside move loops* (`search_engine.rs` lines 6002, 7056, and quiescence's 7923)
rather than at node entry. The budget is therefore consumed at roughly `1 + moves_per_node` per
real node. The arithmetic matches: 852 × ~23 legal moves ≈ 20,000, and 1,160 × ~17 ≈ 20,000. The
same inflated counter drives `time_check_node_counter`, which is why `movetime 1000` returns after
413 ms.

The defect is worst exactly where branching is widest — middlegames with pieces in hand, which is
where games are decided.

**What this invalidates.** Every `nodes`-limited match in Session 28 was measuring an engine
searching ~4% of its stated budget, including the handicap ladder that produced "six-piece odds is
the sparring partner". Fixed-depth work (`search_oracle`, the move-loss sets, this session's
training arms) is unaffected.

**What it reopens.** Session 28 concluded the gap is evaluation quality and not search, on the
grounds that our machinery beats plain alpha-beta — a comparison run at fixed depth, where this
defect is invisible. That conclusion should be re-derived, not inherited.

**A second, separate problem.** Even at matched *actual* node counts we reach half the depth:
20,510 nodes → depth 5 against YaneuraOu's 20,017 → depth 10. Effective branching factor ~4-6
against their ~2. Fixing the budget is worth about two plies; this is the ceiling after that.

**An awkward consequence for this session's result.** In real games the engine currently plays at
depth 2-3 — the regime where the pairwise network is *better* (median 108-111 vs 120-138). It
would likely win a match today. That is a gain contingent on a bug, and it should not be banked
before the bug is fixed and the comparison re-run at honest depth.

---

## Postscript 2: "we only use half our clock" was not a bug

At byoyomi 1000 the search returned after 428–596 ms of a ~900 ms allocation, and iterative
deepening's gate was the reason:

```rust
let past_half = self.max_depth >= 100 && depth > 1 && elapsed_ms * 2 > search_time_limit;
```

It refuses to start an iteration once half the budget is gone. The comment beside it explains why:
an unfinished iteration's result is discarded, so its time would be spent for nothing.

The obvious repair is to make partial results usable and relax the gate. Tried, in three
increasingly careful forms, and measured over 150 neutral positions at 1 s/move:

| build | mean | median |
|---|---|---|
| baseline | **284** | **80** |
| partial result, guarded by "a root move completed" + exact score | 313 | 129 |
| plus "only if it improves on the previous iteration" | 308 | 92 |

Time utilisation rose from 43–80% to 85–97%, and average depth did not move at all (2.4 either
way) — the extra time almost never completes an iteration, so every difference came from adopting
partial results, and they play worse.

The second guard is instructive. The damaging case is a root fail-low: the previous best move is
refuted a ply deeper, the next alternative beats that *refuted* score, and it gets played although
the remaining root moves were never searched. Requiring an improvement over the previous
iteration recovered most of the loss (129 → 92 cp) and still did not reach baseline.

**Reverted.** The 50% figure is not waste; it is the correct consequence of discarding unfinished
iterations. Spending that time well needs the real machinery — fail-low detection with time
extension, so a refuted PV gets resolved rather than replaced — which is a larger piece of work
than "relax the gate".

It is also the wrong thing to build next. At an effective branching factor of 6, a partial extra
ply is worth far less than halving the tree, and the branching factor is where the depth gap
against YaneuraOu (3 vs 16 at the same clock) actually lives.

**Correction to Postscript 1.** The node-budget defect does not affect time-controlled play at all:
`btime`/`byoyomi` searches set no node limit, so the broken counter never applied to them. It
invalidated node-limited *measurements* — the handicap ladder, every `nodes`-limited match, and the
`--player-go "nodes 3000"` used to generate every self-play corpus since Session 25 — but the game
that prompted the investigation was lost at depth 3 for the reasons in this postscript and the
next, not because of that bug.

---

## Postscript 3: the search had no PVS, so its forward pruning did nothing

`search-diag` (commit `d94a7f0`) reports the three things that decide how wide the tree is. At
depth 4 with the deployed network:

| | startpos | midgame |
|---|---|---|
| first-move cutoff rate | 81% | 99% |
| quiescence share | 82% | 84% |
| per-ply node ratio | 5.5–7.4 | 4.3–7.4 |

Move ordering was the obvious suspect and is fine. Quiescence share is high, but disabling
quiescence checks moves the share (84% → 62%) without changing the total (52,406 → 52,222): the
main search simply expands to fill the gap.

Disabling each pruning technique in turn found the real answer:

| disabled | startpos d4 | midgame d4 |
|---|---|---|
| (baseline) | 7,743 | 52,406 |
| null-move | 7,743 (**0%**) | 52,406 (**0%**) |
| LMR | 8,641 (−10%) | 52,406 (**0%**) |
| other pruning | 8,347 | 344,670 (−85%) |

Null-move contributed nothing anywhere, and `null_move_stats.attempts` was **zero**: it had never
once fired. The cause was one line. Non-first moves were searched with the full window
`(-beta, -alpha)` instead of a null-window scout, so `beta - alpha > 1` nearly everywhere and null
move's `!is_pv_node` gate never opened. LMR, the only other source of zero-window nodes, was
carrying the job alone.

### PVS (commit `4a49471`)

Scout non-first moves with `(-alpha-1, -alpha)`; re-search only when the scout lands inside the
window. Depth 6, start position:

| | nodes | null-move | LMR |
|---|---|---|---|
| PVS off | 60,108 | 0% | 10% |
| PVS on | 51,916 | **20%** | **55%** |

Null-move attempts over the same search: 14 → 221. Re-search rate 2.7–12%.

**The first measurement said the opposite.** At depth 4, PVS *costs* 21–28%: the
scout-plus-re-search overhead dominates until the pruning it enables has enough depth to pay, and
the crossover is near depth 5. Stopping at the depth that happened to be measured first would have
reverted a change that removes 14% of the tree and switches on two dead techniques.

### It buys no strength today

150 neutral positions at 1 s/move, PVS off vs on: mean 321 → 327, median 114 → 92, >200 cp 37%
either way, **depth 2.4 both**. No detectable difference, which is what depth 2.4 predicts — we
never reach the crossover.

**This inverts the priority argued earlier in this session.** The claim that branching factor is
worth ~8× more than nps came from `depth = log(nodes)/log(EBF)`, which assumes we operate in the
regime where EBF governs. We do not: the pruning only engages from depth 5 and we reach 2.4, so
today EBF improvements buy nothing and raw speed decides everything. The two are not alternatives —
**nps is what unlocks the pruning gains.** With ~37k nps against YaneuraOu's ~1.36M, per-node cost
(task #13) is the binding constraint.

### The ladder, recalibrated

With `go nodes N` honest again, ours at 20,000 nodes vs YaneuraOu at 50, counting only our games as
Black holding the odds (20 games per rung):

| rung | W–D–L | score |
|---|---|---|
| even | 3–1–16 | 18% |
| two-piece | 6–0–14 | 30% |
| four-piece | 8–0–12 | 40% |
| **six-piece** | 10–0–10 | **50%** |

Six-piece odds remains the responsive rung. Session 28 measured 46% there with a search that was
really using ~850 nodes; 50% now is within noise at this sample size, so the node-budget fix cannot
be claimed as a strength gain.
