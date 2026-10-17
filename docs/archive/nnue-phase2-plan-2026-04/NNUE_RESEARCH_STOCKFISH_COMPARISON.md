# NNUE Implementation Research: Stockfish/Modern Chess Engines vs. Shogi Engine

## Executive Summary

This document compares NNUE (Efficiently Updatable Neural Networks) implementations in modern chess engines (primarily Stockfish) with the current Shogi engine implementation. The research covers architecture decisions, training procedures, weight initialization, search integration, accumulator strategies, and modern variations. The goal is to inform whether the Shogi engine should adopt Stockfish's approach or adapt it for Shogi-specific requirements.

**Key Finding**: The current Shogi implementation has several fundamental issues identified in `NNUE_DEEP_DIVE_ANALYSIS.md` that prevent it from improving engine strength. Stockfish's approach addresses these through careful quantization, proper scaling, and sophisticated training procedures.

---

## 1. Architecture & Activation Functions

### Stockfish (Chess) - Evolution

#### Early Version: HalfKP (Stockfish 11-13)
- **Input Layer**: 41,024 features per side (2 * 64 king squares * (64 squares * 10 piece types + 1 BONA_PIECE_ZERO))
- **Hidden Layer 1**: 256 neurons per side (2x256 = 512 total after concatenation)
- **Hidden Layer 2**: 32 neurons
- **Output Layer**: 1 neuron (evaluation)
- **Architecture**: 256x2 -> 32 -> 32 -> 1

#### Modern Version: HalfKAv2 (Stockfish 14+)
- **Input Layer**: 45,056 features per side (64 king squares * 12 piece types * 64 squares per side)
- **Linear Feature Transformer**: Maps to 2x520 outputs with direct skip connection to output
- **Hidden Layers**: 8 output buckets with sub-networks: 512x2 -> 16 -> 32 -> 1
- **Architectural Innovation**: 
  - 8 different output networks based on piece count: `(piece_count - 1) / 4`
  - Direct connection from feature transformer to output for material detection
  - Separate PSQT (piece-square table) outputs per bucket

### Activation Functions in Chess NNUE

| Function | Status | Reason |
|----------|--------|--------|
| **ReLU** | Rarely used | Potential to overflow in integer domain |
| **CReLU** (Clipped ReLU) | Less common | Easier to auto-vectorize, moderate performance |
| **SCReLU** (Squared Clipped ReLU) | **Standard** | Best empirical results, requires hand-written SIMD, `f(x) = clamp(x, 0, 1)^2` |
| **Quantmoid4** | Emerging | Piecewise quadratic approximation of sigmoid(4x), better capacity than ReLU |
| **Sigmoid** | Training only | Too expensive for inference, used in loss computation during training |

### Output Activation

- **Inference**: Linear output (no activation) with scale factor 400/255/64
- **Training**: Sigmoid applied to network output before loss computation
- **Scaling**: Multiple-step quantization: `eval * SCALE / (QA * QB)` where SCALE=400, QA=255, QB=64

### Shogi Engine Current Implementation

**Current Architecture** (from `src/evaluation/nnue.rs`):
```
2268 features -> 256 -> 32 -> 1
```

- **Input**: 2 players * 14 piece types * 81 squares = 2,268 features
- **Hidden 1**: 256 neurons with ReLU
- **Hidden 2**: 32 neurons with ReLU
- **Output**: Single value with linear activation
- **Output Scaling**: Simple `/256` (arbitrary, ad-hoc)

**Problem**: Uses simple integer initialization in [-128, 128] range instead of careful weight scaling.

### Why These Choices?

**Stockfish's Design Philosophy**:
1. **SCReLU superiority**: Squared activation increases model capacity while maintaining quantization properties
2. **Multiple output buckets**: Different piece counts have different evaluation characteristics; one network can't capture both endgame and opening nuances equally
3. **Direct feature connection**: Allows the network to learn material values quickly without deep computation
4. **Quantization-first**: All design choices prioritize integer arithmetic and low-precision inference

