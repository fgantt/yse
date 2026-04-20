# NNUE Implementation - Quick Reference

**This document is a companion to `NNUE_IMPLEMENTATION_PLAN.md`**  
Use this for fast lookup of specific tasks, decisions, and file locations.

---

## Phase-at-a-Glance

| Phase | Objective | Duration | Expected Outcome | Go/No-Go |
|-------|-----------|----------|------------------|----------|
| **Phase 1** | Fix weight init + output scaling | 1 day | Training improves | Proceed always |
| **Phase 2** | Fix training algorithm | 3 days | Strength gain visible | Conditional (if P1 fails) |
| **Phase 3** | Speed optimization + deep arch | 5 days | 2-3× faster + test networks | Proceed if P1-P2 work |
| **Phase 4** | ELO benchmarking | 4 days | Measure actual strength | Proceed always |
| **Phase 5** | Deployment + long-term training | Ongoing | Production ready | Proceed if P4 succeeds |

---

## Phase 1: Quick Implementation

### What to Change

| File | Lines | What | Why |
|------|-------|------|-----|
| `src/evaluation/nnue.rs` | 84-128 | Weight init: `Normal(0, 0.01)` | 256× reduction |
| `src/evaluation/nnue.rs` | 300-325 | Output scale: `* 400 / 16320` | Quantization |

### Code Locations Checklist

```
src/evaluation/nnue.rs
├─ Lines 1-50: Imports, constants (ADD HERE: SCALE_FACTOR, etc.)
├─ Lines 84-128: NNUEWeights::new() - CHANGE weight init
├─ Lines 300-325: evaluate() function - CHANGE output scaling
└─ Lines 186-227: refresh() function (Phase 3 target)
```

### Testing Checklist

```
After Fix 1.1 (weight init):
☐ cargo build --release
☐ No compilation errors
☐ Weights ~±100 range (debug output)

After Fix 1.2 (output scaling):
☐ cargo build --release
☐ Startpos eval in ±100-300 centipawn range
☐ Same position always = same eval

Before training:
☐ rm nnue_weights_*.json
☐ cargo run --release --bin nnue_trainer
☐ Check: TD error decreasing? (success indicator)
```

---

## Phase 2: Conditional Fixes

### When to Use

**Only if Phase 1 training shows NO improvement:**
- TD error flat or increasing
- Weights not changing meaningfully

### Code Changes Summary

| File | Change | Why |
|------|--------|-----|
| `src/bin/nnue_trainer.rs` | Store outcome, not evaluation | Break bootstrap |
| `src/evaluation/nnue_training.rs` | Use outcome target | External oracle |
| `src/evaluation/nnue_training.rs` | learning_rate 0.001 | Reduce from 0.01 |

### Key Decision: Target Source

Choose ONE approach:

**Option A: Outcome-based (Simpler)**
```rust
target = match game_result {
    Win => 1.0,
    Draw => 0.5,
    Loss => 0.0,
}
```
**Use this first** ← RECOMMENDED

**Option B: Search-based (Stronger)**
```rust
target = pst_evaluator.evaluate(position, depth=8)
```
**Use this only after A works**

---

## Phase 3: Speed Optimization

### Task 3.1: Parallel Architectures

Test BOTH:
- **ARCH_SHALLOW**: 256 → 32 → 1 (current, fast)
- **ARCH_DEEP**: 512 → 128 → 32 → 1 (slower, stronger?)

Train each separately, compare results at iteration 100.

### Task 3.2: Incremental Updates

**Current**: O(81) operations per eval (full refresh)  
**Target**: O(1) operations per eval (only changed pieces)

**Priority**: HIGH for speed (10-100× improvement)

**Approach**:
- Dirty-flag accumulator
- Only update pieces that moved
- Integrate into search loop

### Task 3.3: SCReLU Activation (Optional)

**Current**: ReLU: `max(0, x)`  
**New**: SCReLU: `clamp(x, 0, 1)²`

**Cost**: Requires retraining  
**Benefit**: Better capacity (what Stockfish uses)

Only do if other optimizations work well.

---

## Phase 4: Testing & Validation

