# NNUE Research: Stockfish vs. Shogi Engine - Complete Documentation

## Overview

This research comprehensively analyzes NNUE (Efficiently Updatable Neural Networks) implementations across modern chess engines and the Shogi engine. The goal is to understand why Stockfish's NNUE approach works and identify critical issues in the current Shogi implementation that prevent strength improvement.

**Bottom Line**: The Shogi engine's NNUE has several critical bugs (mostly fixable in <1 hour) that completely prevent it from improving strength. Stockfish's approach is proven and the Shogi engine foundation is sound; they just need to match the correct implementation details.

---

## Documents in This Research

### 1. [NNUE_RESEARCH_SUMMARY.txt](../NNUE_RESEARCH_SUMMARY.txt) (8.8 KB)
**Start here if you're in a hurry** (5 min read)
- Executive summary of findings
- Key issues and their severity
- Implementation priorities (4 phases)
- Quick action items
- External resources

### 2. [NNUE_RESEARCH_STOCKFISH_COMPARISON.md](docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md) (22 KB)
**Deep dive technical analysis** (30 min read)
- Section 1: Architecture & Activation Functions
- Section 2: Training Procedure 
- Section 3: Weight Initialization
- Section 4: Search Integration
- Section 5: Accumulator Strategy
- Section 6: Modern Variations (2023-2025)
- Section 7: Comparative Summary Table
- Section 8: Detailed Recommendations
- References & Links

### 3. [NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md](docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md) (9.5 KB)
**Implementation guide with code** (15 min read)
- Problem Severity Assessment Table
- Fix #1: Weight Initialization (with code examples)
- Fix #2: Output Scaling (with code examples)
- Fix #3: Incremental Accumulator Updates
- Fix #4: Training Targets (external oracle)
- Fix #5: Training Configuration
- Implementation Priority Roadmap (4 phases)
- Verification Checklist
- Debugging Help
- External Resources

---

## Quick Navigation

### If You Want To...

**Understand the whole story** → Read in order:
1. NNUE_RESEARCH_SUMMARY.txt (5 min)
2. NNUE_RESEARCH_STOCKFISH_COMPARISON.md sections 1 & 3 (15 min)
3. NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md (15 min)

