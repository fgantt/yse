# NNUE Implementation Session Log

## Session 32: Fifteen Idle Threads, and a Gate That Can Decide

**Date**: 2026-09-30 to 2026-10-01
**Phase**: Phase 2w
**Objective**: Session 31's *Next* list, starting with why the root split beats one thread, and
search speed in general -- the constraint since Session 29.

> **READ THIS FIRST.**
>
> 1. **Shipped: PV splitting, +63.9 ± 47.9 Elo** (`f86379b`). Against the Session 30 stable at
>    1 s/move: 129-2-89 over 220 games, SPRT [0, 40] passed. The first change to beat stable since
>    Session 30. Installed as `engines/usi-engine-stable` and `target/release/usi-engine`; the
>    Session 30 engine is now `engines/usi-engine-stable-prev`.
> 2. **The shipped "parallel" search ran on one thread.** A 15 s profile: of 16 pool threads, one
>    busy 86% of the time, the others 2-8%. The root split searched the first root move alone, and
>    that subtree is most of the work; the other root moves' zero-window scouts fail low fast and
>    their threads wait. Session 31's "workers asleep 52-85%" was the same thing seen from the
>    other side.
> 3. **The ship gate is now an SPRT** (`a69300b`, CLAUDE.md). A 120-game match resolves ±60 Elo,
>    which is why Session 31's candidates all read "neutral". `usi_match --sprt-elo0 0
>    --sprt-elo1 40` stops when the test decides; `scripts/sprt.py` checks any match CSV.
> 4. **Not measurable, committed anyway:** SEE and slider move generation cloned the shared magic
>    table's `Arc` on every call. Borrowing it leaves the single-threaded search byte-identical
>    (8,593,711 nodes, 30 positions, depth 5) at the same 44.4k nps.

---

## The profile

`sample` on the Session 31 build (parallel search, as shipped) for 15 s on corpus record 6033
(`l3g1ks1/4spss1/…`), busy samples per thread (total samples minus those in a wait):

| thread | busy |
|---|---|
| one pool thread | 7,455 of 8,688 (86%) |
| the other 15 | 129-1,469 each (1-17%), most 2-8% |

Busy work overall: the NNUE forward pass ~16%, move ordering ~9%, drop generation ~6%.

Two leads that did not survive measurement:

- **Per-iteration worker setup.** `search_root_moves` builds a fresh `SearchEngine` (16 MB table,
  evaluator, orderer) per rayon split and for the first move, every iteration. 0.82 ms each: a
  few ms per move. Not the cost.
- **The shared `Arc` refcount.** Above: identical tree and speed single-threaded; at depth 6 in
  parallel 413 s → 323 s over 12 positions, but the per-position ratio's 95% CI was 0.77-1.02,
  and with 15 threads idle there is little contention to remove. Most likely noise.

## PV splitting

`SplitSearch` in `src/search/parallel_search.rs`. A split node searches its first move by
splitting the child position the same way -- first move alone, the rest as parallel zero-window
scouts against the best score so far, re-searched with the full window when they beat it --
down `pv_split_plies` plies (env `SHOGI_PV_SPLIT_PLIES`, default 2; 1 is the old root split).
Scouts are never split. Children in check (the check extension lives in the serial search),
with no legal moves, or with fewer than 3 plies left are searched serially. A split child's
moves are ordered by `order_moves_for_negamax` with the shared table's move first.

Worker contexts are now pooled per iteration instead of rebuilt per rayon split. They are boxed:
the move orderer keeps a raw pointer to its own engine's table (refreshed at the start of each
search, but `order_moves_for_negamax` can run before that), and the box keeps it valid when a
context moves between threads. That pointer is also the only reason a context is not `Send`.

Time to depth 6, 12 corpus positions, engine defaults (`time_to_depth.py`, session scratchpad):

| split plies | total |
|---|---|
| HEAD (root only) | 413 s |
| 1 (pooled contexts) | 340 s |
| 2 | 248 s |
| 3 | 221 s |

Most of the difference is in the two slowest positions (179 → 78 s, 132 → 94 s at 2 plies).
Splitting changes the tree -- a split node searches every move at full depth, without LMR -- so
this is not a pure speed figure; the gate decides.

### The gate

All vs the Session 30 stable (`98052b4`), 1 s/move, `--random-plies 8`:

| match | result |
|---|---|
| 2 plies, seed 3001, 120 games | 67-0-53, +40.7 ± 64.7 |
| 2 plies, seed 3002, stopped by the SPRT at 100 | 62-2-36 |
| **pooled, 220 games** | **129-2-89, +63.9 ± 47.9, LLR +3.15: PASS** |

No illegal moves, crashes or overruns in either match (`matches/s32_pvsplit2_*`).

**3 plies, rejected.** Against the new (2-ply) stable, SPRT [0, 40], seed 3003: 47-0-71 over
118 games, −71.7 ± 71.5, LLR −3.10: FAIL (`matches/s32_pvsplit3_sprt.*`). Longest think 891 ms,
so not a clock problem. It reached depth 6 faster (221 s vs 248 s) and plays worse: a split node
searches every move at full depth without LMR, and a third ply adds many such nodes for little
extra parallelism. Time to depth is not the measure of a split; games are.

## A third six-piece loss (the PV-split stable)

`matches/user_games/2026-10-02_6mai_loss74.txt`: the user, with the newly installed stable, lost
to YaneuraOu at six-piece odds in 74 moves. YaneuraOu reviewing each of our moves at 3M nodes,
the three large ones re-checked at 30M (scores ours, capped at ±3000):

| move | played | YaneuraOu before → after (30M) | loss | YaneuraOu | ours at 10 s now |
|---|---|---|---|---|---|
| 1-46 | | +3000 throughout | | | |
| 47-51 | 1e1d, 1d1c+, 1c1d, 7g8e, 8g8f | +3000 → +2131 (3M) | ~870 | | |
| 55 | 6f8h | +2200 → +1654 | 546 | S\*7d | 6f8h, depth 6, +1193 |
| 56 | 5i5h (rook takes the dropped pawn) | +1515 → +863 | 652 | 5i7i | 5i5h, depth 7, +838 |
| **59** | **7h6h** | **+676 → −930** | **1,606** | 3f4g (+842) | 5h5f (+511), depth 6, +554 |

Then −250 to −350 a move to mate (`...S*5e`, mate in 1 on the board).

- **Moves 39, 40, 46 are judge artifacts.** At 3M and 30M nodes YaneuraOu reads −3000 after
  them, and +3000 a move later; from the bare SFEN (no move history) all three are +3000. Our
  pawns kept being dropped on 5d/5e and taken, so the same board recurred with White's hand
  larger: YaneuraOu's superior-repetition rule, not the position. The shuffle cost two pawns net
  (our hand 2P → 0, White's 2p → 4p) while still winning.
- **The decisive sequence is the drop attack on our rook and king, 55-59** -- the Session 31
  blind spot again: `...P*5h`, `5i5h`, `...S*6i` and the rook is lost for a silver at 59.
  At 10 s the engine still plays 55 and 56 (depth 6-7). At 59 it now prefers 5h5f, which
  YaneuraOu rates +511 against 7h6h's −624: the game move was partly the luck of a
  nondeterministic parallel search.
- **An outside, engine-less reading of the game** (the user shared one) placed the damage in the
  opening, at 50 (`7g8e`, "loses a knight") and at 66 (the bishop for a silver). YaneuraOu
  disagrees on all three: +3000 to move 46, `7g8e` was its own choice (−28), and by 66 the game
  was already −3000. Its recommendation -- drop threats against our major pieces and king --
  matches the data.

## C1, re-gated: shipped as a correctness fix

Session 31's C1 (`213d58d`): with a shared table set, negamax stored into it but probed the
engine's own table, which for a worker never received an entry -- workers searched without
transposition-table cutoffs. Neutral in Session 31 (−29.0 ± 52.7), when one thread did the work.
On top of the PV split, against the 2-ply stable, SPRT [0, 40], seed 3004:
**318-7-275 over 600 games, +24.9 ± 28.6, LLR +0.95: inconclusive at the cap**
(`matches/s32_c1_sprt.*`). Ahead at 122 games (59%, LLR +1.58), then drifting to the middle.

Shipped by the user's decision as a correctness fix on non-regression evidence (P(worse) ≈ 4%),
not as an SPRT pass -- the one exception to the gate so far. `engines/usi-engine-stable` is
now 2-ply + C1; `-stable-prev` is 2-ply without it.

**Lesson for the bounds:** a true effect between elo0 and elo1 makes the SPRT slow by design.
For correctness fixes, a non-regression test (e.g. [−20, +10]) is the right question.

## A fourth six-piece loss (2-ply + C1 stable)

`matches/user_games/2026-10-03_6mai_loss58.txt`, lost in 58. YaneuraOu: +3000 through move 29,
then (30M nodes, ours; "ours at 10 s" is the installed stable re-searching the position):

| move | played | YaneuraOu before → after | loss | YaneuraOu | ours at 10 s |
|---|---|---|---|---|---|
| 31 | 9d9c+ | +3000 → +2383 | 617 | 3g2e | 9d9c+, d7, +1483 |
| 33 | 7g7f | +2367 → +1995 | 372 | 3g2e | 7g7f, d7, +1421 |
| 35 | 8h7g | +1804 → +983 | 821 | P\*8e | 3d3c+, d7, +980 |
| 36 | 3d3c | +867 → +350 | 517 | P\*8e | 3d3c, d7, +1108 |
| 37 | P\*3d | +208 → −244 | 452 | 3g2e | 3g2e, d7, +699 |

Moves 39-40 read −3000 at 3M: the repetition artifact again (pawns re-dropped on 3f/3e); from
the bare SFEN, −563 and −841.

The same shape as the last three losses. White pushes and drops pawns down the 8-file at our
king (`8e8f`, `P*8f`, `P*8g`, `7e7f`); YaneuraOu blocks with `P*8e` or regroups (`3g2e`); we
keep attacking (`9d9c+`, `3d3c`) and score +1,000 to +1,500 where YaneuraOu's deep search
sees +350 to +2,400 falling. Five moves of ~500 each, not one blunder. Depth 7 at 10 s does
not see it; Session 31 measured the danger in such positions at ~10 plies.

## SPRT

Pentanomial GSPRT, the OpenBench / Fishtest normal approximation: each colour-swapped opening pair
is one sample scored 0, ¼, ½, ¾ or 1; `LLR = n (s1 - s0)(2 mean - s0 - s1) / (2 var)`; Wald bounds
±2.94 at α = β = 0.05. Simulated at 2% draws (`scripts/sprt.py plan 0 40`):

| true Elo | pass rate | median games |
|---|---|---|
| −40 | 0% | 122 |
| 0 | 6% | 314 |
| +40 | 89% | 292 |
| +60 | 98% | 176 |

[−10, 30] passes a neutral change 19% of the time; rejected. Applied in hindsight to Session 31's
13 gate CSVs (`scripts/sprt.py check`): five FAIL (Lazy SMP −7.36, YBWC −6.17, qlegal, king danger
128, C1+C2+C3 at 5 s) and eight undecided, every one with a negative LLR (−0.51 to −2.82). None
was a missed win.

## Method notes

- **Profile per thread, not per function.** Session 31 read "workers asleep 52-85%" from the
  same tool and took it as contention; the per-thread split showed one thread doing the work.
- **A gate that cannot decide wastes the session that feeds it.** Two sessions of ±60 Elo
  matches produced no shippable result and no stackable one either.
- **`cargo fmt` formats the whole crate** (54 files here; the tree is not rustfmt-clean). Don't.

## Next

1. More of the idle threads, without giving up LMR: split at non-PV nodes too (YBWC), keeping
   reductions at split nodes; profile per thread after each step. 3 plies of PV split failed.
2. Single-thread speed: the NNUE forward pass is the largest busy item.
3. Session 31's C2, C3 and qlegal, re-gated by SPRT (C1 shipped, above).
