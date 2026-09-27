# NNUE Implementation Session Log

## Session 30: Nodes Per Second

**Date**: 2026-09-24 to 2026-09-27
**Phase**: Phase 2w
**Objective**: Act on Session 29's conclusion that per-node cost is the binding constraint:
start with the NNUE forward pass, then take whatever the profile says is next.

> **READ THIS FIRST.**
>
> 1. **The shipped engine now searches ~4x the nodes at 1 s/move, +0.68 ply.** Nine speed commits,
>    every one behaviour-preserving: its fixed-depth fingerprint is identical to the commit
>    before it (one exception, explained in item 4), and the move-generation changes also pass
>    perft to depth 5 and match YaneuraOu's legal-move lists on 6,741 positions.
> 2. **It plays better: +117 ± 72 Elo** against the pre-session engine, 1 s/move, 120 games
>    (76-7-37). Neutral-set move loss leaned the same way but could not resolve it.
> 3. **Profile the engine you ship.** Three commits worth +38% in single-threaded `search_bench`
>    moved the parallel engine's nodes at 1 s/move from 90k to 92k. A profile of `usi-engine`
>    itself found a different top item, and fixing that moved it to 111k.
> 4. **A fixed-depth search here still reads the clock.** IID decisions use elapsed time, so a
>    pure speedup changed one slow fingerprint position's result. Proven not to be the change.
> 5. **The default (parallel) search never played a mate in one.** A root move that mated was
>    ranked worst. Found from a user's handicap game drawn at move 595 from a won position;
>    fixed in `79f4380`. Every game result before that commit carries the defect.
> 6. **Part 2: nothing ships without a head-to-head win.** After an unvalidated build reached
>    the user's GUI and lost a game, every change was gated on 100-120 games against the current
>    stable engine. Six changes passed and are in `engines/usi-engine-stable`: root PV
>    splitting, a corrected SEE with hanging-drop ordering and pruning, a ±200 aspiration
>    window, and a df-pn mate search (each +55 to +86 Elo against its predecessor).
> 7. **The evaluation rewards collecting promotions in a corner** of a won position (~200 cp
>    above moves YaneuraOu rates equal). The sigmoid loss cannot see it; a linear loss term fixes
>    the bias and costs 35-158 Elo. Not deployed. Open.
> 8. **The user's GUI and our tools must never share an engine binary**, and the engine must
>    not depend on its working directory. Both cost games before they were found.

---

## Where the time went

Baseline, single-threaded `search_bench --depth 5` over 20 corpus positions with
`nnue/default.bin`: 17,758 nps. Every result below comes from three interleaved A/B pairs of
the previous and new binary on 8 positions, because separate runs drift by ±2-5%: one early
comparison showed no gain from a change the paired runs put at +8%.

| commit | what | nps (paired) |
|---|---|---|
| `94149fe` | NEON kernel for NNUE layer 2 | +29% |
| `f86e09c` | stop timing every scored move (5 clock reads per move) | +8% |
| `a316d25` | one move list per node instead of five | +12% |
| `3cef556` | skip legality/check tests whose answer is known | +15% |
| `e771c4f` | count tactical threats without generating every move | +18% |
| `c5238ca` | assess IID complexity only when a decision reads it | +20% |
| `2344ce6` | Fx hash for the history tables | +2% (one pair a tie) |
| `505ab2b` | skip move-orderer phase inputs nothing reads | +12% |
| `a623be3` | cheaper quiescence captures, promotions and checks | +13% |

The paired ratios compound to about 3.3x per node, single-threaded. The 20-position total went
17,758 -> 41,321 nps (2.3x); the difference is position 16, whose search changed after
`e771c4f` (see *Method notes*) to a tree that runs at ~11k nps and dominates the total time.

### The forward pass compiled to scalar code

The Session 29 profile put the NNUE forward pass at 31%. The layer-2 loop (512 x 32
multiply-accumulates, i64 so that one shift at the end is exact) compiled to one scalar
`smaddl` per weight plus a bounds check, with no SIMD at all. Rewriting it as fixed-size rows
did not help: LLVM will not vectorise a 64-bit multiply on NEON, even though `smlal`
(widening i32 x i32 -> i64 multiply-accumulate) is exactly the operation. An explicit
intrinsic kernel keeps the same arithmetic, so evaluation is bit-identical. Forward pass:
~28% of busy samples -> 7.4%. x86 still takes the scalar path.

