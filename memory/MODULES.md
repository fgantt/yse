# Module Responsibilities

## Core Modules

### `src/lib.rs` - ShogiEngine
**Purpose**: Main engine facade and state management

**Key Responsibilities**:
- Initialize and manage engine state
- Coordinate between USI protocol and search/evaluation
- Handle position state (board, captured pieces, current player)
- Manage opening book and tablebase integration
- Expose public API for USI commands

**Public API**:
```rust
impl ShogiEngine {
    pub fn new() -> Self;
    pub fn get_best_move(&mut self, depth, time_limit_ms, stop_flag) -> Option<Move>;
    pub fn apply_move(&mut self, move_: &Move) -> bool;
    pub fn handle_position(&mut self, parts: &[&str]) -> Vec<String>;
    pub fn is_game_over(&self) -> Option<GameResult>;
    // ... setters and configuration
}
```

---

### `src/usi.rs` - USI Protocol Handler
**Purpose**: Parse and respond to USI commands

**Key Responsibilities**:
- USI command parsing
- Time control parsing
- Output formatting for GUI communication

**Key Functions**:
```rust
pub fn run_usi_loop();  // Main command loop
struct UsiHandler { engine: ShogiEngine }
impl UsiHandler {
    pub fn handle_command(&mut self, command_str: &str) -> Vec<String>;
    fn handle_usi(&self) -> Vec<String>;
    fn handle_isready(&self) -> Vec<String>;
    fn handle_go(&mut self, parts: &[&str]) -> Vec<String>;
}
```

---

### `src/bitboards.rs` - Board Representation
**Purpose**: Efficient board state using bitboards

**Key Responsibilities**:
- Store piece positions as bitmasks
- Generate attack bitboards
- Make/unmake moves on bitboard representation
- SFEN parsing and serialization

**Key Types**:
```rust
pub struct BitboardBoard {
    // Per-player, per-piece-type bitboards
    pieces: [[u64; PIECE_TYPE_COUNT]; 2],
    // Full board bitboards per player
    player_boards: [u64; 2],
    // All occupied squares
    occupied: u64,
}
```

---

### `src/types/` - Domain Types
**Purpose**: Core game state types

| File | Types | Purpose |
|------|-------|---------|
| `core.rs` | `Player`, `PieceType`, `Position`, `Piece`, `Move` | Fundamental types |
| `board.rs` | `Board`, `CapturedPieces`, `GameResult` | Board state types |
| `transposition.rs` | `TTEntry`, `TTFlag`, `Bound` | Transposition table types |
| `search.rs` | `SearchResult`, `SearchStats` | Search-related types |
| `evaluation.rs` | `Score`, `Phase` | Evaluation types |

---

### `src/moves.rs` - Move Generation
**Purpose**: Generate legal and pseudo-legal moves

**Key Components**:
- `MoveGenerator` struct
- `generate_legal_moves()` - All legal moves with legality checks
- `generate_pseudo_legal_moves()` - Moves without legality verification
- Move validation utilities

---

## Search Modules (`src/search/`)

### `search_engine.rs` - Core Search
**Purpose**: Alpha-beta search implementation

**Key Functions**:
```rust
pub fn alpha_beta_search(&mut self, ...) -> (Option<Move>, Score);
pub fn negamax(&mut self, ...) -> Score;
```

### `iterative_deepening.rs` - Iterative Deepening
**Purpose**: Time-controlled iterative deepening

**Key Types**:
```rust
struct IterativeDeepening {
    max_depth: u8,
    time_limit_ms: u32,
    stop_flag: Option<Arc<AtomicBool>>,
}
impl IterativeDeepening {
    pub fn search(&mut self, engine, board, captured, player) -> Option<(Move, Score)>;
}
```

### `transposition_table.rs` - Transposition Table
**Purpose**: Cache search results by position hash

**Key Features**:
- Hierarchical/multi-level entries
- Multiple replacement policies (age-based, depth-preferred, etc.)
- Zobrist hashing for position identification

### `parallel_search.rs` - Parallel Search
**Purpose**: Multi-threaded search using YBWC

**Key Types**:
```rust
pub struct ParallelSearchConfig {
    pub enable_parallel: bool,
    pub thread_count: usize,
    pub hash_size_mb: usize,
    pub ybwc_enabled: bool,
    pub ybwc_min_depth: u8,
}
```

