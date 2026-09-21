# NNUE Implementation Session Log

## Session 27: The Search Was Throwing the Evaluation Away

**Date**: 2026-09-20
**Phase**: Phase 2u
**Objective**: Find why move quality got *worse* with search depth.

> **READ THIS FIRST — two corrections to the record.**
>
> 1. **Session 25's headline was wrong.** It concluded "the evaluation is the binding constraint, fix
>    evaluation before adding search features". It is not. `search_oracle` — plain alpha-beta over the
>    *same* evaluation and move generator, with no TT, pruning, reductions, extensions or aspiration —
>    reached a mean move loss of **179** at depth 4 while our engine managed **327**. The evaluation
>    supported far better play than the search was delivering. A whole corpus-generation and
>    retraining cycle was spent on the wrong component.
> 2. **Offline metrics misled twice in two sessions.** Held-out correlation ranked four networks in
>    exactly the reverse of their match results (Session 26). Move loss on the 150-position set now
>    says we are better than YaneuraOu at depth 1 while we lose 21 of 24 games to it at 50 nodes.
>    **Decide with a paired match, at enough games to have a usable interval.**

---

## How it was found

`search_oracle` is the control: same evaluation, same move generator, none of the machinery. Judged by
YaneuraOu at depth 10 over 150 positions from our own games:

| depth | oracle | our engine |
|---|---|---|
| 1 | 364 / 127 | 274 / 105 |
| 4 | **179 / 70** | **327 / 140** |

The oracle improves with depth, as a search should. Ours degraded. That located the fault in the
machinery rather than the evaluation, and `f65e52b` added five env toggles (`SHOGI_NO_TT`,
`_NULLMOVE`, `_LMR`, `_PRUNING`, `_CHECKEXT`) so each difference could be removed without a rebuild.
With all five off the engine reached 190/89 and agreed with the oracle on 116/150 moves (median score
difference 0, against 53/150 and 58 with everything on).

## The defects

1. **Null move ran on PV nodes** (`040258f`). A cutoff returns `beta` — a lower bound, not a value. On
   a window wider than one point that bound replaced the node's true score and propagated to the root,
   so a PV move could be chosen on a null-move bound. Now zero-window only.
2. **LMR skipped its re-search below `re_search_margin`** (default 50cp) (`040258f`). The reduced
   search runs shallower on the zero window `(-alpha-1, -alpha)`, so every score above alpha is a
   lower bound the node may not keep. Moves scoring alpha+1..alpha+50 were kept on that bound. Now
   re-searched whenever the score beats alpha.
3. **The transposition flag was decided against the mutated alpha** (`040258f`). alpha is raised as
   moves improve, so a node that improved it ended with `best_score == alpha` and stored `UpperBound`.
   **This path never wrote a single Exact entry.** Now decided against the window the node was entered
   with.
4. **Mate scores had no distance** (`9f89434`). A flat `-100000` wherever the mate fell: a mate in 3
   was indistinguishable from a mate in 9, and since the TT is keyed by position alone, a mate found 6
   plies from the root came back at ply 2 as though equally near. Scores now count plies from the root
   and are rebased on the way into and out of the table; ply comes from `hash_history` rather than
   being threaded through all 17 `negamax_with_context` call sites. Unit test covers the rebasing.
5. **Move-ordering state leaked across games** (`9f89434`). `clear()` reset the TT, history table and
   killers but not the move orderer's own history, killer and counter-move tables — the ones the
   search actually feeds since Session 25. The same position at the same depth returned different
   moves depending on what had been searched before. **Measurements now reproduce byte-for-byte**;
   any number in an earlier log predating this is suspect.
6. **Checking moves were reduced by LMR** (`45c4bb0`). A checking move is *extended* a ply because
   games here are decided by long sequences of quiet checking drops — and then reduced, cancelling the
   extension. At the reduced depth the mate is invisible, the move scores below alpha, and the
   re-search never fires. On a known mate-in-3 the engine alternated between finding the mate (depths
   3, 5) and a quiet move at ~1500cp (depths 4, 6). Disabling *either* LMR or the check extension
   alone restored the mate at every depth — the signature of two features fighting, not of a fault in
   either. This is the odd/even swing of task #7, which had been marked completed.

Futility pruning was exonerated by the bisect (54/150 against a 53/150 baseline); disabling it makes
play worse.

## Result

Move loss at depth 4, 150 positions, judged by YaneuraOu depth 10:

| stage | mean | median |
|---|---|---|
| session start | 327 | 140 |
| after defects 1–3 | 222 | 106 |
| **after defect 6** | **172** | **82** |
| `search_oracle` | 179 | 70 |

Down 47%. The machinery now adds value over plain alpha-beta instead of destroying it. Mates hold at
every depth 3–7 across all five cases of the mate suite, scores improving monotonically.

**Strength, measured by paired match** (1 s/move, paired openings):

* Fixed search vs the pre-fix binary, same network (only the search code differs):
  * 40 games, seed 1: 23-2-15, **+70.4 ± 111.5** — interval brackets zero, inconclusive.
  * 120 games, seed 2: 86-3-31, **+172.0 ± 72.9** (lower bound +99) — **decision-grade**.
  * Pooled, 160 independent games: 109-5-46, score 0.697, **+145 Elo**.

  This session's search fixes are worth roughly **+100 to +170 Elo** against our own previous engine.
  Note how badly the 40-game match understated it: the point estimate moved +70 -> +172 when the
  sample grew. Small matches do not merely widen the interval, they move the estimate.
* Against YaneuraOu at 50 nodes: **2-1-21**, against 1-1-22 before. Unmoved.

## Next

1. **Build a strength yardstick that tracks play.** Two offline metrics have now pointed the wrong
   way. Move loss over positions from our own games is suspect: it over-represents positions we reach,
   and YaneuraOu "depth 1" carries a full quiescence search, so it is not comparable to a node budget.
2. Remaining known defects, filed not fixed: USI never emits `score mate N` (it reports
   `score cp 100000`); null-move subtrees restart the ply count at zero, so mate distances found
   inside them are measured from the null-move node.
3. The gap to YaneuraOu at 50 nodes is several hundred Elo and none of this session touched it.