### Most of the rest was work whose result nothing used

- **`count_tactical_threats`, ~30% of search time.** Generated every legal move for both
  sides to count captures and promotions. Now generates board moves and tests legality only
  for captures and promotions. Exact.
- **`should_apply_iid` -> `assess_position_complexity`, ~14%.** A full position analysis ran
  first, at every node, although the depth and TT-move gates reject most nodes without it.
- **`initialize_advanced_move_orderer`, ~9%.** An evaluation and a check-counting legal-move
  generation per node, feeding `determine_game_phase` — which reads them only while its
  `move_count` is 20..=60, and `move_count` is the node count (see *Left as found*).
- **`score_move`'s timers, ~7%.** Five clock reads per scored move, measuring a
  sub-microsecond operation in whole milliseconds: every sample recorded zero.

### Move generation

- **Five lists per node, now one.** A HashSet and an unread HashMap per drop generation, two
  piece lists grown from empty, a fresh Vec from the sliding generator, and a `collect` for
  the legality filter, each copied into the next.
- **Two attack tests per pseudo-legal move, ~23%.** Unless a side is already in check, a move
  can expose its own king only by moving it or vacating a square on a line through it, and can
  check the opponent only by landing where some piece could reach its king or vacating a line
  square. Other moves take neither test. `MoveTestFilter` is tested against the full test on
  every pseudo-legal move along random games.
- **Quiescence**, the top item in the parallel engine's profile (see below): checks played
  every pseudo-legal move — every drop included — on a fresh clone of the board and the hands.

**A diagnosis that did not survive.** The NNUE accumulator snapshots (a heap `Vec` clone per
make, twice with dual perspectives) looked like the allocator's main customer. The profile put
them under 0.5%; move generation was ~6%.

## Profile the engine you ship

After `505ab2b` the single-threaded benchmark had gained 38% over three commits, and the
shipped engine at 1 s/move had not moved: median nodes 90k -> 92k, mean depth 4.26 both times.
`sample` on `usi-engine` during a 30 s `go byoyomi` search, on a position with full hands,
showed `generate_quiescence_moves_bounded` as the top self-time item by far — barely visible
in `search_bench`'s position mix. Fixing it (`a623be3`) moved the shipped engine to 111k.

Every candidate after this should be chosen from a profile of the shipped engine.

## Depth at 1 s/move, as shipped

`scripts/depth_ab.py`: default options (parallel search), `byoyomi 1000`, the 34 positions the
`search_bench` stride yields, old and new alternating per position.

| engine | mean depth | median nodes |
|---|---|---|
| before this session (`8376f77`) | 3.74 | ~28k (21-32k across runs) |
| after all nine commits | **4.41** | **111k** |

Deeper on 21 positions, same on 13, shallower on none. The gain matches the tree's growth rate:
with ~6x more nodes per ply, 4x buys log(4)/log(6) = 0.77 ply. **Each further ply costs ~6x**,
so from here speed work pays in fractions of a ply.

## Strength

### Neutral-set move loss

The Session 28-29 tooling and position file were not on disk (they lived in `/tmp`).
`scripts/move_loss.py` rebuilds the measurement: 150 positions evenly spaced over the held-out
teacher games (`nnue_corpus_yaneura_d10.jsonl`, `game_id % 20 == 0`), each engine at
`byoyomi 1000`, YaneuraOu depth 10 judging the played move with `searchmoves`, scores clamped
to ±3000 cp.

| run | pre-session mean | current mean | paired (current − pre) |
|---|---|---|---|
| 1 | 168 | 156 | −11 cp (SE 20) |
| 2 (order swapped) | 177 | 157 | −20 cp (SE 16.5) |

Both runs favour the current engine by about one SE; the live positions (|score| < 2000, 114
of 150) say the same. Consistent with a gain, not evidence of one.