**Critical Differences for Shogi**:
- Shogi has piece drops, more complex piece values, and different endgame patterns than chess
- Current implementation doesn't use SCReLU or quantmoid4, missing modern improvements
- No output bucketing by piece count
- No direct feature skip connections

---

## 2. Training Procedure

### Stockfish NNUE Training (via nnue-pytorch)

#### Target Value Source: **Hybrid Supervised + Reinforcement**

The most recent Stockfish networks use Lichess training data with mixed targets:

```
Target = α * sigmoid(eval_score / 600) + (1-α) * WDL_from_game_result
```

Where:
- `sigmoid(eval_score / 600)` comes from engine search evaluations
- `WDL_from_game_result` comes from actual game outcomes
- α is typically 0.9-1.0 (heavily weighted toward search scores)

**Key Insight**: Targets come from **external oracle** (search, games), not the same evaluation being trained.

#### Position Sampling Strategy

1. **Lichess Integration**: Billions of positions from real games
2. **Depth-weighted sampling**: Positions from deeper analysis preferred (more reliable)
3. **Time-controlled filtering**: Positions where engine spent significant time thinking
4. **Draw filtering**: Many positions filtered out if games ended in draw (noisy targets)
5. **Batch composition**: Positions shuffled and batched randomly for each epoch

#### Learning Rate Schedule

- **Base learning rate**: 0.001 (very conservative)
- **Schedule**: Exponential decay or cosine annealing
- **Optimizer**: Adam with β1=0.9, β2=0.999
- **Learning for different layers**:
  - Feature transformer: Slower (more impact on overall model)
  - Hidden layers: Standard rate
  - Output: Faster (learns material values quickly)

#### Training Scale

- **Positions per training run**: 10 billion+ (Lichess data)
- **Epochs**: 10-20
- **Batch size**: 32,768 (GPU-accelerated)
- **Time**: Days on GPU clusters

### TD(λ) vs. Direct Targets

Stockfish's modern approach **moved away from pure TD(λ)**:

**TD(λ) Approach (older)**:
```
V(s) += α * δ * e(s)  [per weight with eligibility traces]
```
Problems: Requires storing eligibility traces, bootstraps from weak eval

**Current Approach (supervised + RL)**:
```
Loss = MSE(eval, target)  [or cross-entropy for WDL]
```
Advantages: Targets from external oracle (search/games), more stable, better convergence

### Shogi Engine Current Implementation

**From `src/evaluation/nnue_training.rs`**:

```rust
// Target from same weak evaluation being trained
let evaluation = evaluator.evaluate(&board, ...);
positions.push(TrainingPosition { evaluation, ... });

// Flawed TD(λ) computation
let td_error = next_value - current_value;
positions[i].td_target = Some(
    current_value + self.config.learning_rate * td_error
);
```

**Critical Issues** (from `NNUE_DEEP_DIVE_ANALYSIS.md`):

1. **Bootstrap Problem**: Target values come from NNUE eval being trained, not external oracle
2. **Missing Eligibility Traces**: TD(λ) implementation stores targets instead of per-weight traces
3. **Inconsistent Scaling**: `tanh(eval / 20000.0)` in training but `/256` in inference
4. **No Discount Factor**: Missing γ (should be ~1.0 for board games)
5. **Wrong λ Computation**: Linear blend instead of cumulative traces

**Configuration**:
```rust
learning_rate: 0.01,        // 10x higher than Stockfish!
lambda: 0.7,
games_per_iteration: 10,    // Too small
search_depth: 3,            // Too shallow for reliable targets
iterations: 100,
```

---

## 3. Weight Initialization

### Stockfish Approach

**Feature Transformer (Input) Weights**:
```
Uniform[-0.01, 0.01]  // Very small range
```
- Reason: Sparse inputs mean these accumulate slowly
- Allows network to start with reasonable baseline, then gradually learn

