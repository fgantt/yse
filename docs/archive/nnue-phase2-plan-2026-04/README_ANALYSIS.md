# NNUE Deep-Dive Analysis - Complete Report

This directory contains three comprehensive analysis documents investigating why the NNUE implementation hasn't improved engine strength:

## Documents

### 1. **NNUE_DEEP_DIVE_ANALYSIS.md** (18 KB)
Main analysis document covering all 6 areas:
- **Evaluation Calibration**: Arbitrary `/256` scaling breaks search
- **Training Quality**: TD(λ) implementation is fundamentally broken
- **Data Quality**: Bootstrap loop (self-play with weak eval)
- **Network Architecture**: 2268→256→32→1 is suboptimal
- **Search Integration**: NNUE not used in move ordering
- **Performance Measurement**: No actual strength tests exist

Each section includes:
- Root cause analysis
- Code locations and line numbers
- Impact on engine strength
- Recommended fixes

### 2. **NNUE_ISSUES_REFERENCE.txt** (20 KB)
Quick reference guide with:
- 7 critical issue categories
- Exact file paths and line numbers
- Problem statements and evidence
- Diagnostic checklist (currently 0/10 passing)

Use this for quick lookup of specific issues.

### 3. **NNUE_CODE_ISSUES_DETAILED.md** (40+ KB)
Before/after code comparisons for the 5 most critical issues:

1. **Arbitrary Evaluation Scaling** → Add tanh() activation
2. **Broken TD(λ)** → Implement proper eligibility traces + discount factor
3. **Incomplete Backpropagation** → Full gradient computation through all layers
4. **Bootstrap Problem** → Use strong (PST) evaluator for self-play moves
5. **Bad Weight Initialization** → Xavier initialization instead of [-128, 128]

Each includes:
- Current broken code
- Detailed problem explanation
- Fixed code with comments
- Key changes summary

## Quick Diagnosis

The NNUE fails to improve strength due to a **cascade of fundamental issues**:

```
Random weights → Garbage self-play
              ↓
No external oracle (PST, opening book, tablebase)
              ↓
Positions learned = weak model's biases, not real evaluation
              ↓
Broken TD(λ) training → doesn't improve even from garbage data
              ↓
Arbitrary /256 scaling → search heuristics break
              ↓
Search ignores NNUE eval anyway → no impact on move quality
              ↓
No one measured it → no one knows it failed
```

## File Locations (Absolute Paths)

### Critical Implementation Files
- `/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue/src/evaluation/nnue.rs` - Forward pass, weight storage
- `/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue/src/evaluation/nnue_training.rs` - TD(λ), weight updates
- `/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue/src/bin/nnue_trainer.rs` - Self-play training loop
- `/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue/src/evaluation.rs` - PositionEvaluator integration

### Documentation Files (Claims vs Reality)
- `docs/NNUE_INTEGRATION_COMPLETE.md` - Claims "ready for gameplay" (NO strength data)
- `docs/NNUE_SCALING_ISSUE.md` - Shows scaling changes as band-aids, not fixes
- `docs/NNUE_TRAINING_FIXES.md` - Claims fixes but doesn't address root causes
- `examples/nnue_evaluate_trained.rs` - Shows eval differences (not strength)

### Data Files
- `nnue_weights_trained.json` (6.0 MB) - Last updated Jan 21, quality unknown
- `nnue_weights_iter_*.json` - Iteration snapshots (700+ iterations trained)

## Key Metrics (What We Know)

**Training**:
- 700 iterations
- 21,000 games (30 games × 700 iterations)
- ~175,000 positions processed
- Final TD error: 0.000455 (very low convergence)

**But No Strength Data**:
- ELO vs PST: UNKNOWN
- Time-controlled games: NO COMPARISON
- Benchmark positions: EV comparison only, not move quality
- Handicap matches: NONE RUN

## Recommendations by Urgency

### STOP (Days)
- [ ] Stop using `/256` scaling (arbitrary, unjustified)
- [ ] Stop using weights initialized to [-128, 128] (too large)
- [ ] Stop training on NNUE-generated positions in iteration 1-10 (garbage)

### START (Days)
- [ ] Add tanh() activation to output layer
- [ ] Switch to Xavier weight initialization  
- [ ] Use PST for self-play moves, train NNUE on results
- [ ] Implement ELO test harness (100+ games needed)

### FIX (Weeks)
- [ ] Implement proper backpropagation through all layers
- [ ] Add gradient clipping and weight decay
- [ ] Fix TD(λ): Add discount factor γ, implement eligibility traces
- [ ] Measure actual strength improvement

### OPTIMIZE (Months)
- [ ] Study Stockfish 15+ NNUE implementation
- [ ] Try deeper architecture: 2268 → 512 → 256 → 32 → 1
- [ ] Add layer normalization
- [ ] Use ensemble: blend NNUE + PST scores
- [ ] Train on Lichess database, not just self-play

## Quick Start for Fixing

If you have 8 hours, fix these in order:

1. **Fix weight initialization** (1 hour)
   - File: `src/evaluation/nnue.rs:84-119`
   - See: `NNUE_CODE_ISSUES_DETAILED.md` Issue #5

2. **Add tanh activation** (30 min)
   - File: `src/evaluation/nnue.rs:308-320`
   - See: `NNUE_CODE_ISSUES_DETAILED.md` Issue #1

3. **Disable bootstrap NNUE** (1 hour)
   - File: `src/bin/nnue_trainer.rs:257`
   - See: `NNUE_CODE_ISSUES_DETAILED.md` Issue #4

4. **Add ELO test infrastructure** (2 hours)
   - Create benchmark with 100+ games
   - Run NNUE vs PST comparison

5. **Implement proper backprop** (3.5 hours)
   - File: `src/evaluation/nnue_training.rs:232-333`
   - See: `NNUE_CODE_ISSUES_DETAILED.md` Issue #3

After these fixes, NNUE should show measurable improvement (±50 ELO minimum).

## Contact & Questions

Refer to the three analysis documents for:
- **Quick facts**: `NNUE_ISSUES_REFERENCE.txt`
- **Deep analysis**: `NNUE_DEEP_DIVE_ANALYSIS.md`
- **Code examples**: `NNUE_CODE_ISSUES_DETAILED.md`

All issues are with specific line numbers, code examples, and proposed fixes.
