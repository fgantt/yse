# NNUE Implementation Session Log

## Session 22: Search Harness and Search Audit — NNUE Has Never Chosen a Move

**Date**: 2026-09-18
**Phase**: Phase 2p
**Objective**: Session 21 Priority 0 — find out why bake-offs run at ~2k nodes/s and mostly depth 2,
and, at the user's prompt, audit the gameplay algorithms (search and evaluation) the way Session 21
audited the NNUE feature spaces.

> **READ THIS FIRST.** In the current search, **NNUE does not affect which move is played.** Quiescence
> search scores every leaf with the classical tapered evaluator, which never consults NNUE. NNUE reaches
> the search only through the static eval used for pruning. Measured: pure-classical, POV-NNUE and
> baseline-NNUE searches return **identical best moves and identical scores on 28/28** depth-bound
> positions at depth 2. Every NNUE bake-off since Session 5 therefore measured NNUE's effect on pruning
> and timing, not its quality as an evaluator. No NNUE training result can show up in play until
> Task 9 below is fixed.

---

## Headline Results

1. **NNUE never reaches leaf evaluation.** `quiescence_search_with_hint` scores leaves with
   `PositionEvaluator::evaluate_with_context`, whose implementation (`evaluation.rs` ~714–845) has no
   reference to NNUE or to the integrated evaluator. NNUE is reached only via `evaluate()` →
   `evaluate_position`, used for the pruning/LMR static eval in `search_move_with_lmr`, the root, and
   time-out fallbacks.

   Empirical check, `search-bench`, 30 corpus positions, depth 2 (28 depth-bound):

   | comparison | identical score | identical move |
   |---|---|---|
   | classical vs POV NNUE | 28/28 | 28/28 |
   | classical vs baseline NNUE | 28/28 | 28/28 |
   | POV NNUE vs baseline NNUE | 28/28 | 28/28 |

   Node counts differ slightly (32,926 / 33,093 / 33,088), so NNUE is active — only in pruning, and
   never enough to change the result. This explains every NNUE-vs-NNUE bake-off coming back null, and
   why Session 21's held-out gain (0.449 → 0.641) did not appear in play.

2. **The search uses two incompatible evaluators.** On the same 16 corpus positions:

   | | `evaluate()` → IntegratedEvaluator | `evaluate_with_context()` → tapered |
   |---|---|---|
   | range | −6,673 … **+31,547** cp | −3,784 … +2,757 cp |
   | colour-symmetric | **no** — 15/16 differ from mirror by 10–521 cp | yes — 16/16 |
   | sign vs teacher on #3 / #7 / #10 | **wrong** | right |

   A near-opening position (teacher +160) scores +1,169 through `evaluate()`; ordinary middlegames
   score 13,000–31,000 cp, the range usually reserved for forced mates. Pruning margins computed from
   that static eval are meaningless, and scores from the two evaluators are mixed in one tree — a
   likely mechanism for the odd/even swing Session 21 measured (+553 cp classical, +1,210 NNUE).

3. **The search is not colour-symmetric.** Search score differs from the colour-mirror's on 2/16
   positions at depth 1 and 5/16 at depth 2, by up to 805 cp.

4. **NNUE `evaluate()` ignores its `player` argument and can return stale values.** It calls
   `evaluate_incremental(board)`, which returns the score for the accumulator's own `current_stm` and
   refreshes only when `needs_refresh` is set. Evaluating 16 unrelated positions in sequence returned
   **387 for every one**. The search depends on every board change going through the NNUE make/unmake
   hooks. Null-move search does not: it recurses with `player.opposite()` on an unchanged board, so the
   static eval at a null-move child comes from the wrong side's perspective.

5. **Magic bitboards have never been used.** No `BitboardBoard` constructor attaches a magic table
   (`new_with_magic_support` is never called), and `MagicTable::new()` fails after ~195 s with
   `GenerationFailed { square: 4, Rook }`. The shared-table path silently falls back to an empty
   default table. Every slider attack has gone through ray-casting.

