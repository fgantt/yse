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

Running at the end of the session: 3 plies against the new stable, SPRT [0, 40], seed 3003
(`matches/s32_pvsplit3_sprt.*`).

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

1. The 3-ply SPRT (running). If it passes, make it the default.
2. More of the idle threads: split the first move's subtree further than the PV (YBWC at
   non-PV nodes, or splitting re-searches). Profile per thread after each step.
3. Single-thread speed: the NNUE forward pass is the largest busy item.
4. Session 31's C1-C3 table fixes and qlegal, re-gated by SPRT: with the threads actually
   working, a shared table that workers can read may now matter.
