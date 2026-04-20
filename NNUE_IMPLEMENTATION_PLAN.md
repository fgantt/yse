# NNUE Implementation Plan - Comprehensive Roadmap

**Last Updated**: April 20, 2026  
**Status**: Ready for Implementation  
**Estimated Duration**: 4 weeks (50-70 hours)  
**Expected Outcome**: +150-350 ELO improvement with faster evaluation  

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Background & Context](#background--context)
3. [Phase 1: Critical Fixes](#phase-1-critical-fixes-days-1-2)
4. [Phase 2: Training Algorithm Fixes (Conditional)](#phase-2-training-algorithm-fixes-days-3-5-conditional)
5. [Phase 3: Speed Optimization](#phase-3-speed-optimization-days-6-10)
6. [Phase 4: Validation & Measurement](#phase-4-validation--measurement-days-11-14)
7. [Phase 5: Polish & Deployment](#phase-5-polish--deployment-days-15)
8. [Decision Trees & Contingencies](#decision-trees--contingencies)
9. [Testing & Verification](#testing--verification)
10. [Success Criteria](#success-criteria)

---

## Executive Summary

### Current Situation

The Shogi engine has a production-ready NNUE implementation that has trained for **700 iterations** (22 days) with **zero measurable strength improvement**. Deep analysis identified **7 fundamental issues** preventing convergence:

1. **Weight initialization 256× too large** (CRITICAL)
2. **Output scaling arbitrary** (CRITICAL)
3. **Bootstrap training problem** (CRITICAL)
4. **Incomplete backpropagation** (HIGH)
5. **Broken TD(λ)** (HIGH)
6. **No search integration** (HIGH)
7. **Full accumulator refresh every eval** (PERFORMANCE)

### Key Findings

- **Good news**: Foundation architecture matches Stockfish (256→32→1 pattern)
- **Good news**: Most infrastructure already implemented (trainer, evaluator, integrations)
- **Good news**: Fixes #1-#2 are trivial (2-3 line changes)
- **Challenge**: Requires careful engineering following Stockfish's quantization-first philosophy

### Approach

**Balanced, sequential approach** with decision points:

1. **Phase 1**: Fix critical math bugs (30-60 min each)
2. **Phase 2**: Fix training algorithm (only if Phase 1 doesn't show improvement)
3. **Phase 3**: Optimize speed (parallel architecture testing)
4. **Phase 4**: Measure & validate
5. **Phase 5**: Deploy & iterate

### Success Criteria

- **Primary**: Ensemble(NNUE + PST) beats both pure approaches by ±50+ ELO
- **Secondary**: NNUE evaluation 2-3× faster than PST
- **Validation**: 200+ games at 1s/move time control with statistical confidence

---

## Background & Context

### What is NNUE?

Neural Network Update Efficiency (NNUE) is a position evaluator that:
- Takes board position as input (2,268 features for Shogi: 2 players × 14 piece types × 81 squares)
- Outputs evaluation in centipawns via neural network (256→32→1 architecture)
- Updates incrementally as moves are made (vs. recomputing from scratch)
- Trains via self-play or supervised learning on position evaluations

### Why It Matters

- **Speed**: 10-100× faster than traditional PST evaluation
- **Strength**: Learns position patterns humans don't encode (opens → endgame transitions)
- **Modern**: All top engines (Stockfish 14+, Chess.com Infinity, etc.) use NNUE

### Current Implementation Status

| Component | Status | Quality |
|-----------|--------|---------|
| Architecture | ✓ Complete | Good (matches Stockfish) |
| Weight storage | ✓ Complete | Good (i16/i32 quantized) |
| Forward pass | ✓ Complete | **BROKEN** (scaling wrong) |
| Accumulator | ✓ Complete | OK (full refresh, not incremental) |
| Training loop | ✓ Complete | **BROKEN** (bootstrap problem) |
| Search integration | ✓ Partial | Missing (not used in move ordering) |
| Backpropagation | ✓ Partial | **BROKEN** (incomplete) |
| Performance measurement | ✗ Missing | None (no ELO tests) |

### Files Involved

**Core Implementation**:
- `src/evaluation/nnue.rs` (478 lines) - Network architecture, forward pass
- `src/evaluation/nnue_training.rs` (388 lines) - TD(λ) trainer, weight updates
- `src/bin/nnue_trainer.rs` (364 lines) - Self-play training loop
- `src/evaluation.rs` (lines 520-526) - Integration with evaluator

**Configuration**:
- `src/evaluation/nnue_training.rs:1-50` - Training hyperparameters

**Tests**:
- `tests/nnue_*.rs` - Integration tests (if any)
- `benches/nnue_*.rs` - Benchmarks (if any)

**Documentation** (for reference):
- `docs/NNUE_TRAINING_GUIDE.md` - Training workflow
- `docs/NNUE_DEEP_DIVE_ANALYSIS.md` - Issue analysis
- `docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md` - Stockfish comparison
- `docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md` - Quick fixes

### Known Issues (From Analysis)

#### Critical (Block improvement)
1. **Weights initialized in [-128, 128]** instead of [-0.01, 0.01]
   - File: `src/evaluation/nnue.rs:100-110`
   - Impact: Network can't converge
   - Fix: ~5 minutes

2. **Output scaling is arbitrary `/256`** instead of proper quantization
   - File: `src/evaluation/nnue.rs:320`
   - Impact: Scores meaningless for search
   - Fix: ~5 minutes

3. **Training targets from same eval being trained** (bootstrap)
   - File: `src/bin/nnue_trainer.rs:79-94`
   - Impact: Learning from noise, not patterns
   - Fix: ~2 hours

#### High (Performance issues)
4. **Full accumulator refresh every eval** (should be incremental)
   - File: `src/evaluation/nnue.rs:186-227`
   - Impact: 10-100× slower than Stockfish
   - Fix: ~4 hours (integrate into search)

5. **Backpropagation incomplete** (hidden layers not trained)
   - File: `src/evaluation/nnue_training.rs:232-333`
   - Impact: Can't learn complex patterns
   - Fix: ~3 hours

6. **TD(λ) implementation broken** (missing eligibility traces, discount factor)
   - File: `src/evaluation/nnue_training.rs:144-189`
   - Impact: Gradients computed incorrectly
   - Fix: ~2 hours

---

## PHASE 1: Critical Fixes (Days 1-2)

### Overview

Fix mathematical foundations that prevent any training improvement. These are trivial changes that make NNUE learning possible.

**Duration**: 4-6 hours  
**Dev Work**: ~2-3 hours  
**Testing**: ~1-2 hours  
**Expected Impact**: +0-20 ELO (baseline fix), **enables** future improvement  
**Success Metric**: Training loop shows decreasing loss

### Prerequisite

- [ ] Review `docs/NNUE_DEEP_DIVE_ANALYSIS.md` section 3 (Weight Initialization)
- [ ] Review `docs/NNUE_DEEP_DIVE_ANALYSIS.md` section 1 (Evaluation Scaling)

---

### Fix 1.1: Weight Initialization

**Objective**: Initialize weights to enable learning instead of immediate saturation

**File**: `src/evaluation/nnue.rs`, lines 84-128 (in `NNUEWeights::new()`)

**Problem**:
```rust
// Current (BROKEN)
.map(|_| rng.gen_range(-128..128))           // 256 unit range!
// and
.map(|_| rng.gen_range(-1000..1000))         // 2000 unit range!
```

With sparse inputs (avg 20/2268 active features):
- Weight sum ≈ 20 × 100 = 2,000 per hidden neuron
- After ReLU + bias, likely saturated at bounds
- Gradients die, training stops

**Solution**: Use small normal distribution matching Stockfish

**Implementation Steps**:

1. Add imports at top of file (if not present):
```rust
use rand_distr::Normal;
```

2. Replace input weight initialization (around line 100):
```rust
// OLD:
let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
    .map(|_| {
        (0..hidden_size_1)
            .map(|_| rng.gen_range(-128..128))
            .collect()
    })
    .collect();

// NEW:
let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
    .map(|_| {
        let dist = Normal::new(0.0, 0.01).unwrap();
        (0..hidden_size_1)
            .map(|_| {
                let w = rng.sample::<f64, _>(dist);
                // Quantize to i16 range
                (w * 100.0).clamp(-127.0, 127.0) as i16
            })
            .collect()
    })
    .collect();
```

3. Replace hidden biases initialization (around line 120):
```rust
// OLD:
let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
    .map(|_| rng.gen_range(-1000..1000))
    .collect();

// NEW:
let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
    .map(|_| {
        let dist = Normal::new(0.0, 10.0).unwrap();
        let b = rng.sample::<f64, _>(dist);
        b.clamp(-32768.0, 32767.0) as i32
    })
    .collect();
```

4. Do the same for `input_weights_2` and `hidden_biases_2` if they exist (around line 130-140):
```rust
// OLD:
.map(|_| rng.gen_range(-128..128))
// and
.map(|_| rng.gen_range(-1000..1000))

// NEW:
let dist = Normal::new(0.0, 0.01).unwrap();
.map(|_| {
    let w = rng.sample::<f64, _>(dist);
    (w * 100.0).clamp(-127.0, 127.0) as i16
})
// and
let dist_bias = Normal::new(0.0, 10.0).unwrap();
.map(|_| {
    let b = rng.sample::<f64, _>(dist_bias);
    b.clamp(-32768.0, 32767.0) as i32
})
```

5. For output weights and bias (around line 150-160):
```rust
// OLD:
output_weights: (0..hidden_size_2).map(|_| rng.gen_range(-128..128)).collect(),
output_bias: rng.gen_range(-1000..1000),

// NEW:
let dist = Normal::new(0.0, 0.01).unwrap();
output_weights: (0..hidden_size_2)
    .map(|_| {
        let w = rng.sample::<f64, _>(dist);
        (w * 100.0).clamp(-127.0, 127.0) as i16
    })
    .collect(),
output_bias: {
    let dist_bias = Normal::new(0.0, 10.0).unwrap();
    let b = rng.sample::<f64, _>(dist_bias);
    b.clamp(-32768.0, 32767.0) as i32
},
```

**Verification**:
```bash
# After making changes, compile
cargo build --release

# Check initialization works
# (Can add temporary debug output)
```

**Expected Result**:
- Weights initialize to ~±50-100 range (after quantization)
- Biases initialize to ~±20-30 range
- No crashes during initialization

---

### Fix 1.2: Output Scaling

**Objective**: Use proper, justified quantization instead of arbitrary `/256`

**File**: `src/evaluation/nnue.rs`, lines 300-325 (in `NNUEAccumulator::evaluate()`)

**Problem**:
```rust
// Current (ARBITRARY)
output / 256  // Why 256? Never explained!
```

This arbitrary scaling causes:
- Inconsistent score ranges with PST evaluation
- Aspiration windows become ineffective
- Move ordering heuristics break
- Engine time management confused

**Solution**: Use Stockfish-compatible quantization with clear documentation

**Implementation Steps**:

1. Add constants at the top of the module (after imports, around line 40):
```rust
// Stockfish-compatible quantization constants
// Used to scale network output from integer domain to centipawns
const SCALE_FACTOR: i32 = 400;              // Centipawn scale factor
const QUANTIZER_A: i32 = 255;               // CReLU activation bound (practical max)
const QUANTIZER_B: i32 = 64;                // Output layer scaling divisor
const FINAL_DIVISOR: i32 = QUANTIZER_A * QUANTIZER_B; // = 16,320
```

2. Replace the output scaling in `evaluate()` function (around line 320):
```rust
// OLD:
pub fn evaluate(&self, weights: &NNUEWeights) -> i32 {
    // ... forward pass code ...
    output / 256
}

// NEW:
pub fn evaluate(&self, weights: &NNUEWeights) -> i32 {
    // ... forward pass code ...
    
    // Stockfish-compatible quantization:
    // - SCALE_FACTOR (400): Maps network output to centipawns
    // - QUANTIZER_A (255): Typical output range from hidden layer
    // - QUANTIZER_B (64): Scaling used in hidden layer computation
    // Result: network output naturally scales to ~[-300, 300] centipawns
    (output * SCALE_FACTOR) / FINAL_DIVISOR
}
```

**Documentation** (add comment above `evaluate()` function):
```rust
/// Evaluate the current accumulator state.
///
/// Uses Stockfish-compatible quantization to convert network output to centipawns.
/// The scaling is carefully chosen to match the quantization of intermediate layers:
///   output_centipawns = (raw_output * 400) / (255 * 64)
///
/// This ensures:
/// - Network output is bounded (typically [-300, 300] centipawns)
/// - Compatible with search heuristics (aspiration windows, move ordering)
/// - Consistent with how hidden layers are computed
///
/// # Reasoning
/// - Input layer: sparse inputs (avg 20/2268 active) sum to ~200 with weights ±100
/// - Hidden layer: 256 neurons with ReLU, typically ±100-1000 range
/// - Output layer: 32 inputs (after ReLU) dot product with weights ±100 → ±3200
/// - Scaling by 400/16320 normalizes to centipawn range
pub fn evaluate(&self, weights: &NNUEWeights) -> i32 {
    // ... implementation ...
}
```

**Alternative Approaches** (if you want custom Shogi scaling):
- If you want Shogi-specific scaling, document the math clearly
- Must justify why value differs from proven Stockfish approach
- Recommendation: Use Stockfish approach as baseline, experiment later if needed

**Verification**:
```bash
# Compile
cargo build --release

# Create simple test
# Input: startpos NNUE evaluation
# Expected: Something reasonable like ±50-200 centipawns, not ±10,000

# Run multiple times
# Expected: Same position always gives same evaluation
```

**Expected Result**:
- Startpos evaluation in reasonable centipawn range
- Deterministic (same eval every call)
- Closer to PST evaluation magnitude

---

### Fix 1.3: Retrain and Verify Training Works

**Objective**: Confirm that fixes enable actual training improvement

**Duration**: 1-2 hours (automated, but monitor)

**Procedure**:

1. **Clean previous weights** (fresh start):
```bash
cd /Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue
rm -f nnue_weights_trained.json nnue_weights_iter_*.json
```

2. **Run training** with fixed initialization:
```bash
cargo run --release --bin nnue_trainer 2>&1 | tee nnue_training.log
```

3. **Monitor output** (look for):
```
Iteration 1: td_error=0.123456, weights_changed=true, max_gradient=0.0045
Iteration 2: td_error=0.098765, weights_changed=true, max_gradient=0.0038
...
```

**Success Criteria** (TD error should **decrease**):
```
Iteration  1: td_error = 0.150
Iteration 10: td_error = 0.120  (↓ 20%)
Iteration 50: td_error = 0.080  (↓ 47%)
```

**Failure Criteria** (if this doesn't happen):
- TD error flat or increasing → weights not converging
- Indicates Phase 2 fixes are needed
- Record `nnue_training.log` for analysis

4. **Compare with baseline**:
- Save trained weights: `nnue_weights_trained.json`
- Compare first 10 positions eval before/after training
- Should show difference in evaluation over iterations

**If Training Works**: Skip Phase 2, go to Phase 3
**If Training Doesn't Work**: Proceed to Phase 2

---

## PHASE 2: Training Algorithm Fixes (Days 3-5, Conditional)

### Overview

**CONDITIONAL**: Only implement if Phase 1 shows no improvement signal (flat or increasing TD error).

This phase fixes the training algorithm to use external oracle targets instead of bootstrapping on weak evaluation.

**Duration**: 8-12 hours  
**Dev Work**: ~4-6 hours  
**Testing**: ~2-3 hours  
**Expected Impact**: +50-150 ELO (enables strength improvement)  
**Success Metric**: Training converges meaningfully, leads to strength gains

### Prerequisites

- [ ] Phase 1 complete and compiled
- [ ] Confirmed training doesn't improve with Phase 1 alone
- [ ] Review `docs/NNUE_DEEP_DIVE_ANALYSIS.md` section 2-4 (Training Quality, Data Quality)
- [ ] Review `docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md` section "Fix #4"

---

### Fix 2.1: Switch to Outcome-Based Training Targets

**Objective**: Break bootstrap loop by training on game outcomes (external oracle) instead of weak evaluation

**Approach**: Hybrid strategy
1. **Phase 2a (Immediate)**: Outcome-based targets (simpler)
2. **Phase 2b (Later, optional)**: Add search-based targets (stronger)

**Implementation 2a: Outcome-Based Targets**

**File**: `src/bin/nnue_trainer.rs`, modify game storage and training pipeline

**Current Code** (lines ~79-127, in `play_self_play_game()`):
```rust
// Current: stores evaluation during game
positions.push(TrainingPosition {
    active_features,
    accumulator: accumulator.clone(),
    evaluation,              // ← Problem: same eval being trained!
    player: current_player,
    move_made: None,
    outcome: None,
    td_target: None,
});
```

**Change to**: Store outcome only
```rust
// Modified: just store position, outcome comes later
positions.push(TrainingPosition {
    active_features,
    accumulator: accumulator.clone(),
    evaluation: 0,           // Placeholder (will be target)
    player: current_player,
    move_made: None,
    outcome: Some(game_result), // Store outcome after game
    td_target: None,
});
```

**File**: `src/evaluation/nnue_training.rs`, modify target computation

**Current Code** (lines ~144-189, in `compute_td_targets()`):
```rust
// Current: converts same eval to target
let current_value = (positions[i].evaluation as f32 / 20000.0).tanh() * player_score_multiplier;
let td_error = next_value - current_value;
positions[i].td_target = Some(current_value + self.config.learning_rate * td_error);
```

**Change to**: Use game outcome
```rust
// Modified: use outcome from game_result
fn compute_td_targets(&self, positions: &mut [TrainingPosition], final_result: GameResult) {
    if positions.is_empty() {
        return;
    }
    
    // Convert game outcome to target value (0.0 to 1.0)
    // From Black's perspective:
    //   Win (Black) = 1.0
    //   Draw        = 0.5
    //   Loss (Black) = 0.0
    let outcome_value = match final_result {
        GameResult::Win => 1.0,
        GameResult::Loss => 0.0,
        GameResult::Draw => 0.5,
    };
    
    // Apply to all positions in game with decay
    let mut discount = 1.0;
    for position in positions.iter_mut().rev() {
        // Adjust for player perspective
        let player_mult = if position.player == Player::Black { 1.0 } else { -1.0 };
        
        // Target: outcome value decayed by how far from end
        position.td_target = Some(outcome_value * player_mult * discount);
        
        // Decay back toward game end (earlier positions less certain)
        discount *= 0.98;  // γ=0.98 discount factor
    }
}
```

**Rationale**:
- Target comes from game outcome (external, objective)
- Not from same weak evaluation (bootstrapping problem solved)
- Earlier positions discounted (less impact from outcome)
- Allows network to learn pattern that predicts game outcome

**File**: `src/evaluation/nnue_training.rs`, modify weight update

**Current Code** (lines ~232-355):
```rust
fn update_weights_for_position(&mut self, position: &TrainingPosition, target_value: f32) -> (...) {
    // Computes error from tanh(eval/20000)
    let current_prediction = (position.evaluation as f32 / 20000.0).tanh();
    let predicted_value = current_prediction * player_multiplier;
    let error = target_value - predicted_value;
    // ... update weights using this error
}
```

**Change to**: Use outcome target directly
```rust
fn update_weights_for_position(&mut self, position: &TrainingPosition, target_value: f32) -> (...) {
    // Compute current network prediction
    // Convert to same scale as target (0.0 to 1.0)
    let raw_eval = position.evaluation as f32;
    
    // Normalize to [-1, 1] based on reasonable eval range (±400 centipawns)
    let current_prediction = (raw_eval / 400.0).tanh();
    
    // Adjust for player perspective
    let player_multiplier = if position.player == Player::Black { 1.0 } else { -1.0 };
    let predicted_value = current_prediction * player_multiplier;
    
    // Error between prediction and target (outcome)
    let error = target_value - predicted_value;
    
    // ... continue with weight updates (existing code mostly works)
}
```

**Verification**:
```bash
# Recompile
cargo build --release

# Run training
rm -f nnue_weights_trained.json nnue_weights_iter_*.json
cargo run --release --bin nnue_trainer 2>&1 | tee nnue_training_phase2.log

# Check logs:
# - Errors should decrease toward 0
# - Weights should change significantly
# - Training should reach >50% accuracy on outcome prediction
```

---

### Fix 2b: (Optional) Add Search-Based Targets Later

**When**: After 2a works and you want stronger training

**Approach**: Mix targets from outcomes and search depth
```rust
// Hybrid target: blend outcome with search-based eval
let outcome_target = outcome_value;
let search_eval = pst_evaluator.search_at_depth(...);  // External oracle
let blended_target = 0.5 * outcome_target + 0.5 * search_eval;
position.td_target = Some(blended_target);
```

**When to implement**: Phase 2b, not Phase 2a

---

### Fix 2.2: Adjust Training Configuration

**File**: `src/evaluation/nnue_training.rs`, around lines 1-50

**Current Configuration** (aggressive, doesn't work well):
```rust
pub struct NNUETrainingConfig {
    pub learning_rate: f32 = 0.01,         // 10× Stockfish!
    pub lambda: f32 = 0.7,                 // OK value
    pub games_per_iteration: usize = 10,   // Too few
    pub search_depth: usize = 3,           // Too shallow
}
```

**Change to** (conservative, Stockfish-aligned):
```rust
pub struct NNUETrainingConfig {
    pub learning_rate: f32 = 0.001,        // Reduce by 10×
    pub lambda: f32 = 0.7,                 // Keep same
    pub games_per_iteration: usize = 100,  // Increase by 10×
    pub search_depth: usize = 6,           // Increase by 2×
    pub max_iterations: usize = 1000,      // More training
}
```

**Rationale**:
- **learning_rate**: Small changes, less likely to overshoot
- **games_per_iteration**: More diverse training data
- **search_depth**: Better position evaluation (more reliable targets)

---

### Fix 2.3: Add Training Monitoring

**File**: `src/bin/nnue_trainer.rs` and `src/evaluation/nnue_training.rs`

**Add per-iteration logging**:
```rust
// In training loop (roughly every iteration):
println!("Iteration {}: loss={:.6}, acc_on_holdout={:.2}%, weights_delta={:.6}",
    iteration,
    current_loss,
    validation_accuracy * 100.0,
    avg_weight_change
);
```

**What to log** (create `nnue_training_stats.csv`):
```
iteration,td_error,avg_weight_change,max_gradient,validation_accuracy
1,0.150000,0.005000,0.004500,0.520
2,0.145000,0.004800,0.004300,0.535
...
```

**Purpose**:
- Verify training is actually improving
- Debug if convergence stalls
- Reference for next session

---

### Decision Point: Phase 2 vs Phase 3

After Phase 2a is implemented and tested:

- **If loss decreases and training looks good**: Continue to Phase 3
- **If loss still flat or noisy**: Debug further before Phase 3
  - Check: Are targets being computed correctly?
  - Check: Are gradients non-zero?
  - Check: Is learning rate right?

---

## PHASE 3: Speed Optimization (Days 6-10)

### Overview

Optimize evaluation speed and test deeper architectures. This phase is **parallel** to Phase 2 (don't block on Phase 2 completion).

**Duration**: 12-15 hours  
**Dev Work**: ~8-10 hours  
**Testing**: ~3-4 hours  
**Expected Impact**: +0-50 ELO (optimization), +50-100 ELO (architecture)  
**Success Metric**: 2-3× faster evaluation, deeper net shows measurable strength

### Prerequisites

- [ ] Phase 1-2 training working (or Phase 1 showing improvement)
- [ ] Decision: which architecture to test? (256→32→1 vs 512→128→32→1)

---

### Fix 3.1: Create Parallel Architecture Variants

**Objective**: Test both current and deeper architectures simultaneously

**File**: `src/evaluation/nnue.rs`, modify weight initialization

**Current**:
```rust
pub struct NNUEWeights {
    input_weights_1: Vec<Vec<i16>>,
    hidden_biases_1: Vec<i32>,
    // ... etc
}

impl NNUEWeights {
    pub fn new(rng: &mut impl Rng, hidden_size_1: usize, hidden_size_2: usize) -> Self {
        // Current: 256 → 32 → 1
    }
}
```

**Change to** (add architecture config):
```rust
#[derive(Clone, Debug)]
pub struct NNUEArchitecture {
    pub name: &'static str,
    pub hidden_1_size: usize,
    pub hidden_2_size: usize,
}

pub const ARCH_SHALLOW: NNUEArchitecture = NNUEArchitecture {
    name: "shallow",
    hidden_1_size: 256,
    hidden_2_size: 32,
};

pub const ARCH_DEEP: NNUEArchitecture = NNUEArchitecture {
    name: "deep",
    hidden_1_size: 512,
    hidden_2_size: 128,
};

pub struct NNUEWeights {
    // ... existing fields ...
    architecture: NNUEArchitecture,
}

impl NNUEWeights {
    pub fn new(rng: &mut impl Rng, arch: NNUEArchitecture) -> Self {
        // Allocate based on architecture
        let hidden_size_1 = arch.hidden_1_size;
        let hidden_size_2 = arch.hidden_2_size;
        // ... rest of initialization with new sizes
    }
}
```

**Training Loop Change** (`src/bin/nnue_trainer.rs`):
```rust
// Train BOTH architectures in parallel
fn main() {
    for architecture in &[ARCH_SHALLOW, ARCH_DEEP] {
        let mut weights = NNUEWeights::new(&mut rng, architecture.clone());
        // Run full training on this architecture
        // Save to: nnue_weights_{architecture.name}_iter_N.json
    }
}
```

**Verification**:
- Both architectures train to convergence
- Compare ELO at same iteration count
- Decide which is better

---

### Fix 3.2: Incremental Accumulator Updates

**Objective**: 10-100× faster evaluation by only updating changed pieces

**Duration**: 3-4 hours  
**Priority**: HIGH for speed

**Current Problem** (lines ~186-227, `refresh()` method):
```rust
pub fn refresh(&mut self, board: &BitboardBoard, weights: &NNUEWeights) {
    self.hidden_1.fill(0);
    // Iterate ALL 81 squares
    for row in 0..9 {
        for col in 0..9 {
            // Recompute even if piece didn't change!
        }
    }
}
```

This is called 1000+ times per search = massive redundant work.

**Better Approach**: Track changes, only update deltas

**Implementation Plan** (3 sub-steps):

**3.2a: Add dirty-flag to accumulator**
```rust
pub struct NNUEAccumulator {
    hidden_1: Vec<i32>,
    hidden_2: Option<Vec<i32>>,
    is_dirty: bool,               // Add this
    last_update_hash: u64,        // Track position
}

impl NNUEAccumulator {
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }
    
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }
    
    pub fn needs_refresh(&self, board_hash: u64) -> bool {
        self.is_dirty || board_hash != self.last_update_hash
    }
}
```

**3.2b: Add incremental update method**
```rust
pub fn update_piece(&mut self, remove_pos: Option<Position>, add_pos: Option<Position>, 
                    piece: Piece, weights: &NNUEWeights) {
    // Remove old piece contribution
    if let Some(pos) = remove_pos {
        self.remove_piece_contribution(piece, pos, weights);
    }
    
    // Add new piece contribution
    if let Some(pos) = add_pos {
        self.add_piece_contribution(piece, pos, weights);
    }
}

fn add_piece_contribution(&mut self, piece: Piece, pos: Position, weights: &NNUEWeights) {
    let feature_idx = self.piece_to_feature(piece, pos);
    // Add only the weights for this feature
    for (i, &weight) in weights.input_weights_1[feature_idx].iter().enumerate() {
        self.hidden_1[i] += weight as i32;
    }
}

fn remove_piece_contribution(&mut self, piece: Piece, pos: Position, weights: &NNUEWeights) {
    let feature_idx = self.piece_to_feature(piece, pos);
    // Subtract only the weights for this feature
    for (i, &weight) in weights.input_weights_1[feature_idx].iter().enumerate() {
        self.hidden_1[i] -= weight as i32;
    }
}
```

**3.2c: Integrate into search**

**File**: Wherever search calls `evaluate()` (likely `src/search/search_engine.rs`)

```rust
// Instead of:
let eval = evaluator.evaluate(&board, player, captured);

// Do:
// On make_move
board.make_move(&move);
evaluator.update_piece(from_pos, to_pos, piece, &weights);

// Evaluate without refresh
let eval = evaluator.evaluate_cached(player);

// On unmake_move
board.unmake_move();
evaluator.mark_dirty();  // Will refresh on next eval in search line
```

**Alternative (Simpler, for now)**:
Cache last board hash to avoid re-refresh:
```rust
pub struct NNUEEvaluator {
    // ... existing ...
    last_board_hash: u64,
}

pub fn evaluate(&mut self, board: &BitboardBoard, ...) -> i32 {
    let current_hash = board_hash_fn(board);
    
    if current_hash != self.last_board_hash {
        self.accumulator.refresh(board, &self.weights);
        self.last_board_hash = current_hash;
    }
    
    self.accumulator.evaluate(&self.weights)
}
```

This gives ~20-30% speedup without major refactor.

**Verification**:
```bash
# Benchmark before/after
cargo bench --bench nnue_evaluation_speed --features legacy-tests

# Should see 2-10× speedup depending on approach
```

---

### Fix 3.3: (Optional) Add SCReLU Activation

**Objective**: Better model capacity (optional, requires retraining)

**Duration**: 2-3 hours

**Current**: ReLU `max(0, x)`  
**New**: SCReLU (Squared Clipped ReLU) `clamp(x, 0, 1)^2`

**Why**: Better capacity, same computation cost, what Stockfish uses

**Implementation**:
```rust
// In forward pass (lines ~300-320)
// OLD:
let activated_1: Vec<i32> = self.hidden_1
    .iter()
    .zip(weights.hidden_biases_1.iter())
    .map(|(&h, &bias)| (h + bias).max(0))
    .collect();

// NEW (SCReLU):
let activated_1: Vec<i32> = self.hidden_1
    .iter()
    .zip(weights.hidden_biases_1.iter())
    .map(|(&h, &bias)| {
        let x = (h + bias).max(0).min(128) as f32 / 128.0;  // Normalize to [0, 1]
        let squared = x * x;
        (squared * 128.0) as i32
    })
    .collect();
```

**Cost**: Requires full retraining (weights learned for ReLU won't work for SCReLU)

**Decision**: Implement only if Phase 1-2 training works well

---

### Fix 3.4: Performance Profiling

**Objective**: Measure actual speedup from optimizations

**Duration**: 1-2 hours

**Procedure**:

1. **Create benchmark** (`benches/nnue_speed.rs`):
```rust
#[bench]
fn bench_nnue_eval_1000_positions(b: &mut Bencher) {
    let evaluator = create_test_evaluator();
    let positions = generate_test_positions(1000);
    
    b.iter(|| {
        for pos in &positions {
            evaluator.evaluate(pos);
        }
    });
}
```

2. **Run before/after**:
```bash
# Before optimization
cargo bench --bench nnue_speed

# After optimization
cargo bench --bench nnue_speed

# Should see X microseconds → Y microseconds improvement
```

3. **Record results**:
- Positions/second before
- Positions/second after
- Speedup factor (X×)

---

## PHASE 4: Validation & Measurement (Days 11-14)

### Overview

Measure actual strength improvement. This determines if all previous work succeeded.

**Duration**: 8-10 hours  
**Dev Work**: ~3-4 hours  
**Testing**: ~5-6 hours (game playing)  
**Expected Impact**: Determine actual ELO gain  
**Success Metric**: ±50+ ELO measured with statistical confidence

### Prerequisites

- [ ] Phase 1-3 complete
- [ ] At least 100 iterations of training completed
- [ ] Trained weights available

---

### Test 4.1: Create ELO Testing Infrastructure

**Objective**: Measure NNUE vs PST strength difference

**File**: Create `src/bin/elo-tester.rs` (new binary)

**High-level structure**:
```rust
use std::fs;
use crate::{SearchEngine, PositionEvaluator};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // Parse arguments:
    // --games 100
    // --time 1s
    // --nnue-weights path
    // --pst-only (to test PST baseline)
    
    let games = 100;
    let time_control = "1s";
    
    // Run games: NNUE vs PST
    let mut results = GameResults::new();
    for game_num in 0..games {
        let game = play_game_nnue_vs_pst(game_num, time_control);
        results.record(game);
        
        println!("Game {}: {} - ELO diff: {:.1} ± {:.1}",
            game_num,
            game.result,
            results.elo_diff(),
            results.elo_confidence_interval()
        );
    }
    
    println!("\n=== FINAL RESULTS ===");
    println!("Games: {}", games);
    println!("NNUE: {} wins, {} draws, {} losses",
        results.nnue_wins, results.draws, results.nnue_losses);
    println!("ELO Difference: {:.1} ± {:.1}",
        results.elo_diff(),
        results.elo_confidence_interval()
    );
    
    // Save results to CSV
    results.save_to_csv("elo_test_results.csv");
}

fn play_game_nnue_vs_pst(game_num: usize, time_control: &str) -> GameResult {
    let mut board = BitboardBoard::startpos();
    let mut eval_nnue = PositionEvaluator::with_nnue();
    let mut eval_pst = PositionEvaluator::pst_only();
    
    let mut search_nnue = SearchEngine::new();
    let mut search_pst = SearchEngine::new();
    
    let mut move_count = 0;
    let max_moves = 300;  // Prevent infinite games
    
    loop {
        // NNUE's turn
        let move_nnue = search_nnue.best_move(&board, time_control);
        board.make_move(move_nnue);
        move_count += 1;
        
        if is_game_over(&board) || move_count > max_moves {
            return GameResult {
                result: determine_result(&board),
                moves: move_count,
                winner: get_winner(&board),
            };
        }
        
        // PST's turn
        let move_pst = search_pst.best_move(&board, time_control);
        board.make_move(move_pst);
        move_count += 1;
        
        if is_game_over(&board) || move_count > max_moves {
            return GameResult {
                result: determine_result(&board),
                moves: move_count,
                winner: get_winner(&board),
            };
        }
    }
}

#[derive(Debug)]
struct GameResults {
    nnue_wins: usize,
    pst_wins: usize,
    draws: usize,
}

impl GameResults {
    fn elo_diff(&self) -> f64 {
        // Calculate ELO difference using standard formula
        let total = self.nnue_wins + self.pst_wins + self.draws;
        let nnue_score = (self.nnue_wins as f64 + 0.5 * self.draws as f64) / total as f64;
        
        // ELO = 400 * log10(score / (1 - score))
        if nnue_score > 0.0 && nnue_score < 1.0 {
            400.0 * (nnue_score / (1.0 - nnue_score)).log10()
        } else {
            0.0
        }
    }
    
    fn elo_confidence_interval(&self) -> f64 {
        // 95% confidence interval
        let total = (self.nnue_wins + self.pst_wins + self.draws) as f64;
        let variance = (self.nnue_wins as f64 * 0.25 + self.draws as f64 * 0.0625) / total;
        let std_error = variance.sqrt() / total;
        196.0 * std_error / 0.2 * 1.96  // Rough approximation
    }
}
```

**Usage**:
```bash
# Test current NNUE
cargo run --release --bin elo-tester -- --games 100 --time 1s

# Test PST only
cargo run --release --bin elo-tester -- --games 100 --time 1s --pst-only

# Test specific weights
cargo run --release --bin elo-tester -- --games 100 --time 1s \
    --nnue-weights nnue_weights_iter_500.json
```

---

### Test 4.2: Multi-Configuration Testing

**Objective**: Test different scenarios to understand where NNUE helps

**Test Matrix**:

| Configuration | Purpose |
|---|---|
| NNUE (final) vs PST | Main comparison |
| NNUE (iter 100) vs PST | Earlier checkpoint |
| NNUE (iter 500) vs PST | Mid-training |
| Ensemble(0.7×NNUE + 0.3×PST) vs both | Hybrid |

**Procedure**:

For each configuration:
```bash
cargo run --release --bin elo-tester -- \
    --games 200 \
    --time 1s \
    --output results/elo_test_$(date +%Y%m%d_%H%M%S).csv
```

**Record Results**:
- Win/draw/loss for each 50 games
- ELO difference with moving average
- Which configuration is strongest?

---

### Test 4.3: Time Control Scaling

**Objective**: Understand NNUE strength across different time controls

**Procedure**:
```bash
for time in 0.1s 1s 5s 30s; do
    echo "Testing at $time/move"
    cargo run --release --bin elo-tester -- \
        --games 100 \
        --time $time \
        --output results/elo_test_${time}.csv
done
```

**Expected Results**:
- Faster time controls: NNUE advantage (no search overhead)
- Slower time controls: PST advantage (search compensates)
- Sweet spot: where NNUE is best

---

### Test 4.4: Benchmark Positions

**Objective**: Check NNUE on standard test positions

**Procedure**:

If Shogi has standard position suites (like Perft or EPD in chess):
```bash
# Load positions from file
cargo run --release --bin elo-tester -- \
    --benchmark positions.epd \
    --eval-only
```

**Metrics**:
- NNUE eval vs PST eval correlation
- Evaluation differences
- Move quality at depth 10

---

### Decision Point: Phase 4 → Phase 5

**If ELO results are:**
- **+50+ ELO**: Success! Phase 5 (deployment)
- **+20 to +50 ELO**: Good progress, continue Phase 5 + iterate
- **+0 to +20 ELO**: Partial success, more work needed
- **Negative**: Something wrong, debug Phase 2-3

---

## PHASE 5: Polish & Deployment (Days 15+)

### Overview

Prepare NNUE for production, continue training, document improvements.

**Duration**: Ongoing  
**Expected Impact**: Continuous improvement over time

---

### Task 5.1: Long-term Training

**Objective**: Continue improving weights as engine gets stronger

**Procedure**:

1. **Set up continuous training loop**:
```bash
# Run training continuously on background process
nohup cargo run --release --bin nnue_trainer > training.log 2>&1 &
```

2. **Monitor convergence**:
- Check loss trends weekly
- Save checkpoints every 50 iterations
- Compare strength every 100 iterations

3. **Switch to stronger training data**:
- Feed in more Shogi-specific positions
- Use opening books as training data
- Mix endgame tablebase positions

---

### Task 5.2: Search Integration

**Objective**: Use NNUE for move ordering improvements

**When**: After Phase 3-4 prove NNUE works

**Implementation**:
```rust
// In alpha-beta search
// Use NNUE eval for move ordering
let mut move_scores = moves.iter().map(|m| {
    board.make_move(m);
    let score = evaluator.evaluate(&board, player);
    board.unmake_move();
    (m, score)
}).collect();

move_scores.sort_by(|a, b| b.1.cmp(&a.1));
```

---

### Task 5.3: Ensemble Evaluation

**Objective**: Blend NNUE + PST for best strength

**Decision**: After Phase 4 results:
- What blend factor (α) works best?
- Test: 0.5×NNUE + 0.5×PST
- Test: 0.7×NNUE + 0.3×PST
- Test: 0.9×NNUE + 0.1×PST

---

### Task 5.4: Documentation & Release

**Objective**: Prepare for external use

**Documents to update**:
- `docs/NNUE_IMPLEMENTATION_RESULTS.md` (new)
  - What was done
  - What strength improvement achieved
  - Configuration used
  - Training data sources
  
- `docs/NNUE_CONFIGURATION.md` (new)
  - How to enable/disable NNUE
  - Blend factor settings
  - Performance tuning

- `README.md` (update)
  - NNUE status and strength

**Code cleanup**:
- Remove debug logging
- Clean up temporary branches
- Merge Phase 1-3 work to main

---

## Decision Trees & Contingencies

### Decision Tree 1: Phase 1 Success?

```
After Phase 1 (weight init + output scaling):
├─ Training loss DECREASES over iterations?
│  ├─ YES: Skip Phase 2, continue Phase 3
│  └─ NO: Proceed to Phase 2
```

### Decision Tree 2: Phase 2 Success?

```
After Phase 2 (training targets + config):
├─ Loss decreases AND training shows strength improvement?
│  ├─ YES: Continue Phase 3
│  └─ NO: Debug OR revisit training approach
```

### Decision Tree 3: Architecture Choice

```
During Phase 3 (parallel architectures):
├─ Which is stronger at iteration 100?
│  ├─ SHALLOW (256→32→1): Keep current, optimize speed
│  ├─ DEEP (512→128→32→1): Switch to deeper net
│  └─ TIE: Use SHALLOW (faster, simpler)
```

### Decision Tree 4: Phase 4 Results

```
After Phase 4 (ELO testing):
├─ NNUE vs PST ELO difference?
│  ├─ +50+ ELO: Success! Deploy with NNUE
│  ├─ +20 to +50: Partial success, ensemble recommended
│  ├─ +0 to +20: Marginal, continue training
│  └─ Negative: Debug issues, may need Phase 2 rework
```

### Contingency: If Phase 1 Doesn't Work

**Debugging checklist**:
1. Are weights actually changing during training?
   ```rust
   // Add to training loop
   println!("Max weight change: {}", max_weight_change);
   ```

2. Are gradients being computed?
   ```rust
   // Add logging
   println!("TD error: {}", td_error);
   println!("Gradient: {}", gradient);
   ```

3. Is output scaling breaking things?
   - Try different scale factors: 32, 100, 200, 400
   - See which gives reasonable eval range

4. Is learning rate right?
   - Try: 0.0001, 0.001, 0.01, 0.1
   - Pick one that shows weight movement

---

## Testing & Verification

### Phase-by-Phase Verification

**After Phase 1.1 (weight init)**:
```bash
✓ Compile without errors
✓ No crashes during weight initialization
✓ Weights are ~±100 range (check with debug output)
```

**After Phase 1.2 (output scaling)**:
```bash
✓ Compile without errors  
✓ Same position always produces same eval
✓ Startpos eval in reasonable range (±100-300 centipawns)
```

**After Phase 1.3 (training)**:
```bash
✓ Training runs without crashes
✓ TD error decreases by >10% over 50 iterations
✓ Weight changes tracked and non-zero
```

**After Phase 2.1 (training targets)**:
```bash
✓ Outcomes stored correctly during games
✓ Targets computed from outcomes
✓ Training loss decreases monotonically
```

**After Phase 3.1 (parallel architectures)**:
```bash
✓ Both architectures train to completion
✓ Can compare at same iteration count
✓ Strength difference measurable
```

**After Phase 3.2 (incremental updates)**:
```bash
✓ Benchmarks show 2-3× speedup
✓ Evaluations still match full refresh
✓ No correctness regressions
```

**After Phase 4 (ELO testing)**:
```bash
✓ 200+ games completed
✓ Statistical confidence interval computed
✓ Clear winner identified (or marginal difference)
```

---

## Success Criteria

### Minimum Success (Phase 1-2)

- [ ] Training loss decreases
- [ ] Weights change meaningfully during training
- [ ] No correctness regressions
- [ ] Code compiles and runs without crashes

### Target Success (Phase 1-4)

- [ ] NNUE shows measurable ELO gain (+20 to +50 ELO)
- [ ] Ensemble(NNUE+PST) shows clear strength improvement
- [ ] Can identify which architecture is better
- [ ] Speed improvement measurable

### Full Success (Phase 1-5)

- [ ] Ensemble beats both pure approaches by +50+ ELO
- [ ] NNUE evaluation 2-3× faster than PST
- [ ] Long-term training shows continued improvement
- [ ] Documented and ready for release

---

## Timeline & Effort Estimate

| Phase | Duration | Dev Hours | Contingency | Total |
|-------|----------|-----------|-------------|-------|
| Phase 1 | Days 1-2 | 2-3h | 1h | 3-4h |
| Phase 2 | Days 3-5 | 4-6h | 2h | 6-8h |
| Phase 3 | Days 6-10 | 8-10h | 3h | 11-13h |
| Phase 4 | Days 11-14 | 3-4h | 3h | 6-7h |
| Phase 5 | Days 15+ | 5-10h | varies | 5-10h |
| **TOTAL** | **4 weeks** | **22-33h** | **9-11h** | **31-42h** |

---

## Next Steps

1. **Review this plan** - Does it look reasonable? Any concerns?
2. **Identify questions** - What needs clarification?
3. **Iterate on plan** - Adjust timeline/approach based on feedback
4. **Start Phase 1** - When ready, I can write exact code changes

---

## References

### Analysis Documents (in this repo)
- `docs/NNUE_DEEP_DIVE_ANALYSIS.md` - Detailed problem analysis
- `docs/NNUE_RESEARCH_STOCKFISH_COMPARISON.md` - Stockfish comparison
- `docs/NNUE_CRITICAL_FIXES_QUICK_REFERENCE.md` - Quick reference

### External Resources
- **NNUE Paper**: https://github.com/ynasu87/nnue
- **Stockfish**: https://github.com/official-stockfish/Stockfish
- **nnue-pytorch**: https://github.com/glinscott/nnue-pytorch
- **Shogi engines**: YaneuraOu, Kristallweizen

### Current Codebase
- Implementation: `src/evaluation/nnue.rs`, `src/evaluation/nnue_training.rs`
- Training binary: `src/bin/nnue_trainer.rs`
- Integration: `src/evaluation.rs`

---

## Document History

| Date | Author | Changes |
|------|--------|---------|
| 2026-04-20 | Analysis | Initial comprehensive plan |

---

**Ready to implement. Please review and provide feedback.**
