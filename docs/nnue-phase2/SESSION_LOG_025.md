# NNUE Implementation Session Log

## Session 25: Search and Time Management — and Where the Strength Actually Goes

**Date**: 2026-09-19
**Phase**: Phase 2s
**Objective**: After a lost game against YaneuraOu, find and fix what costs playing strength:
time management, then search correctness, measured against YaneuraOu rather than against ourselves.

> **READ THIS FIRST.** The engine's search machinery is **not** the binding constraint any more; the
> **evaluation** is. With our network, move quality stops improving after depth 2 — for the engine and
> for a plain reference alpha-beta alike (`search_oracle`). YaneuraOu at **depth 1** picks better moves
> (median loss 52 cp) than our engine at depth 3 (91 cp). Fix evaluation before adding search features.

---

## Measuring tools added

* **`usi-match`**: plays two USI engine binaries with paired openings (each opening twice, colours
  swapped), a real clock, legality and game-end adjudication by our move generator, a paired-CI Elo,
  each side's longest think, and a warning for any move over 2 s. This is how strength is measured
  now; `elo-tester` can only compare evaluators inside one process at fixed depth.
* **`search_oracle`**: plain alpha-beta, captures-only quiescence, same evaluation and move generator,
  no TT/pruning/reductions/extensions/aspiration. Separates "search machinery is wrong" from
  "evaluation is wrong".
* **`search_bench`**: `--sfen` / `--moves`, per-depth tree counters (nodes, moves, cutoffs),
  quiescence/null-move/LMR counters, first-move cutoff rate. `SHOGI_DIAG_ASPIRATION` prints the
  aspiration window of every root search.