**Not comparable with earlier logs.** Medians here are ~0 against Session 29's 80-120 cp: this
scores the judge's own best move as exactly 0 and the played move from the same search, and
~55% of moves match the judge. The old figures probably scored the child position with a
separate search, which gives even the best move a nonzero loss.

### Head-to-head, 1 s/move

`matches/s30_speed_h2h.csv`: current (`a623be3`) vs pre-session (`8376f77`), 120 games,
`byoyomi 1000`, seed 3001, 8 random opening plies in colour-swapped pairs.

**76 wins, 7 draws, 37 losses: +117.2 ± 71.9 Elo** (paired over 60 openings).

The first strength gain the 1 s/move gate has confirmed since the network plateau sessions,
and it came from search speed with the evaluation untouched. Both engines in this match still
had the mate-in-one defect below, so it measures the speed work alone.

## The engine could not finish a won game

A user played YaneuraOu with 8-piece odds and drew: winning from around move 60, the game ran
to move 595 while YaneuraOu shuffled a tokin between 9h and 9i. Replaying it, YaneuraOu finds a
forced mate for us in 73 positions, **58 of them mate in one** (`6d6c+` stood available from
move 72 to 107), and the engine played none of them.

With `ParallelEnable false` the engine plays every one of those mates; `search_bench` (single
threaded) scores them 99999 at every depth. The fault is the parallel root split:
`search_root_child` makes a root move and calls `search_at_depth` on the child. On a checkmated
child there is no move to return, `search_at_depth` answers `None`, and the worker reported the
root move at `i32::MIN / 2` — the worst possible score. The engine took a longer mate, and next
turn declined the new mate in one the same way.

`79f4380` scores a game-ending root move as the single-threaded search does (mated one ply from
the root; 0 if not in check), and rebases the workers' other mate scores by one ply, which also
fixes mates being reported one short (`mate 4` for a mate in 5). After it, the engine mates in
all 58 mate-in-one positions of that game, each verified with YaneuraOu, and from YaneuraOu's
mate in 5 onward it reports and follows the mate. `parallel_search_plays_mate_in_one` fails
without the fix and passes with it.

Every match played with the default parallel search before `79f4380` — this session's
head-to-head, the Session 29 ladder, anything using `usi-engine` defaults — lost won games this
way. Its size on the ladder is unknown and worth measuring: the handicap rungs are exactly the
games we should be winning.

---

# Part 2: user games, and a head-to-head gate

From here the work was driven by handicap games the user played against YaneuraOu (8-piece
odds, 60 min + 10 s byoyomi, default engine settings), and every change had to beat the current
stable engine head-to-head before reaching the user.

## The ladder, re-taken

Six-piece odds against YaneuraOu at 50 nodes, 40 counted games (ours as Black) per run:

| our engine | score |
|---|---|
| pre-session, 20k nodes | 38% (12-6-22) |
| with the mate fix, 20k nodes | 34% (12-3-25) |
| **with the mate fix, 1 s/move** | **79% (30-3-7)** |

Session 29's 50% on this rung came from 20 games; the node-limited rung is really near 35%,
and a node budget hides every speed gain. At the shipped time control the rung is too easy:
a 1 s/move ladder should start at four-piece odds or fewer.

## What the user's games showed

Each game was replayed with YaneuraOu judging every one of our moves (3M nodes, 30M for the
tactical stretches) and our engine re-run at the critical positions.

- **Lost game, 77 moves.** +3,500 for 60 moves without progress: YaneuraOu's move was `2g2f`
  (the standard rook-pawn attack) nearly every turn and we never played it. Our evaluation rose
  from +1,403 to +1,973 over the shuffling -- as much as along YaneuraOu's own winning line --
  because every far-corner promotion scored 100-430 cp above YaneuraOu's preferred move. Then
  moves 61-70 lost it: `P*4c` (-714 cp) and `S*4b` (-1,521). With 20-30 s per move the search
  still stopped at depth 5, taking 18.3M nodes for it.
- **Lost game, 43 moves.** Played with an uncommitted experimental build that had overwritten
  `target/release/usi-engine` (see *Delivering the engine*).
