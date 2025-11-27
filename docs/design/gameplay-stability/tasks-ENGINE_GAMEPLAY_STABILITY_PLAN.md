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

- [x] 3.0 Update search and move-ordering logic for stability awareness
  - [x] 3.1 Adjust `move_ordering/mod.rs` bonuses so castle-progressing and storm-response moves rise in the ordering queue.
  - [x] 3.2 Bias iterative deepening (`iterative_deepening.rs`) toward lines that satisfy development milestones before move 12, reducing effort on king-first continuations unless they already score ≥ +150cp.
  - [x] 3.3 Enhance `quiescence.rs` and late-move pruning to revisit nodes where SEE indicates a self-destructive capture or where promoted-pawn threats exist.
  - [x] 3.4 Wire initiative/attack debt metrics into search statistics so offensive pressure translates into selective deepening when coordination is present.

- [x] 4.0 Build pawn-storm and attack-response framework
  - [x] 4.1 Define storm-state tracking structs (consecutive pushes, file ownership, time since response) accessible to evaluation and search layers.
  - [x] 4.2 Expand drop heuristics (pawn/gold drops such as `▲8八歩打`, `▲7八金`) so they trigger automatically when storm severity crosses a threshold.
  - [x] 4.3 Implement initiative tracking that recognizes climbing silver, edge attacks, and prepared pawn breaks; ensure scoring kicks in only after development prerequisites are met.
  - [x] 4.4 Add "attack debt" penalties that escalate when the engine amasses attacking resources but fails to convert within a configurable ply window.

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

## Task 3.0 Completion Notes

- **Task 3.1 (Move Ordering Bonuses)**: Added `castle_progress_weight` (1200) and `storm_response_weight` (1500) to `OrderingWeights` configuration. Implemented `score_castle_progress_move()` and `score_storm_response_move()` methods that detect moves progressing castle formation or responding to pawn storms. Integrated these bonuses into `score_move_with_all_heuristics()` so defensive moves rise in the ordering queue.

- **Task 3.2 (Iterative Deepening Bias)**: Added `estimate_is_opening_phase()` and `is_king_first_move()` helper methods to detect opening phase and king-first moves. Integrated logic into iterative deepening loop to reduce search effort (25% time reduction) on king-first continuations before move 12 unless they score ≥ +150cp. This biases search toward development milestones.

- **Task 3.3 (Quiescence Enhancements)**: Enhanced `QuiescenceHelper` with `is_self_destructive_capture()` and `has_promoted_pawn_threats()` methods. Updated `should_prune_futility()` to exclude self-destructive captures and positions with promoted pawn threats from pruning, ensuring these critical positions are revisited. Updated both `quiescence.rs` and `search_engine.rs` implementations.

- **Task 3.4 (Initiative Metrics)**: Added `InitiativeMetrics` struct to `SearchStatistics` to track offensive pressure and coordination. Implemented `evaluate_initiative_coordination()` to detect coordinated major pieces and attackers near opponent king. Added `should_deepen_for_initiative()` method that triggers selective deepening when initiative score ≥ 50cp, translating offensive pressure into deeper search when coordination is present.

All stability-aware search improvements are now integrated and ready for testing. The engine will prioritize defensive moves, reduce effort on problematic king-first lines, revisit critical tactical positions, and deepen search when offensive coordination is detected.

## Task 4.0 Completion Notes

- **Task 4.1 (Storm-State Tracking)**: Created comprehensive `StormState` and `FileStormState` structs in `src/evaluation/storm_tracking.rs` that track consecutive pawn pushes, file ownership, time since response, and penetration depth. The `StormState::analyze()` method processes the board and updates storm metrics accessible to both evaluation and search layers. Integrated into `KingSafetyEvaluator` via `get_storm_state()` method, with storm penalties that escalate based on time since response.

- **Task 4.2 (Storm-Aware Drop Heuristics)**: Added `evaluate_storm_aware_drops()` method to `PositionFeatureEvaluator` that automatically triggers pawn/gold drop bonuses when storm severity crosses a threshold (1.5). The method recommends blocking drops (e.g., `▲8八歩打`, `▲7八金`) on files with active storms, with bonuses scaling by storm severity. Gold drops receive higher bonuses than pawn drops, and king-zone defensive drops (7八金 style) get additional priority.

- **Task 4.3 (Initiative Tracking)**: Implemented comprehensive initiative tracking in `src/evaluation/initiative_tracking.rs` with `InitiativeState` that recognizes climbing silver attacks, edge attacks (files 1/9), prepared pawn breaks, coordinated major pieces, and rook file openings. All scoring is gated by `check_development_prerequisites()` which requires at least one developed major piece and 6+ moves before initiative bonuses apply, ensuring scoring only kicks in after development milestones are met.

- **Task 4.4 (Attack Debt Penalties)**: Added `AttackDebt` struct that tracks when the engine accumulates attacking resources (initiative score ≥ 30cp) but fails to convert them within a configurable ply window (default 8 plies). Penalties escalate based on how many plies have passed beyond the window, calculated as `base_penalty * (1.0 + debt_multiplier)` where the multiplier increases by 0.1 per extra ply. The attack debt is integrated into `InitiativeState::analyze()` and automatically applied when resources are present but unconverted.

All pawn-storm and attack-response framework components are now integrated. The engine can detect storms, recommend defensive drops, track offensive initiative patterns, and penalize failure to convert attacking advantages, providing a comprehensive framework for handling both defensive and offensive gameplay stability.

