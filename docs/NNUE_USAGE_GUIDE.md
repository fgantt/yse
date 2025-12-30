# NNUE Usage Guide

This guide explains how to use NNUE (Efficiently Updatable Neural Networks) evaluation in the Shogi engine.

## Quick Start

### 1. Enable NNUE with Random Weights (for testing)

```rust
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::{BitboardBoard, CapturedPieces, Player};

let mut evaluator = PositionEvaluator::new();

// Enable NNUE with default architecture (256 -> 32 -> 1)
evaluator.enable_nnue(256, 32);

// Evaluate a position
let board = BitboardBoard::new();
let captured_pieces = CapturedPieces::new();
let score = evaluator.evaluate(&board, Player::Black, &captured_pieces);
```

### 2. Load Trained NNUE Weights

```rust
let mut evaluator = PositionEvaluator::new();

match evaluator.enable_nnue_with_weights("path/to/nnue_weights.json") {
    Ok(()) => println!("NNUE weights loaded successfully"),
    Err(e) => eprintln!("Failed to load weights: {}", e),
}
```

### 3. Save NNUE Weights

```rust
let mut evaluator = PositionEvaluator::new();
evaluator.enable_nnue(256, 32);

match evaluator.save_nnue_weights("my_weights.json") {
    Ok(()) => println!("Weights saved"),
    Err(e) => eprintln!("Failed to save: {}", e),
}
```

## Integration with Search Engine

The `SearchEngine` automatically uses NNUE when enabled in its evaluator:

```rust
use shogi_engine::search::search_engine::SearchEngine;

let mut search_engine = SearchEngine::new(None, 16);

// Enable NNUE
search_engine.evaluator.enable_nnue(256, 32);

// All searches will now use NNUE evaluation
let best_move = search_engine.search(&board, &captured_pieces, player, depth);
```

## Integration with ShogiEngine

If you're using the high-level `ShogiEngine`:

```rust
use shogi_engine::ShogiEngine;

// The engine creates a SearchEngine internally
// You would need to access the search engine's evaluator
// This depends on the engine's API for configuration
```

## Architecture Options

NNUE supports different architectures. Choose based on your needs:

- **Small & Fast**: `enable_nnue(128, 16)` - Good for testing
- **Standard**: `enable_nnue(256, 32)` - Balanced performance/accuracy
- **Large**: `enable_nnue(512, 128)` - Best accuracy, slower

The format is: `enable_nnue(hidden_layer_1_size, hidden_layer_2_size)`

- `hidden_layer_1_size`: Number of neurons in first hidden layer (typically 128-512)
- `hidden_layer_2_size`: Number of neurons in second hidden layer (typically 0-128, 0 means no second layer)

## Feature Representation

NNUE uses piece-square features:
- **2 players** (Black, White)
- **14 piece types** (7 base + 7 promoted)
- **81 squares** (9x9 Shogi board)
- **Total**: 2 × 14 × 81 = 2,268 features

Each feature represents whether a specific piece type of a specific player is on a specific square.

## Incremental Updates

NNUE supports efficient incremental updates during search. The accumulator maintains hidden layer values and can be updated incrementally when making/unmaking moves, avoiding full recomputation.

This is automatically handled internally - you don't need to do anything special.

## Weight File Format

NNUE weights are stored in JSON format:

```json
{
  "input_weights_1": [[...], [...]],
  "hidden_biases_1": [...],
  "input_weights_2": [[...], [...]] | null,
  "hidden_biases_2": [...] | null,
  "output_weights": [...],
  "output_bias": 0
}
```

## Training NNUE Weights

To get useful NNUE evaluation, you need to train the weights. Common approaches:

1. **Supervised Learning**: Train on positions with known evaluations from strong engines
2. **Self-Play**: Train using reinforcement learning from self-play games
3. **Fine-tuning**: Start from random weights and tune using your existing evaluation data

The current implementation provides the infrastructure. You'll need to implement the training loop separately.

## Disabling NNUE

To fall back to traditional evaluation:

```rust
evaluator.disable_nnue();
```

## Checking Status

```rust
if evaluator.is_nnue_enabled() {
    println!("NNUE is active");
} else {
    println!("Using traditional evaluation");
}
```

## Performance Considerations

- NNUE is typically faster than complex traditional evaluation
- With random weights, NNUE will produce arbitrary evaluations - you need trained weights
- Incremental updates make NNUE very efficient during search
- Memory usage depends on architecture size (256->32 uses ~2.3MB for weights)

## Example: Complete Workflow

```rust
use shogi_engine::evaluation::PositionEvaluator;
use shogi_engine::{BitboardBoard, CapturedPieces, Player};

fn main() {
    // Create evaluator
    let mut evaluator = PositionEvaluator::new();

    // Try to load trained weights, fall back to random if not available
    if evaluator.enable_nnue_with_weights("nnue_weights.json").is_err() {
        println!("No trained weights found, using random initialization");
        evaluator.enable_nnue(256, 32);
        
        // Optionally save for later
        let _ = evaluator.save_nnue_weights("nnue_weights_random.json");
    }

    // Evaluate positions
    let board = BitboardBoard::new();
    let captured = CapturedPieces::new();
    
    let score = evaluator.evaluate(&board, Player::Black, &captured);
    println!("Evaluation: {} centipawns", score);
}
```

## Troubleshooting

**Problem**: NNUE always returns 0 or strange values
- **Solution**: You're likely using random weights. Train or load trained weights.

**Problem**: Cannot load weights file
- **Solution**: Check file path and format. Ensure JSON is valid.

**Problem**: Evaluation is slower than expected
- **Solution**: Try a smaller architecture (128->16) or ensure incremental updates are being used.

**Problem**: Memory usage is high
- **Solution**: Use a smaller architecture. Each neuron in hidden layer 1 uses ~2KB of weights.