- **Won game, 58 moves.** No move worse than ~20 cp for 33 moves, where earlier games lost
  700-1,500 cp to single drops. YaneuraOu saw a forced mate from move 35 that took until 58;
  at move 41 we let it slip for two moves.
- **Won game, 47 moves.** Forced mate from move 34, delivered at 47.

## Every change now passes a head-to-head

Neutral-set move loss at 10 s/move could not settle 30-50 cp differences (paired SD ~400 cp
over 150 positions; two runs of the same comparison disagreed by 48 cp), so matches decided:

| change (alone, on top of the stable build) | games, time | result |
|---|---|---|
| root PV splitting (`c2a735d`) | 100 + 100, 2 s | +35 ± 73, then +74 ± 76 (115-1-84 combined) |
| SEE fix + hanging-drop ordering (`ea6b48d`) | 120, 1 s | **+76.5 ± 64.6** |
| hanging-drop pruning (`74d197c`) | 120, 1 s | **+82.6 ± 70.9** |
| aspiration ±200 (`9518fdd`) | 120, 1 s | **+82.6 ± 67.5** |
| df-pn mate search (`98052b4`) | 120, 1 s | **+85.7 ± 62.5** |
| network, linear win term 0.3 | 120, 1 s | −157.7 ± 61.7 |
| network, linear win term 0.1 | 60, 1 s | −64.4 ± 81.7 |
| network, handicap data only | 60, 1 s | −34.9 ± 96.3 |

Two matches ended 73-2-45; the game records were compared and share no game. Gains measured
against a predecessor do not add up, and self-play gains shrink against other opponents.

## The parallel root was not alpha-beta

The parallel root searched every root move at once, in move-generator order, each with the
full window and no knowledge of the others' scores: no root move could be refuted cheaply by a
good one found earlier. Now the moves are ordered (previous best first), the first is searched
alone with the full window, and the rest are scouted in parallel with a zero window against a
shared best score, re-searched only when they beat it. On the lost game's move-64 position with
a 20-minute clock, depth 5 fell from 18.3M nodes / 27.6 s to 2.2M / 5.5 s, and 30 s reached
depth 7. At 10 s/move mean depth +0.42; at 1 s/move nothing, since the parallel root takes over
only from depth 5.

The same work fixed the info stream (`b5c3ba4`): three printers, two of them timed from the
start of an iteration, and every PV read from the TT root, which can hold a failed aspiration
line. The "bestmove differs from the last PV" item in *Left as found* was this display bug; the
move played always was the search's result.

## SEE was wrong about which pieces attack

`find_attackers_defenders` had pawns, lances and knights moving the wrong way for both colours
and gave golds, silvers and promoted minors a king's eight squares. It now uses the board's
attack tables. `see_quiet` extends the exchange to quiet moves and drops (minus what the
opponent nets by taking the piece), which orders a hanging drop below every other quiet move
and, at depth <= 2 on zero-window nodes, prunes it. The lost game's `S*4b` was played 6/6 times
by the pre-session engine and 0/6 by the current one.

## Aspiration windows

Scores move by hundreds of centipawns between depths in sharp positions, so ±50 failed and
re-searched at most depths. Depth 5 over 15 positions: ±100 about the same as ±50, **±200 11%
fewer nodes and half the re-searches**, ±400 and no window both worse.

## Mate search

`src/search/mate_search.rs`: df-pn over checking sequences, run at the root with ~10 nodes per
ms of move time (5k-200k); a proved mate is played at once. Plain df-pn could not solve a mate
in 9 with pieces in hand on both sides in 8M nodes; with a check's proof number initialised to
the defender's number of replies and the 1+ε threshold trick it takes 160. Against YaneuraOu on
every forced-mate position from three user games: every mate whose line is all checks is solved
(58 mates in one, mates of 3-15 plies); every unsolved one has a non-checking move in
YaneuraOu's own line, so many of the long "forced mates" in these games are out of its reach by
design. `mate_solve` solves SFENs and reports which moves of a line give check.

## The evaluation's false gradient (open)

