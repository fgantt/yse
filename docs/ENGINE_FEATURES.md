# Yggdrasil Engine Features

What the engine does as shipped (`engines/usi-engine-stable`, `3bd5228`, 2026-10-13), where
each feature lives, and how it compares with YaneuraOu. Session logs
(`docs/nnue-phase2/SESSION_LOG_*.md`) hold the measurements behind each item.

## Search

The default search is the **lean search** (`src/search/lean.rs`, USI `SearchAlgorithm` Lean;
Classic selects the older `negamax` in `search_engine.rs`).

| Feature | What it does | Where | Notes |
|---|---|---|---|
| Iterative deepening | Searches depth 1, 2, 3 ... until time runs out | `IterativeDeepening::search` (`search_engine.rs`) | Partial last iteration can give the move |
| Principal variation search | First move full window, the rest with a null window and re-search on fail-high | `lean_node` | |
| Aspiration windows | Each depth starts with a window around the previous score | `IterativeDeepening::search` | USI `EnableAspirationWindows`, `AspirationWindowSize` |
| Lazy SMP | Helper threads search the same root and share the hash table; Stockfish 9 skip schedule; a deeper, higher-scoring helper result is played | `src/search/lazy_smp.rs` | Default since `56ce0aa`; USI `ThreadAlgorithm` Lazy/Split; `USI_Threads` (16) |
| Root/PV split (alternative) | YBWC split at the root and first PV plies, workers with own tables | `parallel_search.rs` | `ThreadAlgorithm` Split |
| Transposition table | Shared, lock-free stores; key stored as `hash ^ data` so torn reads miss; depth/age replacement | `thread_safe_table.rs`, `replacement_policies.rs` | `USI_Hash` default 256 MB (was 16 MB, which starved Lazy SMP) |
| Null-move pruning | Pass the move; if still >= beta at reduced depth, prune | `lean_node` | R = 3 + depth/3 + margin term; not in check, depth >= 3 |
| Reverse futility / razoring | Static-evaluation cut-offs near the leaves | `lean_node` | Razoring drops to quiescence |
| Late move pruning | Skip late quiet moves at depth <= 8 on non-PV nodes | `lean_node` move loop | Threshold 3 + depth^2 |
| Futility pruning | Skip quiet moves when static eval + margin <= alpha | `lean_node` move loop | depth <= 6 |
| SEE pruning | Skip quiet moves, drops and captures that lose material | `lean_node` move loop | depth <= 6; -80*depth quiet, -150*depth captures |
| Late move reductions | Late quiet moves searched shallower, re-searched if they fail high | `lean_node`, `lmr_table` | Log-log table; less at PV, more at cut nodes |
| Check extension | Checking moves searched a ply deeper (bounded per path) | `lean_node` | Since `3bd5228` not for checks with negative SEE, which are also prunable |
| Internal iterative deepening | Shallow search for a move at PV nodes with no table move | `lean_node` | depth >= 5 |
| Quiescence search | Captures and promotions to 8 plies, quiet checks at the first ply, all evasions in check | `lean_qsearch` | Delta and SEE filters; own direct-mapped table |
| Mate search | df-pn over checking sequences at the root; a proven mate is played at once | `src/search/mate_search.rs` | Session 30 |
| Repetition | Sennichite draws; perpetual check lost for the checker; superior/inferior hand repetition | `hand_repetition_score` | Session 32 fixed perpetual check scored as draw |
| Time management | Byoyomi and main time, adaptive allocation, move overhead | `time_management.rs` | USI `MoveOverhead`, `TimeSafetyMargin` |

## Move ordering

`src/search/move_ordering/`, called from `order_moves_for_negamax`. Every legal move is
generated and scored before the first is searched.

| Order | Moves |
|---|---|
| 1 | Hash-table move (passed as the IID move) |
| 2 | PV move |
| 3 | Captures, most valuable victim first |
| 4 | Promotions |
| 5 | Killer moves, then counter-moves |
| 6 | Quiet moves by history score and positional bonuses (castle, storm, flank) |
| last | Quiet moves and drops that hang the piece (quiet-move SEE, Session 30) |

Ordering is ~38% of search time in drop-heavy positions (Session 34 profile).

## Evaluation

| Feature | What it does | Where |
|---|---|---|
| NNUE | 256 -> 32 -> 1 network; king-relative (HalfKP-style), piece-in-hand and side-to-move features; incremental accumulator; exact integer SIMD forward pass | `src/evaluation/nnue.rs`, weights `nnue/default.bin` |
| Classical fallback | Piece-square tables, tapered opening/endgame, king safety, castle recognition | `src/evaluation/` |

## Other

- Opening book (`src/opening_book.rs`, embedded data), prefilled into the table.
- Micro endgame tablebase (`src/tablebase/`).
- Magic bitboards for sliding pieces (`resources/magic_tables/`; a binary that cannot find
  them falls back to ray casting, which chokes multi-threaded search).
- USI protocol with pondering; preferences (thread count) persist in
  `~/Library/Application Support/shogi-vibe/engine_prefs.json` (`SHOGI_PREFS_DIR` overrides).

## Compared with YaneuraOu

| YaneuraOu has | What it does | Our state |
|---|---|---|
| ~1,000k nodes/s per thread | | ~120k (8x slower) on drop-heavy positions |
| Staged move generation | Captures first, quiet moves only if needed; legality checked lazily | Full legal generation and full ordering at every node |
| Mate-in-1 in quiescence | Catches short mates without searching every checking drop | Quiet checks at quiescence ply 1, every evasion searched |
| Singular extensions | Extends a table move that is much better than all alternatives | None |
| ProbCut | Prunes when a shallow search already beats beta by a margin | None |
| Continuation history | History keyed by the previous moves | Plain history and counter-moves |
| Larger, better-trained network | Trained on billions of positions | Trained on ~67k-155k positions; misjudges attacks on our king |

## Feature audit (2026-10-13)

Each feature re-checked for bugs, after the 16 MB table turned out to have hidden Lazy SMP's
value. Details in `SESSION_LOG_034.md`.

| Feature | Finding |
|---|---|
| Transposition table replacement | **Bug**: `increment_age` was never called, so depth alone decided; once full, a slot's first deep entry refused later ones and same-position updates were dropped. Fix: search generations (`new_search`), fresh entries replace stale ones. 16 threads, 10 s/move: +0.7-0.9 ply. Gating |
| Time management | Design issue: 1/30 of main time + byoyomi per move front-loads (95 s on move 1 of 60 min + 10 s; ~30 s by move 50). Not yet tested |
| NNUE incremental updates | Sound: equal to a full refresh on 31,451 moves of three handicap games (`tests/nnue_incremental_games.rs`) |
| Aspiration windows | Sound: +-200 as tuned (the USI option's 25 does not drive the lean search) |
| Null move | Sound (`search-diag` printed the wrong counter, fixed) |
| History heuristic | Sound: decays every 1,000 updates |
| Repetition | Sound; minor: repetitions across a null move are still detected |
| Opening book | Not used in handicap games (exact-position lookup, even-game book) |
| Mate search | Sound: ~10 nodes/ms budget, our checking mates only |
| Lazy SMP | Fixed earlier this session (table size); result selection to review |
