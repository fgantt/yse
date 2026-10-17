# Deep-Dive Analysis: Why NNUE Might Not Be Improving Engine Strength

## Executive Summary

The NNUE implementation in this codebase has **fundamental architectural and training issues** that prevent it from improving engine strength. The main problems are:

1. **NNUE is only evaluated as a fallback, not integrated into search**
2. **Evaluation scaling (/ 256) creates score ranges incompatible with search heuristics**
3. **Training lacks proper target values and uses flawed TD(λ) implementation**
4. **Data quality is unknown - self-play with weak evaluation bootstraps weak model**
5. **Network architecture is not optimized for the problem**
6. **No actual performance benchmarks comparing NNUE vs PST**

---

## 1. Evaluation Calibration - SCORES ARE MEANINGLESS

### Problem: Completely Wrong Scale

**File**: `src/evaluation/nnue.rs` lines 314-320

```rust
// Scale output to reasonable evaluation range (centipawns)
// Using / 256 as a compromise that works for both existing trained weights 
// and future training
output / 256
```

**Issues**:
- Output scaling is **ad-hoc and arbitrary** (/ 256)
- No justification for this specific value
- Hidden layer 2 uses intermediate scaling: `(value * weight as i32) / 64` (line 311)
- This creates **cascading scale factors** that don't align

### Evidence of Bad Scaling:

**File**: `docs/NNUE_SCALING_ISSUE.md`

- Original problem: NNUE scores ~26x larger than traditional (~-12,330 vs ~463 centipawns)
- "Fixed" by changing scale from `/16` to `/400` then to `/256`
- **This is a symptom of deeper problems**, not a fix

### Why This Breaks Search:

1. **Move ordering relies on relative score differences**: Alpha-beta cutoffs, killer moves, history heuristics all assume scores have a consistent scale
2. **Aspiration windows break**: If NNUE eval produces wildly different ranges, aspiration windows (typically ±50 centipawns) become ineffective
3. **Time management breaks**: Engine thinks position is much better/worse than reality

### Solution Needed:

- Proper weight initialization (uniform [-0.01, 0.01] instead of [-128, 128])
- No intermediate scaling factors in forward pass
- Output normalization based on position complexity, not arbitrary divisor
- **Tanh activation** on output layer to naturally bound [-1, 1], then scale by typical position value (~200cp)

---

## 2. Training Quality - TD(λ) IMPLEMENTATION IS BROKEN

### Problem 1: Flawed TD(λ) Implementation

**File**: `src/evaluation/nnue_training.rs` lines 144-189

```rust
fn compute_td_targets(&self, positions: &mut [TrainingPosition], final_result: GameResult) {
    let final_score = match final_result {
        GameResult::Win => 1.0,      // Black wins
        GameResult::Loss => -1.0,    // White wins
        GameResult::Draw => 0.0,
    };
    
    let mut next_value = final_score;
    for i in (0..n).rev() {
        let current_value = (positions[i].evaluation as f32 / 20000.0).tanh() * player_score_multiplier;
        
        // TD error: δ = r + γ*V(s') - V(s)
        let td_error = if i == n - 1 {
            final_score * player_score_multiplier - current_value
        } else {
            next_value - current_value
        };
        
        // TD(λ) update: V(s) += α * δ * e(s)
        positions[i].td_target = Some(current_value + self.config.learning_rate * td_error);
        
        // Update next_value using λ-weighting
        next_value = self.config.lambda * next_value + (1.0 - self.config.lambda) * current_value;
    }
}
```

**Critical Issues**:

1. **No eligibility traces**: Proper TD(λ) uses eligibility traces per weight. This code doesn't compute per-weight traces.
2. **Wrong TD error calculation**: 
   - Uses network output (`positions[i].evaluation`) which is stale (before training updates it)
   - Doesn't account for discount factor γ (missing)
   - `current_value` uses `tanh()` conversion that's **inconsistent with forward pass**
   
3. **Missing discount factor**: 
   - TD formula is: `δ = r + γ*V(s') - V(s)` where γ ≈ 1.0 for chess-like games
   - Code completely omits γ

4. **Lambda computation is wrong**:
   ```rust
   next_value = self.config.lambda * next_value + (1.0 - self.config.lambda) * current_value;
   ```
   Should compute cumulative returns with traces, not linear blend

### Problem 2: No Proper Target Values

**File**: `src/bin/nnue_trainer.rs` lines 79-94

```rust
// Evaluate current position
let evaluation = evaluator.evaluate(&board, current_player, &captured_pieces);

// Store position
positions.push(TrainingPosition {
    active_features,
    accumulator: accumulator.clone(),
    evaluation,
    player: current_player,
    move_made: None,
    outcome: None,
    td_target: None,
});
```

