# Yggdrasil Engine Features

What the engine does as shipped (`main`: search `4dfe9c4` + the 2026-11-09 council-review fixes,
network `nnue/default.bin` = `a1`, 2026-11-01), where
each feature lives, and how it compares with YaneuraOu. Session logs
(`docs/nnue-phase2/SESSION_LOG_*.md`) hold the measurements behind each item.

## Search

The default search is the **lean search** (`src/search/lean.rs`, USI `SearchAlgorithm` Lean;
Classic selects the older `negamax` in `search_engine.rs`).

| Feature | What it does | Where | Notes |
|---|---|---|---|
| Iterative deepening | Searches depth 1, 2, 3 ... until time runs out | `IterativeDeepening::search` (`search_engine.rs`) | A cut-off iteration is discarded with Lazy SMP (the split's partial-iteration use does not apply); a fixed 50 ms comes off the move's time on top of `MoveOverhead` |
| Principal variation search | First move full window, the rest with a null window and re-search on fail-high | `lean_node` | |
| Aspiration windows | Each depth starts with a window around the previous score; the root is fail-soft, so a fail-low widens from the true bound | `IterativeDeepening::search`, `search_at_depth` | USI `EnableAspirationWindows`; `AspirationWindowSize` does not drive the lean search |
| Root stack | Root LMR for late quiet moves, `lean_hist` root ordering, root TT flag against the original window, helper aspiration windows, correction history on the quiescence stand-pat, helpers keep their quiescence table | `search_at_depth`, `lean.rs` (`root_stack`) | `SHOGI_ROOT_STACK` bit set, default 63; Session 40 gate +73 |
| Lazy SMP | Helper threads search the same root and share the hash table; Stockfish 9 skip schedule; a deeper, higher-scoring helper result is played | `src/search/lazy_smp.rs` | Default since `56ce0aa`; USI `ThreadAlgorithm` Lazy/Split; `USI_Threads` (16) |
| Correction history | Static eval corrected by learned per-thread tables (pawn structure, pieces in hand, opponent's last move), before it drives pruning; learns only from quiet best moves and non-mate, non-hand-repetition scores | `src/search/lean_corr.rs`, `corr_learnable` | Default since Session 40; `SHOGI_LEAN_CORR=0` off; weights `SHOGI_LEAN_CORR_W` |
| Per-game search tables | History, correction and quiescence tables persist across a game's moves on every thread (Lazy SMP helpers park theirs by id) and clear on `usinewgame`; killers at plies 1-2 clear each move | `lean_hist.rs` (`park`/`unpark`, `GAME_EPOCH`, `new_move`), `lean.rs` (`with_qtable`) | Session 40; the quiescence table and root killers since 2026-11-09 |
| Tunable search constants | Pruning margins and LMR divisors, defaults from SPSA run v1 | `src/search/lean_params.rs`, `scripts/spsa.py` | `SHOGI_LEAN_PARAMS=name=v,...` |
| Root/PV split (alternative) | YBWC split at the root and first PV plies, workers with own tables | `parallel_search.rs` | `ThreadAlgorithm` Split |
| Transposition table | Shared, lock-free stores (since 2026-11-09; the bucket lock before guarded nothing); key stored as `hash ^ data` so torn reads miss; a generation per search, fresh entries replace stale ones, else depth/bound; one entry per slot | `thread_safe_table.rs`, `replacement_policies.rs` | `USI_Hash` default 256 MB is an upper bound: the largest power of two of 24-byte entries that fits (192 MB at 256; before 2026-11-09, 96 MB) |
| Null-move pruning | Pass the move; if still >= beta at reduced depth, prune | `lean_node` | R = 3 + depth/3 + margin term; not in check, depth >= 3 |
| Reverse futility / razoring | Static-evaluation cut-offs near the leaves | `lean_node` | Razoring drops to quiescence |
| Late move pruning | Skip late quiet moves at depth <= 8 on non-PV nodes | `lean_node` move loop | Threshold 3 + depth^2 |
| Futility pruning | Skip quiet moves when static eval + margin <= alpha | `lean_node` move loop | depth <= 6 |
| SEE pruning | Skip quiet moves, drops and captures that lose material | `lean_node` move loop | depth <= 6; -86*depth quiet, -171*depth captures (Session 40 SPSA; was 80/150) |
| Late move reductions | Late quiet moves searched shallower, re-searched if they fail high | `lean_node`, `lmr_table` | Log-log table; less at PV, more at cut nodes |
| Check extension | Checking moves searched a ply deeper (bounded per path) | `lean_node` | Since `3bd5228` not for checks with negative SEE, which are also prunable |
| Internal iterative deepening | Shallow search for a move at PV nodes with no table move | `lean_node` | **Off** (`SHOGI_LEAN_IID=1`): until 2026-11-09 it re-entered its own position, hit the repetition test and never searched; fixed but never measured |
| Quiescence search | Legal captures and promotions to 8 plies, quiet checks at the first ply, all evasions in check; every node below the root tested for the superior/inferior hand rule, repetition and perpetual check (also at the ply cap; the hand rule first, as in the main search) and kept on both paths | `lean_qsearch`, `generate_quiescence_moves_bounded` | Delta and SEE filters; own direct-mapped table. Cache entries record the plies they had left; results that met a repetition are not cached. Before 2026-11-09 promotions and checks were pseudo-legal (an illegal check could score as mate), a checking capture was searched twice, and no repetition was seen |
| Mate search | df-pn over checking sequences at the root; a proven mate is played at once | `src/search/mate_search.rs` | Session 30; bounded by nodes, a quarter of the move's time and `stop` |
| Repetition | Sennichite draws; perpetual check lost for the checker; superior/inferior hand repetition, also against the last 16 game positions; both in the main search and in quiescence; below a null move, no further back than it | `path_repetition`, `hand_repetition_score`, `rep_floor` | Session 32 fixed perpetual check scored as draw; hand repetition across the root since 2026-11-09 |
| Time management | Byoyomi and main time, adaptive allocation, move overhead | `time_management.rs` | USI `MoveOverhead`, `TimeSafetyMargin` |

## Move ordering

The lean search orders with `src/search/lean_hist.rs` (default since 2026-10-16;
`SHOGI_LEAN_HIST=0` restores the older general orderer in `src/search/move_ordering/`). Every
legal move is generated and scored before the first is searched.

| Order | Moves |
|---|---|
| 1 | Hash-table move |
| 2 | Captures with SEE >= 0, and promotions other than silver |
| 3 | Two killer moves per ply |
| 4 | Counter move |
| 5 | Quiet moves by history: per side (origin square or dropped piece x target), plus continuation history one and two plies back |
| 6 | Quiet moves and drops that hang the piece |
| last | Captures with SEE < 0 (ordered by capture history) |

History entries move toward +/-16,000: a reward for the move that cut off, a penalty for the
same-kind moves tried before it. First-move cutoffs rose from 79% to 88% (Session 35).

## Evaluation

| Feature | What it does | Where |
|---|---|---|
| NNUE | 256 -> 32 -> 1 network; king-relative features with 9 king buckets, piece-in-hand and side-to-move features; incremental accumulator; exact integer SIMD forward pass. Network a1 (Session 39): trained from scratch with the GPU trainer (`trainer/`) on `mix_a1`, 1.14B quiet positions: our own games labelled by YaneuraOu at depth 8 (222.6M quiet) plus 200 files of nodchip's depth-9 Hao set (913.5M quiet); lineage in `docs/nnue-phase2/DATA_LINEAGE.md` | `src/evaluation/nnue.rs`, weights `nnue/default.bin` (Git LFS) |
| Classical fallback | Piece-square tables, tapered opening/endgame, king safety, castle recognition | `src/evaluation/` |

## Other

- Opening book (`src/book.rs`): YaneuraOu text `.db` format, binary-searched on disk, White's
  positions looked up flipped; every book move checked against our legal moves. Off by default
  (`BookFile no_book`); `BookEvalDiff`, `BookMaxPly`, `USI_OwnBook`. Used only in games from the
  even start position and not for `go infinite`. See `docs/REPOSITORY.md` for the Peta-shock book.
- Micro endgame tablebase (`src/tablebase/`): **off in games** since 2026-11-09 (`EnableTablebase`,
  default false). A hit plays its move without a search, and the rook and silver solvers claimed
  wins against a full army; they now require a bare enemy king and empty hands.
- Magic bitboards for sliding pieces (`resources/magic_tables/`; a binary that cannot find
  them falls back to ray casting, which chokes multi-threaded search).
- USI protocol with pondering (since 2026-11-09: `ponderhit` gives the search the move's time from
  then, and a pondering search holds its `bestmove` until `ponderhit` or `stop`; the engine sends no
  ponder move itself); `position ... moves` stops at an illegal move; preferences (thread count) persist in the OS config directory
  (`~/Library/Application Support/shogi-vibe/engine_prefs.json` on macOS; `SHOGI_PREFS_DIR`
  overrides).

## Compared with YaneuraOu

| YaneuraOu has | What it does | Our state |
|---|---|---|
| ~1,080k nodes/s per thread | | ~470k counting every node (USI reports quiescence nodes since Session 35; the earlier "8x" compared unequal counts) |
| Staged move generation | Captures first, quiet moves only if needed; legality checked lazily | Full legal generation and full ordering at every node |
| Mate-in-1 in quiescence | Catches short mates without searching every checking drop | Quiet checks at quiescence ply 1, every evasion searched |
| Singular extensions | Extends a table move that is much better than all alternatives | Implemented, off (`SHOGI_LEAN_SE_DEPTH`); screens showed no gain |
| ProbCut | Prunes when a shallow search already beats beta by a margin | Implemented, off (`SHOGI_LEAN_PROBCUT=1`); no gain |
| Continuation history | History keyed by the previous moves | Yes, since 2026-10-16 (`lean_hist.rs`) |
| Larger, better-trained network | Trained on billions of positions | 1.14B positions (a1); a stronger teacher's labels changed nothing measurable (Session 40, N4) |

## Feature audit (2026-10-13)

Each feature re-checked for bugs, after the 16 MB table turned out to have hidden Lazy SMP's
value. Details in `SESSION_LOG_034.md`.

| Feature | Finding |
|---|---|
| Transposition table replacement | **Bug**: `increment_age` was never called, so depth alone decided; once full, a slot's first deep entry refused later ones and same-position updates were dropped. Fixed (`a115260`): search generations (`new_search`), fresh entries replace stale ones. 16 threads, 10 s/move: +0.7-0.9 ply; SPRT +191 PASS; four-piece vs YaneuraOu 68.3% -> 83.3% |
| Time management | Design issue: 1/30 of main time + byoyomi per move front-loads (95 s on move 1 of 60 min + 10 s; ~30 s by move 50). Not yet tested |
| NNUE incremental updates | Sound: equal to a full refresh on 31,451 moves of three handicap games (`tests/nnue_incremental_games.rs`) |
| Aspiration windows | Sound: +-200 as tuned (the USI option's 25 does not drive the lean search) |
| Null move | Sound (`search-diag` printed the wrong counter, fixed) |
| History heuristic | Sound: decays every 1,000 updates |
| Repetition | Sound; minor: repetitions across a null move were still detected (fixed 2026-11-10: `rep_floor`, as Stockfish stops at the last null move) |
| Opening book | Off by default. Peta-shock book vs no book, 4-thread 250 ms self-play from startpos (2026-10-24): +3.8 Elo, 95% [-12, +20] over 1,738 games; screen FAIL, so the 1 s SPRT was not run. Games stay in book a median 12-16 plies |
| Mate search | Sound: ~10 nodes/ms budget, our checking mates only; since 2026-11-09 also bounded by a quarter of the move's time and by `stop` |
| Lazy SMP | Fixed earlier this session (table size); result selection (deeper and higher-scoring helper) reviewed, sound |