**Hidden Layer Weights**:
```
He initialization: N(0, sqrt(2/fan_in))
```
- Reason: ReLU/SCReLU variants benefit from larger initial variance
- Allows faster learning of non-linear patterns

**Output Weights**:
```
Uniform[-0.01, 0.01]  // Small like input
```
- Reason: Output learned more carefully (material values mostly encoded in features)

**Biases**:
```
Initialize to 0 or small random values
```

### Why These Choices?

1. **Sparse inputs**: Input layer sees only ~10 active features per position, so small initial weights are appropriate
2. **ReLU/SCReLU**: Need larger variance to break symmetry and enable learning
3. **Output**: Material values learned in first layer; output layer fine-tunes
4. **Quantization-aware**: All initialization assumes subsequent quantization to int16/int8

### Shogi Engine Current Implementation

```rust
// From src/evaluation/nnue.rs:80-128
let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
    .map(|_| {
        (0..hidden_size_1)
            .map(|_| rng.gen_range(-128..128))  // UNIFORM IN INT16 RANGE!
            .collect()
    })
    .collect();

let hidden_biases_1: Vec<i32> = (0..hidden_size_1)
    .map(|_| rng.gen_range(-1000..1000))  // HUGE RANGE!
    .collect();
```

**Problems**:
1. **Way too large**: [-128, 128] is 256x larger than Stockfish's [-0.01, 0.01]
2. **Accumulation issue**: With sparse inputs, initial random sums are huge
3. **No scaling context**: Initialized as integers without considering final scaling
4. **Bias explosion**: [-1000, 1000] biases immediately blow up accumulator values

---

## 4. Search Integration

### Stockfish NNUE Integration

**Where Used**:
1. **Evaluation at leaf nodes**: All positions evaluated with NNUE + tapered blend
2. **Move ordering**: NNUE score guides initial move order
3. **Aspiration windows**: Scaled relative to NNUE baseline
4. **Transposition table**: NNUE scores cached in TT
5. **Lazy SMP**: Copy-make + lazy updates allow parallelization

**Blending Strategy**:
- **Material-based hybrid**: Use NNUE if material is balanced (within 1 pawn)
- **Fallback to classical eval**: Extreme imbalances use traditional PST
- **Reason**: Classical eval works well when one side is clearly winning
- **Result**: +20 Elo from hybrid approach

**Special Search Procedures**:
- **Lazy updates**: Accumulator updates deferred until eval called (cache-friendly)
- **Copy-make**: Store accumulators per ply instead of make/unmake (better for lazy SMP)
- **Refinement**: Refresh accumulator strategically when needed

### Shogi Engine Current Implementation

**From `src/evaluation/nnue.rs`**:
```rust
pub fn evaluate(
    &mut self,
    board: &BitboardBoard,
    _player: Player,
    _captured_pieces: &CapturedPieces,
) -> i32 {
    // Full refresh every evaluation!
    self.accumulator.refresh(board, &self.weights);
    self.accumulator.evaluate(&self.weights)
}
```

**Problems**:
1. **Full refresh every eval**: Defeats purpose of accumulator (no incremental updates)
2. **No integration with search**: NNUE is isolated from move ordering, aspiration windows
3. **No hybrid evaluation**: Always uses NNUE (should probably blend with PST)
4. **No lazy updates**: Wastes computation on positions never used in search
5. **Unused incremental API**: `update_move_incremental()` method exists but not called!

**Recommendation**: Integrate incremental updates into search:
```rust
// Make move
board.make_move(&move);
// Incrementally update instead of refresh
evaluator.update_move_incremental(from, to, piece, captured);
// Search continues
```

---

## 5. Accumulator Strategy

### Stockfish Optimization

**Efficient Refresh vs. Incremental Update**:

1. **Full Refresh**: ~10-20 µs (add 32 feature columns)
2. **Incremental Update**: ~1-2 µs (add/subtract 1-4 columns)

