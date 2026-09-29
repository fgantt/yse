# NNUE Implementation Session Log

## Session 31: Four Bugs That Did Not Matter, and One Blind Spot That Does

**Date**: 2026-09-27 to 2026-09-28
**Phase**: Phase 2w
**Objective**: Session 30's *Next* list: a 1 s/move handicap ladder, the bugs Session 30 left
alone because they change play, and the evaluation's known biases. Every change gated on a
head-to-head against `engines/usi-engine-stable`.

> **READ THIS FIRST.**
>
> 1. **Nothing shipped.** Stable is the Session 30 engine (`98052b4`). Five candidates were
>    gated (seven matches, 840 games); none beat it. Their patches are in
>    `docs/nnue-phase2/patches/session31/`.
> 2. **The four "left as found" bugs are real and fixing them does not help.** The game-phase
>    quirk (two fixes), the quiescence dedup, and pseudo-legal quiescence moves: neutral, neutral,
>    worse, and worse over 240 games (−42 ± 42). See *The gates*.
> 3. **Our engine is blind to attacks on its own king, and so is YaneuraOu's evaluation.** A
>    user's six-piece loss turned on a move the engine still chooses at 60 s / depth 8 (+294 cp;
>    YaneuraOu −586). Across the user games, our error is ~9x larger when our king is under
>    pressure, always optimistic. **But YaneuraOu at depth 1 is exactly as optimistic on the
>    same positions (+0.141 vs our +0.140); it takes it ~10 plies to see the danger.** The
>    blind spot is search depth, not evaluation: Session 29's conclusion stands. An earlier
>    version of this log said "depth 3 barely helps, so it is eval" -- depth 3 was far too
>    shallow a test. See *It was depth after all*.
> 4. **`scripts/eval_gap.py` measures it in 6 seconds** (`66f44dd`), from 830 positions of the
>    user's games labelled by YaneuraOu. Use it before any evaluation change is gated.
> 5. **It is not mate threats, and not attacked squares.** Only 1 of the 61 close, pressured
>    positions has a forced (all-checks) mate for the opponent. The worst errors have few
>    attackers near our king, 4-5 defenders, and 1-4 enemy pieces in hand: slow drop attacks on a
>    solid-looking castle. A squared attack-unit term removes the average bias without hurting
>    quiet positions, but does not track which positions are wrong, and does not win a gate
>    (−8.7 ± 58.0 at scale 256, −43.7 ± 55.4 at 128).
> 6. **The handicap ladder rung is two-piece odds** at 1 s/move: 66% (26-1-13). Four-piece
>    (78%) and six-piece (79%, Session 30) are too easy to respond.

---

## The ladder at 1 s/move

`usi-stable` at `byoyomi 1000` against YaneuraOu at 50 nodes, 80 games per rung, our games as
Black (holding the odds) counted, `--random-plies 2`, seed 2910 (`matches/ladder_s31/`).

| rung | counted games | score |
|---|---|---|
| four-piece (`1nsgkgsn1/…`) | 30-2-8 | 78% |
| **two-piece** (`lnsgkgsnl/9/…`) | 26-1-13 | **66%** |

The four-piece run shared the machine with two gate matches; the two-piece run shared it with
one. Our engine's nodes per move depend on load, so compare later ladder runs under similar
load, or idle.

## The gates

All vs `usi-stable`, 120 games, `byoyomi 1000`, `--random-plies 8`, colour-swapped pairs,
seed 3001 unless noted. Patches are in the session scratchpad; none is committed.

| candidate | result | verdict |
|---|---|---|
| phase A: move orderer's phase from the real game ply, set once per search | 61-1-58, +8.7 ± 59.2 | neutral |
| phase B: no phase switching (default weights) | 52-0-68, −46.6 ± 63.4 | rejected |
| qdedup: quiescence keeps promotion twins, drops dominated P/B/R non-promotions, no duplicates | 58-3-59, −2.9 ± 48.5 | neutral |
| qlegal: quiescence promotions and checks filtered for legality | 50-1-69, −55.5 ± 60.2 | — |
| qlegal, seed 3002 | 53-4-63, −29.0 ± 59.4 | rejected (240 games: −42 ± 42) |
| king danger, scale 128 (see below) | 52-1-67, −43.7 ± 55.4 | rejected |
| king danger, scale 256 | 58-1-61, −8.7 ± 58.0 | neutral |

