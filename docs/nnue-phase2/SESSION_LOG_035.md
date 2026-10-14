# NNUE Implementation Session Log

## Session 35: Speed work, and our node counts were wrong

**Date**: 2026-10-14
**Phase**: Phase 2w
**Objective**: Session 34's "Next": exact speed-ups, stacked and gated once.

> **READ THIS FIRST** (state at the end of this session).
>
> 1. **Our reported node counts leave out most of the search.** USI `info nodes`/`nps`
>    count main-search nodes only, never quiescence nodes; `search_bench`'s `nodes=` (and
>    its fingerprint) is only the last `search_at_depth` call's main-search nodes. On
>    `pos_loss70_m50` at depth 10 (one thread): 1,306,956 main + 4,045,276 quiescence =
>    **5.35M nodes searched**, against 498,832 reported by `search_bench`. YaneuraOu counts
>    every node.
> 2. **So the speed gap is ~2.3x, not ~8x.** Same position, one thread: YaneuraOu
>    **1.08M nodes/s**; ours **~480k nodes/s** of real nodes (`search_bench` now prints
>    `all_nodes`/`all_nps`). Session 34's "YaneuraOu ~8x faster per thread" and "123k nps"
>    compared its all-node count with our main-node count.
> 3. **And the tree-size gap is larger than we thought.** Every comparison of node counts
>    with YaneuraOu ("needs 10-100x fewer nodes to the right move", the odds analysis'
>    10k vs 1M budgets) undercounted ours by roughly 4x (quiescence is ~75% of our nodes).
>    Our `go nodes N` stops on the same main-node counter (`TimeManager::node_limit_reached`
>    reads `GLOBAL_SEARCH_NODES`), so a "1M node" search of ours did ~4M nodes of work.
>    Tree efficiency (what we search), not node speed, is the larger gap.
> 4. **Speed stack, not yet gated** (`22372e0`, `d50a313`, `066da73`): time to depth 10
>    **-10.5%** on both saved positions (12.31 -> 11.03 s, 1.25 -> 1.11 s, interleaved
>    pairs). All exact except `d50a313` (below). Too small to pass an SPRT [0, 40] on its
>    own; stack more before gating.
> 5. Stable is unchanged (`a115260`, `engines/usi-engine-stable`). No matches were run.

---

## Method

As Session 34: `sample` on `search_bench --depth 10` over `pos_loss70_m50.sfen`, change one
thing, `search_bench --depth 5 --fingerprint` over the default 30 positions byte-identical to
`matches/s34_threads/speed/fp_a115260.txt`, interleaved A/B of time to depth 10 on the two
saved positions, tests that guard the change. Builds in the scratchpad target directory; the
GUI's `target/release` untouched.

Starting profile (search thread, inclusive): move ordering 34% (quiet-move SEE 12%, its
`is_square_attacked_by` pre-test 4%, quiet bonuses 8%, sort 3%, counter-move 2.5%),
quiescence 42% (evaluation 23%), legal move generation 13%.

## What changed

| commit | change | time to depth 10 |
|---|---|---|
| `22372e0` | Quiet-move bonuses' board scans (storm pawns, edge dragon, developed defenders, defender near the king) once per ordering call instead of once per quiet move | -1% |
| | SEE attacker list: listed squares in a bitboard (was a linear `contains`), empty piece types skipped, insertion sort, `add_sliders` exits when no unseen slider is on a ray, no re-scan after a knight recaptures (it stands on no line through the target) | -3% |
| | `AttackTables::get_attack_pattern` inlined; `is_square_attacked_by` tests gold and the four promoted minors with one lookup (their tables are the gold table) | -2% |
| `d50a313` | Move-ordering cache removed; drops keyed apart from moves off square 0 in the score cache (**not exact**, below) | -1.5% |
| `066da73` | Each target square's SEE attacker list and attack test shared within an ordering call (drops of several pieces onto one square, pieces converging on it) | -2% to -3% |

