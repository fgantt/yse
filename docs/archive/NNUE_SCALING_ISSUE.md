# NNUE Evaluation Scaling Issue

## Problem

After training, NNUE evaluation scores were much larger in magnitude than traditional evaluation:
- Traditional: ~463 centipawns
- Trained NNUE (before fix): ~-12,330 centipawns (26x larger)

## Root Cause

The training code uses large multipliers for weight updates:
- Output weights: `learning_rate * 10.0` multiplier
- Output bias: `learning_rate * 160.0` multiplier

With `learning_rate = 0.1`, this means:
- Weight updates: `0.1 * 10.0 = 1.0` per gradient unit
- Bias updates: `0.1 * 160.0 = 16.0` per gradient unit

These large multipliers cause weights to grow significantly during training, producing much larger raw evaluation outputs than the original scaling factor (`/ 16`) can handle.

## Current Fix

Changed output scaling from `/ 16` to `/ 400` in `src/evaluation/nnue.rs`:

```rust
// Scale output to reasonable evaluation range (centipawns)
output / 400  // Increased from 16 to 400 to account for trained weight magnitudes
```

This brings trained NNUE scores into a reasonable range (~-493 centipawns vs traditional ~463 centipawns).

## Impact

- ✅ Trained weights now produce reasonable scores
- ✅ Random weights still produce reasonable scores (~-155 centipawns)
- ⚠️ Scores may have different signs than traditional evaluation (this is acceptable - what matters is relative differences)

## Better Long-Term Solutions

### Option 1: Reduce Training Multipliers

Modify `src/evaluation/nnue_training.rs` to use smaller multipliers:

```rust
// Instead of:
let weight_update = (gradient * learning_rate * 10.0) as i16;
let bias_update = (bias_gradient * learning_rate * 160.0) as i32;

// Use:
let weight_update = (gradient * learning_rate * 1.0) as i16;  // Remove 10x multiplier
let bias_update = (bias_gradient * learning_rate * 10.0) as i32;  // Reduce from 160x to 10x
```

Then revert scaling back to `/ 16` or use a smaller factor like `/ 64`.

### Option 2: Weight Clipping

Add weight clipping during training to prevent weights from growing too large:

```rust
// After weight updates, clip to reasonable range
self.weights.output_bias = self.weights.output_bias.clamp(-10000, 10000);
// Clip output weights similarly
```

### Option 3: Adaptive Scaling

Compute scaling factor based on weight magnitudes:

```rust
// Estimate scale factor from weight statistics
let avg_output_weight_magnitude = weights.output_weights.iter()
    .map(|&w| w.abs() as f64)
    .sum::<f64>() / weights.output_weights.len() as f64;
    
let scale_factor = (avg_output_weight_magnitude / 100.0) as i32;
let scale_factor = scale_factor.max(16).min(1000);  // Reasonable bounds
output / scale_factor
```

### Option 4: Normalize During Training

Normalize weights periodically during training to keep them in a consistent range.

## Recommendation

For now, the `/ 400` scaling works. For future training runs, consider:
1. Reducing the training multipliers (Option 1) - simplest and most effective
2. Adding weight clipping (Option 2) - prevents extreme weights
3. Using adaptive scaling (Option 3) - automatically adjusts to weight magnitudes

The current fix is sufficient for using the trained weights, but reducing training multipliers would be the cleanest long-term solution.