**Problems**:
- **Target value comes from the same weak evaluation being trained** 
- No external oracle (search results, GM games, opening book)
- This creates a **bootstrap problem**: NNUE training on NNUE play
- No depth-dependent targets (e.g., from deeper search results)

### Problem 3: Weight Update is Incomplete

**File**: `src/evaluation/nnue_training.rs` lines 232-333

```rust
fn update_weights_for_position(&mut self, position: &TrainingPosition, target_value: f32) -> (f32, f32, f32) {
    let current_prediction = (position.evaluation as f32 / 20000.0).tanh();
    let predicted_value = current_prediction * player_multiplier;
    let error = target_value - predicted_value;
    
    // Update output layer weights (simplified gradient descent)
    let gradient = error * (value as f32 / 64.0);
    let weight_update = (gradient * learning_rate) as i16;
    self.weights.output_weights[i] = self.weights.output_weights[i]
        .saturating_add(weight_update)
        .max(-32768)
        .min(32767);
    
    // Update input-to-hidden weights (simplified - only for active features)
    for &feature_idx in &position.active_features {
        for (i, &hidden_val) in hidden_1_activated.iter().enumerate() {
            if hidden_val > 0 && i < self.weights.input_weights_1[feature_idx].len() {
                let gradient = error * (hidden_val as f32 / 64.0) * learning_rate * 0.01;
                let weight_update = gradient as i16;
```

**Critical Issues**:
1. **No proper backpropagation**: 
   - Only updates output layer meaningfully (gradient of 1.0)
   - Hidden layer updates are 100x smaller (`* 0.01` line 315)
   - Hidden-to-hidden layer 2 is never updated
   
2. **Saturation arithmetic breaks gradients**:
   - `saturating_add()` silently clamps weights to [-32768, 32767]
   - Once saturated, gradient becomes 0, training stops
   - No weight decay to prevent saturation
   
3. **Simplified gradients**:
   - Should compute `∂Loss/∂w` properly through entire network
   - Current approach is hand-wavy and arbitrary
   
4. **No regularization**:
   - L1/L2 weight decay would help with saturation
   - No dropout or batch normalization

### Problem 4: Self-Play Quality is Unknown

**File**: `src/bin/nnue_trainer.rs` lines 32-142

```rust
fn play_self_play_game(
    search_engine: &mut SearchEngine,
    evaluator: &mut PositionEvaluator,
    weights: &NNUEWeights,
    config: &NNUETrainingConfig,
) -> TrainingGame {
    // ...
    let mut iterative_search = IterativeDeepening::new(
        config.search_depth,      // Depth 7 (line 200)
        config.time_per_move_ms,  // 300ms
        None,
    );
    let best_move = iterative_search.search(search_engine, &board, ...);
    // ...
}
```

**Issues**:
- **Search depth only 7 plies** → very weak play
- **300ms per move** is also weak
- **Evaluator is being trained while generating training data** (line 257-259):
  ```rust
  evaluator.enable_nnue_with_weights_internal(current_weights.clone());
  search_engine.get_evaluator_mut().enable_nnue_with_weights_internal(current_weights.clone());
  ```
- This means **early training iterations use completely random weights** for move selection
- **Bootstrap problem**: First 100 games use random NNUE (completely garbage) for self-play

---

## 3. Search Integration - NNUE IS NEVER USED

### Problem: NNUE Evaluation is Behind a Flag, Not In Main Search

**File**: `src/evaluation.rs` lines 519-526

```rust
pub fn evaluate(
    &mut self,
    board: &BitboardBoard,
    player: Player,
    captured_pieces: &CapturedPieces,
) -> i32 {
    // Use NNUE if enabled and available
    if self.use_nnue {
        if let Some(ref mut nnue) = self.nnue_evaluator {
            let nnue_score = nnue.evaluate(board, player, captured_pieces);
            return nnue_score;
        }
    }
    
    // Try cache first...
    // Use integrated evaluator...
}
```

**Problems**:

1. **NNUE is evaluated, but results never checked against PST**:
   - File: `examples/nnue_evaluate_trained.rs` exists but:
     - Not integrated into search
     - Not used by strength-tester
     - Not in actual gameplay

2. **Search never compares NNUE vs PST**:
   - No A/B testing infrastructure
   - No handicap matches
   - No ELO measurements

3. **NNUE evaluation not used for move ordering**:
   - Search uses PST evaluation for killer moves, history, etc.
   - NNUE eval could improve move ordering if integrated properly
   - Currently NNUE only used in position evaluation (already marginal impact)

### Missing Benchmarks:

**Files checked**:
- `docs/performance/reports/README.md` - Only describes report infrastructure, no actual NNUE benchmarks
- `docs/NNUE_INTEGRATION_COMPLETE.md` - Claims "automatically loaded" but no strength data
- `examples/nnue_evaluate_trained.rs` - Shows score differences but no move quality assessment
- No ELO diff measurements anywhere

**What's Missing**:
```
NNUE vs PST Strength Comparison
- Time control: 1s/move, 5s/move, etc.
- Games: 100+ games per comparison
- Expected ELO diff: ±50 (at minimum to matter)
- Actual ELO diff: UNKNOWN
```

---

## 4. Network Architecture Issues

### Problem 1: Architecture Not Optimized for Shogi

**File**: `src/evaluation/nnue.rs` lines 44-48

```rust
/// Default size of first hidden layer
pub const DEFAULT_HIDDEN_SIZE_1: usize = 256;

/// Default size of second hidden layer (0 means no second layer)
pub const DEFAULT_HIDDEN_SIZE_2: usize = 32;
```

**Issues**:

1. **2268 → 256 → 32 → 1 is too small**:
   - Input features: 2268 (2 players × 14 pieces × 81 squares)
   - This is sparse (typically 30-40 pieces on board, ~480 active features)
   - 256 hidden neurons is reasonable for input layer compression
   - **But 32 second-layer neurons is too aggressive** (512-1024 is standard in modern NNUE)
   
2. **No activation function specified for output layer**:
   - Should use `tanh()` to bound [-1, 1]
   - Currently just linear, which explains massive scale explosion
   - File line 311: `output += (value * weights.output_weights[i] as i32) / 64;`
   - No activation on final result before scaling

3. **ReLU might not be optimal**:
   - Shogi evaluation should handle both positive (good for player) and negative (bad) evaluations
   - Could lose information in dead ReLU neurons
   - Consider **ELU or GELU** for better gradient flow

4. **No layer normalization**:
   - Hidden layers not normalized, leads to **internal covariate shift**
   - Gradients may vanish/explode during training
   - Layer norm would stabilize training

### Problem 2: Weight Initialization is Bad

**File**: `src/evaluation/nnue.rs` lines 84-119

```rust
let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
    .map(|_| {
        (0..hidden_size_1)
            .map(|_| rng.gen_range(-128..128))  // RANGE IS HUGE
            .collect()
    })
    .collect();

let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
    .map(|_| rng.gen_range(-1000..1000))  // BIASES ARE HUGE TOO
    .collect();
```

**Issues**:
- **Input weights [-128, 128]**: Proper initialization should be ~±√(2/fan_in) ≈ ±0.03
- **Biases [-1000, 1000]**: Should be 0 or ~N(0, 0.1)
- This causes **vanishing/exploding gradients from the start**
- Output will naturally saturate the [-32768, 32767] i16 range during first forward pass

---

## 5. Data Quality - Bootstrap Problem

### The Vicious Cycle:

**Iteration 1**:
- Weights are random → search eval is garbage
- Self-play with garbage eval → random position outcomes
- Training on garbage data → slightly-less-garbage weights

**Iteration 2-700**:
- Each iteration trains on positions from PREVIOUS iteration's self-play
- Previous iteration used PREVIOUS weights for move selection
- Positions are biased toward moves that previous model thought were good
- **This slowly locks in biases rather than learning real evaluation**

### Evidence:

**File**: `src/bin/nnue_trainer.rs` lines 253-259

```rust
// Get current weights from trainer
let current_weights = trainer.get_weights().clone();

// Update evaluator with current weights
evaluator.enable_nnue_with_weights_internal(current_weights.clone());
// Also update search engine's evaluator
search_engine.get_evaluator_mut()
    .enable_nnue_with_weights_internal(current_weights.clone());

// Play self-play games
for game_num in 0..config.games_per_iteration {
    let game = play_self_play_game(&mut search_engine, &mut evaluator, &current_weights, &config);
```

**The problem**: 
- Training loop uses the model being trained for self-play
- No reference to external oracle (PST eval, opening book, endgame tablebase)
- Positions are only "good" according to the weak model

### What Should Happen Instead:

```rust
// Self-play with STRONG evaluator (PST with all features)
// OR use opening book for opening positions
// OR use tablebase for endgame positions
// Generate positions: game_weight * strong_eval + (1 - game_weight) * outcome
let target_value = if is_endgame {
    tablebase_value  // Perfect information
} else if is_opening {
    opening_book_weight * search_depth_10_eval
} else {
    0.5 * search_depth_8_eval + 0.5 * game_outcome
};
```

---

## 6. Actual Performance - UNKNOWN

### No Actual Benchmarks Exist

**Searched for**:
- ELO difference measurements: **NOT FOUND**
- Time-handicap comparisons: **NOT FOUND**  
- Strength test results: **NOT FOUND**
- Game records (NNUE vs PST): **NOT FOUND**