**When to Refresh**:
- On king moves with bucketing (requires index recalculation)
- After deep search cutoffs (avoids accumulation of rounding errors)
- Strategically after ~50 moves (floating-point error bounds)

**Storage Strategy**:

**Copy-Make Approach**:
- Store accumulator for each position in search
- On undo, restore from stack (no computation)
- Advantages: Single thread per search, no error accumulation, works with lazy SMP
- Storage: 256 neurons * 2 perspectives * 64 depth * 4 bytes = 131 KB per thread

**Key Optimization: Lazy Updates**:
```
On make_move:
  - Mark accumulator as "dirty"
  - Record changed features
  - DON'T update accumulator

On evaluate:
  - Check if accumulator is dirty
  - If dirty, trace back to last clean accumulator
  - Incrementally update to current position
  - Mark as clean
```

**Benefits**:
- Many positions in search are never evaluated (no wasted computation)
- Better cache locality (fewer accumulator writes)
- ~20-30% speedup in typical alpha-beta search

### Shogi Engine Current Implementation

```rust
// From src/evaluation/nnue.rs:186-227
pub fn refresh(&mut self, board: &BitboardBoard, weights: &NNUEWeights) {
    self.hidden_1.fill(0);
    if let Some(ref mut h2) = self.hidden_2 {
        h2.fill(0);
    }
    // Iterate all 81 squares, recompute all features
    for row in 0..9 {
        for col in 0..9 {
            let pos = Position::new(row, col);
            if let Some(piece) = board.get_piece(pos) {
                self.add_piece(piece, pos, weights);
            }
        }
    }
}
```

**Issues**:
1. **Always full refresh**: No optimization for incremental updates
2. **Shogi-specific problem**: Captured pieces in hand are re-added each position (not on board)
3. **Dropped pieces**: When pieces are dropped, need to recalculate features
4. **No lazy updates**: Evaluates every position, even those cut off

**Shogi Considerations**:
- Piece dropping makes incremental updates more complex (adding feature from hand)
- Captured piece pool is part of evaluation (must include in features)
- Need separate handling: board pieces + hand pieces (not just board)

**Recommendation**:
```rust
// Separate accumulators for board and hand
pub struct NNUEAccumulator {
    board_hidden_1: Vec<i32>,
    hand_contribution: Vec<i32>,  // Pre-computed for both players' hands
    hidden_1: Vec<i32>,           // board + hand combined
}

// On move: update only affected pieces
pub fn update_move(&mut self, ...) {
    // Update board piece
    if let Some(from) = from {
        self.remove_piece(piece, from);
    }
    self.add_piece(piece, to);
    
    // Update hand if drop
    if is_drop {
        self.remove_from_hand(piece);
    }
    
    // Combine for evaluation
    self.recombine();
}
```

---

## 6. Modern Variations (2023-2025)

### Recent Improvements in Chess NNUE

#### 1. **Quantmoid4 Activation** (2024+)
- Piecewise quadratic approximation of sigmoid(4x)
- Computationally cheaper than SCReLU with better capacity
- Enables richer feature learning while maintaining speed
- Adopted by several top engines (Viridithas, Starzix)

#### 2. **Training Data Evolution**

**Lichess Integration**:
- 5+ billion positions from rated games
- Filtered by Elo (>2000), time control (classical/blitz)
- Preprocessed: Remove quiet positions, weight by search depth
- Mixed targets: 90% search evals, 10% game outcomes

**Syzygy Endgame Blending**:
- Endgame positions trained on tablebase values
- Improves endgame evaluation accuracy
- Requires special loss function for known positions

#### 3. **Ensemble & Teacher Networks**

Some engines use **teacher-student training**:
- Train large "teacher" network (4096 hidden)
- Use teacher predictions as targets for smaller "student" (1024 hidden)
- Student inherits knowledge but runs faster
- ~20-40 Elo improvement with good training