---

## Harness Performance Fixes (commit `7504994`)

Profiling a live search (macOS `sample`) showed the time going to overhead, not search:

| # | Defect | Fix |
|---|---|---|
| 1 | `GlobalOptimizer::bit_scan_forward` etc. built a new `BitScanningOptimizer` per call — `Mutex` alloc/init/lock/destroy to bump an unread counter | integer intrinsics; equivalence test on ~8k bitboards |
| 2 | 135 `debug_log(&format!(..))` sites: the no-op function's `format!` argument is still evaluated, so `make_move`, `place_piece`, `remove_piece`, `is_square_attacked_by` built strings on every call | rewritten to the existing lazy `debug_log_fast!`/`trace_log_fast!` |
| 3 | Slider attacks through an LRU cache whose `get` pushes onto its queue on every hit and never trims it — unbounded growth plus SipHash | uncached ray walk (cache key was exact, so results unchanged) |
| 4 | Per-depth info thread slept in 100 ms slices; `join` waited out the remainder — ~100 ms dead time per depth | `park_timeout` + `unpark` |

**Verification**: new `search-bench` binary (fixed-depth search, diffable fingerprint of move / score /
nodes) against a clean build of `cf7637e`. 30 positions at depth 2: identical on all 28 depth-bound
positions, **2.5× faster**. The other 2 are in check with ≤10 replies, where the search overrides the
caller's budget with a 2 s / 5 s cap, so their result depends on machine speed; `search-bench` tags them
`TIME-BOUND(check)`.

After the fixes the profile is dominated by real search work — quiescence is ~43% of search time.
Remaining overhead: `Vec` growth in hot loops, and very frequent clock reads.

**Correction to `CLAUDE.md`**: it says `verbose-debug` "string formatting fully compiled out when off".
That was false for these 135 call sites until this commit. It also describes magic bitboards as the
slider implementation; they have never been used (Finding 5).

---

## Tests Added

- `tests/search_semantics.rs` — oracle tests: static-eval colour symmetry, search colour symmetry at
  depths 1 and 2, and **`search_uses_one_evaluation_function`** (classical and NNUE). The four
  defect-encoding tests are `#[ignore]`d with reasons; each is the acceptance test for its fix.
- `tests/magic_equivalence.rs` — magic attacks must equal ray-cast attacks for every slider, square
  and occupancy. `#[ignore]`d until `MagicTable::new()` succeeds; it is the gate for enabling magic.
- `bitboards::integration::tests::global_optimizer_matches_adaptive`.

Pre-existing, unchanged by this session: 6 `bitboards::` test failures (identical at `cf7637e`), and
`popcount_performance_comparison`, a wall-clock benchmark that flakes under load.

---

## Open Tasks, in Priority Order

**Task 9 — Make the search use one evaluation function.** The blocker for everything NNUE. Until
leaves are scored by the evaluator under test, no NNUE improvement can change a move. Gate:
`search_uses_one_evaluation_function`. Decide which evaluator is canonical for classical play — the
tapered one is colour-symmetric and closer to the teacher.

**Task 11 — NNUE `evaluate()` perspective and staleness.** Honour `player`; fix the null-move hook; audit
the unhooked `make_move` in `search_at_depth` (3 makes, 2 hooks). Must land with Task 9, since once NNUE
scores leaves, a stale or wrong-perspective accumulator is directly in the result.

**Task 10 — IntegratedEvaluator scale and symmetry.** Possibly moot if Task 9 retires it from the
search, but its asymmetry needs explaining. Gate: `static_eval_is_colour_symmetric`.

**Then re-run Session 21's Arm A.** It is the first bake-off that could actually measure an NNUE
difference.

**Also open**: Task 5 (time budget not enforced inside an iteration), Task 8 (magic tables), the odd/
even swing (re-measure after Task 9 — it may disappear with the mixed evaluators).

---

