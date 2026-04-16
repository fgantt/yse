# Agent Context - Shogi Engine

## Project Identity

- **Name**: Yggdrasil
- **Type**: Shogi (Japanese chess) engine
- **Language**: Rust (edition 2021)
- **Protocol**: USI (Universal Shogi Interface)
- **License**: (see LICENSE file)

## What This Does

Yggdrasil plays Shogi by:
1. Receiving positions via USI protocol from a GUI
2. Searching the game tree using alpha-beta search with optimizations
3. Evaluating positions using piece values, piece-square tables, king safety, etc.
4. Returning the best move found

## Key Characteristics

- **Board**: 9x9 board with 14 piece types (7 base + 7 promoted)
- **Move Notation**: USI format (e.g., `7g7f`, `R*5c`)
- **Position Hash**: Zobrist hashing for transposition tables
- **Time Control**: Supports byoyomi and incremental time

## Directory Structure

```
src/
├── main.rs              # Entry point, panic/signal handlers
├── lib.rs               # ShogiEngine struct, main API
├── usi.rs               # USI protocol handler
├── bitboards.rs         # Bitboard board representation
├── moves.rs             # Move generation
├── types/               # Domain types
│   └── core.rs          # Player, PieceType, Position, Piece, Move
├── search/              # Search algorithm
│   ├── search_engine.rs # Alpha-beta core
│   ├── transposition_table.rs
│   ├── iterative_deepening.rs
│   ├── parallel_search.rs
│   └── ...
├── evaluation/           # Position evaluation
│   ├── evaluation.rs
│   ├── piece_square_tables.rs
│   ├── king_safety.rs
│   └── ...
├── opening_book.rs      # Opening book
├── tablebase/           # Endgame tablebase
└── bin/                 # Utility binaries
    ├── tuner.rs
    ├── analyzer.rs
    └── ...
```

## Important Patterns

### USI Command Flow
```
stdin → UsiHandler.handle_command() → ShogiEngine method → stdout
```

### Search Flow
```
get_best_move() → IterativeDeepening → AlphaBeta → TranspositionTable/Evaluation
```

### Evaluation Flow
```
AlphaBeta(depth=0) → Quiescence → Evaluation → Component evaluators
```

## Common Commands

```bash
# Build
cargo build --release --bin usi-engine

# Test
cargo test

# Lint
cargo clippy --all-targets --all-features

# Format
cargo fmt

# Benchmark
cargo bench --features legacy-tests,simd
```

## Key Types

```rust
// Player (side to move)
enum Player { Black, White }

// Piece type
enum PieceType { Pawn, Lance, Knight, Silver, Gold, Bishop, Rook, King,
                 PromotedPawn, PromotedLance, PromotedKnight, PromotedSilver,
                 PromotedBishop, PromotedRook }

// Position (0-8 for row, 0-8 for col)
struct Position { row: u8, col: u8 }

// Move (from, to, piece_type, promotion, player)
struct Move { from: Option<Position>, to: Position,
              piece_type: PieceType, promotion: bool, player: Player }

// Score (centipawns or mate)
type Score = i32;
```

## Position Notation

- **Row**: 0 = rank 9 (top), 8 = rank 1 (bottom)
- **Column**: 0 = file a (right), 8 = file i (left)
- **Example**: `7g7f` = row 6 col 6 → row 6 col 5 (7g=source, 7f=dest)
- **Drop**: `P*5c` = Pawn drop at row 4 col 2
- **Promotion**: appended char (e.g., `7g7f+`)

## Board Coordinate System

```
     a  b  c  d  e  f  g  h  i
   ┌──┬──┬──┬──┬──┬──┬──┬──┬──┐
 9 │  │  │  │  │  │  │  │  │  │  Row 0
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 8 │  │  │  │  │  │  │  │  │  │  Row 1
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 7 │  │  │  │  │  │  │  │  │  │  Row 2
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 6 │  │  │  │  │  │  │  │  │  │  Row 3
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 5 │  │  │  │  │  │  │  │  │  │  Row 4
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 4 │  │  │  │  │  │  │  │  │  │  Row 5
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 3 │  │  │  │  │  │  │  │  │  │  Row 6
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 2 │  │  │  │  │  │  │  │  │  │  Row 7
   ├──┼──┼──┼──┼──┼──┼──┼──┼──┤
 1 │  │  │  │  │  │  │  │  │  │  Row 8
   └──┴──┴──┴──┴──┴──┴──┴──┴──┘

   Col 0  1  2  3  4  5  6  7  8
```

**Black** moves "up" (decreasing row numbers)
**White** moves "down" (increasing row numbers)

## Performance Features

- **SIMD**: AVX2/SSE (x86_64), NEON (ARM64)
- **Parallel**: YBWC thread clustering
- **Memory**: Transposition table, evaluation cache
- **Move Ordering**: MVV-LVA, killers, history heuristics

## Code Style

- Rust 2021 edition
- Clippy pedantic lints enabled
- Error handling with `thiserror`
- Logging with `log` crate
- Serialization with `serde`

## Testing

- Unit tests in `src/**/*_tests.rs`
- Integration tests in `tests/`
- Benchmarks in `benches/`
- Feature flags: `legacy-tests`, `simd`

## External Resources

- USI spec: Various shogi engines
- Similar to: Stockfish (chess engine) architecture
- Shogi rules: Standard Japanese shogi

## Important Files for Common Tasks

| Task | Files to Look At |
|------|------------------|
| Add new search feature | `src/search/search_engine.rs`, `src/search/mod.rs` |
| Modify evaluation | `src/evaluation/evaluation.rs`, component files |
| USI protocol changes | `src/usi.rs`, `src/lib.rs` handle_* methods |
| Board representation | `src/bitboards.rs`, `src/moves.rs` |
| Configuration | `src/config/`, `src/lib.rs` ShogiEngine struct |
| Tests | `src/**/*_tests.rs`, `tests/` |