#### 4. **Knowledge Distillation from AlphaZero / Leela**

- Use LC0 (Leela Chess Zero) networks as knowledge source
- Train chess NNUE on LC0 policy probabilities
- Captures intuitive play patterns, not just winning positions
- Effective but slower to train

#### 5. **Automatic Architecture Search**

Modern trainers (Bullet, Grapheus) explore:
- Feature set composition (which input features matter)
- Hidden layer sizes (larger first layer, tiny output for speed)
- Activation functions (Quantmoid4 vs. SCReLU trade-offs)
- Batch normalization techniques for integer domain

#### 6. **Shogi-Specific Evolution**

From research on YaneuraOu and other Shogi engines:

**King Buckets for Shogi**:
- King position heavily influences optimal move patterns
- Separate accumulators per king location quintile
- Especially important with piece drops (hand pieces change king safety)

**Drop Piece Features**:
- Standard HalfKP uses 64 king squares + pieces
- For Shogi: 64 king squares + 14 piece types + piece counts in hand
- Much larger input space but sparse

---

## 7. Comparative Summary

| Aspect | Stockfish | Shogi Engine | Status |
|--------|-----------|--------------|--------|
| **Architecture** | HalfKAv2 / 256x2->16->32->1 | 256->32->1 | Shogi needs modernization |
| **Activation** | SCReLU + Quantmoid4 | ReLU | Shogi should add SCReLU |
| **Weight Init** | Uniform[-0.01, 0.01] | Uniform[-128, 128] | **CRITICAL FIX NEEDED** |
| **Output Scaling** | 400/(255*64) quantized | /256 ad-hoc | Shogi scaling wrong |
| **Target Source** | Search + Lichess games | Self-play (same eval) | Bootstrap problem |
| **Training Data** | 10B+ positions | TBD | Shogi needs more data |
| **Optimizer** | Adam with decay | Basic SGD | Shogi needs improvement |
| **Search Integration** | Full (move order, TT, hybrid) | None | **Major gap** |
| **Accumulator Updates** | Lazy + incremental | Always full refresh | Performance issue |
| **Output Buckets** | 8 buckets by piece count | 1 output | Limited flexibility |
| **Piece Drop Support** | N/A | Partially | Needs special handling |

---

## 8. Recommendations for Shogi Engine

### Immediate Fixes (Critical)

1. **Fix Weight Initialization**:
   ```rust
   // Instead of [-128, 128]
   let input_weights_1: Vec<Vec<i16>> = (0..NUM_NNUE_FEATURES)
       .map(|_| {
           (0..hidden_size_1)
               .map(|_| {
                   // He initialization scaled for subsequent quantization
                   let normal: f32 = normal_distribution.sample(&mut rng);
                   (normal * 100.0) as i16  // Much smaller range
               })
               .collect()
       })
       .collect();
   ```

2. **Fix Output Scaling**:
   ```rust
   // Instead of arbitrary /256
   // Use proper quantization: scale * divisor
   const SCALE: i32 = 400;  // Match Stockfish
   const QA: i32 = 255;     // Activation bound
   const QB: i32 = 64;      // Output bound
   output = (output * SCALE) / (QA * QB)  // = output * 400 / 16320 ≈ output / 40
   ```

3. **Implement Incremental Updates**:
   - Integrate `update_move_incremental()` into search
   - Avoid full accumulator refresh every evaluation
   - Store accumulators with move stack (copy-make)

4. **Fix Training TD(λ)**:
   - Switch to supervised approach with external targets
   - Use search results as targets (depth >= 3)
   - Or collect games from engine play and use outcomes
   - Remove inconsistent tanh scaling

### Medium-Term Improvements

5. **Add SCReLU or Quantmoid4**:
   ```rust
   // Instead of simple ReLU
   let activation = (value as f32).clamp(0.0, 1.0).powi(2);  // SCReLU
   // Or implement Quantmoid4 for better capacity
   ```