### Game phase

`initialize_advanced_move_orderer` passed the node count as `move_count`, so after a search's
60th node every position used the Endgame ordering weights. The phase only rescales quiet
moves within their tier (history against the castle / storm / positional bonuses); PV,
captures, promotions, killers and counter-moves are fixed tiers. Phase A (the intended
behaviour, via a process-wide game ply set in `handle_position`) is neutral. Phase B (default
weights: history 2500 against bonuses ~100) is worse, so the Endgame weights the engine
reached by accident are better than the defaults. Left as is.

### Quiescence dedup

The dedup removed only *adjacent* duplicates by (from, to, piece), ignoring the promotion flag.
A capture that checks is listed as a capture and as a check, and the two sort apart, so both
survived and were searched twice (test: `8e7c+` twice); a silver, knight or lance capture lost
one of its promotion twins. The fix (`quiescence_dedup_respects_promotion` fails without it)
is neutral in play.

### Quiescence legality

When not in check, quiescence took promotions and checks from the pseudo-legal list and plays
what it is given: a pinned piece could promote or give a discovered check and leave its own
king capturable. The fix costs nothing (depth 5 over 42 positions: 118k vs 117k nps, 3% fewer
nodes) and is worse over 240 games. A guess, untested: the bug cut both ways, and the
opponent's illegal checks in our quiescence made the search more pessimistic about our king,
offsetting the blind spot below. Retry once the evaluation sees king danger.

**Correct is not the same as better.** All four are plainly wrong in the code, and none is
worth Elo at 1 s/move. The user's games are lost elsewhere.

## A six-piece loss

