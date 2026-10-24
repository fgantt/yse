# Yggdrasil Engine Features

What the engine does as shipped (`main`: history move ordering 2026-10-16, network `nnue/default.bin`
= Session 36 `p9` 2026-10-20), where
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
| Transposition table | Shared, lock-free stores; key stored as `hash ^ data` so torn reads miss; a generation per search, fresh entries replace stale ones, else depth/bound | `thread_safe_table.rs`, `replacement_policies.rs` | `USI_Hash` default 256 MB (was 16 MB, which starved Lazy SMP) |
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
| NNUE | 256 -> 32 -> 1 network; king-relative features with 9 king buckets, piece-in-hand and side-to-move features; incremental accumulator; exact integer SIMD forward pass. Trained on 249M YaneuraOu self-play positions labelled at depth 8 | `src/evaluation/nnue.rs`, weights `nnue/default.bin` (Git LFS) |
| Classical fallback | Piece-square tables, tapered opening/endgame, king safety, castle recognition | `src/evaluation/` |

## Other

- No opening book. The old embedded book never loaded (wrong JSON layout) and was removed in
  October 2026; `USI_OwnBook` is accepted but has no effect until a new book exists.
- Micro endgame tablebase (`src/tablebase/`).
- Magic bitboards for sliding pieces (`resources/magic_tables/`; a binary that cannot find
  them falls back to ray casting, which chokes multi-threaded search).
- USI protocol with pondering; preferences (thread count) persist in the OS config directory
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
| Larger, better-trained network | Trained on billions of positions | 249M positions; still misjudges drop attacks on our king in handicap games |

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
| Repetition | Sound; minor: repetitions across a null move are still detected |
| Opening book | None (the old one never loaded; removed) |
| Mate search | Sound: ~10 nodes/ms budget, our checking mates only |
| Lazy SMP | Fixed earlier this session (table size); result selection (deeper and higher-scoring helper) reviewed, sound |
