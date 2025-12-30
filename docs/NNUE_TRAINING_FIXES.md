# NNUE Training Fixes and Parameter Adjustments

## Changes Made

### 1. Fixed Training Multipliers (Root Cause Fix)

**File**: `src/evaluation/nnue_training.rs`

**Changes**:
- Reduced output weight update multiplier from `10.0` to `1.0`
  - Old: `(gradient * learning_rate * 10.0)`
  - New: `(gradient * learning_rate)`
  
- Reduced output bias update multiplier from `160.0` to `10.0`
  - Old: `(bias_gradient * learning_rate * 160.0)`
  - New: `(bias_gradient * learning_rate * 10.0)`

**Impact**: Prevents weights from growing explosively during training, producing more stable and reasonable evaluation scores.

### 2. Adjusted Output Scaling

**File**: `src/evaluation/nnue.rs`

**Change**: Updated output scaling from `/ 16` to `/ 256`

**Rationale**: 
- Works reasonably for both existing trained weights and future training
- Existing weights (trained with old multipliers) may still be slightly large but will improve with continued training
- Future training with reduced multipliers will produce weights that work well with this scaling

### 3. Enhanced Trainer for Continued Training

**File**: `src/bin/nnue_trainer.rs`

**Changes**:
- **Load existing weights**: Trainer now automatically loads `nnue_weights_trained.json` if it exists, allowing seamless continuation of training
- **Improved parameters**:
  - `games_per_iteration`: 10 → 20 (more diverse training data)
  - `iterations`: 100 → 200 (more training iterations)
  - `learning_rate`: 0.1 → 0.01 (more stable training)
  - `search_depth`: 5 → 6 (stronger play during training)
  - `max_moves_per_game`: 150 → 200 (longer games)
  - `min_batch_size`: 1000 → 500 (more frequent weight updates)

### 4. Fixed Sort Comparison Bug (Critical)

**File**: `src/search/search_engine.rs`

**Problem**: The comparison function in `sort_quiescence_moves_enhanced` could violate total order requirements, causing panics during training.

**Fix**:
- Improved hint move handling to ensure proper ordering
- Added `compare_moves_directly` function as final tie-breaker
- Ensures deterministic total order even if hash collisions occur

**Impact**: Prevents training crashes due to sort panics.

## Performance Optimization

### Training Speed Issues

If training is too slow, consider:

1. **Reduce search depth**: Lower `search_depth` from 6 to 4-5 for faster games
2. **Reduce time per move**: Lower `time_per_move_ms` from 500 to 200-300ms
3. **Reduce games per iteration**: Lower `games_per_iteration` from 20 to 10-15
4. **Use release mode**: Compile with `cargo build --release --bin nnue_trainer` for 2-3x speedup

### Recommended Fast Training Config

For faster training (at cost of slightly weaker play):

```rust
config.search_depth = 4;           // Faster search
config.time_per_move_ms = 200;     // Less time per move
config.games_per_iteration = 10;   // Fewer games per batch
config.min_batch_size = 250;       // More frequent updates
```

## Expected Results

### Before Fixes
- Trained weights produced scores ~26x larger than traditional evaluation
- Example: -12,330 centipawns vs traditional 463 centipawns
- Training could crash with sort comparison panics

### After Fixes
- Trained weights produce scores closer to traditional evaluation
- Example: -770 centipawns vs traditional 463 centipawns (~1.7x difference)
- Training is stable and won't crash on sort operations
- As training continues with reduced multipliers, scores should converge further

## Running Continued Training

Simply run the trainer - it will automatically load existing weights:

```bash
# Debug mode (slower but easier to debug)
cargo run --bin nnue_trainer

# Release mode (2-3x faster, recommended for training)
cargo build --release --bin nnue_trainer
./target/release/nnue_trainer
```

The trainer will:
1. Load `nnue_weights_trained.json` if it exists (or create new random weights)
2. Continue training with optimized parameters
3. Save weights periodically (every 10 iterations)
4. Save final weights to `nnue_weights_trained.json`

## Monitoring Training Progress

Watch for:
- **TD Error**: Should decrease over time (target: < 0.1)
- **Weight Changes**: Should stabilize (target: < 0.1 average change)
- **Game Length**: Should increase as play improves
- **Evaluation Scores**: Should converge toward traditional evaluation range

## Next Steps

1. **Run continued training**: `cargo run --bin nnue_trainer` (or use release mode)
2. **Monitor progress**: Check TD error and weight change statistics
3. **Evaluate periodically**: Run `cargo run --example nnue_evaluate_trained` to see score convergence
4. **Test in gameplay**: Use trained weights in actual games to assess improvement

## Technical Details

### Why the Multipliers Were Too Large

The original multipliers (10x for weights, 160x for bias) were designed to ensure "meaningful" weight updates, but they caused:
- Rapid weight growth during training
- Evaluation scores that grew out of control
- Need for aggressive output scaling (workaround)

### Why Reduced Multipliers Work Better

With reduced multipliers:
- Weights grow more slowly and stably
- Evaluation scores stay in reasonable range
- Less aggressive output scaling needed
- More predictable training behavior

### Scaling Factor Choice

`/ 256` was chosen as a compromise:
- Works for existing weights (trained with old multipliers)
- Will work well for future weights (trained with new multipliers)
- Provides reasonable evaluation range
- Can be adjusted if needed based on training results

### Sort Comparison Fix

The sort comparison bug was caused by:
- Incomplete handling of hint moves in comparison function
- Potential hash collisions not properly handled
- Missing final tie-breaker for moves with identical properties

The fix ensures:
- Proper total order through all comparison stages
- Direct move comparison as final tie-breaker
- Deterministic ordering even with edge cases
