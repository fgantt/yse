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
> 5. **USI now counts every node** (`849b44a`): `info nodes`/`nps` and the `go nodes N` stop
>    include quiescence nodes, as YaneuraOu does. One thread reports ~471k nps (YaneuraOu
>    1.08M). A `go nodes N` search now does about a quarter of the work it did before, so
>    node-limited results from earlier sessions (ladder rungs, `depth_ab.py`,
>    `eval_gap_build.py`, `tt_time.py`, `odds_rejudge.py`) are not comparable with new ones.
> 6. **Quiescence prunes losing checks** (SEE < 0), default since `db7eae0`: depth-7 tree
>    -27% (1.33M -> 0.97M nodes), **SPRT +95.6 ± 67.7, 51-2-29, PASS after 82 games**
>    (self-play, 1 s/move, 16 threads, `matches/s35/q1_sprt.log`).
> 7. **The "improving" flag failed** (`6e1642b`, `SHOGI_IMPROVING`, default 0): all three
>    uses shrink the depth-7 tree 24% but scored **-34.9 ± 53.5, 66-3-81, FAIL after 150
>    games** (`matches/s35/imp_sprt.log`). A smaller tree is not a better one.
> 8. **Shipped: stable is now `db7eae0`** (speed stack + every node counted + losing-check
>    pruning; old stable kept as `engines/usi-engine-stable-a115260`). SPRT vs `a115260`
>    **+62.4 ± 47.6, 104-4-72, PASS at 180 games** (`matches/s35/stack_sprt.log`).
>    Four-piece vs YaneuraOu 20k nodes, Black games: **50-9-1, 84.2%** vs stable's 83.3%
>    (same seed 2930; paired: both won 43, each alone 7, both lost 3;
>    `matches/ladder_s35/yonmai20k_t16_stack.*`). **Unchanged: the second search gain in a
>    row (after Session 34's +202) that does not transfer to YaneuraOu.**

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

## Quiescence: losing checks (`0c5007d`, `db7eae0`)

Quiescence searched every checking move. `SHOGI_Q_PRUNE_LOSING_CHECKS` skips checks whose SEE
is negative (1: all checks, 2: quiet checks only); `SHOGI_Q_EVASION_PRUNING` skips losing
quiet evasions. Fixed-depth tree (`all_nodes`, NNUE, 32 MB):

| setting | 30 positions, depth 7 | pos_loss70 d10 | pos_loss84 d10 |
|---|---|---|---|
| base | 1.33M | 5.35M (9d9e) | 0.53M (4e3c+) |
| losing checks pruned (1) | 0.97M (-27%, -13% time) | 2.30M (9d9e) | 1.07M (1i1f) |
| losing quiet checks pruned (2) | 1.01M | | |
| evasion pruning | -2 to -3% | | |

SPRT, setting 1 against the same build without it: 51-2-29, +95.6 ± 67.7, LLR +3.00, PASS
after 82 games. Default on.

## The improving flag (`6e1642b`)

Static evaluation compared with two plies earlier (four if that ply was in check; unknown
counts as improving). `SHOGI_IMPROVING` bits: 1 reverse futility margin `120 * (depth -
improving)` (Stockfish's direction: prunes more when improving), 2 late-move pruning count
`(3 + d²) / (1 + !improving)`, 4 one more LMR reduction when not improving.

| bits | 30 positions, depth 7 | pos_loss70 d10 | pos_loss84 d10 |
|---|---|---|---|
| 0 (with losing-check pruning) | 0.97M | 2.30M | 1.07M |
| 1 | 1.16M* | 1.47M | 0.51M |
| 2 | 1.28M* | 1.77M | 0.65M |
| 4 | 1.29M* | 2.70M | 0.70M |
| 3 | 0.71M | 2.42M (9d9e) | 0.64M (4e5c+) |
| 7 | 1.02M* (-24%) | 1.74M | 0.46M (4e3c+) |

(* measured without losing-check pruning, against 1.33M.)

SPRT, bits 7 on top of losing-check pruning against losing-check pruning alone: 66-3-81,
-34.9 ± 53.5, LLR -3.07, FAIL after 150 games. Left off. Bits 1 and 2 alone (3) shrink the
tree about as much as 7, so the extra reduction is not what shrank it; whether the pruning
uses lose too is untested. Our futility and move-count margins were tuned without the flag;
the flag may need them retuned to help.

## Screens before matches

Matches take hours per idea and most ideas fail, so two cheaper screens were checked against
two known results: losing-check pruning (SPRT +95.6, PASS) and the improving flag (-34.9,
FAIL). Script `matches/s35/screen/` (rows and logs).

**Neutral-set move loss: useless as a screen.** 150 held-out positions, 16 threads, 1 s/move,
YaneuraOu depth 10 as judge. Mean loss (cp): before losing-check pruning 83, with it 121, the
same binary again 129, improving 108. The two runs of one binary differ by 8 cp and the worst
engine (before pruning) scores best; medians are all 0. As in Session 34, it separates nothing.

**Fast games: right on both, twice each.** Four matches at once, 4 threads each, 250 ms/move,
SPRT [0, 40], two seeds per comparison:

| comparison | seed | result | verdict | wall time |
|---|---|---|---|---|
| losing-check pruning vs before | 3601 | 227-5-178, +41.7 ± 34.1 | PASS at 410 games | 80 min |
| | 3602 | 105-5-70, +68.4 ± 50.9 | PASS at 180 games | 28 min |
| improving vs without | 3603 | 126-3-137, -14.4 ± 42.5 | FAIL at 266 games | 42 min |
| | 3604 | 63-1-82, -45.5 ± 59.5 | FAIL at 146 games | 22 min |

The full gates took about 1.5 h (82 games) and 2.5 h (150 games) for one verdict each; the
fast screen gave four in 80 min, and each of its four slots can test a different candidate.
Two known results is a small validation set: treat a fast-screen verdict as a filter, run the
16-thread 1 s SPRT against stable only on survivors, and keep checking the screen's verdicts
against the full gates as they accumulate. None of this predicts the YaneuraOu rung.

## The user's games: what loses them (2026-10-15)

A four-piece loss in 58 moves on the new stable (`matches/user_games/2026-10-15_4mai_loss58.txt`)
after a day of self-play gains. The user: the measurements did not reflect their games. Right:

- **The yardstick was wrong.** The four-piece rung gives YaneuraOu 20,000 nodes; the GUI gives
  it 4 threads, 1 GB, 60 min + 10 s (millions of nodes a move). We score 84% on the rung, so it
  barely moves, and it never shows a strong opponent's slow drop attack.
- **"10 s per move" probes were not the game either.** Our allocation (`src/usi.rs`
  `allocate_ms`) is remaining/30 + byoyomi every move: ~130 s on move 1, ~80 s at move 16,
  ~63 s at 25, ~33 s at 50. The user saw 43 of 60 minutes used by move 57, YaneuraOu 33.

### Test set from the user's games (`scripts/loss_set.py`, `matches/s35/losses/`)

All 728 of our moves in the 11 saved GUI four-piece games (9 losses, 2 wins) judged by
YaneuraOu (12-16 threads, 20M nodes; a played move's score by searching the position after
it, since `searchmoves` ignores drops). Opening moves 0-19: mean loss 43-84 cp, 8 mistakes
>= 250 cp. **Moves 20-59 lose the games**: a run of 250-1,000 cp mistakes from +2,000-3,000,
then collapse. The two wins have none. Set: 81 mistakes (>= 250 cp, game open:
-1,500 < best < 8,000) and 30 controls (moves 15-60, within 30 cp of YaneuraOu).

Stable `db7eae0`, 16 threads, each position searched for 2x its GUI allocation, the move at
each time read from the iterations:

| time | mistakes repeated (>= 250) | controls broken |
|---|---|---|
| 0.25x GUI | 54 / 81 | 2 / 30 |
| 0.5x | 48 | 2 |
| 1x (~60 s, depth 20+) | 45 | 2 |
| 2x | 43 | 3 |

YaneuraOu alone, one thread, on the same positions: 1,000 nodes 37 / 81; 10,000 nodes 32;
100,000 21; 1,000,000 13. (The judge is YaneuraOu, which flatters its own shallow choices.)
**A minute of our search plays these positions about like YaneuraOu at a few thousand
nodes, and more time stops helping past half the allocation.** Time allocation is not the
lever (a plan moving opening time to the middlegame was written and discarded); the
evaluation is: deeper search on it keeps choosing the same wrong moves.

Mistakes by kind: no clean pattern (YaneuraOu's choice stays in our camp in 25 / 81; we
played into the enemy camp in 14 / 81).

Fixed-depth screen for networks (`loss_set.py fixed`, depth 12, one thread, 12 in parallel,
deterministic): stable network 57 / 81 mistakes repeated, 5 / 30 controls broken.

### Correction

The deployed network was trained on 14.0M positions (Session 26: 9.89M teacher corpus plus our
own games), not the 67,000 of `nnue_corpus_yaneura_d10.jsonl` that CLAUDE.md names.

### A four-piece network: no gain on openings it had not seen

Teacher self-play from the four-piece start is useless as data (Black wins 95%, evals +2,900 to
+3,500). Instead, positions our engine actually reaches against YaneuraOu, labelled by
YaneuraOu depth 14 (`scripts/label_games.py`): 2,040 ladder games (193,696 positions, 81%
> +2,500) and 322 games against YaneuraOu at 300,000 nodes (us 4 threads, 500 ms, seeds
3701-3704; 28,404 positions; we won 19 of 161 as Black). Mix `corpus_s35/mix_4mai.jsonl`:
the 1.98M `value_sub` base + ladder x2 + vs-300k x6 + ladder positions with |eval| < 2,500 x4
= 2.62M. Fine-tuned from `nnue/default.bin`, 2 epochs, lr 0.001, Session 30 flags ->
`weights/session35/yon.bin`; control on the base alone -> `control.bin`.

| network | screen d12 mistakes / 81 | controls >= 250 | as Black vs 300k, seeds 3701-04 (in training) | seeds 3901-02 (fresh) |
|---|---|---|---|---|
| stable | 57 | 5 | 23 / 161 | 21 / 160 |
| control | 54 | 3 | - | - |
| yon | 44 | 4 | 33 / 161 | 22 / 160 |

(`matches/s35/net/`; 4 threads, 500 ms, YaneuraOu 300k nodes one thread, our games as Black
paired by game index.) On the training seeds yon won 29 games stable lost, against 19;
**on fresh seeds 16 against 17**. The network memorised the openings it was trained on and
learned nothing that transfers. The screen's 57 -> 44 did not predict games either; the screen
positions are from the same kind of game but not in training, so it measures something, just
not strength. Not shipped.

### More data, and a target that sees past +2,000 (`weights/session35/wide.bin`)

The trainer's target is `sigmoid(clamp(eval, +/-2000) / 410)`: every position above +2,000
gets the same target, and 81% of four-piece ladder positions are above +2,500, which is where
the user's games are lost. 2,440 more games against YaneuraOu 300k (8 runs, us 2 threads,
500 ms, seeds 4001-4008, `corpus_s35/vs300k_r3_*.csv`; games sharing their first 8 moves with
a test game dropped) labelled at depth 14: 217,104 positions (`vs300k_r3_d14.jsonl`). Mix
`mix_4mai_big.jsonl`: base + ladder x2 + all 245,508 vs-300k positions x3 + erosion x4 = 3.19M.
Two networks, otherwise Session 30 flags, from `nnue/default.bin`, 2 epochs:

| network | target | as Black vs 300k, fresh seeds 3901-02 | paired vs stable (only-net / only-stable) |
|---|---|---|---|
| stable | - | 21 / 160 | - |
| yon (above) | clamp 2000, scale 410, outcome 0.3 | 22 | 17 / 16 |
| big | clamp 2000, scale 410, outcome 0.3 | 24 | 21 / 18 |
| **wide** | **clamp 6000, scale 1000, outcome 0** | **36** | **29 / 14 (p ~ 0.02)** |

More data alone did nothing; the wider target is the first network change in Session 35 to
win more games against YaneuraOu on openings it was not trained on (+71%, bar set before the
result: 32).

Even games (self-play, wide vs stable network, same binary, 4 x 4 threads, 250 ms, SPRT
[-40, 0], `matches/s35/wide/even_*`): one FAIL, one PASS, two at the 300-game cap; pooled
**417-4-463, -18 +/- 23 Elo** over 884 games. Probably a little weaker in even games, not
established. Not installed: the trade (four-piece +71% wins vs YaneuraOu, even games ~-18) is
the user's call.

**Installed 2026-10-15 at the user's request**: `nnue/default.bin` is now `wide.bin` (every
engine that does not set `NNUEFile`, including `engines/usi-engine-stable`, loads it); the
Session 26 network is kept as `nnue/default-s26.bin`. Experiments that need the old network
must name it. The user's goal is even games against YaneuraOu; four-piece games are their
practice, so the wide target is a lead for even-game training, not a four-piece special.