### Create Test Binary

```bash
cargo run --release --bin elo-tester -- --games 100 --time 1s
```

### Test Configurations

| Config | Purpose |
|--------|---------|
| NNUE (final) vs PST | Main test |
| NNUE (iter 500) vs PST | Earlier checkpoint |
| Ensemble (0.7×NNUE + 0.3×PST) vs both | Hybrid |

### Minimum Results Required

```
Games: 200+
Confidence: 95%
Expected ELO range: -50 to +200
```

### Success Threshold

- **+50+ ELO**: Full success ✓
- **+20 to +50**: Partial success, continue
- **+0 to +20**: Minimal, still valid
- **Negative**: Debug needed

---

## Decision Points (Go/No-Go)

### After Phase 1: Training Improves?

```
TD error decreasing (>10% by iter 50)?
├─ YES → Continue Phase 3
└─ NO → Do Phase 2
```

### After Phase 2: Strength Visible?

```
Measurable strength improvement on test positions?
├─ YES → Continue Phase 3
├─ MAYBE → Do more iterations
└─ NO → Debug or reconsider approach
```

### After Phase 3: Architecture Choice?

```
SHALLOW (256→32→1) vs DEEP (512→128→32→1)?
├─ SHALLOW stronger → Keep
├─ DEEP stronger → Switch
└─ TIE → Keep SHALLOW (faster)
```

### After Phase 4: Deploy?

```
ELO result from 200+ games?
├─ +50+ ELO → Deploy immediately
├─ +20 to +50 → Likely deploy
├─ +0 to +20 → Consider ensemble
└─ Negative → Stay with PST
```

---

## Files to Modify (Checklist)

### Phase 1
- [ ] `src/evaluation/nnue.rs` - Weight init + output scale

### Phase 2 (Conditional)
- [ ] `src/bin/nnue_trainer.rs` - Store outcomes
- [ ] `src/evaluation/nnue_training.rs` - Compute targets, adjust config

### Phase 3
- [ ] `src/evaluation/nnue.rs` - Add architecture config (3.1)
- [ ] `src/evaluation/nnue.rs` - Incremental updates (3.2)
- [ ] `src/evaluation/nnue.rs` - SCReLU activation (3.3, optional)

### Phase 4
- [ ] Create `src/bin/elo-tester.rs` (new file)

### Phase 5
- [ ] `src/evaluation.rs` - Search integration
- [ ] `docs/` - Update documentation

---

## Typical Session Structure

### Session 1 (1-2 hours): Phase 1.1 - Weight Init

1. Read `NNUE_IMPLEMENTATION_PLAN.md` Phase 1.1 (10 min)
2. Edit `src/evaluation/nnue.rs:84-128` (20 min)
3. Compile and verify (10 min)
4. Review changes (10 min)

### Session 2 (1-2 hours): Phase 1.2 - Output Scaling

1. Read `NNUE_IMPLEMENTATION_PLAN.md` Phase 1.2 (10 min)
2. Edit `src/evaluation/nnue.rs:300-325` (20 min)
3. Compile and verify (10 min)

### Session 3 (2-3 hours): Phase 1.3 - Training

1. Clean weights: `rm nnue_weights_*.json`
2. Run training: `cargo run --release --bin nnue_trainer`
3. Monitor logs for 30 min+
4. Decide: Phase 2 needed?

### Session 4 (2-3 hours): Phase 2.1 - Training Targets (if needed)

1. Read Phase 2.1 section (15 min)
2. Edit training files (60 min)
3. Test retraining (45 min)

### Sessions 5-6 (4-6 hours): Phase 3 - Optimization

1. Architecture variants (60 min)
2. Incremental updates (120 min)
3. Benchmarking (60 min)

### Session 7 (4-5 hours): Phase 4 - Benchmarking

1. Create `elo-tester.rs` (60 min)
2. Run 200 games (120-180 min)
3. Analyze results (30 min)

---

## Debugging Quick Guide

### Issue: Weights Don't Change During Training

**Check**:
1. Is learning rate zero? (should be ~0.001)
2. Are gradients computed? (add logging)
3. Do targets come from external oracle? (not same eval)
4. Are position.td_target values non-zero?

