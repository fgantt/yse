## Relevant Files

- `docs/design/gameplay-stability/ENGINE_GAMEPLAY_STABILITY_PLAN.md` - Source PRD outlining stability heuristics and validation goals.
- `src/evaluation/component_coordinator.rs` - Aggregates evaluation terms; needs wiring for castle progress, storm alerts, and initiative scoring.
- `src/evaluation/castles.rs` & `src/evaluation/king_safety.rs` - Implement castle progression metrics, pawn shield checks, and king safety bonuses.
- `src/evaluation/opening_principles.rs` & `src/moves.rs` - Track opening debt, redundant major-piece moves, and SEE safeguards at move generation time.
- `src/search/move_ordering/mod.rs`, `src/search/iterative_deepening.rs`, `src/search/quiescence.rs` - Adjust move ordering, blunder filters, and search biases to prioritize safe development.
- `src/opening_book.rs`, `src/opening_book/validation.rs`, `src/ai/openingBook.json` - Enforce king-first bans and vetted template coverage inside the opening book tooling.
- `tests/evaluation/castle_progress_tests.rs` (new) - Unit tests for castle/king-safety scoring behaviors.
- `tests/search/stability_regressions.rs` (new) - Regression harness for storm responses, redundant move penalties, and SEE guardrails.
- `tests/self_play/fens/storm_regressions.fen` (new) - Targeted FEN suite for △8七歩成 and ▲4四角?? scenarios.
- `benches/simd_performance_benchmarks.rs` & `benches/simd_nps_benchmarks.rs` - Performance guardrails after heuristic changes.
- `docs/design/performance-analysis/AI_ENGINE_ANALYSIS.md` - Document the empirical impact of gameplay stability changes.

### Notes

- Keep new evaluation penalties soft-scaled and allow tactical principal-variation overrides to avoid suppressing creative play.
- Cache storm-state and castle-progress indicators where possible so evaluation + move ordering stay SIMD friendly.
- Co-locate new regression tests beside existing evaluation/search test modules, following the repo’s pattern of pairing `*.rs` source files with `*_tests.rs` companions.

## Tasks

- [x] 1.0 Integrate stability heuristics into the evaluation layer
  - [x] 1.1 Audit `component_coordinator.rs` and existing feature weights to map insertion points for castle progress, redundant move tax, storm pressure, initiative bonuses, and SEE checks.
  - [x] 1.2 Implement castle progression scoring in `castles.rs`/`king_safety.rs`, including configurable milestones (simple gold castle, yagura shell) and decay once opponent storms begin.
  - [x] 1.3 Track opening debt and redundant major-piece moves by extending `opening_principles.rs` plus a lightweight history buffer in `moves.rs`; surface penalties when the same piece moves twice without gain.
  - [x] 1.4 Add pawn-storm detection signals (file advance counters, time-since-response) and feed them into evaluation penalties/bonuses.
- [x] 1.5 Extend `KingSafetyConfig`/`OpeningPrincipleConfig` so castle progress, storm pressure, redundant-move, and opening-debt heuristics have explicit weight knobs for future tuning.
  - [x] 1.6 Update evaluation weight configs/tuning defaults so new terms get sensible initial values and can be tuned later.

- [x] 2.0 Enforce disciplined opening templates and king-safety policies
  - [x] 2.1 Review `openingBook.json` plus generation scripts to enumerate approved templates (static rook, ranging rook, disciplined Ureshino).
  - [x] 2.2 Encode “no king-first / no early rook swings” constraints inside `opening_book.rs` and validation routines so illegal sequences are filtered during book compilation.
  - [x] 2.3 Inject heuristic priors (e.g., bonuses for `▲7六歩`, `▲2六歩`) into `opening_principles.rs` to mirror the curated book even when out-of-book.
  - [x] 2.4 Document the updated template rules and regeneration procedure in `docs/design/gameplay-stability/` and cross-link from opening-book docs.