Corner-promotion bias -- our score of each far-corner promotion minus our score of YaneuraOu's
preferred move, over 9 moves of the lost game, where YaneuraOu rates them equal -- is **+196 cp**
for the deployed network. Why training does not correct it: the material path is frozen (a
tokin is 540 cp anywhere), and at sigmoid scale 410 a +2,000 and a +3,500 position both target
~1.0, so centipawn values in won positions are unconstrained while the search reads them raw.

- Handicap data (`scripts/handicap_corpus.sh`: 155k positions from 1,200 games at 2-8 piece
  odds, our engine playing, YaneuraOu labelling): bias +191 (x1), +185 (x4).
- `--win-linear-weight` adds `w * ((t - out) / 1000)^2 / 2` on centipawns beyond ±1,500:
  bias +151 / +105 / **+49** at w = 0.03 / 0.1 / 0.3 -- and −158 Elo at 0.3. It rescales won
  positions (our +1,400 towards YaneuraOu's +3,500), which the search's margins are not tuned
  for, and costs fit near equality.

Untried: constraining only differences between sibling moves in won positions, or letting only
the material path learn per-square values.

## Delivering the engine

Three separate faults kept the user from playing the engine that had been measured:

- **Experimental builds overwrote the GUI's binary.** The GUI runs
  `target/release/usi-engine`; a plain `cargo build` for an experiment replaced it, and a game
  was lost to an unvalidated build. Experiments now build with `CARGO_TARGET_DIR` in the
  scratchpad; `engines/usi-engine-stable` holds the validated engine, `-prev` the one before.
- **The network was found only relative to the working directory** (`67debfe`). GUIs start
  engines from their own directory, where `nnue/default.bin` did not resolve and the engine
  silently played with classical evaluation. It now also resolves against the executable.
- **macOS killed the engine at launch** (SIGKILL, exit 137; the GUI reported "did not respond
  with 'usiok'"). Overwriting a binary in place always does this. Beyond that, a file launched
  by one application was refused when another launched it -- first the GUI after our matches,
  later our matches after the GUI -- while an identical copy under another name ran. Our tools
  now use private copies and never launch the GUI's file. The mechanism is unconfirmed (the
  system log is not readable here).


## Method notes

- **A fixed-depth fingerprint can change from a speedup alone.** After `e771c4f`, fingerprint
  position 16 (the slowest, ~55 s) went from −29 / 1,419,202 nodes to 99992 / 590,686. It is
  deterministic per build. A build computing the old and new counts on every call and
  asserting they matched never fired, and — slower, otherwise identical — reproduced the old
  result. IID depth and IID skipping read elapsed time relative to the limit even under
  `search_bench`'s 600 s budget. The 99992 "mate" itself was not investigated and may be a bug.
- **Single A/B runs are not enough.** Run-to-run drift is ±2-5%; interleave pairs.
- **Every behaviour-preserving claim is checked against the previous commit's fingerprint**,
  not the session baseline, so position 16 does not mask anything later.
- **Move loss over 150 positions cannot gate a change worth 30-50 cp.** Use 100-120-game
  matches against the current stable engine; 2 s/move for changes that act only from depth 5.
- **A 1 s/move match is internally fair under background load**; two matches can run at once.

## Left as found

Each of these would change play, so none belonged in a speed commit:

- **`determine_game_phase` is fed the node count as `move_count`.** After a search's 60th
  node the move orderer uses the Endgame strategy weights, whatever the position.
- **`generate_checks` does not test legality**, and quiescence may search what it returns.
- **The quiescence dedup ignores the promotion flag**, so a promoting and a non-promoting
  version of the same move collapse to whichever sorts first.
- `record_timing` in move ordering has had no callers since `f86e09c`.
- The mate search's reported length is the shortest mate *in its proof tree*, not always the
  shortest mate (one position reports 15 plies in the engine where `mate_solve` finds 9).

## Next

1. The evaluation's corner-promotion bias, by a route that does not rescale won positions:
   sibling-difference constraints, or a trainable material path.
2. A 1 s/move handicap ladder against YaneuraOu, starting at four-piece odds, as the
   benchmark that tracks the user's games.
3. Profile `usi-engine` again and take its top item.
4. The game-phase quirk, gated by a head-to-head.