**`d50a313` is not exact.** The ordering cache hit 39 times in 435k orderings over a 12 s
search; every ordering was cloned and inserted to get them. The score cache's key gave a drop
the same `from` as a move off square 0, which scores a development bonus, so a cached score
depended on which came first. The 30-position depth-5 fingerprint is unchanged; at depth 10
`pos_loss70_m50` searches 498,832 instead of 498,417 (reported) nodes, same move. Both caches
made ordering depend on call history; without them the order is a pure function of the
position and the history/killer/counter tables.

New depth-10 reference fingerprints for the stack: same best moves and node counts at
`066da73` as at `d50a313` on both saved positions.

## Why a staged ordering cannot be exact

At depth-1 nodes (55% of main nodes) we generate and order ~73 legal moves to search 3.9
(`search-diag`, depth 10; depth 2: 82 -> 3.7). Staging the ordering would skip most of that,
but the order of the moves searched depends on scores computed before the loop: child searches
change the history and killer tables during the loop, and late-move pruning and LMR use each
move's exact rank, which needs every quiet move's score. An exact staged ordering would have
to snapshot the tables. A non-exact one was tried in Session 34 (no gain, larger trees).

## The node-count finding

Counted directly (temporary counters at `lean_node` past the depth-0 return, and at
`lean_qsearch` entry), `pos_loss70_m50`, depth 10, one thread:

| table | main nodes | quiescence nodes | reported by `search_bench` | time |
|---|---|---|---|---|
| 32 MB | 1,306,956 | 4,045,276 | 498,832 | 11.4 s |
| 64 MB | 824,280 | 2,512,402 | 33,097 | 7.0 s |
| 256 MB | 1,535,955 | 4,941,137 | 750,708 | 13.6 s |

`SearchStatistics::reset_nodes` runs at every `search_at_depth`, so `get_nodes_searched`
(what `search_bench` reports) is the last iteration or aspiration re-search only. USI output
uses `GLOBAL_NODES_SEARCHED`, fed by `increment_nodes`, which only `lean_node` calls; quiescence
nodes go to `quiescence_stats.nodes_searched`. Evaluations: 4.48M forward passes, 3.33M of them
quiescence stand-pats; one forward pass costs ~0.3 µs and 72% of the 512 first-layer
activations are non-zero, so the NEON kernel is near its limit (~3,000 multiply-accumulate
instructions per evaluation).

YaneuraOu V900 (`Threads 1`, 256 MB, no book) on the same position, `go movetime 10000`:
1,078,551 nodes/s; depth 18 at 2.66M nodes, depth 19 at 10.8M.

## Where the time goes now (`066da73`, search thread)

Quiescence 45% (evaluation 17.6%, capture/check generation 7.4%, in-check legal generation
4.5%, NNUE incremental updates 3.8%); move ordering ~30% (SEE ~10%, quiet bonuses, sort);
legal move generation 13% (the make/unmake legality and check test is 60% of it).

## Next

Decide before more speed work (question for the user):

- **Count every node in USI output** (main + quiescence, as YaneuraOu and Stockfish do), so
  node-limited comparisons with YaneuraOu mean the same thing. It changes what `go nodes N`
  buys our engine; game play at a time control is unaffected. Then redo the "nodes to a good
  move" comparison (Session 34, odds positions) with true counts.
- **Tree efficiency over node speed.** With true counts the gap is ~2.3x in speed and much
  larger in nodes. Candidates: quiescence share (~75% of nodes; first-ply quiet checks, 204k
  of 1.6M quiescence nodes in `search-diag` at depth 10, and in-check evasions), depth-1
  nodes ordering 73 moves to search 4.
- **Remaining exact speed work** (perhaps 10-20% more): direct check test for drops (a drop
  cannot uncover a check, so gives-check is one pattern lookup instead of make/unmake),
  quiescence move generation, `refresh_pov` on king moves (Finny tables, ~3%). Stack to
  ~+30% before an SPRT: +12% alone would not resolve against [0, 40].