**Files checked**:
- `/docs/performance/reports/` - Infrastructure only
- `/docs/NNUE_INTEGRATION_COMPLETE.md` - Claims "ready for gameplay testing" but no results
- `/examples/nnue_evaluate_trained.rs` - Eval differences, not strength differences
- `/benches/` - No NNUE-specific strength benchmarks

### What We Know:

From `nnue_diagnose_weights.rs`:
```
Random weights: output_bias ≈ [-1000, 1000], weights in [-128, 128]
Trained weights: ??? (no example output shown)
```

From `NNUE_TRAINING_FIXES.md`:
```
Before fix: -12,330 centipawns (way too large)
After fix (/ 256): -770 centipawns (still arbitrary scaling)
```

**But no actual strength measurement!**

---

## 7. Root Cause Summary

| Component | Status | Impact |
|-----------|--------|--------|
| **Evaluation Scaling** | Broken (/ 256 ad-hoc) | Search alpha-beta breaks, move ordering fails |
| **TD(λ) Implementation** | Broken (no traces, wrong formula) | Training doesn't optimize for good moves |
| **Target Values** | Broken (bootstrap on weak eval) | Learning from noise, not real patterns |
| **Network Architecture** | Suboptimal (2nd layer too small) | Not enough capacity to model complex patterns |
| **Weight Initialization** | Bad (too large) | Immediate saturation, gradient death |
| **Self-Play Quality** | Weak (depth 7, no oracle) | Positions are garbage, weak model locked in |
| **Search Integration** | Missing (not used in search) | NNUE eval ignored by move ordering |
| **Performance Measurement** | Missing (no ELO tests) | No evidence NNUE helps at all |

---

## 8. Why Strength Didn't Improve

The NNUE implementation suffers from a **cascade of failures**:

1. **Untrained network starts with random weights** → garbage evaluation
2. **Self-play uses garbage evaluation** → learns nothing useful
3. **TD(λ) training is broken** → doesn't improve even from available data
4. **Weight scaling explodes** → final eval meaningless for search
5. **Search ignores eval anyway** → NNUE never impacts move selection
6. **No one tested it** → no one knows it failed

**Result**: NNUE is inert. It's loaded but not used. When used, it's garbage. When trained, it trains on garbage.

---

## Recommendations

### Short-Term Fixes (Days)

1. **Stop using ad-hoc scaling**:
   ```rust
   // In forward_pass, add tanh activation:
   let output_logit = output; // Before scaling
   let activated_output = (output_logit as f32 / 1000.0).tanh(); // Bound to [-1, 1]
   let score_centipawns = (activated_output * 200.0) as i32; // Scale to ±200cp range
   ```

2. **Fix weight initialization**:
   ```rust
   let std_dev = (2.0 / (NUM_NNUE_FEATURES as f32)).sqrt();
   let weight = rng.sample(Normal::new(0.0, std_dev).unwrap());
   ```

3. **Disable bootstrap NNUE** (use PST for self-play):
   ```rust
   evaluator.disable_nnue(); // Use PST for move selection
   // Play games...
   trainer.add_training_game(game); // Train NNUE on PST-generated positions
   ```

4. **Measure actual ELO**:
   ```bash
   cargo run --bin strength-tester -- \
     --nnue-weights nnue_weights_trained.json \
     --games 100 \
     --time-control 1s+0.1s
   ```

### Medium-Term Fixes (Weeks)

1. **Implement proper backpropagation**:
   - Full gradient computation through all layers
   - Proper weight updates (not saturating_add)
   - Weight decay and gradient clipping

2. **Use external oracle for targets**:
   - Self-play with PST eval depth 10
   - Mix positions: 50% opening book, 25% search-generated, 25% self-play
   - Use multi-stage targets (move 1→depth 4, move 2→depth 6, etc.)

3. **Fix TD(λ)**:
   - Implement eligibility traces properly
   - Add discount factor γ = 0.99
   - Use correct accumulation formula

4. **Optimize architecture**:
   - Try 2268 → 512 → 128 → 32 → 1 (deeper)
   - Add layer normalization
   - Use ELU or GELU activations

### Long-Term Fixes (Months)

1. **Learn from modern NNUE**:
   - Study Stockfish 15+ NNUE implementation
   - Use accumulator updates (already partially implemented)
   - Implement refresh triggers for position changes

2. **Ensemble evaluation**:
   - Run both NNUE and PST in parallel
   - Blend scores: `0.3 * nnue + 0.7 * pst`
   - Gradually shift blend as NNUE improves

3. **Automated tuning**:
   - Use GPU for faster training
   - Implement Lichess/ChessTempo positions for training
   - Cross-validate with holdout test set