The user played the stable engine against YaneuraOu with six-piece odds and lost in 71 moves
(`matches/user_games/2026-09-27_6mai_loss.txt`). YaneuraOu judging each of our moves at 3M nodes
(numbering is the game's; White moves first):

| move | played | YaneuraOu before → after | loss | YaneuraOu |
|---|---|---|---|---|
| 25 | 4e4d | 3138 → 2805 | 333 | 8g8f |
| 34 | 6h7i (king towards White's tokin) | 2578 → 1970 | 608 | 8i7g |
| 55 | 2d2f | 1243 → 827 | 416 | 3f4e |
| 57 | 2e2f | 864 → 408 | 456 | 2e1e |
| **63** | **6c5c** | **232 → −605** | **837** | 7h7g |

The same shape as Session 30's 77-move loss: +3,200 bled away over 60 moves, then one move
decided it. Move 63 trades the gold attacking White's king for a silver and hands White a gold
to drop beside our king (6h, with White's horse on 8i and tokins on 8h/7h). Our engine chooses
it at 10 s (+418) and at 60 s, depth 8, 10.8M nodes (+294). Depth does not fix it.

Our 1 s score against YaneuraOu's along the game: ~0.5x YaneuraOu's centipawns while winning
(the known compression), then flat at +430..+1,200 while YaneuraOu fell from +1,500 to +185 as
White's attack built (moves 46-63), and +420..+735 at moves 64-66 where YaneuraOu says −867 to
−2,365.

## A second six-piece loss

`matches/user_games/2026-09-28_6mai_game2.txt`, lost in 53. Its YaneuraOu review at 3M nodes
shows four of our drops (26 P\*5c, 36 S\*8d, 39 S\*7c, 46 G\*6b) each followed by a 600-1,000 cp
fall in the judge's next evaluation. **At 30M nodes all four are as good as YaneuraOu's own
choice** (S\*8d 2088 vs S\*5e 2042; G\*6b −60 vs 5h5g −153): the falls were the judge at 3M
misjudging sharp positions and correcting itself a move later. No single move lost this game
within a 30M-node judge's horizon.

What it does show is the blind spot again. Deep (30M) YaneuraOu against our 1 s score:

| moves | YaneuraOu | ours |
|---|---|---|
| 20-26 | +2,990 to +3,410 | +1,350 to +1,660 |
| 40-46 | +148 to −150 | **+915 to +1,344** |
| 47-48 | −1,101, −1,320 | +752, +800 |

A fixed-node judge's per-move loss is only as good as its horizon: before calling a move a
blunder in a sharp position, check it with a deeper search and `searchmoves`.

Added to the set (936 positions from seven games). The close subset grows to 89; with our king
under mid+high pressure 0.201 (depth 1) / 0.184 (depth 3), +0.14. Fourteen close positions are
tagged low pressure yet carry an error of 0.355, all optimistic: the pressure tag misses the
danger in them.

## The eval-gap set

`scripts/eval_gap_build.py` labels every position of `matches/user_games/*.txt` (both sides to
move) with YaneuraOu at 3M nodes, one thread, and tags each king from the board alone:
*pressure* = enemy pieces within two squares + enemy non-pawn pieces in hand (low 0-1, mid 2-3,
high >= 4). 830 positions from six games (one six-piece loss, five eight-piece games from
Session 30; the 595-move draw is cut to its first 120 moves).

`scripts/eval_gap.py ENGINE [--option NNUEFile=…] --depth 1` searches each position at a
fixed depth, single-threaded, from a fresh game, and compares in win probability,
`p = 1/(1+exp(-cp/410))` -- the trainer's `--sigmoid-eval-scale` for both sides, which is what
`--use-sigmoid-loss` trains to match. `--compare a.csv b.csv` is paired. Depth 1 takes 6 s.

Stable (`eval_gap/results/stable_d1.csv`):

| subset | n | mean abs dp, depth 1 | depth 3 | mean dp |
|---|---|---|---|---|
| our king, low pressure | 676 | 0.017 | 0.016 | 0.00 |
| our king, mid | 82 | 0.152 | 0.127 | +0.07 |
| our king, high | 72 | 0.160 | 0.136 | +0.12 |
| close (abs YaneuraOu < 1500), our king mid+high | 61 | 0.166 | | +0.09 |

In handicap games a close position is nearly always one where our king is under attack (61 of
66). **The subset to fix is the last row**; the pressure rows alone partly measure centipawn
scale in won positions. Session 30's linear-win-term networks cut the pressure rows from 0.160
to ~0.115 and leave the close subset at 0.19-0.20 -- they rescale won positions and lost
35-158 Elo. The handicap-data fine-tune (`x1`) moves the close subset 0.194 -> 0.189.

**Limits.** 61 close positions from three games, 44 from one. Neighbouring plies are
correlated, so the paired SE `--compare` prints is optimistic. Triage only; gates decide.

## What the blind spot is not

- **Not forced mates.** With White to move, `mate_solve` (1M nodes) finds a mate in 1 of the 61
  close, pressured positions. The errors are elsewhere.
- **Not attacked squares.** Over the 61, the per-position error correlates weakly with every
  board tag: enemy pieces near our king −0.23, enemy hand +0.17, enemy promoted pieces near
  +0.18, our adjacent defenders +0.24. The largest errors (dp +0.35 to +0.81) have 0-2 enemy
  pieces near our king, 3-5 defenders, and 1-4 enemy pieces in hand: drop attacks against a
  castle that looks solid, several quiet moves deep.

### A king-danger term (rejected)

`src/evaluation/king_danger.rs` (not committed; `patches/session31/king_danger.patch`): attack units on a king = 2 per adjacent square
the enemy attacks + the enemy's hand (2 for G/S/R/B, 1 for N/L); the term is
`scale * units^2 / 64` per king, added to the NNUE score; USI option `KingDanger`, default 0.
At 0 the eval-gap output is identical to stable. Scale chosen on the six-piece game, checked on
the two eight-piece losses:

| scale | fit: 6-piece close+pressure | test: 8-piece close+pressure | low pressure (all) |
|---|---|---|---|
| 0 | 0.114 (+0.045) | 0.301 (+0.211) | 0.017 |
| 64 | 0.117 (+0.035) | 0.293 (+0.202) | 0.017 |
| 128 | 0.121 (+0.025) | 0.286 (+0.191) | 0.017 |
| 256 | 0.131 (−0.000) | 0.269 (+0.171) | 0.017 |
| 512 | 0.187 (−0.066) | 0.244 (+0.139) | 0.016 |

It removes bias and leaves quiet positions alone, but on the game it was fitted to, the error
*rises* as the bias falls: it does not know which positions are dangerous. The gates agree:
scale 128 is 52-1-67 (−43.7 ± 55.4), scale 256 is 58-1-61 (−8.7 ± 58.0)
(`matches/s31_kd128_h2h.log`, `matches/s31_kd256_h2h.log`). A term that moves the average
without tracking the positions is not an improvement, which is what the eval-gap split
predicted.

## Training for it, and why it failed

The deployed network already has king-conditioned hand features: `KING_FEATURE_STRIDE`
repeats the whole perspective space, hand features included, per own-king bucket. (CLAUDE.md's
"hand features are not king-conditioned" predates the 9-bucket network.) Nor is the frozen
material path the cause: in the close, pressured positions Black is +2,067 in material and
the network already scores +820, taking back 60%. It takes back the same ~60% in won,
quiet positions; YaneuraOu's deep search takes back 74% in the first and 49% in the second.

The Session 30 handicap corpus holds 13,985 such positions (Black >= +1,000 material,
YaneuraOu < +1,500) but they were <1% of the fine-tuning mix. Two fine-tunes, Session 30
recipe (`weights/s31_kingattack/`, not in git):

| network | close + pressure (75) | close, low tag (14) | low pressure (752) |
|---|---|---|---|
| stable | 0.201 (+0.140) | 0.355 | 0.021 |
| S30 x1 | 0.199 (+0.100) | 0.344 | 0.024 |
| x4 (hc corpus x4) | 0.209 (+0.073) | 0.339 | 0.028 |
| bs20 (hc + those positions x20, 12%) | 0.229 (+0.036) | 0.326 | 0.030 |

The same as the king-danger term: the bias falls, the per-position error rises, quiet
positions get worse. Not gated.

## It was depth after all

YaneuraOu searching the eval-gap set's close positions at fixed depth, scored against its own
3M-node labels:

| searcher | close + pressure | close, low tag |
|---|---|---|
| ours, depth 1 | 0.201 (+0.140) | 0.355 (+0.355) |
| YaneuraOu, depth 1 | 0.157 (+0.141) | 0.307 (+0.302) |
| YaneuraOu, depth 3 | 0.137 (+0.123) | 0.284 |
| YaneuraOu, depth 6 | 0.119 (+0.101) | 0.282 |
| YaneuraOu, depth 10 | 0.057 (+0.038) | 0.183 |

A far stronger network is exactly as optimistic at depth 1. The danger in these positions is
found by search, around 10 plies, not seen by evaluation -- which is why a handcrafted term
and two fine-tunes all moved the bias without tracking the positions. Part of it is selection:
positions chosen for a *deep* score near zero are the ones where deep search disagrees with
the material, which makes any shallow evaluator look optimistic. Both engines face that
equally.

**What the eval-gap set is good for, then:** comparing evaluations against each other at the
same depth (ours is 0.201 against YaneuraOu's 0.157 at depth 1, a real but modest gap), and
the low-pressure rows. The close subset is a *search-depth* benchmark: judge search changes
by our error at a fixed time, not evaluation changes by our error at depth 1.

## Part 3: the profile, a mate the engine threw away, and Lazy SMP

### Profile of the shipped engine

`sample` on `usi-stable` (default parallel search) for 20 s on two positions from the user's
games: of 20 rayon workers, each was **asleep 52-85% of the time**. The root split searches the
first root move alone with the full window and splits the rest only after it, and only from
depth 5. In a position with full hands, SEE's `sliding_attackers` (Session 30's hanging-drop
test) was 41% of busy samples.

`YBWCEnable=true` (off by default) reached +0.21 ply at 1 s and lost its gate, 42-3-75
(−98.1 ± 59.0); its longest think was 1,606 ms against a 1,000 ms byoyomi.

### A checks-only mate the solver proved and then discarded (`00c03d5`)

In a user's win against Apery 2.1.0 (`matches/user_games/2026-09-28_even_vs_apery_win.txt`),
YaneuraOu finds a mate in 13 at move 43 in which every one of our moves checks; the engine
played 6f7d and mated at 51. df-pn *proved* the mate -- the root check 5c6c and every reply were
stored with pn = 0 -- but `mate_length`'s cycle guard memoised "no mate" for each position it
evaluated while a repetition was on the line, and `solve` returned no line. A first guess (the
graph-history interaction in df-pn itself) was implemented, did not fix the test, and was
reverted; the numbers in the table located the real fault. Now: mate 19 in 30 ms there, the same
62/73 on Session 30's mate positions, the same 83/936 on the user-game positions (rare). Gate:
61-0-59, +5.8 ± 66.6. **Committed to the branch, not installed**: it ships with the next change
that wins a gate.

### Three defects in the parallel search's transposition table

- **Workers searched without one.** When a shared table is set, stores go to it, but negamax
  probed the engine's own table -- which, for a worker, never received an entry. Only the move
  orderer read the shared table. (Fixing it alone: +0.06 ply at 1 s; with write-through,
  +0.09.)
- **Statistics serialise every access.** `performance_optimized()` enables statistics, and
  each probe and store then takes a table-wide mutex to bump a counter; replacing stores take
  two more (age stamp, replacement policy). Nothing in the search reads the counters.
- **Every node bumped shared atomics** (`GLOBAL_NODES_SEARCHED`, `GLOBAL_SEARCH_NODES`, the
  `DIAG_*` counters), whose cache lines bounce between CPU clusters.

Also: every `go` builds a new `ParallelSearchEngine` -- a new 20-thread pool and a new
4M-entry (~160 MB) shared table.

### Lazy SMP (rejected)

Persistent helper threads and table, the deepest completed iteration wins, a lock-free table
(key XOR data, depth-preferred for the same position), statistics off, node counters batched,
diagnostics counters gated. Patch: `patches/session31/lazy_smp_and_shared_table.patch`.

Time to depth 6, 8 positions: one thread 142 s; Lazy SMP 4 threads 66-78 s, 8 threads 78 s,
12 threads 92 s, 16 threads 123-185 s (so it was capped at 8). Depth at matched time was
0.10-0.15 ply below the shipped root split. **Gate: 35-0-85, −154.1 ± 73.0.** Diagnostics, 60
games at 1 s, same binary both sides: table only (no helpers) vs one thread −35 ± 98; full Lazy
SMP vs one thread +35 ± 85. So Lazy SMP as built plays about like one thread, and the shipped
root split is far stronger than both -- by something depth at matched time does not show.

### The engine persists `USI_Threads` for every launch

`setoption USI_Threads` writes `~/Library/Application Support/shogi-vibe/engine_prefs.json`,
read at startup by every engine process, the user's GUI engine included; parallel search is
enabled at startup only if the saved count is > 1, and a later `setoption USI_Threads 4` does
not re-enable it. A thread-count sweep left it at 4 (restored to 16) and silently made several
measurements single-threaded. Tools must set `SHOGI_PREFS_DIR`.

## Method notes

- **Depth at matched time did not predict strength for parallel search.** Lazy SMP was 0.10-0.15
  ply behind and lost 154 Elo. Gate parallel-search changes on matches, not depth.
- **Isolate engine preferences** (`SHOGI_PREFS_DIR`) in every tool that sets options.

- **Before blaming the evaluation, run the teacher at the same depth.** Four experiments this
  session (a king-danger term, its gate, two fine-tunes) chased an "eval" gap that YaneuraOu's
  own evaluation shares. The test took five minutes and should have come first.

- **A 120-game gate resolves about ±60 Elo.** Small correctness fixes land inside it. When a
  result is borderline, a second match on fresh openings (different `--seed`) is cheaper than
  arguing about it (qlegal: −55 then −29).
- **`eval_gap.py` depth 1 is deterministic and reproducible** (`KingDanger 0` gives a
  byte-identical CSV to stable). Depth 3 adds little for evaluation questions.
- **Check an explanation against the positions before building on it.** "Mate threats"
  was the natural reading of the six-piece loss; 1 of 61 positions supports it.

## Left as found

- The five gated candidates, as patches against `98052b4` in
  `docs/nnue-phase2/patches/session31/` (phase B is `phase_a_game_ply.patch` with its
  `update_ordering_phase` call removed). qlegal is worth retrying after the evaluation changes.
- `determine_game_phase`'s node-count input and `record_timing` (no callers) are still there.
- The engine prints `DEBUG: Bestmove recommendation …` on stderr on every move.

## Next

1. The root split's table defects, one gate each, on top of `00c03d5`: workers probing the
   shared table, statistics off, batched node counters, one pool and table per engine rather
   than per move. Then the idle workers.
2. Why the root split beats one thread by so much more than its depth gain -- the answer
   decides what parallel search should look like.
3. Search depth in king-attack positions (the close subset of `eval_gap.py`, at fixed time). The network sees piece squares relative to its own king's bucket but
   hand pieces without the king; drop attacks need both. Candidates: king-conditioned hand
   features, or more positions like the close subset (the Session 30 handicap corpus was mostly
   won positions). Judge each network on `eval_gap.py`'s close subset first.
4. More user games into `matches/user_games/` and a rebuild of the eval-gap set.