### `quiescence.rs` - Quiescence Search
**Purpose**: Evaluate standing pat positions by searching captures

**Key Functions**:
```rust
pub fn quiescence_search(&mut self, ...) -> Score;
```

### `reductions.rs` - Late Move Reductions
**Purpose**: Reduce search depth for late moves based on heuristics

**Key Functions**:
```rust
pub fn calculate_lmr(&mut self, move_count, depth, is_check, ...) -> i8;
```

### `null_move.rs` - Null Move Pruning
**Purpose**: Prune positions where a null move improves beta

### `pvs.rs` - Principal Variation Search
**Purpose**: Null window search for PV moves

### `time_management.rs` - Time Control
**Purpose**: Intelligent time allocation

---

## Evaluation Modules (`src/evaluation/`)

### `evaluation.rs` - Main Evaluator
**Purpose**: Combine all evaluation components

**Key Functions**:
```rust
pub fn evaluate_position(&mut self, ...) -> Score;
```

### `material.rs` - Material Evaluation
**Purpose**: Evaluate piece values on board

**Key Functions**:
```rust
pub fn evaluate_material(&self, board, captured, phase) -> Score;
```

### `piece_square_tables.rs` - PST Evaluation
**Purpose**: Position-dependent piece values

**Key Types**:
```rust
pub struct PieceSquareTable {
    tables: [[i32; 81]; PIECE_TYPE_COUNT],  // 9x9 board
}
```

### `tapered_eval.rs` - Tapered Evaluation
**Purpose**: Interpolate between opening and endgame scores

```rust
pub fn tapered_score(opening: i32, endgame: i32, phase: Phase) -> Score;
```

### `king_safety.rs` - King Safety
**Purpose**: Evaluate king position and attack exposure

**Key Functions**:
```rust
pub fn evaluate_king_safety(&mut self, ...) -> Score;
```

### `castles.rs` - Castle Evaluation
**Purpose**: Evaluate castle position (王手飛車加速度, etc.)

### `attacks.rs` - Attack Detection
**Purpose**: Detect and evaluate piece attacks

### `patterns/` - Pattern Recognition
**Purpose**: Common tactical and positional patterns

### `eval_cache.rs` - Evaluation Cache
**Purpose**: Cache evaluation results by position hash

---

## Knowledge Modules

### `src/opening_book.rs` - Opening Book
**Purpose**: Pre-computed opening moves

**Key Functions**:
```rust
pub fn get_best_move(&self, fen: &str) -> Option<Move>;
pub fn get_moves(&self, fen: &str) -> Option<Vec<BookMove>>;
pub fn load_from_json(&mut self, data: &str) -> Result<()>;
```

### `src/tablebase/` - Endgame Tablebase
**Purpose**: Perfect play in common endgames

**Key Types**:
```rust
pub struct MicroTablebase { ... }
impl MicroTablebase {
    pub fn probe(&self, board, player, captured) -> Option<TablebaseResult>;
}
```

**Submodules**:
- `micro_tablebase.rs` - Main tablebase implementation
- `endgame_solvers/` - Specific endgame solvers
- `pattern_matching.rs` - Endgame pattern detection

---

## Utility Modules

### `src/config/` - Configuration
**Purpose**: Engine configuration management

### `src/tuning/` - Tuning Utilities
**Purpose**: Parameter tuning support

### `src/debug_utils.rs` - Debug Utilities
**Purpose**: Debug logging and timing

### `src/error.rs` - Error Types
**Purpose**: Engine error types

### `src/kif_parser.rs` - KIF Parser
**Purpose**: Parse KIF format files

### `src/opening_book_converter.rs` - Book Conversion
**Purpose**: Convert between opening book formats

---

## Data Flow

```
USI Command (stdin)
       ↓
   UsiHandler.handle_command()
       ↓
   ShogiEngine method
       ↓
┌──────┴──────┐
│   Search    │  ←→  TranspositionTable
│   Engine    │  ←→  OpeningBook
└──────┬──────┘
       ↓
┌──────┴──────┐
│  Evaluation │  ←→  EvalCache
└──────┬──────┘
       ↓
   BitboardBoard
       ↓
   MoveGenerator
       ↓
   Legal Moves
```
