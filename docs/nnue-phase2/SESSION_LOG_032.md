# NNUE Implementation Session Log

## Session 32: Idle Threads, Fake Draws, and a Gate That Can Decide

**Date**: 2026-09-30 to 2026-10-07
**Phase**: Phase 2w
**Objective**: Session 31's *Next* list, starting with why the root split beats one thread, and
search speed in general -- the constraint since Session 29.

> **READ THIS FIRST** (final state; the sections below are in the order the work happened).
>
> 1. **Stable is `11220d2`** (`engines/usi-engine-stable`, = `target/release/usi-engine`;
>    `-stable-prev` = `a98b1b2`). Shipped this session, each against the stable before it:
>    2-ply PV split +63.9 ± 47.9; C1 (workers probe the shared table) +24.9 ± 28.6;
>    overhead stack (C2, LRU index, flank clone) +36.7 ± 29.3; partial iterations
>    +42.8 ± 35.0; **four search-correctness bugs +137.0 ± 88.2** (LMR/verification re-searches
>    scored as 0 draws, colour-blind Zobrist keys, perpetual check scored as a draw, aborted
>    searches stored); superior/inferior repetition +135.8 ± 86.8 (even; neutral at six-piece);
>    NNUE layer 2 in i32 and a memo, +11% nps, tree-identical. Self-play gains overstate gains
>    against other engines.
> 2. **Against YaneuraOu (ladder, our games as Black holding the odds):** two-piece vs 50 nodes
>    76% → 86%; **six-piece vs 2,000 nodes 61% → 78%** -- use this rung as the yardstick. The
>    user won their first recorded six-piece game (`matches/user_games/2026-10-06_6mai_win55.txt`).
> 3. **Gates:** SPRT via `usi_match --sprt-elo0 0 --sprt-elo1 40` (`--sprt-min-pairs`, default
>    20: 8 pairs once stopped a test on 14-2). For anything aimed at the user's games, gate from
>    the six-piece start (`--start-sfen`, colours swapped). Non-regression: [−20, +10] with a
>    game cap and a decision rule fixed in advance (a neutral change needs ~800 games otherwise).
> 4. **The open problem is evaluation, not search:** every user loss (5 six-piece, 2 four-piece)
>    is our king attacked by pawns and drops while our score stays +600 to +1,600 above
>    YaneuraOu's. Not pruning, not node count (64x nodes barely moves it), not a horizon effect
>    (20-ply line-walks: material constant, YaneuraOu negative throughout). **But moving the
>    evaluation toward YaneuraOu's numbers plays worse:** a fitted threat correction lost 110
>    (even) and 180 Elo (six-piece; won 9/20 as Black against stable's 18/20). Eval-gap error is
>    a diagnostic, not a target. Candidate next step: threat inputs in the network, judged only
>    by handicap games.
> 5. **No absolute Elo yet.** Proposed: even games against YaneuraOu at 1/10/100/1,000/10,000
>    nodes to find "ours ≈ YaneuraOu at N nodes".

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

## The overhead the split multiplied (shipped, +36.7 ± 29.3)

With 16 threads working, a per-thread profile showed busy threads doing ~2.6x less per sample
than the single busy thread had. Three fixes, each leaving the single-threaded `search_bench`
fingerprint byte-identical (30 positions, depth 5):

| commit | what | profile position, parallel |
|---|---|---|
| `cde6b55` | C2: table statistics off (with C1 every worker probe hit a table-wide stats mutex: `increment_misses` 7,489 of ~8,250 mutex-wait samples), node counters batched, `DIAG_*` gated | 153k → 242k nps, busy 30-37% → 59-80% |
| `c0e7bd7` | ordering cache LRU eviction by an ordered index instead of a 10,000-entry scan per insert | single-threaded 43.6k → 50.0k nps |
| `5834da2` | `score_flank_security_move` cloned the board for every quiet move and always returned `None` (`make_move` returns the captured piece) -- 13% of busy time; now returns before the clone | 228k → 367k nps |

Found by marking ordering helpers `#[inline(never)]` in a throwaway build: the cost was inlined
into `order_moves_with_all_heuristics` and invisible by name. **The flank bonus has never
applied to any move**; making it work changes the search and is a separate, gated change.

At 1 s/move the gain is smaller than on one deep position: over 34 positions, median nodes
+22%, mean depth 4.74 → 4.88 (6 deeper, 27 same, 1 shallower) -- short searches spend more of
their time outside the nodes.

**Gate:** against the 2-ply + C1 stable, SPRT [0, 40], seed 3005: **321-19-258 over 598 games,
+36.7 ± 29.3, LLR +3.07: PASS** (`matches/s32_overhead_sprt.*`). Installed;
`-stable-prev` is now 2-ply + C1.

## Short searches: not setup cost

`sample` over 26 consecutive `go byoyomi 1000` searches (corpus positions, the stable above):

- **Per-move setup is ~4% of busy time** (`clone_for_worker` 1.8%, table construction 1.0%,
  `SearchEngine::new` 0.8%), though each `go` still builds ~18 threads and a new shared table.
- **About 1.8 cores busy on average**, against 10+ in a long search. Iterations finish at e.g.
  d3 @ 54 ms, d4 @ 179 ms, and the engine then stops rather than start an iteration it predicts
  will not finish: mean time used 0.62-0.70 s of 1 s. Depths 1-3 are single-threaded
  (`ParallelMinDepth` 4).
- **`ParallelMinDepth` 3 or 2 changes nothing** at 1 s/move: mean depth 4.91 → 4.88 for both
  (34 positions, same binary).

So at the match time control the engine's time is set by iteration granularity: each iteration
costs ~3-4x the last, and the unfinished one is discarded. Session 29 found that playing on
into it made things worse -- in the single-threaded engine; the root split now searches the
previous best move first, so a partial iteration's result may be usable where it was not.

## A fifth six-piece loss (overhead-stack stable)

`matches/user_games/2026-10-04_6mai_loss71.txt`, lost in 71 (the user: "we lasted a bit
longer"). YaneuraOu: +3000 through move 42 (+2,923 at 40), then (30M nodes):

| move | played | YaneuraOu before → after | loss | YaneuraOu | ours at 10 s |
|---|---|---|---|---|---|
| **43** | **2f4f** | **+3000 → +721** | **2,279** | 2f2g | 2f4f, d6, +1629 |
| 46 | 7b6b | +524 → −110 | 634 | 6f7f | 7b6b, d8, +835 |
| 53 | 4g3h | −670 → −1254 | 584 | P\*7f | 4g3h, d5, +603 |

YaneuraOu's refutation of 43 is `...4d4e`, the pawn kicking the rook and starting on our silver
on 4g and the king beside it; after `2f2g` it rates the position +3,057. Move 55 (`7f7e`, −3000
at 3M) is the repetition artifact again: from the bare SFEN, about −1,400 before and after.

**One blunder this time, not a slow bleed** -- but the same optimism: at 10 s the engine plays
all three again and scores them +600 to +1,600 where YaneuraOu sees +721 to −1,254. At move 53
we think we are winning by 600 in a position YaneuraOu has lost by 1,250.

## Using the cut-short iteration (shipped, +42.8 ± 35.0)

`2713558`. Why Session 29's attempt failed: a `search_at_depth` the clock stopped returned the
interrupted subtree's score as real (and the first move when none had finished), and the
parallel root took those scores at face value. Now an interrupted move is dropped and the search
is marked interrupted; the root split treats it as no result and records whether its first move
(the previous best) finished. When it did, and the score is inside the window, iterative
deepening plays the best of the finished moves. Iterations now start until 0.7 of the time has
gone (was 0.5). Switches: `SHOGI_NO_PARTIAL_ITER`, `SHOGI_ITER_START_FRAC`.

Fixed-depth fingerprint byte-identical. At 1 s/move, 34 positions: time used 0.67 → 0.76 s,
4-5 moves from the cut-short iteration (0.85 used 0.78 s for nothing more).

**Gate:** partial + 0.7 against the overhead-stack stable, SPRT [0, 40], seed 3006:
**180-6-140 over 326 games, +42.8 ± 35.0, LLR +2.97: PASS** (`matches/s32_partial07_sprt.*`).
Longest think 1,007 ms against stable's 1,003. Installed; `-stable-prev` is the overhead stack.

## King-attack work: four search bugs (shipped, +137 ± 88)

The plan was search depth in king-attack positions. Measured first, on the eval-gap close subset
(|YaneuraOu| < 1500, our king under mid/high pressure), node-limited and single-threaded
(`eval_gap.py --go "nodes N" --close-only`):

- **The pruning switches change nothing** at 200k nodes: baseline, `SHOGI_NO_NULLMOVE`,
  `SHOGI_NO_LMR`, `SHOGI_NO_PRUNING` all 0.185-0.188 mean |dp|.
- **64x the nodes barely helps**: 0.187 (50k) → 0.181 (3.2M, median depth 7). The bias falls
  (+0.133 → +0.066); the per-position error does not. YaneuraOu's falls 0.157 → 0.119 from
  depth 1 to 6 on the Session 31 set.

Walking YaneuraOu's line through the worst position (2026-09-28 game 2, ply 93: YaneuraOu −933,
ours +557) showed our score at exactly 0 on several White-to-move plies. Tracing that 0 found:

1. **LMR re-searches scored a 0 draw.** The reduced search leaves the child's entry on the path
   and the full-depth re-search found its own position there as a repetition. The null-move
   verification (always 0) and mate-threat searches reused the null search's path the same way;
   the IID helpers kept siblings on theirs.
2. **Zobrist keys had no owner.** A Black and a White piece on one square hashed alike; both
   hands shared keys, so equal holdings cancelled (one pawn each = none); hand counts were capped
   at 7. Positions differing in ownership shared table entries and read as repetitions.
3. **Perpetual check scored as a draw.** In shogi the checking side loses; a side under attack
   could "escape" by checking forever (~460 perpetual cycles in one 800k-node search).
4. **Aborted searches stored placeholder scores** in the table as full-depth results.

`a98b1b2`. Depth-5 `search_bench`: 8.59M → 12.38M nodes -- the fake draws had been pruning.
The close subset is unchanged at 800k nodes (0.236 → 0.240, +0.004 ± 0.006 paired; best move
agrees 15 → 20 of 52): **these bugs are not the king-attack optimism.**

**Gate:** against the partial-iteration stable, SPRT [0, 40]: 14-0-2, stopped after 8 pairs --
too early to trust; `usi_match --sprt-min-pairs` (default 20) added. On fresh openings (seed
3008): **37-3-16, +137.0 ± 88.2, LLR +3.24: PASS**; pooled 72 games 51-3-18, LLR +5.32
(`matches/s32_searchfix_sprt*.{csv,log}`). Installed; `-stable-prev` is partial iterations.

## The king-attack optimism is evaluation, not horizon

With the fixed engine the close subset is unchanged (0.240 at 800k nodes). Walking YaneuraOu's
best line (5M nodes per ply) for 20 plies from four of the worst close positions, ours at 800k
nodes per ply, with the board's material balance (`scratchpad linewalk.py`):

| start | YaneuraOu over 20 plies | ours | material (Black) |
|---|---|---|---|
| 2026-09-28 game 2, ply 91 | −820 to −1,180 | +880 to −50 | +1,440 to +3,015, mostly +1,890 |
| loss71, ply 105 | −1,000 to −1,610 | +570 to −640 | +495 to −720 |
| loss74, ply 121 | −1,160 to −3,630 | +605 to −1,010 | +180 to +2,745 |
| loss71, ply 103 | −445 to −1,075 | +706 to −455 | +1,530 to +675 |

- **Not a horizon effect.** In the first line our score never reaches YaneuraOu's in 20 plies
  of its own best play, the material hardly moves, and YaneuraOu shows no mate: the danger is
  positional and it scores it from the start.
- **Our score follows material.** Loss74: ours falls when material falls (+180 at ply 8) and
  rises to +605 when it comes back, while YaneuraOu reads −3,500 with Black 2,500 up.
- **Large side-to-move swing.** Ours is +600 to +900 with Black to move and ~0 to +300 with White
  to move along the same line (YaneuraOu: ~±100). Each side's attack registers only on its own
  move -- an evaluation that does not score threats, worst in mutual-attack races.

So the lever is the evaluation of king danger in races, which two Session 31 fine-tunes and a
handcrafted term did not move (bias fell, per-position error rose). Candidates: a threat-aware
input (attacks on the squares around each king, by side to move), training on positions from
these races with deep labels, or a mate-threat search in the opponent's favour at shallow nodes.

## Threat inputs: the feasibility test passes

`scripts/threat_feasibility/`. 20,000 handicap-corpus positions (YaneuraOu depth-8 labels,
split by game, 16,124 train after parsing) and the 1,074 non-mate eval-gap positions; our score
at depth 1; per-king threat features from the board (enemy attacks on the king zone, defenders,
escape squares, enemy hand by piece class, nearby enemy pieces, each also times side to move);
ridge regression of p(YaneuraOu) − p(ours):

| set | n | ours | + threat correction | + bias-only correction |
|---|---|---|---|---|
| held-out handicap games | 3,876 | 0.175 | **0.137** | 0.171 |
| held-out, \|yo\| < 1500 | 1,428 | 0.227 | 0.216 | 0.224 |
| user games, all | 1,074 | 0.093 | **0.074** | 0.094 |
| user games, our king low pressure | 848 | 0.043 | **0.036** | 0.043 |
| user games, close + our king pressured | 98 | 0.264 | 0.245 (bias +0.23 → +0.11) | 0.273 |

Unlike Session 31's king-danger term and fine-tunes, per-position error falls everywhere,
quiet positions included, on games the model never saw. The hardest subset gains least
(−7%): a linear model of hand-made features is a floor for what threat inputs to the network
can do. Largest weights: enemy attack pairs on each king zone, enemy gold/silver and major
pieces in hand, attacked zone squares, escape squares, attacks × side to move.

## The threat correction in the engine: accurate, and weaker (rejected)

`patches/session32/threat_correction_symmetric.patch` (reverted): `BitboardBoard::attacker_count`,
`evaluation::threat_correction` (the fitted linear model applied in win-probability space at
every leaf, through one `eval_leaf` helper -- a first build corrected negamax's static eval but
not quiescence's stand-pat, mixing corrected and uncorrected scores and making things worse).
Rust features match the Python ones exactly on 500 positions.

**First fit (separate Black/White weights), even-game SPRT: 17-2-33, −110.5 ± 91.3, FAIL**
(`matches/s32_threat_sprt.*`). Fitted on handicap games only, where Black is always the
stronger side, it gave Black +110 to +180 cp at the start position.

**Colour-symmetric refit** (`fit_sym.py`: features from the side to move's view, handicap +
even-game positions; start position −0.004 in p, a tempo term). Held-out error: handicap
0.175 → 0.156, even games 0.122 → 0.121, user games 0.093 → 0.079, our king high pressure
0.294 → 0.240. In the engine, at every leaf: 55.8k → 41.5k nps single-threaded.

**Six-piece handicap SPRT (both versions from the user's start, colours swapped): 9-3-28,
−179.5 ± 96.9, FAIL** (`matches/s32_threat2_6mai_sprt.*`). As Black (the side with the extra
pieces, the user's games) the correction won 9 of 20; stable won 18 of 20. Closer to
YaneuraOu's judgement of king danger, it stops pressing the advantage it should convert.

**Session 31 again: an evaluation closer to YaneuraOu's numbers can play worse.** The eval-gap
error is a diagnostic, not a target. Whatever replaces this has to be judged by games from the
start, and the handicap SPRT above is the right gate for anything aimed at the user's games.

## First win against six-piece YaneuraOu, and the ladder

**`matches/user_games/2026-10-06_6mai_win55.txt`: the user's first recorded win against
YaneuraOu at six-piece odds**, with the stable as of `a98b1b2` (all of this session's shipped
changes), in 55 moves. YaneuraOu reviewing each of our moves at 3M nodes: +3000 (its cap)
after every one, no move costing 200 or more. The previous five six-piece games in this session's
records were lost from +3000.

Handicap ladder, Session 31 setup (YaneuraOu fixed nodes, one thread, no book; ours 1 s/move;
`--random-plies 2`, seed 2910; only our games as Black, holding the odds, counted; both
engines run at the same time, `matches/ladder_s32/`):

| rung | Session 30 stable | Session 32 stable |
|---|---|---|
| two-piece, YaneuraOu 50 nodes | 30-1-9, 76% | **34-1-5, 86%** |
| six-piece, YaneuraOu 2,000 nodes | 22-5-13, 61% | **29-4-7, 78%** |

Losses as the side holding the odds fall from 13 to 7 at six-piece odds (difference ~1.7 SE at
40 games each). No illegal moves or crashes; longest think 1,052 / 991 ms.

## A four-piece loss (stable as of `a98b1b2`)

`matches/user_games/2026-10-06_4mai_loss52.txt`, lost in 52 at four-piece odds (YaneuraOu starts
the game at about +2,800). 30M nodes; ours = the stable re-searching at 10 s:

| move | played | YaneuraOu before → after | loss | YaneuraOu | ours at 10 s |
|---|---|---|---|---|---|
| 24 | 6h7h | +2,497 → +2,254 | 243 | 4i3h | 6g6f, d7, +1,532 |
| 25 | 3g3f | +2,590 → +2,356 | 234 | 8h5e | 8h5e, d7, +1,525 |
| **26** | **7d7c+** | **+2,347 → +1,738** | **609** | 6g7f | 7d7c+, d7, +1,379 |
| **31** | **N\*6f** | **+1,134 → +192** | **942** | 7h7f | N\*6f, d7, +1,212 |
| 32 | G\*7d | +63 → −137 | 200 | 6i6h | G\*7d, d7, +1,009 |

Moves 27 and 39 read −3000 at 3M: artifacts (bare SFEN: 27 costs nothing, 39 about 400). The
current engine avoids 24 and 25 but still plays the two costliest, scoring +1,200 where
YaneuraOu sees +200: our drops against White's king while White's pawns and pieces gather at
ours (7f7g+, P\*7g, B\*2h). The same blind spot. Labelled into `eval_gap/positions_s32.jsonl`
(`eval_gap_build.py` now knows the four-piece start).

## The seven six-piece ladder losses, and the hand-repetition rule (neutral)

Reviewing the stable's 7 losses as Black in the six-piece/2,000-node rung (YaneuraOu 1M nodes per
move, bare-SFEN rechecks):

- **Pawn feeding (game 57).** `P*4d`, `4d4c+`, silver takes; `P*4d`, `4d4c+`, silver takes
  (plies 49-59), then the same on the 2-file. A pawn per cycle, each position still +3000 -- the
  pattern of the first user loss (moves 37-46).
- **Game 39, ply 105, `6b5b`** (+2,425 → −2,592 on the bare board): reproduced at 1 s and 10 s
  (+837 to +1,063); the king takes the piece and White's counterattack follows. The blind spot.
- **Game 31, ply 107, `G*6c`** (+1,488 → −2,578): not reproduced -- the stable now plays
  YaneuraOu's `6h5g`. Parallel nondeterminism.
- The rest: slow slides, as in the user's games.

**Superior/inferior repetition** (YaneuraOu's 優等/劣等局面; `patches/session32/hand_repetition.patch`,
not shipped): a wrapper around negamax keeps (board-only hash, hands, side to move) per path
entry; the same board with the same side to move and strictly fewer pieces in our hand scores
−30,000 (more: +30,000). Parallel workers search from below the real root, so the root split
now passes the ancestors down (`set_hand_path_prefix`); without that the rule never fired in
the shipped engine. ~1% nps. Off (`SHOGI_NO_HAND_REPETITION`): byte-identical to stable. On
game 57 it stops completing the cycle at plies 53 and 57 (`1d1c+` -- YaneuraOu's move -- or
`P*2d` instead of `4d4c+`), not at 63/67.

**Six-piece handicap SPRT: 91 pairs, −9.5 ± 39.4, LLR −2.96: FAIL** (neutral; as Black 32-3-6
against stable's 32-3-5 in the mirrored games). It fixes the cycle and does not move results.

## A second four-piece loss

`matches/user_games/2026-10-06_4mai_loss64.txt`, lost in 64 from about +2,800. 30M nodes; ours =
the stable at 10 s (now depth 8-9 on these positions):

| move | played | YaneuraOu before → after | loss | YaneuraOu | ours at 10 s |
|---|---|---|---|---|---|
| 32 | 5g5f | +2,662 → +2,464 | 198 | 6h6g | 5g5f, d8, +1,444 |
| 33 | 5f5e | +2,396 → +2,192 | 204 | 6h6g | 5f5e, d9, +1,684 |
| **35** | **1i1d** | **+2,196 → +1,686** | **510** | 4f4e | 1i1d, d8, +1,501 |
| **38** | **2a3a** | **+1,510 → +843** | **667** | 2i1g | 1d1c, d9, +1,275 |
| **39** | **4h5g** | **+825 → +121** | **704** | 3a4a | 1d1c, d8, +1,062 |

A slow slide again (+2,662 at 32 to +94 at 41): the engine pushes its own attack (central pawns,
an edge lance, the tokin) while White's drops gather on the 1- and 2-files and behind our camp
(`P*1g`, `1g1h+`, `1h2i`). At 10 s the stable avoids 38 and 39 but still plays 32-35, scoring
+1,450 to +1,700. Labelled into `eval_gap/positions_s32.jsonl` (128 positions; set now 638).

## Shipped: hand repetition (on the user's decision), and two exact speed-ups

- **Superior/inferior repetition** (`11220d2`, see above): even-game non-regression SPRT
  [−20, +10]: **58-2-26, +135.8 ± 86.8, PASS** (`matches/s32_handrep_even_nonreg.*`) -- far
  better than neutral, unlike the six-piece result (−9.5 ± 39.4). Untested guess: in even
  self-play both sides fall into material-losing cycles and the rule exploits the old engine's.
- **NNUE layer 2 in 32-bit** (`3183362`): activations measured ≤ 1,008 over 10.8M evaluations,
  |w| ≤ 155, column sums ≤ 13,679; 16-bit multiplies into 32-bit sums when provably exact.
- **`count_tactical_threats` memoised per board** (`1134098`): LMR's escape-move test asked it
  2-3 times per move for the same board.

Both speed-ups leave the fixed-depth fingerprint byte-identical. Depth-5 `search_bench`, idle
machine: old stable 54.3k nps, new 60.4k (+11%); 6% more nodes from the repetition rule.
Installed as `engines/usi-engine-stable`; `-stable-prev` is the search-fix build (`a98b1b2`).
`search::cache_management::tests::test_performance_monitoring` failed once and passed 3/3 on
rerun: a sleep-based timing test, unrelated.

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
2. The king-attack "optimism" is evaluation (material-led, side-to-move swing), but correcting
   it toward YaneuraOu's numbers lost 110-180 Elo. Gate anything here with the six-piece
   handicap SPRT. Otherwise: the NNUE forward pass (~15% of busy time).
3. Session 31's C2, C3 and qlegal, re-gated by SPRT (C1 shipped, above).
