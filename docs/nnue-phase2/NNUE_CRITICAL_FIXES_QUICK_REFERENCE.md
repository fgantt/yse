# NNUE Critical Fixes - Quick Reference

## Problem Severity Assessment

| Issue | Severity | Impact | Fix Complexity |
|-------|----------|--------|-----------------|
| Weight initialization (256x too large) | **CRITICAL** | Prevents convergence | Easy (1 line change) |
| Output scaling (ad-hoc /256) | **CRITICAL** | Scores meaningless for search | Medium (3-5 lines) |
| Full refresh every eval | **HIGH** | 10-100x slower than needed | Medium (integrate into search) |
| Flawed TD(λ) training | **HIGH** | Doesn't actually improve strength | Hard (redesign training) |
| No search integration | **HIGH** | NNUE not used for move ordering | Hard (integrates with search) |
| Missing SCReLU activation | **MEDIUM** | Lower capacity than Stockfish | Medium (implement activation) |
| Inconsistent scaling in training | **MEDIUM** | Gradients computed wrong | Easy (fix tanh scaling) |

---

## Fix #1: Weight Initialization (IMMEDIATE)

### Current Code (BROKEN)
```rust
// File: src/evaluation/nnue.rs, lines 84-128
let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
    .map(|_| {
        (0..hidden_size_1)
            .map(|_| rng.gen_range(-128..128))  // 256x too large!
            .collect()
    })
    .collect();

let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
    .map(|_| rng.gen_range(-1000..1000))  // Explosion!
    .collect();
```

### Fixed Code
```rust
use rand_distr::Normal;

let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
    .map(|_| {
        let normal = Normal::new(0.0, 0.01).unwrap();
        (0..hidden_size_1)
            .map(|_| {
                // He initialization: small uniform in [-0.01, 0.01]
                let w = rng.sample::<f64, _>(normal);
                (w * 100.0).clamp(-128.0, 127.0) as i16
            })
            .collect()
    })
    .collect();

let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
    .map(|_| {
        // Initialize to 0 or very small
        let bias = rng.sample::<f64, _>(Normal::new(0.0, 10.0).unwrap());
        bias.clamp(-32768.0, 32767.0) as i32
    })
    .collect();
```

**Why**: 
- Sparse inputs (avg ~20 active out of 2268) mean small weights accumulate to reasonable sums
- With [-128, 128] range, even 20 inputs sum to ±2560 = completely unreasonable
- Stockfish uses [-0.01, 0.01] → sums to ±0.2 → scaled to centipawns properly

---

## Fix #2: Output Scaling (IMMEDIATE)

### Current Code (ARBITRARY)
```rust
// File: src/evaluation/nnue.rs, lines 314-320
output / 256  // Why 256? Arbitrary!
```

### Fixed Code - Option A: Match Stockfish
```rust
// Constants matching Stockfish's quantization
const SCALE: i32 = 400;           // Centipawn scale factor
const QA: i32 = 255;              // Activation (CReLU) bound
const QB: i32 = 64;               // Output layer divisor
const DIVISOR: i32 = QA * QB;     // 16,320

// In evaluate function:
output = (output * SCALE) / DIVISOR  // output * 400 / 16320 ≈ output * 0.0245
```

### Fixed Code - Option B: Document & Justify
If using different scaling, explain the math:
```rust
// Quantization scheme for Shogi NNUE
// Input weights: [-128, 128] × 2268 features → hidden layer
// Max value: 2268 * 128 = 290,304
// Actual: ~20 active × 100 (scaled weight) = 2000
// After ReLU + bias: typically [-1000, 1000]
// Hidden layer output: 32 values
// Max dot product: 32 × 1000 = 32,000 centipawns range
const OUTPUT_SCALE: i32 = 32;     // Centipawn per unit

// In evaluate function:
output / OUTPUT_SCALE
```

**Key**: Whatever you choose, **document it clearly** and ensure it's consistent with initialization.

---

## Fix #3: Incremental Accumulator Updates (MEDIUM - LONG TERM)

### Current Problem
```rust
// File: src/evaluation/nnue.rs, evaluate() method
pub fn evaluate(&mut self, board: &BitboardBoard, _player: Player, ...) -> i32 {
    // FULL REFRESH ON EVERY EVAL!
    self.accumulator.refresh(board, &self.weights);
    self.accumulator.evaluate(&self.weights)
}
```

This is called 1000+ times per search = refreshing entire accumulator from scratch each time!

### Solution: Integrate into Search Loop

In search engine (wherever `evaluate()` is called):

```rust
// Instead of:
let eval = evaluator.evaluate(&board, player, captured);

// Do this:

// Before move:
search_engine.push_accumulator_state();

// After make_move:
evaluator.update_move_incremental(from, to, piece, captured);

// Evaluate without refresh:
let eval = evaluator.evaluate_cached(player);

// After unmake_move:
search_engine.pop_accumulator_state();
evaluator.restore_accumulator();
```

### For Now: Add a Flag
Minimize damage while redesigning search:

```rust
pub struct NNUEEvaluator {
    weights: NNUEWeights,
    accumulator: NNUEAccumulator,
    enabled: bool,
    last_board_hash: u64,  // Add this
}

pub fn evaluate(&mut self, board: &BitboardBoard, ...) -> i32 {
    let current_hash = board_hash(board);
    
    // Only refresh if position changed
    if current_hash != self.last_board_hash {
        self.accumulator.refresh(board, &self.weights);
        self.last_board_hash = current_hash;
    }
    
    self.accumulator.evaluate(&self.weights)
}
```

---

## Fix #4: Training Targets (HIGH - REDESIGN)

### Current Problem
```rust
// Targets come from the SAME evaluation being trained!
let evaluation = evaluator.evaluate(&board, current_player, &captured_pieces);
// ...
let td_target = Some(current_value + self.config.learning_rate * td_error);
```

**This is a bootstrap problem**: you're trying to learn from yourself.

### Solution: Use External Targets

Option A: Search-based targets (best)
```rust
// Before training iteration:
// Run deeper search on each position
// Store (position, search_eval_at_depth_8) pairs
// Train NNUE to predict search evaluation

// In training:
let external_eval = search_evaluation;  // From depth 8 search
let nnue_eval = evaluator.evaluate(...);
let td_error = external_eval - nnue_eval;
```

Option B: Game outcomes (simpler)
```rust
// Play self-play games
// For each position, store (position, game_outcome)
// Convert outcome to target:
//   Win = 1.0, Draw = 0.5, Loss = 0.0
// Train NNUE to predict outcomes

let outcome_target = match game_result {
    GameResult::Win => 1.0,
    GameResult::Draw => 0.5,
    GameResult::Loss => 0.0,
};
```

**Why it works**: You're training against an objective, not your own predictions.

---

## Fix #5: Training Configuration

### Current (TOO AGGRESSIVE)
```rust
learning_rate: 0.01,        // 10x Stockfish!
games_per_iteration: 10,    // Too few
search_depth: 3,            // Too shallow
iterations: 100,            // OK, but unreliable targets
```

### Recommended
```rust
learning_rate: 0.001,       // Match Stockfish
games_per_iteration: 100,   // More diverse data
search_depth: 6,            // Deeper = better targets
iterations: 1000,           // More iterations to converge
max_moves_per_game: 400,    // Let games play out naturally
```

---

## Implementation Priority

### Phase 1: Critical (Do First)
1. **Fix weight init** (30 min) → Re-train
2. **Fix output scaling** (30 min) → Re-train
3. **Test if weights improve** → If not, continue to Phase 2

### Phase 2: High (Fix if Phase 1 doesn't work)
4. **Fix training targets** (2 hours) → Switch to search/outcome targets
5. **Fix training config** (30 min) → More conservative learning rate
6. **Train with 10B+ positions** (days/weeks)

### Phase 3: Medium (Optimization)
7. **Implement incremental updates** (4 hours) → 10-100x speedup
8. **Add SCReLU activation** (2 hours) → Better capacity
9. **Output bucketing** (4 hours) → Handle different piece counts

### Phase 4: Advanced (Polish)
10. **King buckets** (1 day) → Shogi-specific improvement
11. **Search integration** (1 day) → Use NNUE for move ordering

---

## Verification Checklist

After fixing #1 & #2:
```
□ Weights initialize to reasonable range (±100 after quantization)
□ NNUE eval at startpos is reasonable centipawn value (±50)
□ Same position always produces same eval (deterministic)
□ Re-train for 100 iterations
□ Check if loss decreases over iterations
□ Compare trained eval vs. classical eval
```

After fixing #4 (training targets):
```
□ Target values come from external oracle (search or games)
□ TD error computed as: external_target - nnue_prediction
□ Weight updates push NNUE toward external targets
□ Loss should decrease per iteration
□ Validation on holdout positions shows improvement
```

---

## Debugging Help

### Issue: Weights don't change during training
**Check**: 
- Is learning rate zero? (should be 0.001 or 0.01)
- Is target coming from same eval? (will converge to 0)
- Are gradients computed? (add logging)

### Issue: Loss gets worse over time
**Check**:
- Learning rate too high (0.1 is probably too much)
- Targets inconsistent between iterations
- Initialization ruined by first update

### Issue: NNUE scores don't correlate with search
**Check**:
- Output scaling wrong (/ 256 vs / 400)
- Weights not trained properly
- Accumulator refresh not actually happening

---

## External Resources

### To Understand Quantization
- NNUE-PyTorch docs: https://github.com/glinscott/nnue-pytorch/blob/master/docs/nnue.md#quantization
- Stockfish source: https://github.com/official-stockfish/Stockfish/blob/master/src/nnue/evaluate.h

### To Implement Training
- Bullet trainer: https://github.com/jnsl/bullet (Rust, well-structured)
- Grapheus trainer: https://github.com/cosmicpudding/grapheus (C++, industrial-grade)
- nnue-pytorch: https://github.com/glinscott/nnue-pytorch (PyTorch, if you want GPU)

### Validation
- CCRL testing: http://ccrl.chessdom.com/ccrl/404/ (for reference)
- Fishtest framework: https://tests.stockfishchess.org/ (for continuous testing)