- [ ] 3.0 Update search and move-ordering logic for stability awareness
  - [ ] 3.1 Adjust `move_ordering/mod.rs` bonuses so castle-progressing and storm-response moves rise in the ordering queue.
  - [ ] 3.2 Bias iterative deepening (`iterative_deepening.rs`) toward lines that satisfy development milestones before move 12, reducing effort on king-first continuations unless they already score ≥ +150cp.
  - [ ] 3.3 Enhance `quiescence.rs` and late-move pruning to revisit nodes where SEE indicates a self-destructive capture or where promoted-pawn threats exist.
  - [ ] 3.4 Wire initiative/attack debt metrics into search statistics so offensive pressure translates into selective deepening when coordination is present.

- [ ] 4.0 Build pawn-storm and attack-response framework
  - [ ] 4.1 Define storm-state tracking structs (consecutive pushes, file ownership, time since response) accessible to evaluation and search layers.
  - [ ] 4.2 Expand drop heuristics (pawn/gold drops such as `▲8八歩打`, `▲7八金`) so they trigger automatically when storm severity crosses a threshold.
  - [ ] 4.3 Implement initiative tracking that recognizes climbing silver, edge attacks, and prepared pawn breaks; ensure scoring kicks in only after development prerequisites are met.
  - [ ] 4.4 Add “attack debt” penalties that escalate when the engine amasses attacking resources but fails to convert within a configurable ply window.

- [ ] 5.0 Expand validation and performance safeguards
  - [ ] 5.1 Capture FENs for △8七歩成 and ▲4四角?? critical positions; craft targeted self-play suites verifying protective responses.
  - [ ] 5.2 Run 200-game self-play batches per canonical opening, tracking castle completion rates and redundant-move frequencies.
  - [ ] 5.3 Extend evaluation/search unit tests (`tests/evaluation/castle_progress_tests.rs`, `tests/search/stability_regressions.rs`) to cover new penalties, bonuses, and SEE constraints.
  - [ ] 5.4 Re-run SIMD benches (`simd_performance_benchmarks.rs`, `simd_nps_benchmarks.rs`) after each major heuristic change to watch for regressions; document findings in `AI_ENGINE_ANALYSIS.md`.
  - [ ] 5.5 Gate PR merges on the new validation checklist, capturing outcomes in the performance analysis doc for traceability.

## Completion Notes

- Added castle progress tracking and storm-aware king-safety scoring across `src/evaluation/castles.rs` and `src/evaluation/king_safety.rs`, including configurable progress thresholds plus storm penalties that amplify when progress lags.
- Extended `KingSafetyConfig` defaults with progress/ storm weights so the new heuristics appear in tuning manifests out of the box.
- Expanded `OpeningPrincipleConfig` and `evaluate_opening_penalties` to model redundant major-piece shuffles and opening-debt accumulation, giving early-line evaluations concrete incentives to develop instead of oscillating.
- Wired redundant-move penalties to favor productive moves (captures/checks) and added opening debt scaling that matches move count, keeping enforcement soft but unavoidable.

- Registered Static Rook, Ranging Rook, and Disciplined Ureshino templates in `src/opening_book/templates.rs`; validation now reports coverage and blocks king-first / premature rook-swing moves per template policy.
- Added opening priors in `opening_principles.rs` so the evaluator rewards early `7g7f`/`2g2f` pawn pushes out of book, keeping the engine aligned with curated plans.
- Authored `docs/design/gameplay-stability/OPENING_TEMPLATE_POLICY.md` (cross-linked from `docs/design/opening-book/README.md`) describing regeneration steps and guardrails enforced during book compilation.

Next Steps:
- Wire the new storm/initiative signals into move ordering (Task 3.0) to ensure search prioritizes the defensive resources now scored in evaluation.
- Backfill regression coverage in `tests/evaluation/castle_progress_tests.rs` and `tests/search/stability_regressions.rs`, then rerun SIMD benches before merging.

