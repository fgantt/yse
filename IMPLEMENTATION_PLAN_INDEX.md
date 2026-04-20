# NNUE Implementation Plan - Master Index

**Complete overview of the comprehensive NNUE improvement roadmap.**

---

## 📋 Documents Overview

This implementation plan consists of multiple documents working together:

### 1. **NNUE_IMPLEMENTATION_PLAN.md** (Main Document, 43 KB)
**The complete, detailed roadmap** covering all phases and decisions.

**Best for**:
- Understanding the full scope
- Deep dives into each phase
- Detailed code change instructions
- Decision trees and contingencies

**Key sections**:
- Executive Summary
- Phase 1: Critical Fixes (days 1-2)
- Phase 2: Training Algorithm (conditional, days 3-5)
- Phase 3: Speed Optimization (days 6-10)
- Phase 4: Validation & Measurement (days 11-14)
- Phase 5: Deployment (ongoing)

**Start here** if you want to understand everything.

---

### 2. **IMPLEMENTATION_QUICK_REFERENCE.md** (10 KB)
**Quick lookup guide** for fast navigation and decision-making.

**Best for**:
- Finding specific code locations
- Quick decision reminders
- Session planning
- Debugging guides

**Key sections**:
- Phase-at-a-glance table
- File locations checklist
- Decision points
- Debugging quick guide
- Session structure template

**Use this** when you know what you need and just want the specifics.

---

### 3. **SESSION_LOG_TEMPLATE.md** (5 KB)
**Template for tracking progress** through implementation sessions.

**Best for**:
- Recording what you did in each session
- Documenting decisions and findings
- Building institutional knowledge
- Preparing for next sessions

**Use this** to create a log after each implementation session.

---

### 4. **This Document** (IMPLEMENTATION_PLAN_INDEX.md)
Navigation guide and quick reference to all planning documents.

---

## 🎯 Quick Start Guide

### For First-Time Review
1. Read this document (5 min)
2. Read Executive Summary in `NNUE_IMPLEMENTATION_PLAN.md` (10 min)
3. Review `IMPLEMENTATION_QUICK_REFERENCE.md` Phase 1 section (10 min)
4. Decide: Ready to start Phase 1?

### For Implementation Sessions
1. Open `IMPLEMENTATION_QUICK_REFERENCE.md`
2. Find your current phase/task
3. Use `SESSION_LOG_TEMPLATE.md` to track progress
4. Reference full plan as needed

### For Debugging
1. Go to `IMPLEMENTATION_QUICK_REFERENCE.md` "Debugging Quick Guide"
2. Find your issue
3. Apply suggested fix
4. Document in session log

---

## 📊 Implementation Overview

### Timeline & Effort

| Phase | Duration | Dev Hours | Expected ELO Gain |
|-------|----------|-----------|-------------------|
| **Phase 1** | Days 1-2 | 2-3h | +0-20 (baseline) |
| **Phase 2** | Days 3-5 | 4-6h | +50-150 (conditional) |
| **Phase 3** | Days 6-10 | 8-10h | +0-100 (optimization) |
| **Phase 4** | Days 11-14 | 3-4h | Measurement |
| **Phase 5** | Days 15+ | 5-10h | Continuous improvement |
| **TOTAL** | 4 weeks | 22-33h | **+150-350 ELO** |

---

### Success Criteria

**Minimum** (Phase 1-2):
- [ ] Training loss decreases
- [ ] Code compiles without errors
- [ ] No functionality regression

**Target** (Phase 1-4):
- [ ] ±20-50 ELO gain measured
- [ ] Ensemble stronger than both pure approaches
- [ ] Clear winner between architectures

**Full Success** (Phase 1-5):
- [ ] Ensemble beats both by ±50+ ELO
- [ ] 2-3× speed improvement
- [ ] Production-ready implementation

---

## 📁 Files to Modify

### Phase 1 (Critical Fixes)
- `src/evaluation/nnue.rs` (weight init + output scaling)

### Phase 2 (Training Fixes, conditional)
- `src/bin/nnue_trainer.rs` (training targets)
- `src/evaluation/nnue_training.rs` (compute targets + config)

### Phase 3 (Optimization)
- `src/evaluation/nnue.rs` (architecture config, incremental updates, SCReLU)
- `src/bin/nnue_trainer.rs` (train multiple architectures)

### Phase 4 (Testing)
- `src/bin/elo-tester.rs` (new file for ELO benchmarking)

### Phase 5 (Deployment)
- `src/evaluation.rs` (search integration)
- `docs/` (documentation updates)

---

## 🔍 Key Decisions

### Decision 1: Phase 2 Required?
**After Phase 1**: Does training show improvement signal?
- **YES** (loss decreasing): Skip Phase 2, go to Phase 3
- **NO** (loss flat/increasing): Do Phase 2