**Session Lead**: Claude (AI)
**Status**: Harness overhead fixed (2.5×, behaviour-identical). Search audit found that NNUE has never
influenced move choice, that the search mixes two incompatible evaluators, and that the classical
static eval is asymmetric and mis-scaled. No gameplay fixes made yet — these change engine behaviour
and need a decision on which evaluator is canonical.

---

## Update — Single Evaluator Landed (commit `72eadab`)

**NNUE now scores every node the search evaluates.** `PositionEvaluator::evaluate` is the search's only
evaluation function: NNUE for the requested player when enabled, otherwise the tapered classical
evaluator with fixed context flags. Quiescence goes through it. The IntegratedEvaluator is opt-in.

| search-bench, 30 positions, depth 2 (28 depth-bound) | same best move | same score |
|---|---|---|
| classical vs POV NNUE | **3/28** (was 28/28) | **0/28** (was 28/28) |
| classical vs baseline NNUE | 3/28 | 0/28 |
| POV vs baseline NNUE | 9/28 | 0/28 |

**A second, worse bug surfaced on the way — board corruption on unmake.** `make_move_with_info`
recorded the captured piece unpromoted (right for the hand), and `unmake_move` restored that field. So
capturing a tokin, horse or dragon and unmaking put a pawn, bishop or rook back on the board. Every
search has corrupted its own board for the later siblings of such a capture, classical and NNUE alike.
The NNUE hook removed the wrong feature from the same field, which is what made the new verification
mode report 336,528 incremental-vs-scratch mismatches in one depth-2 search. `MoveInfo` now carries
`captured_on_board`. Gate: `tests/board_semantics.rs`.

After both fixes:
- NNUE incremental accumulator equals a from-scratch evaluation at **every** evaluated node (0
  mismatches), with an occupancy-based safety net that rebuilds if a board change ever bypasses the
  hooks.
- Classical static eval colour-symmetric 16/16 (was 1/16).
- Search colour symmetry: 1/16 residual (59 cp depth 1, 7 cp depth 2, same move) — down from 2/16 and
  5/16 with up to 805 cp. The board corruption was the main source.

**Speed is now the binding problem.** One depth-2 search made ~846,000 evaluations; 30 positions at
depth 2 take 6–9 minutes. Cause, from `MoveGenerator::generate_quiescence_moves`: at *every* quiescence
ply (max 8) it searches all captures, **all checks including drops**, all promotions, and — when fewer
than 16 moves — quiet "tactical threat" moves. That is close to a second full-width search. Standard
practice is captures and promotions at every ply, checks at most at the first, never quiet moves.

**Deployment consequence**: `usi-engine` auto-loads `nnue_weights_trained.json` (Session 3,
colour-absolute) with no feature flags, and it now scores every leaf. Weight files carry no feature-set
metadata. Task 12.

---

## Update — Rules, Quiescence, Harness, Deployment

Continuing in the agreed order (bounded quiescence → bake-off → deployment) surfaced defects below
the evaluation layer. Every game this engine has played, including every bake-off, was affected.

### Shogi rules in move generation (commit `afc7343`)

Found because a mate-in-1 test failed: the king was allowed to capture a pawn-defended gold.

| Defect | Cause |
|---|---|
| **Knights moved and attacked backwards** | attack table used row +2 for Black, which advances toward row 0 |
| **Dragons and horses moved one step, like kings** | generators read them from static tables, which can only hold the steps |
| **Pawns and lances never attacked** — no pawn/lance checks, kings could enter/capture defended squares | `AttackTables` had no entry for them; fell through to an empty set |

Three pre-existing `bitboards::` failures pointed at this and were never followed up; they encoded
wrong geometry and are corrected. **Perft from the start position now matches the published values
at depths 1–5** (30 / 900 / 25,470 / 719,731 / 19,861,490 — depth 5 is the first with drops), and
`attacks_match_move_destinations_for_every_piece` covers every piece, colour and square.

### Bounded quiescence (same commit)