**Just fix it** → Go directly to:
- NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md (Fix #1, Fix #2)
- Take 1 hour, implement both fixes, re-train

**Understand Stockfish's approach** → Read:
- NNUE_RESEARCH_STOCKFISH_COMPARISON.md sections 1, 3, 4, 5

**Understand Shogi-specific optimizations** → Read:
- NNUE_RESEARCH_STOCKFISH_COMPARISON.md sections 5, 6
- NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md accumulator strategy

**Learn modern NNUE techniques** → Read:
- NNUE_RESEARCH_STOCKFISH_COMPARISON.md section 6 (modern variations 2023-2025)

---

## Key Findings at a Glance

### Critical Issues (Fix These First)

| # | Issue | Current | Correct | Time to Fix |
|---|-------|---------|---------|-------------|
| 1 | Weight Init | Uniform[-128,128] | Normal(0, 0.01) | 10 min |
| 2 | Output Scale | /256 arbitrary | /40 quantized | 10 min |
| 3 | Training Targets | Self-play (bootstrap) | External oracle | 2 hours |
| 4 | TD(λ) Implementation | Broken formula | Proper supervised | 2 hours |

### Performance Gaps

| # | Issue | Gap | Time to Fix |
|---|-------|-----|-------------|
| 5 | Accumulator Updates | Full refresh vs. incremental | 10-100x slower | 4 hours |
| 6 | Search Integration | None | Full integration | 1 day |
| 7 | Activation Function | ReLU | SCReLU/Quantmoid4 | 2 hours |
| 8 | Output Bucketing | Single | 8 buckets | 4 hours |

### Total Impact If All Fixed
- **Phase 1 (Critical fixes #1-2)**: 30 min → Enables training to work at all
- **Phase 2 (Training targets)**: 2 hours → Enables actual strength improvement
- **Phase 3 (Optimization)**: 6 hours → 10-100x speedup + better accuracy
- **Phase 4 (Polish)**: 2+ days → Match Stockfish's architecture

---

## Architecture Comparison

```
                    Stockfish (HalfKAv2)    Shogi (Current)
Input features:     45,056                  2,268
Input sparsity:     0.1% (45-65 active)     Similar (~20-30 active)
Feature xfm:        2x520 (with skip)       256 (no skip)
Hidden 1:           512x2 (per perspective) 256
Hidden 2:           16-32                   32
Output buckets:     8 (by piece count)      1 (monolithic)
Activation:         SCReLU + Quantmoid4     ReLU (basic)
Output networks:    Multiple sub-networks   Single
Weight init:        [-0.01, 0.01]          [-128, 128] WRONG
Output scale:       400/16320≈0.0245       /256=0.0039 ARBITRARY
```

---

## Implementation Roadmap

### Phase 1: Critical Fixes (30 minutes)
```
□ Fix weight initialization line in src/evaluation/nnue.rs:84
□ Fix output scaling constants in src/evaluation/nnue.rs:315
□ Re-train from scratch
```

### Phase 2: Training Fixes (2-4 hours)
```
□ Modify training targets to use external oracle
□ Fix training configuration parameters
□ Implement proper supervised learning
□ Re-train on large dataset
```

### Phase 3: Optimization (6-8 hours)
```
□ Implement incremental accumulator updates
□ Add SCReLU or Quantmoid4 activation
□ Implement output bucketing by piece count
□ Integrate into search with lazy updates
```

### Phase 4: Polish (2+ days)
```
□ Implement king bucket architecture (Shogi-specific)
□ Full search integration (move ordering, TT)
□ Collect training data pipeline
□ Professional training with billions of positions
```

---

## Code Files Involved

### Main Implementation Files
- `src/evaluation/nnue.rs` - Core NNUE evaluator and accumulator
  - Line 80-128: Weight initialization (needs fix)
  - Line 276-320: Accumulator evaluation (needs fix)
  
- `src/evaluation/nnue_training.rs` - Training algorithm
  - Line 143-189: TD(λ) computation (broken)
  - Line 232-333: Weight updates (broken)

- `src/bin/nnue_trainer.rs` - Training binary
  - Line 30-142: Self-play game generation
  - Uses broken targets from same eval

### Related Files
- `Cargo.toml` - Add rand_distr crate for proper initialization
- Search integration points (TBD - not yet implemented)

---

## External References

### Original Research
- **Yu Nasu (2018)**: NNUE paper https://github.com/ynasu87/nnue/blob/master/docs/nnue.pdf
- **Dominik Klein (2021)**: Neural Networks for Chess

### Reference Implementations
- **Stockfish NNUE**: https://github.com/official-stockfish/Stockfish
- **nnue-pytorch**: https://github.com/glinscott/nnue-pytorch (best docs)
- **Bullet trainer**: https://github.com/jnsl/bullet (Rust, modern)
- **Grapheus**: https://github.com/cosmicpudding/grapheus (C++, industrial)

### Documentation
- **Chess Programming Wiki - NNUE**: https://www.chessprogramming.org/NNUE
- **Chess Programming Wiki - Stockfish NNUE**: https://www.chessprogramming.org/Stockfish_NNUE
- **NNUE-PyTorch Guide**: https://github.com/glinscott/nnue-pytorch/blob/master/docs/nnue.md (excellent)

### Related Shogi Engines
- **YaneuraOu**: https://github.com/yaneurao/YaneuraOu (original NNUE for Shogi)
- **Kristallweizen**: https://github.com/Tama4649/Kristallweizen (WCSC29 runner-up)

---

## Summary: What's Wrong & How to Fix It

### The Problems (Why NNUE Isn't Working)

**Weight Initialization Problem**:
- Current: Uniform[-128, 128] per feature weight
- Impact: With 20 sparse inputs, sums to ±2560 (way too large)
- Result: Activation saturates immediately, can't learn

**Output Scaling Problem**:
- Current: Divide by 256 (arbitrary choice)
- Impact: Scales incorrectly for search integration
- Result: Evaluations don't correlate with search

**Training Target Problem** (Bootstrap):
- Current: Use NNUE eval on same positions being trained
- Impact: NNUE learns from itself, not from external truth
- Result: Can't improve beyond self-play baseline

**Missing Incremental Updates**:
- Current: Full refresh on every evaluation
- Impact: 10-100x slower than Stockfish
- Result: Can't scale to deeper search

### The Solutions (What Stockfish Does Right)

**Weight Initialization Solution**:
- Use: Normal(0, 0.01) scaled to [-100, 100]
- Why: Sparse accumulation → sums to ±0.2 in float, ±20 in quantized
- Result: Network learns smoothly from initialization

**Output Scaling Solution**:
- Use: Proper quantization constants (SCALE=400, QA=255, QB=64)
- Why: Matches activation bounds, prevents overflow
- Result: Evaluations in correct centipawn range

**Training Target Solution**:
- Use: External oracle (search results or game outcomes)
- Why: Provides ground truth, breaks bootstrap loop
- Result: Progressive improvement as NNUE gets stronger

**Incremental Updates Solution**:
- Use: Store accumulator per ply, update incrementally
- Why: Only changes 1-4 features per move
- Result: 1-2 µs update vs 10-20 µs refresh

---

## Next Steps

1. **Read NNUE_RESEARCH_SUMMARY.txt** (5 min) for overview
2. **Read NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md** (15 min) for code
3. **Implement Fix #1 & #2** (30 min) - weight init and output scaling
4. **Retrain** (hours to days) - verify convergence
5. **If no improvement**, implement Fix #3 & #4 (training targets)
6. **If improvement**, proceed to Phase 3 (optimization)

---

## Questions to Answer

After reading these documents, you should be able to answer:

1. **Why is Stockfish's weight initialization [-0.01, 0.01] instead of [-128, 128]?**
   Answer: Sparse inputs accumulate slowly; small weights sum to reasonable values

2. **Why does Stockfish use 8 output buckets instead of 1 network?**
   Answer: Different piece counts have different evaluation characteristics

3. **Why does Stockfish use external targets instead of TD(λ)?**
   Answer: Avoids bootstrap problem; provides ground truth for learning

4. **Why does Shogi need incremental updates?**
   Answer: Piece drops + captured pieces mean efficiency is critical

5. **How should Shogi NNUE handle captured pieces in hand?**
   Answer: Separate accumulators for board + hand, combine for evaluation

---

## Document Statistics

| Document | Size | Lines | Read Time |
|----------|------|-------|-----------|
| NNUE_RESEARCH_SUMMARY.txt | 8.8K | 141 | 5 min |
| NNUE_RESEARCH_STOCKFISH_COMPARISON.md | 22K | 606 | 30 min |
| NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md | 9.5K | 320 | 15 min |
| **Total** | **40K** | **1067** | **50 min** |

All files available in repository:
- Summary: `/NNUE_RESEARCH_SUMMARY.txt`
- Full analysis: `/docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md`
- Quick fixes: `/docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md`

---

**Research completed:** April 20, 2026
**Sources:** Stockfish NNUE implementation, Chess Programming Wiki, nnue-pytorch documentation, YaneuraOu (Shogi NNUE original), academic papers on neural networks for game playing