* **`order_probe`**: prints the search's move order for a position with each scoring component.
* Analysis scripts (in the job's tmp dir, worth re-creating if needed): per-move loss judged by
  YaneuraOu depth 10; a mate-in-1/3 suite built from our own games.

## Time management (commits `70281dd`, `a333927`, `a0a85dd`, `07462c3`)

1. **A hard-coded 60 s limit.** Iterative deepening replaced the caller's limit with 60 s whenever
   depth was unlimited — which is what the USI engine always does. Every game move took up to a minute.
2. **The deadline check did almost nothing.** `TimeManager::should_stop` looked at the clock every
   1024th call and reset its counter *even when the limit had passed*, so it reported the deadline to
   one node and "not yet" to the next 1023. Real games: 23–35 s moves at 1 s byoyomi. Once exceeded it
   now reports true on every call.
3. **Unfinished work could become the move.** Aspiration fail-low/fail-high adopted the failed
   search's move (only a bound), and a search cut off by the clock had its partial result accepted.
   The bishop-losing `8h3c+` in the user's game was a depth-4 fail-low result.
4. **Short time controls reached only depth 1**: a flat 500 ms reserve before starting an iteration.
   Now 5% of the budget (5–500 ms).
5. **`go` limits**: `depth`, `movetime`, `nodes`, `infinite`, `binc`/`winc` were all ignored.
   Allocation is now ~1/30 of the mover's main time + increment + byoyomi, capped at a quarter of main
   time on top of those, less `MoveOverhead` (new option, default 100 ms). `go nodes N` is enforced in
   the time manager and runs single-threaded.
6. **`stop` during a search**: stdin is read on its own thread, so `stop`/`quit` reach a running
   search (`go infinite` never returned before). A `stop` read before its `go` is dispatched is not
   lost.

Measured: `depth 3` 88 ms, `movetime 500` 382 ms, `nodes 5000` 66 ms, `infinite`+`stop` 1551 ms,
60 s+1 s inc 2878 ms (target 2900), 10 s+1 s byoyomi 1214 ms (1233), byoyomi 1 s 871 ms. In matches the
longest think is now ~1.0–1.1 s at 1 s byoyomi (was 35 s).

## Search correctness and cost (commits `9c1aec2`, `b770b50`, `5181e37`, `2bdabf6`)

* **Move ordering was inverted for captures.** A capture was scored by its SEE alone, so taking a
  bishop in an even trade (SEE −70) ranked below quiet moves with castle bonuses: in a test position
  the captures were 27th–29th of 32 and only 29–40% of cutoffs came from the first move. Ordering is
  now tiered (PV > captures by MVV-LVA > promotions > killers > counter-moves > quiet heuristics).
  SEE is not used for ordering: it scores some even exchanges as heavily losing (−700 for pawn takes
  pawn recaptured by a rook) — **SEE itself is still suspect and unaudited**.
* **Killers and history never reached the orderer.** The search updated its own two-slot killer list
  and a from-square-only history table, not the tables the move orderer reads. Now wired, with drops
  included (they were skipped entirely for lack of a source square).
* **Repetition detection corrupted scores.** Every searched node was added to a game-wide history
  that was never popped (last 1,000 visits), so any position visited 4 times in the recent search —
  which iterative deepening and transpositions guarantee near the root — scored as a four-fold
  repetition draw. Repetition is now detected on the current search path.
* **The root never stopped on a fail-high.** After an aspiration pass's first move scored ≥ beta, the
  remaining root moves were searched with alpha ≥ beta (an inverted window) and their replies searched
  every move. A 151-move position: depth 2 1.64 s → 0.18 s, depth 3 3.5 s → 0.72 s.
* **Aspiration re-searches were unbounded** and rebuilt the window around the *previous iteration's*
  score, so an unstable search could fail low, high, low… until the clock ran out. Now the window
  widens around the returned score with a doubling margin, full width after `max_researches`.
* **Pruning was disabled below depth 2** — i.e. never at the frontier, where futility pruning belongs.
  Enabled from depth 1. For this to be safe, `gives_check` is now set by the legal move generator
  (it was only ever set in quiescence's check generator).
* **Check extension** (bounded per path by the iteration depth): games are decided by mating attacks,
  and a mate in 3 against us needed depth 6+ to see.
* **Attack detection rewritten** (~40% of search time): `is_square_attacked_by` iterated every
  attacking piece and computed its attack set (sliders by uncached ray casting). It now works
  backwards from the target square, one pattern per piece type. Verified against the old version
  (kept as `is_square_attacked_by_reference`) on every square of 40 random games.
* Per-node waste removed: the node's static evaluation and game phase were recomputed for every move
  (twice, counting the LMR path); legality testing cloned the board per move; NNUE evaluation
  allocated a Vec per call.

Benchmarks (8 corpus positions, depth 3): **9.3 s → 3.3 s**, same move choices; depth 4 finishes in
seconds on positions that previously did not finish at all.

Corrections to earlier notes: null-move pruning **does** work (41% cutoff rate; the earlier "0 cutoffs"
was a wrong counter in `search_bench`), and LMR applies rarely simply because most nodes are shallow.

## Strength: where it actually goes

All matches: our engine 1 s/move, YaneuraOu node- or depth-limited, paired openings.

| Opponent | Result (12–24 games) |
|---|---|
| YaneuraOu, 50 nodes/move | 1–3 wins in 24 |
| YaneuraOu, 150 / 400 nodes | 1 in 12 / 1 in 12 |
| YaneuraOu, **depth 1** | 2 in 24 |

Every loss is by checkmate; none on time. Diagnosis, in order:

1. **Mate-finding is fine.** A suite built from our games: mate in 1 found **68/68**, mate in 3
   **48/53** (the misses are positions where only depth 2 was reached in 1 s).
2. **Games are lost strategically, early.** Judged by YaneuraOu depth 10, our moves lose on average
   **305 cp** (median 84); 36% of moves lose >200 cp; in plies 8–30 the average loss is 188 cp. Games
   fall below −1000 by ply 19–41; the mates at the end are consequences.
3. **Depth does not help.** Move loss by depth (150 positions, judged at depth 10):

   | depth | engine mean / median | reference (`search_oracle`) mean / median |
   |---|---|---|
   | 1 | 265 / 104 | 336 / 123 |
   | 2 | 253 / 94 | 258 / 86 |
   | 3 | 222 / 91 | 259 / 82 |

   The engine and the plain reference agree on 100–132 of 150 moves (median score difference 0–15 cp),
   so the search machinery is not corrupting results. **YaneuraOu at depth 1: median loss 52 cp.**
4. **The evaluation is weaker on our own games than on its training distribution**: pearson 0.698 /
   spearman 0.770 on positions from our games, against 0.80 on the corpus's held-out games. Its scale
   is compressed (slope 0.26 vs the teacher) and material is worth about half what it should be
   (pawn 50, gold 361, rook 447 cp).

## Next

1. **Train on our own games** (`corpus-gen --player-engine`, added this session): our engine plays,
   the teacher labels every position at depth 10. 12,000 self-play games are being generated; mix with
   the 9.9M-position corpus and retrain the 9-bucket king network.
2. Re-measure with the same three yardsticks: correlation on game positions, move loss by depth, and
   `usi-match` against YaneuraOu at 50 nodes.
3. Search work that remains worthwhile: SEE is unaudited and looks wrong; the NNUE forward pass is
   ~30% of search time; two benchmark positions still expand to millions of nodes at depth 4.