Quiescence searched every capture, every check including drops, and quiet threats at every ply.
Now captures and promotions at every ply, checks at the first ply only, no quiet moves; in check it
searches all evasions (no stand-pat) and returns mate when there are none. 30 positions at depth 2:
classical 381 s → 20 s, POV NNUE 137 s → 12 s. Mate-in-1 is found at depth 1.

### Dropped pieces never left the hand (commit `0750ec4`)

Game loops added captures to the hand but never removed dropped pieces, so hands only grew and a
piece could be dropped indefinitely. A live bake-off game was found holding ~15 bishops.

- **elo-tester** — every bake-off in this project was played with inexhaustible drops.
- **usi-engine `position ... moves`** — after a real game's first drop the engine believes the piece
  is still in hand and can play an illegal phantom drop.
- Also the online trainer's self-play, three tablebase solvers, check-detection temp boards, and
  search_integration's PV extension (which also credited captures to the wrong player).

All now use `BitboardBoard::make_move_with_hand`. `corpus-gen` was already correct, so the training
corpus is unaffected. `elo-tester` now asserts 40-piece conservation after every move.

**Consequence: every bake-off result recorded in Sessions 5–22 is void.** They were played under
wrong piece movement, wrong check rules, and unlimited drops, by both sides.

### NNUE deployment (commit `3ca91da`)

- **Self-describing weight files** (v2: magic header + `NNUEFeatureSet` + weights). Loading one
  configures the evaluator; legacy files still load, without metadata. The trainer writes v2;
  `nnue-convert-weights --feature-set` stamps existing files.
- **USI options** `UseNNUE` and `NNUEFile` (default `nnue/default.bin`). A network is used only if
  its file declares a feature set; otherwise classical evaluation, with the reason on `isready`.
- **`setoption USI_Hash` silently dropped NNUE** (it rebuilds the search engine); now re-applied.
- **Parallel workers searched with classical evaluation** while the main thread used NNUE (fresh
  evaluators in `ParallelSearchEngine` contexts and YBWC thread-local engines). Workers now get the
  main engine's network, keyed by network id.

### The network barely values material

`eval-correlation` scores evaluators against the teacher on the trainer's held-out games:

| held-out | pearson[clamp] | spearman[raw] | slope vs teacher |
|---|---|---|---|
| classical (tapered) | +0.644 | +0.580 | 0.42 |
| POV network | +0.641 | +0.593 | 0.33 |
| baseline network (stm+hand) | +0.471 | +0.420 | 0.25 |

The POV network ties the classical evaluator on held-out correlation. But removing one opponent piece:

| piece | classical gain | network gain |
|---|---|---|
| pawn | +130 | **+1** |
| lance | +296 | +47 |
| rook | +988 | **+191** |
| dragon | +1,302 | +423 |
| promoted lance | +506 | **−25** |

The corpus is 67K positions from strong-engine games, where material rarely hangs, so the network
learned to rank quiet positions but not what pieces are worth. The search spends most of its time in
positions right after captures, where that is fatal: in the pre-harness-fix match the network lost
1–17 to classical at fixed depth 2. (That match was on the broken harness; re-run in progress.)

This is the main obstacle to a full NNUE engine. Options, for a decision:
1. **Material-residual training** — the network predicts `teacher − material`, and evaluation is
   `material + network`. Exact material values for free; the network spends its capacity on position.
2. **PSQT bypass** — the modern NNUE design (HalfKAv2): a linear piece-square path summed into the
   output, initialised to material values and trained with the rest.
3. **More, and more varied, training data** — positions from our own search (after captures, with
   imbalances) labelled by the teacher. Needed eventually regardless; `corpus-gen` exists.

### Still open

Task 5 (time budget overrun inside an iteration), Task 8 (magic tables), Task 10 (IntegratedEvaluator,
now opt-in), Task 13 (search speed: depth 3 ~6 s/position; `go depth 3` reported depth 4), residual
1/16 search asymmetry at depth 2, 3 pre-existing tablebase checkmate-detection test failures.
