# Gameplay Stability Validation Checklist

## Task 5.5: PR Merge Validation Checklist

This checklist must be completed before merging gameplay stability changes.

### Pre-Merge Requirements

- [ ] **Task 5.1**: Critical FEN positions captured and self-play suites created
  - [ ] `tests/self_play/fens/storm_regressions.fen` contains Game A and Game B positions
  - [ ] Self-play tests verify protective responses in critical positions
  - [ ] Tests pass: `cargo test --test stability_regression_tests`

- [ ] **Task 5.2**: Self-play batches completed
  - [ ] 200 games run for static rook opening
  - [ ] 200 games run for ranging rook opening
  - [ ] 200 games run for Ureshino opening
  - [ ] Castle completion rates tracked and documented
  - [ ] Redundant move frequencies recorded

- [ ] **Task 5.3**: Unit tests extended
  - [ ] `tests/evaluation/castle_progress_tests.rs` covers castle progress scoring
  - [ ] `tests/search/stability_regressions.rs` covers search stability
  - [ ] `tests/stability_regression_tests.rs` covers critical positions
  - [ ] All tests pass: `cargo test --test stability_regression_tests --test castle_progress_tests`

- [ ] **Task 5.4**: SIMD benchmarks re-run
  - [ ] `cargo bench --bench simd_performance_benchmarks` shows no regressions
  - [ ] `cargo bench --bench simd_nps_benchmarks` shows no regressions
  - [ ] Performance findings documented in performance analysis doc
  - [ ] Any regressions < 5% are acceptable; > 5% require investigation

- [ ] **Code Quality**
  - [ ] All linter errors resolved: `cargo clippy`
  - [ ] Code formatted: `cargo fmt`
  - [ ] No compiler warnings

### Validation Criteria

#### Castle Completion
- [ ] At least 80% of games show castle formation by move 15
- [ ] No games show king-first improvisation in first 10 moves
- [ ] Castle progress penalties apply correctly when progress < threshold

#### Storm Response
- [ ] Engine detects pawn storms on files 1, 8, 9, and central files
- [ ] Defensive moves (pawn drops, gold placements) are prioritized when storms detected
- [ ] Storm penalties escalate correctly based on time since response

#### Redundant Move Prevention
- [ ] Redundant major-piece moves (same piece twice without capture) are penalized
- [ ] Opening debt accumulates correctly for non-developing moves
- [ ] SEE constraints prevent self-destructive trades (e.g., ▲4四角??)

#### Search Stability
- [ ] Move ordering prioritizes castle-progressing moves
- [ ] Iterative deepening reduces effort on king-first continuations
- [ ] Quiescence revisits positions with promoted pawn threats

### Performance Benchmarks

Baseline performance (before stability changes):
- SIMD performance benchmarks: [Record baseline]
- SIMD NPS benchmarks: [Record baseline]

After stability changes:
- SIMD performance benchmarks: [Record results]
- SIMD NPS benchmarks: [Record results]
- Regression: [Calculate % change]

**Acceptable thresholds:**
- < 5% regression: Acceptable
- 5-10% regression: Requires justification
- > 10% regression: Must be addressed before merge

### Documentation

- [ ] Completion notes added to `tasks-ENGINE_GAMEPLAY_STABILITY_PLAN.md`
- [ ] Performance findings documented (if applicable)
- [ ] Any new configuration options documented
- [ ] Test coverage documented

### Sign-off

- [ ] All checklist items completed
- [ ] Reviewer approval obtained
- [ ] Ready for merge

---

**Last Updated**: [Date]
**Completed By**: [Name]