### Decision 2: Which Architecture?
**During Phase 3**: Which network is stronger?
- **SHALLOW** (256→32→1): Keep current
- **DEEP** (512→128→32→1): Switch to deeper
- **TIE**: Keep shallow (simpler, faster)

### Decision 3: Deploy?
**After Phase 4**: What's the ELO result?
- **+50+ ELO**: Deploy NNUE immediately
- **+20-50 ELO**: Deploy ensemble
- **+0-20 ELO**: Continue improvement, consider PST fallback
- **Negative**: Keep PST evaluation

---

## 💾 Reference Documents

### In Repository

**Analysis Documents** (existing):
- `docs/NNUE_DEEP_DIVE_ANALYSIS.md` - Problem analysis (7 critical issues)
- `docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md` - Stockfish comparison
- `docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md` - Quick fix reference

**Implementation Plan** (this set):
- `NNUE_IMPLEMENTATION_PLAN.md` - Full detailed plan
- `IMPLEMENTATION_QUICK_REFERENCE.md` - Quick lookup
- `SESSION_LOG_TEMPLATE.md` - Progress tracking

**Code References**:
- `src/evaluation/nnue.rs` (478 lines) - Network implementation
- `src/evaluation/nnue_training.rs` (388 lines) - Training algorithm
- `src/bin/nnue_trainer.rs` (364 lines) - Training loop

### External Resources

**NNUE Background**:
- NNUE Paper: https://github.com/ynasu87/nnue
- Stockfish NNUE: https://github.com/official-stockfish/Stockfish
- nnue-pytorch: https://github.com/glinscott/nnue-pytorch

**Shogi Engines**:
- YaneuraOu: https://github.com/yaneurao/YaneuraOu
- Kristallweizen: https://github.com/Tama4649/Kristallweizen

---

## ✅ Pre-Implementation Checklist

Before starting Phase 1:

- [ ] Read Executive Summary of main plan
- [ ] Understand the 7 critical issues (from analysis docs)
- [ ] Reviewed Phase 1 section in detail
- [ ] Understand weight initialization problem
- [ ] Understand output scaling problem
- [ ] Know success criteria for Phase 1
- [ ] Have access to code editor
- [ ] Can run `cargo build --release`
- [ ] Can run `cargo run --release --bin nnue_trainer`
- [ ] Have session log template ready

---

## 🚀 Session Templates

### Typical Session Structure

**Session Type 1: Implementation** (1-2 hours)
1. Review relevant section (10-15 min)
2. Make code changes (30-45 min)
3. Compile and test (15-30 min)
4. Document in session log (10-15 min)

**Session Type 2: Verification** (2-3 hours)
1. Run tests (1-2 hours)
2. Monitor progress (30 min)
3. Analyze results (30 min)
4. Update session log (10-15 min)

**Session Type 3: Benchmarking** (2-4 hours)
1. Set up test (30 min)
2. Run 100+ games (1-3 hours)
3. Analyze results (30 min)
4. Document findings (15 min)

### Recommended Session Order

| Session | Focus | Duration | Phase |
|---------|-------|----------|-------|
| 1 | Weight initialization | 1-2h | 1.1 |
| 2 | Output scaling | 1-2h | 1.2 |
| 3 | Training verification | 2-3h | 1.3 |
| 4 | Training targets (if needed) | 2-3h | 2.1 |
| 5 | Training adjustment | 1-2h | 2.2-2.3 |
| 6 | Architecture variants | 2-3h | 3.1 |
| 7 | Incremental updates | 3-4h | 3.2 |
| 8 | Performance profiling | 2-3h | 3.4 |
| 9-11 | ELO benchmarking | 4-6h | 4.1-4.3 |
| 12+ | Deployment & improvement | Ongoing | 5.x |

---

## 🔧 Common Tasks

### "I want to start implementation"
→ Read: `NNUE_IMPLEMENTATION_PLAN.md` (Executive Summary + Phase 1)  
→ Use: `IMPLEMENTATION_QUICK_REFERENCE.md` for code locations  
→ Track: `SESSION_LOG_TEMPLATE.md` for your first session

### "I finished a phase and need to move on"
→ Read: Next phase section in main plan  
→ Check: Decision criteria in quick reference  
→ Copy: Session log template for next session

### "Something went wrong"
→ Check: "Debugging Quick Guide" in quick reference  
→ Read: Relevant phase in main plan  
→ Document: Issue in session log

### "I need to understand why we're doing this"
→ Read: `docs/NNUE_DEEP_DIVE_ANALYSIS.md`  
→ Reference: `docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md`