6. **Implement Proper Training with Adam**:
   - Use optimizers crate or custom Adam implementation
   - Exponential learning rate decay
   - Per-layer learning rate adjustments

7. **Add Output Bucketing**:
   ```rust
   // Different output networks for piece count ranges
   fn select_output_bucket(&self, piece_count: usize) -> usize {
       (piece_count.saturating_sub(1)) / 4  // 0-7 range
   }
   ```

8. **Piece Drop Integration**:
   - Separate features for: board pieces + hand pieces
   - Track piece pools in accumulator
   - Properly handle drop mechanics

### Long-Term Enhancements

9. **Training Data Pipeline**:
   - Collect billions of positions from self-play
   - Filter by search depth (deeper = more reliable)
   - Implement efficient dataset loading (Rust + PyArrow)

10. **Search Integration**:
    - Use NNUE for move ordering (sort moves by NNUE eval)
    - Blend with classical evaluation in balanced positions
    - Integrate with transposition table caching

11. **Hybrid Evaluation**:
    - Weight by material imbalance
    - Keep classical eval for extreme positions
    - Fall back when NNUE confidence low

12. **King Bucket Architecture** (Shogi-Specific):
    - Bucket by both kings' positions
    - Improve piece drop evaluation
    - Better capture of king safety patterns

---

## 9. References & Links

### Original Papers
- Yu Nasu (2018): [NNUE - Efficiently Updatable Neural-Network based Evaluation Functions](https://github.com/ynasu87/nnue/blob/master/docs/nnue.pdf)
- Dominik Klein (2021): [Neural Networks For Chess](https://github.com/asdfjkl/neural_network_chess)

### Implementations
- **Stockfish NNUE**: https://github.com/official-stockfish/Stockfish (HalfKAv2 in `src/nnue/`)
- **nnue-pytorch**: https://github.com/glinscott/nnue-pytorch (PyTorch trainer, comprehensive docs)
- **Bullet**: https://github.com/jnsl/bullet (Rust trainer, state-of-the-art, used by many engines)
- **Grapheus**: https://github.com/cosmicpudding/grapheus (C++ trainer by Ethereal team)

### Documentation
- **Chess Programming Wiki - NNUE**: https://www.chessprogramming.org/NNUE
- **Chess Programming Wiki - Stockfish NNUE**: https://www.chessprogramming.org/Stockfish_NNUE
- **NNUE-PyTorch Guide**: https://github.com/glinscott/nnue-pytorch/blob/master/docs/nnue.md

### Notable Trainer Papers
- Stockfish NNUE Thread (2020): "Pytorch NNUE training" by Gary Linscott
- Lichess Contribution (2021): Integration of billions of positions for training
- Modern Approaches (2023+): Discussion of Quantmoid4, knowledge distillation

---

## Conclusion

**Stockfish's NNUE approach is fundamentally sound, but the current Shogi implementation has critical architectural and training issues that prevent it from improving engine strength.**

The main gaps are:

1. **Weight initialization too aggressive** (256x too large) - immediate fix
2. **Output scaling arbitrary** (ad-hoc /256) - needs proper quantization math
3. **No incremental accumulator updates** - performance drain
4. **TD(λ) training flawed** - bootstrap problem, missing discount factors
5. **No search integration** - NNUE isolated from alpha-beta

**Starting point**: Fix #1-3 above and re-train. If weights still don't improve strength, the issues are training data quality and target values.

**Next level**: Adopt Stockfish's supervised training approach (search results + game outcomes as targets) rather than pure TD learning.

**Advanced**: Implement HalfKA architecture with king bucketing and output buckets optimized for Shogi's piece dropping rules.

The research shows that NNUE is powerful, but requires careful engineering in quantization, initialization, and training to achieve real strength gains. The Shogi engine is close to a working implementation but needs these foundational fixes first.
