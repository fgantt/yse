# NNUE Implementation Session Log

## Session 30: Nodes Per Second

**Date**: 2026-09-24
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

## Left as found

Each of these would change play, so none belonged in a speed commit:

- **`determine_game_phase` is fed the node count as `move_count`.** After a search's 60th
  node the move orderer uses the Endgame strategy weights, whatever the position.
- **`generate_checks` does not test legality**, and quiescence may search what it returns.
- **The quiescence dedup ignores the promotion flag**, so a promoting and a non-promoting
  version of the same move collapse to whichever sorts first.
- `record_timing` in move ordering has had no callers since `f86e09c`.
- **`bestmove` does not always match the last reported PV** under parallel search (seen once
  as `bestmove 3e5c` after a final `pv 9d9e`, and once between two mating moves). Something
  after the root split can override its result.

## Next

1. Re-take the six-piece handicap ladder: the engine is both faster and now able to mate, so
   Session 29's 50% rung describes a different engine.
2. Find what overrides the parallel search's `bestmove`.
3. Profile `usi-engine` again and take its top item.
4. The game-phase quirk, judged by the move-loss script, not by speed.