### "I want to know the current status"
→ Check: Session logs (historical)  
→ Review: Last session's findings  
→ Read: Phase success criteria

---

## 📈 Progress Tracking

### Phase 1 Indicators
- [ ] Code compiles without errors
- [ ] Weight initialization in ±100 range
- [ ] Output scaling in ±100-300 centipawn range
- [ ] Training loss decreases by >10% in 50 iterations

### Phase 2 Indicators
- [ ] Targets come from game outcomes (not eval)
- [ ] Loss decreases monotonically
- [ ] Weight changes visible per iteration
- [ ] Strength improvement measurable

### Phase 3 Indicators
- [ ] Both architectures train to completion
- [ ] Benchmarks show 2-10× speedup (if incremental updates done)
- [ ] Deeper architecture measurably stronger (if tested)

### Phase 4 Indicators
- [ ] 200+ games completed without crashes
- [ ] ELO difference computed with confidence interval
- [ ] Results reproducible across multiple test runs

### Phase 5 Indicators
- [ ] NNUE integrated into search move ordering
- [ ] Ensemble evaluation tested
- [ ] Documentation updated
- [ ] Ready for release

---

## 📝 Notes & Tips

### Code Review Checklist

When making changes:
- [ ] Understand why change needed (from plan)
- [ ] Know expected behavior after change
- [ ] Test compiles before making next change
- [ ] Document change in session log
- [ ] Keep backup of working version

### Training Monitoring

When running training:
- [ ] Monitor loss trend (should decrease)
- [ ] Check weight ranges (shouldn't be extreme)
- [ ] Track iteration rate (should be consistent)
- [ ] Stop if TD error increases for 20+ iterations

### Testing Best Practices

When testing strength:
- [ ] Use at least 100 games for statistics
- [ ] Use consistent time controls
- [ ] Have baseline (PST-only) for comparison
- [ ] Record all results for future reference

---

## 🆘 Getting Help

### If stuck on Phase 1
1. Check: Weight initialization math (in plan)
2. Check: Output scaling formula (in plan)
3. Verify: Code compiles
4. Verify: Training loop runs
5. Debug: Add logging to understand issue

### If stuck on Phase 2
1. Verify: Phase 1 complete and working
2. Check: Training targets computation
3. Verify: Game outcomes stored correctly
4. Check: Learning rate appropriate

### If stuck on Phase 3
1. Verify: Phase 1-2 training works
2. Check: Architecture config correct
3. Benchmark: Before/after metrics
4. Compare: Both architectures at same iteration

### If stuck on Phase 4
1. Verify: Weights properly trained
2. Check: Elo-tester binary compiles
3. Verify: Games play to completion
4. Analyze: Results in context of training

---

## 🎓 Learning Resources

### Understanding NNUE

**Quick intro** (30 min):
- Read: `NNUE_DEEP_DIVE_ANALYSIS.md` Executive Summary
- Understand: 7 critical issues

**Deep understanding** (2 hours):
- Read: `docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md` (all sections)
- Read: NNUE paper (nnasu87/nnue)
- Reference: Stockfish source code

**Practical knowledge** (1 hour):
- Read: `NNUE_IMPLEMENTATION_PLAN.md` Phase 1
- Read: Code locations in `src/evaluation/nnue.rs`
- Understand: Weight initialization + output scaling

---

## 📋 Revision History

| Date | Version | Changes |
|------|---------|---------|
| 2026-04-20 | 1.0 | Initial comprehensive plan |

---

## ✨ Next Steps

**Ready to start?**

1. **Review Phase 1** in `NNUE_IMPLEMENTATION_PLAN.md`
2. **Check quick reference** for code locations
3. **Prepare first session** using template
4. **Start implementation** with Session 1

**Questions before starting?**

Refer to:
- Quick reference for decision criteria
- Main plan for detailed explanations
- Analysis docs for technical understanding

---

**Status**: Ready for Implementation  
**Last Updated**: April 20, 2026  
**Duration Estimate**: 4 weeks, 30-40 hours  
**Expected Outcome**: +150-350 ELO improvement

---

## 📚 All Documents at a Glance

| Document | Size | Purpose | Best For |
|----------|------|---------|----------|
| **NNUE_IMPLEMENTATION_PLAN.md** | 43 KB | Complete detailed roadmap | Understanding everything |
| **IMPLEMENTATION_QUICK_REFERENCE.md** | 10 KB | Fast lookup guide | Quick decisions |
| **SESSION_LOG_TEMPLATE.md** | 5 KB | Progress tracking | Recording sessions |
| **IMPLEMENTATION_PLAN_INDEX.md** | This | Navigation guide | Getting oriented |

---

**You are now ready to begin implementation. Choose your first session and start!**