**Fix**:
```rust
// Add logging in update function
println!("error={}, gradient={}, weight_change={}", error, gradient, weight_change);
```

### Issue: Loss Increasing or Flat

**Check**:
1. Is learning rate too high? (try 0.0001)
2. Are positions diverse? (vary games per iteration)
3. Is target inconsistent? (validate targets before training)

**Fix**:
- Reduce learning_rate by 10×
- Increase games_per_iteration by 10×
- Check target computation

### Issue: NNUE Eval Out of Range

**Check**:
1. Is output scaling wrong? (division constant)
2. Are weights saturated? (check with debug output)
3. Did initialization work? (weight ranges in ±100?)

**Fix**:
- Try different scale factors (32, 100, 200, 400)
- Reinitialize weights
- Check forward pass math

### Issue: Training Runs But Shows No Strength

**Check**:
1. Is Phase 2 training targets working? (outcomes not evals?)
2. Are you testing on fresh positions? (not training data)
3. Is depth enough for good targets? (depth ≥ 6)

**Fix**:
- Verify targets are from outcomes/search
- Use cross-validation positions
- Increase search depth

---

## Performance Baselines

### Evaluation Speed

**Current** (full refresh each eval): ~1-10 microseconds per position  
**Target** (incremental updates): ~0.1-1 microsecond per position  
**Benchmark**: `cargo bench --bench nnue_speed`

### Training Time

**Phase 1.3** (100 iterations): ~1-5 minutes per iteration (depending on CPU)  
**Total**: ~100-500 minutes (~2-8 hours) for 100 iterations

### ELO Benchmarking

**100 games**: ~30-60 minutes at 1s/move
**200 games** (recommended): ~60-120 minutes at 1s/move

---

## Success Indicators (What to Look For)

### Phase 1 Success

```
✓ Compilation successful
✓ No runtime errors
✓ TD error decreasing over iterations
✓ Weight magnitudes changing (not constant)
```

### Phase 2 Success

```
✓ Training loss converging
✓ Validation accuracy improving
✓ Test positions show position-dependent evaluations
```

### Phase 3 Success

```
✓ Benchmarks show 2-10× speedup
✓ Both architectures complete training
✓ Deeper network shows measurable strength difference
```

### Phase 4 Success

```
✓ NNUE vs PST: ±50+ ELO with 95% confidence
✓ Ensemble shows clear improvement
✓ Results consistent across multiple test runs
```

---

## When to Stop & Reassess

**Stop Phase 1 and go to Phase 2 if**:
- After 50 iterations, TD error not decreasing
- Weights not changing meaningfully
- Output scaling clearly wrong (evals out of range)

**Stop Phase 2 and reconsider if**:
- After 100 iterations, still no convergence
- Loss increases consistently
- May need to revisit Phase 1 assumptions

**Stop Phase 3 and focus on Phase 4 if**:
- Optimization difficult to integrate
- Speed gains marginal
- Better to prove strength first, optimize later

**Stop Phase 4 and declare success if**:
- Any measurable ELO gain achieved
- Continue with Phase 5 (improvement process)

---

## Contacts & References

### In This Repository

- `NNUE_IMPLEMENTATION_PLAN.md` - Full detailed plan (THIS file's companion)
- `docs/NNUE_DEEP_DIVE_ANALYSIS.md` - Problem analysis
- `docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md` - Stockfish comparison
- `docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md` - Quick fixes

### External Resources

- Stockfish NNUE: https://github.com/official-stockfish/Stockfish/tree/master/src/nnue
- nnue-pytorch: https://github.com/glinscott/nnue-pytorch (excellent docs)
- NNUE Paper: https://github.com/ynasu87/nnue

### Shogi-Specific

- YaneuraOu: https://github.com/yaneurao/YaneuraOu (original NNUE for Shogi)
- Kristallweizen: https://github.com/Tama4649/Kristallweizen

---

**Last Updated**: 2026-04-20  
**Status**: Ready for Implementation  
**Next Step**: Review main plan + start Phase 1
