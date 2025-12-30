# NNUE Post-Training Guide

After completing NNUE training, here are the recommended next steps to evaluate, test, and improve your trained model.

## Current Status

✅ **Training**: Multiple sessions completed (latest: Dec 30, 2025)  
✅ **Integration**: Automatic weight loading implemented  
✅ **Weights**: `nnue_weights_trained.json` (6.0 MB) automatically loaded by engine  
✅ **Ready**: Engine is ready for gameplay testing

## 1. Evaluate Trained Weights

### Quick Evaluation

Run the evaluation example to compare random vs trained weights:

```bash
cargo run --example nnue_evaluate_trained
```

This will:
- Test random NNUE weights
- Test your trained weights (`nnue_weights_trained.json`)
- Compare with traditional evaluation
- Show evaluation scores for benchmark positions

### What to Look For

- **Consistency**: Trained weights should give more consistent evaluations
- **Magnitude**: Scores should be in a reasonable range (typically -500 to +500 centipawns)
- **Correlation**: Trained NNUE should correlate better with traditional evaluation than random weights

## 2. Test in Actual Gameplay

### Enable NNUE in Your Engine

**Automatic (Recommended)**: The engine automatically loads `nnue_weights_trained.json` when it starts. No code changes needed!

```rust
use shogi_engine::ShogiEngine;

// NNUE weights are automatically loaded if nnue_weights_trained.json exists
let engine = ShogiEngine::new();

// Check if NNUE is enabled
if engine.is_nnue_enabled() {
    println!("NNUE is active!");
}
```

**Manual Loading** (if needed for custom paths):

```rust
use shogi_engine::evaluation::PositionEvaluator;

let mut evaluator = PositionEvaluator::new();
evaluator.enable_nnue_with_weights("path/to/nnue_weights_trained.json")?;
```

### Play Test Games

1. **Self-Play**: Engine vs itself (trained vs random weights)
2. **Against Traditional**: Trained NNUE vs traditional evaluation
3. **Time Controls**: Test at different time controls (blitz, rapid, classical)

### Expected Improvements

- Better positional understanding
- More consistent evaluation
- Better endgame play (if trained on endgames)

## 3. Analyze Training Quality

### Check Training Statistics

From your training output:
- **TD Error**: Should decrease over time (yours: 0.112635)
- **Weight Changes**: Should stabilize (yours: avg 0.054876, max 53.0)
- **Game Length**: Should increase as play improves (yours: 18.1 moves)

### Red Flags

- **All Draws**: If all games end in draws, training may be too conservative
- **Very Short Games**: Games ending too quickly may indicate poor play
- **High TD Error**: Error > 1.0 suggests unstable training
- **No Weight Changes**: Weights not updating suggests learning rate too low

### Your Training Analysis

✅ **Good Signs:**
- Weight updates happening (17,124 updates)
- Reasonable TD error (0.112635)
- Consistent game length (18.1 moves)

⚠️ **Areas for Improvement:**
- All games ending in draws (W:0 L:0 D:10) - may need more aggressive play
- Short game length (18.1 moves) - could train longer or adjust search depth

## 4. Further Training Options

### Continue Training

If weights are improving, continue training:

```bash
# Modify nnue_trainer.rs to load existing weights
# Then run more iterations
cargo run --bin nnue_trainer
```

### Adjust Hyperparameters

Based on your results, consider:

1. **Increase Search Depth** (currently 5)
   - Deeper search = better training positions
   - Try depth 6-7 for stronger play

2. **Adjust Learning Rate** (currently 0.1)
   - Lower (0.01) for more stable training
   - Higher (0.2) for faster learning (may be unstable)

3. **Increase Games per Iteration** (currently 10)
   - More games = more diverse training data
   - Try 20-50 games per iteration

4. **Extend Training** (currently 100 iterations)
   - More iterations = better weights
   - Try 500-1000 iterations for stronger play

### Training Variants

1. **Curriculum Learning**: Start with simple positions, gradually increase complexity
2. **Adversarial Training**: Train against stronger opponents
3. **Endgame Focus**: Train specifically on endgame positions
4. **Tactical Training**: Focus on tactical positions

## 5. Compare Evaluation Methods

### Create a Comparison Script

Test the same positions with:
- Random NNUE weights
- Trained NNUE weights
- Traditional evaluation

Look for:
- **Agreement**: Do they agree on which side is better?
- **Magnitude**: Are scores in similar ranges?
- **Consistency**: Does NNUE give consistent scores for similar positions?

## 6. Performance Testing

### Speed Comparison

Measure evaluation speed:
- Traditional evaluation: ~X microseconds
- NNUE evaluation: ~Y microseconds
- NNUE should be faster due to incremental updates

### Memory Usage

Check memory consumption:
- NNUE weights: ~6.3 MB (your current file)
- Accumulator: Small (hidden layer sizes)

## 7. Integration into Search

### Verify Search Integration

Ensure NNUE is being used in search:

```rust
let mut search_engine = SearchEngine::new(None, 16);
let mut evaluator = PositionEvaluator::new();
evaluator.enable_nnue_with_weights("nnue_weights_trained.json")?;
search_engine.set_evaluator(evaluator);
```

### Test Search Quality

- Does search find better moves with NNUE?
- Are search depths consistent?
- Does time management work correctly?

## 8. Next Steps Checklist

- [ ] Run evaluation example to compare weights
- [ ] Test trained weights in actual gameplay
- [ ] Analyze training statistics for improvements
- [ ] Consider continuing training with adjusted hyperparameters
- [ ] Compare evaluation methods (random vs trained vs traditional)
- [ ] Measure performance (speed and memory)
- [ ] Verify search integration
- [ ] Test against other engines (if available)

## 9. Troubleshooting

### Problem: Trained weights perform worse than random

**Possible Causes:**
- Learning rate too high (weights diverged)
- Not enough training data
- Training positions too simple

**Solutions:**
- Lower learning rate and retrain
- Increase games per iteration
- Use stronger search during training

### Problem: All games end in draws

**Possible Causes:**
- Evaluation too conservative
- Search depth too shallow
- Position evaluation too balanced

**Solutions:**
- Increase search depth
- Adjust evaluation scale
- Train on more decisive positions

### Problem: Training is very slow

**Possible Causes:**
- Too many games per iteration
- Search depth too high
- Large network architecture

**Solutions:**
- Reduce games per iteration
- Lower search depth
- Use smaller network (128->16 instead of 256->32)

## 10. Advanced Topics

### Weight Visualization

Visualize weight distributions to understand what the network learned:
- Large weights = important features
- Small weights = less important features

### Feature Analysis

Analyze which features are most important:
- Piece-square combinations
- Piece values
- Positional patterns

### Hyperparameter Tuning

Systematically tune:
- Learning rate
- Lambda (TD(λ) parameter)
- Network architecture
- Training schedule

## Summary

Your training completed successfully with:
- ✅ 1,000 games played
- ✅ 18,084 positions collected
- ✅ 17,124 weight updates
- ✅ Reasonable TD error (0.112635)

**Recommended Next Steps:**
1. Evaluate trained weights on benchmark positions
2. Test in actual gameplay
3. Consider continuing training with adjusted hyperparameters
4. Compare with traditional evaluation

Good luck with your NNUE engine! 🎯



