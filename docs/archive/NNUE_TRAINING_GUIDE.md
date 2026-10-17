# NNUE Self-Play Training Guide

This guide explains how to train NNUE weights using self-play.

## Quick Start

Run the training binary:

```bash
cargo run --bin nnue_trainer
```

This will:
1. Initialize NNUE with random weights (256 -> 32 -> 1 architecture)
2. Play self-play games using the current NNUE weights
3. Train the weights using TD(λ) learning
4. Save weights periodically and at the end

## Training Configuration

The training uses `NNUETrainingConfig` with the following parameters:

- `learning_rate`: 0.01 (how fast weights update)
- `lambda`: 0.7 (TD(λ) parameter, 0.0 = TD(0), 1.0 = Monte Carlo)
- `games_per_iteration`: 10 (games per training batch)
- `max_moves_per_game`: 200 (prevents infinite games)
- `search_depth`: 3 (search depth for self-play moves)
- `time_per_move_ms`: 100 (time limit per move)
- `iterations`: 100 (total training iterations)
- `min_batch_size`: 1000 (positions before weight update)

## Training Process

1. **Self-Play**: Engine plays games against itself using current NNUE weights
2. **Position Extraction**: Each position is saved with its evaluation and features
3. **TD(λ) Learning**: Temporal difference learning updates weights based on game outcomes
4. **Weight Updates**: Weights are updated using gradient descent on TD errors
5. **Iteration**: Process repeats for multiple iterations

## Output Files

- `nnue_weights_iter_N.json`: Weights saved every 10 iterations
- `nnue_weights_trained.json`: Final trained weights

## Using Trained Weights

After training, load the weights:

```rust
use shogi_engine::evaluation::PositionEvaluator;

let mut evaluator = PositionEvaluator::new();
evaluator.enable_nnue_with_weights("nnue_weights_trained.json")?;
```

## Training Tips

1. **Start Small**: Use fewer games per iteration initially (10-20) to test
2. **Increase Gradually**: As training progresses, increase games per iteration
3. **Monitor Progress**: Check saved weights periodically to see improvement
4. **Adjust Learning Rate**: Lower learning rate (0.001) for more stable training
5. **Longer Training**: More iterations = better weights (1000+ recommended)

## Architecture Choices

- **Small (128->16)**: Fast training, lower accuracy
- **Standard (256->32)**: Balanced (default)
- **Large (512->128)**: Slower training, higher accuracy potential

## TD(λ) Parameter

- `lambda = 0.0`: TD(0) - only immediate reward
- `lambda = 0.7`: Default - balances immediate and future rewards
- `lambda = 1.0`: Monte Carlo - full game outcome

## Troubleshooting

**Problem**: Training is very slow
- **Solution**: Reduce `games_per_iteration` or `search_depth`

**Problem**: Weights don't improve
- **Solution**: Increase `learning_rate` or `iterations`

**Problem**: Training crashes
- **Solution**: Reduce `min_batch_size` or check memory usage



