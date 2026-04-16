# Entry Points Reference

## Main Entry Point: `usi-engine`

**Binary**: `src/main.rs`

```rust
fn main() {
    install_panic_hook();
    install_signal_handlers();
    run_with_panic_logging(|| run_usi_loop());
}
```

### Startup Sequence
1. Install panic hook for crash logging
2. Install signal handlers (SIGSEGV, SIGABRT)
3. Enter USI command loop

### USI Command Loop

Located in `src/usi.rs` - `run_usi_loop()`:

```
stdin (line-by-line) → handle_command() → stdout
```

#### Supported Commands

| Command | Handler | Description |
|---------|---------|-------------|
| `usi` | `handle_usi()` | Returns engine ID and options |
| `isready` | `handle_isready()` | Returns readyok |
| `position startpos [moves ...]` | `handle_position()` | Set board position |
| `position sfen <sfen> [moves ...]` | `handle_position()` | Set board from SFEN |
| `go btime X wtime Y byoyomi Z` | `handle_go()` | Start search |
| `stop` | `handle_stop()` | Stop current search |
| `ponderhit` | `handle_ponderhit()` | Opponent played expected move |
| `setoption name X value Y` | `handle_setoption()` | Configure engine |
| `usinewgame` | `handle_usinewgame()` | Prepare for new game |
| `gameover result` | `handle_gameover()` | Game ended |
| `debug on/off` | `handle_debug()` | Toggle debug mode |
| `quit` | (caller handles) | Exit engine |

### Go Command Processing

The `go` command (`src/usi.rs:handle_go()`):
1. Parse time controls: `btime`, `wtime`, `byoyomi`
2. Calculate time allocation (1/40 of remaining time, or byoyomi)
3. Clear stop flag
4. Call `engine.get_best_move()`
5. Output `bestmove <move>` or `bestmove resign`

### Position Command Processing

The `position` command (`src/lib.rs:handle_position()`):
1. Parse `startpos` or `sfen` format
2. Initialize board from SFEN
3. Apply move list sequentially
4. Update captured pieces
5. Switch current player

## Utility Binaries

### `tuner` - Weight Tuning
**Path**: `src/bin/tuner.rs`

Tuning evaluation weights using learning algorithms.

### `analyzer` - Position Analysis
**Path**: `src/bin/analyzer.rs`

Analyzes positions and outputs detailed evaluation.

### `strength-tester` - Testing
**Path**: `src/bin/strength-tester.rs`

Tests engine strength against opponents or test suites.

### `move-assessor` - Move Assessment
**Path**: `src/bin/move_assessor.rs`

Evaluates specific moves in positions.

### `puzzle-gen` - Puzzle Generation
**Path**: `src/bin/puzzle_gen.rs`

Generates tactical puzzles from engine games.

### `pst-tuning-runner` - PST Tuning
**Path**: `src/bin/pst_tuning_runner.rs`

Runs piece-square table tuning experiments.

### `generate_magic_tables` - Magic Bitboards
**Path**: `src/bin/generate_magic_tables.rs`

Precomputes magic numbers for bitboard operations.

### `optimize_magic_numbers` - Optimization
**Path**: `src/bin/optimize_magic_numbers.rs`

Optimizes magic number generation.

## Library API (`src/lib.rs`)

### ShogiEngine Struct

```rust
pub struct ShogiEngine {
    pub board: BitboardBoard,           // Board state
    pub captured_pieces: CapturedPieces, // Hand pieces
    pub current_player: Player,          // Active player
    pub opening_book: OpeningBook,       // Opening knowledge
    pub tablebase: MicroTablebase,        // Endgame knowledge
    pub stop_flag: Arc<AtomicBool>,      // Search cancellation
    pub search_engine: Arc<Mutex<SearchEngine>>,
    pub debug_mode: bool,
    pub pondering: bool,
    pub depth: u8,
    pub thread_count: usize,
    pub parallel_options: ParallelOptions,
    pub pst_config: PieceSquareTableConfig,
}
```

### Key Methods

| Method | Signature | Purpose |
|--------|-----------|---------|
| `new()` | `-> Self` | Create engine with defaults |
| `get_best_move()` | `(depth, time, stop) -> Option<Move>` | Main search entry |
| `apply_move()` | `(&Move) -> bool` | Apply move to board |
| `handle_position()` | `(&[&str]) -> Vec<String>` | Process position command |
| `handle_go()` | `(parts) -> Vec<String>` | Process go command |
| `handle_setoption()` | `(&[&str]) -> Vec<String>` | Process option command |
| `is_game_over()` | `-> Option<GameResult>` | Check terminal state |

### Preferences

Engine preferences stored at:
- Unix: `~/.config/shogi-vibe/engine_prefs.json`
- Override: `$SHOGI_PREFS_DIR/engine_prefs.json`

Persisted settings:
- `thread_count`: Number of search threads
