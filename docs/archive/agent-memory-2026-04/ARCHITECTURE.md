# Shogi Engine Architecture

This document provides a comprehensive overview of the Yggdrasil Shogi Engine architecture for AI agents starting new tasks.

## High-Level Architecture

```mermaid
graph TB
    subgraph "Entry Points"
        USI[usi-engine<br/>Main USI Protocol]
        TUNER[tuner<br/>Evaluation Tuning]
        TRAINER[nnue_trainer<br/>NNUE Self-Play Training]
        ANALYZER[analyzer<br/>Position Analysis]
        TESTER[strength-tester<br/>Engine Testing]
        ASSESSOR[move-assessor<br/>Move Quality Analysis]
        PUZZLE[puzzle-gen<br/>Puzzle Generation]
        MAGIC[generate_magic_tables<br/>Magic Bitboard Gen]
        OPT_MAGIC[optimize_magic_numbers<br/>Magic Optimization]
        PST[pst-tuning-runner<br/>PST Tuning]
    end

    subgraph "Core Engine (lib.rs)"
        ENGINE[ShogiEngine]
        USI_HANDLER[UsiHandler]
    end

    subgraph "Board Representation"
        BITBOARD[BitboardBoard]
        MAGIC_TABLE[MagicTable]
        ZOBRIST[Zobrist Hashing]
    end

    subgraph "Search System"
        SEARCH_ENGINE[SearchEngine]
        ITERATIVE[IterativeDeepening]
        PVS[Principal Variation Search]
        QUIESCENCE[Quiescence Search]
        NULL_MOVE[Null Move Pruning]
        LMR[Late Move Reductions]
        TT[Transposition Table]
        PARALLEL[Parallel Search]
    end

    subgraph "Evaluation System"
        EVALUATOR[PositionEvaluator]
        NNUE[NNUE Network]
        PST_EVAL[Piece Square Tables]
        MATERIAL[Material Evaluation]
        KING_SAFETY[King Safety]
        PATTERNS[Pattern Recognition]
        TAPERED[Tapered Evaluation]
    end

    subgraph "Move Generation"
        MOVE_GEN[MoveGenerator]
        MOVE_ORD[Move Ordering]
        KILLER[Killer Moves]
        HISTORY[History Heuristic]
    end

    subgraph "Support Systems"
        OPENING[Opening Book]
        TABLEBASE[Micro Tablebase]
        TIME_MGR[Time Management]
    end

    USI --> USI_HANDLER
    USI_HANDLER --> ENGINE
    ENGINE --> SEARCH_ENGINE
    ENGINE --> OPENING
    ENGINE --> TABLEBASE
    
    SEARCH_ENGINE --> EVALUATOR
    SEARCH_ENGINE --> MOVE_GEN
    SEARCH_ENGINE --> TT
    SEARCH_ENGINE --> TIME_MGR
    SEARCH_ENGINE --> ITERATIVE
    
    ITERATIVE --> PVS
    PVS --> QUIESCENCE
    PVS --> NULL_MOVE
    PVS --> LMR
    
    EVALUATOR --> NNUE
    EVALUATOR --> PST_EVAL
    EVALUATOR --> MATERIAL
    EVALUATOR --> KING_SAFETY
    EVALUATOR --> PATTERNS
    EVALUATOR --> TAPERED
    
    MOVE_GEN --> BITBOARD
    BITBOARD --> MAGIC_TABLE
    
    MOVE_ORD --> KILLER
    MOVE_ORD --> HISTORY
    
    TT --> ZOBRIST
    
    TRAINER --> ENGINE
    TRAINER --> NNUE
    TUNER --> EVALUATOR
    ANALYZER --> ENGINE
```

## Module Dependency Graph

```mermaid
graph LR
    subgraph "src/"
        LIB[lib.rs]
        MAIN[main.rs]
        USI[usi.rs]
        
        BITBOARDS[bitboards/]
        SEARCH[search/]
        EVAL[evaluation/]
        MOVES[moves.rs]
        TYPES[types/]
        TUNING[tuning/]
        UTILS[utils/]
        
        OPENING[opening_book.rs]
        TABLEBASE[tablebase/]
        KIF[kif_parser.rs]
    end

    LIB --> BITBOARDS
    LIB --> SEARCH
    LIB --> EVAL
    LIB --> MOVES
    LIB --> TYPES
    LIB --> OPENING
    LIB --> TABLEBASE
    LIB --> USI
    LIB --> TUNING
    LIB --> UTILS
    
    MAIN --> LIB
    USI --> LIB
    
    SEARCH --> EVAL
    SEARCH --> MOVES
    SEARCH --> BITBOARDS
    SEARCH --> TYPES
    
    EVAL --> BITBOARDS
    EVAL --> TYPES
    
    MOVES --> BITBOARDS
    MOVES --> TYPES
```

## Data Flow: Search Request

```mermaid
sequenceDiagram
    participant GUI as Shogi GUI
    participant USI as UsiHandler
    participant Engine as ShogiEngine
    participant Search as SearchEngine
    participant Eval as PositionEvaluator
    participant TT as TranspositionTable
    participant MoveGen as MoveGenerator

    GUI->>USI: position startpos moves e2e4...
    USI->>Engine: handle_position()
    Engine->>Engine: Update board state
    
    GUI->>USI: go btime 60000 wtime 60000
    USI->>Engine: get_best_move()
    
    Engine->>Engine: Check opening book
    alt Opening book hit
        Engine-->>USI: Return book move
    else No book move
        Engine->>Search: search()
        
        loop Iterative Deepening
            Search->>MoveGen: generate_legal_moves()
            MoveGen-->>Search: Legal moves
            
            loop Each move
                Search->>TT: probe()
                alt TT hit
                    TT-->>Search: Cached score
                else TT miss
                    Search->>Eval: evaluate()
                    Eval-->>Search: Position score
                    Search->>TT: store()
                end
            end
            
            Search-->>Search: Update best move
        end
        
        Search-->>Engine: Best move + score
    end
    
    Engine-->>USI: bestmove e2e4
    USI-->>GUI: bestmove e2e4
```

## NNUE Training Flow

```mermaid
flowchart TB
    subgraph "Training Loop"
        INIT[Initialize/Load Weights]
        GAME[Play Self-Play Game]
        EXTRACT[Extract Training Positions]
        TD[TD-Lambda Update]
        SAVE[Save Weights Periodically]
    end
    
    subgraph "Self-Play Game"
        BOARD[Initial Position]
        SEARCH[Iterative Deepening Search]
        MOVE[Apply Best Move]
        CHECK[Check Game Over]
        RECORD[Record Position + Eval]
    end
    
    subgraph "Weight Update"
        FEATURES[Extract Active Features]
        FORWARD[Forward Pass]
        BACKWARD[Backward Pass]
        GRADIENT[Compute Gradients]
        UPDATE[Update Weights]
    end
    
    INIT --> GAME
    GAME --> EXTRACT
    EXTRACT --> TD
    TD --> SAVE
    SAVE --> GAME
    
    GAME --> BOARD
    BOARD --> SEARCH
    SEARCH --> MOVE
    MOVE --> CHECK
    CHECK -->|Game continues| RECORD
    RECORD --> SEARCH
    CHECK -->|Game over| EXTRACT
    
    TD --> FEATURES
    FEATURES --> FORWARD
    FORWARD --> BACKWARD
    BACKWARD --> GRADIENT
    GRADIENT --> UPDATE
```

## Search Algorithm Stack

```mermaid
flowchart TB
    subgraph "Search Entry"
        ID[Iterative Deepening]
    end
    
    subgraph "Main Search"
        PVS[Principal Variation Search]
        ASPIRATION[Aspiration Windows]
    end
    
    subgraph "Pruning & Reductions"
        NMP[Null Move Pruning]
        LMR[Late Move Reductions]
        FUTILITY[Futility Pruning]
        SEE[Static Exchange Eval]
    end
    
    subgraph "Move Ordering"
        TT_MOVE[TT Move First]
        KILLER[Killer Moves]
        HISTORY[History Heuristic]
        MVV_LVA[MVV-LVA Captures]
    end
    
    subgraph "Leaf Evaluation"
        QUIESCE[Quiescence Search]
        EVAL[Static Evaluation]
    end
    
    ID --> PVS
    PVS --> ASPIRATION
    
    ASPIRATION --> NMP
    NMP --> LMR
    LMR --> FUTILITY
    
    PVS --> TT_MOVE
    TT_MOVE --> KILLER
    KILLER --> HISTORY
    HISTORY --> MVV_LVA
    
    FUTILITY --> QUIESCE
    QUIESCE --> SEE
    SEE --> EVAL
```

## Evaluation Components

```mermaid
flowchart LR
    subgraph "Input"
        BOARD[Board State]
        PLAYER[Side to Move]
        CAPTURED[Captured Pieces]
    end
    
    subgraph "NNUE Path"
        NNUE_FEAT[Feature Extraction]
        NNUE_ACCUM[Accumulator]
        NNUE_L1[Hidden Layer 1<br/>256 neurons]
        NNUE_L2[Hidden Layer 2<br/>32 neurons]
        NNUE_OUT[Output<br/>1 value]
    end
    
    subgraph "Classical Path"
        MAT[Material Count]
        PST[Piece Square Tables]
        KING[King Safety]
        PATTERN[Pattern Recognition]
        MOBILITY[Mobility]
    end
    
    subgraph "Tapered Evaluation"
        PHASE[Game Phase Detection]
        OPEN[Opening Weights]
        END[Endgame Weights]
        INTERP[Interpolation]
    end
    
    BOARD --> NNUE_FEAT
    NNUE_FEAT --> NNUE_ACCUM
    NNUE_ACCUM --> NNUE_L1
    NNUE_L1 --> NNUE_L2
    NNUE_L2 --> NNUE_OUT
    
    BOARD --> MAT
    BOARD --> PST
    BOARD --> KING
    BOARD --> PATTERN
    
    MAT --> PHASE
    PHASE --> OPEN
    PHASE --> END
    OPEN --> INTERP
    END --> INTERP
    
    NNUE_OUT --> FINAL[Final Score]
    INTERP --> FINAL
```

## Bitboard Architecture

```mermaid
flowchart TB
    subgraph "Bitboard Representation"
        BB[Bitboard: u128]
        BB_DESC["9x9 = 81 squares<br/>Fits in 128 bits"]
    end
    
    subgraph "Piece Bitboards"
        BLACK_PAWN[Black Pawns]
        BLACK_LANCE[Black Lances]
        BLACK_KNIGHT[Black Knights]
        BLACK_SILVER[Black Silvers]
        BLACK_GOLD[Black Golds]
        BLACK_BISHOP[Black Bishops]
        BLACK_ROOK[Black Rooks]
        BLACK_KING[Black King]
        WHITE_PIECES[White Pieces...]
    end
    
    subgraph "Magic Bitboards"
        MAGIC_TABLE[Magic Number Table]
        ATTACK_TABLE[Attack Tables]
        BLOCKER_MASK[Blocker Masks]
    end
    
    subgraph "Operations"
        AND[AND - Intersection]
        OR[OR - Union]
        XOR[XOR - Toggle]
        SHIFT[Shift - Movement]
        POPCOUNT[Popcount - Count]
        BITSCAN[Bitscan - Find]
    end
    
    BB --> BLACK_PAWN
    BB --> BLACK_LANCE
    BB --> BLACK_KNIGHT
    BB --> BLACK_SILVER
    BB --> BLACK_GOLD
    BB --> BLACK_BISHOP
    BB --> BLACK_ROOK
    BB --> BLACK_KING
    BB --> WHITE_PIECES
    
    MAGIC_TABLE --> ATTACK_TABLE
    BLOCKER_MASK --> MAGIC_TABLE
    
    BB --> AND
    BB --> OR
    BB --> XOR
    BB --> SHIFT
    BB --> POPCOUNT
    BB --> BITSCAN
```

## Transposition Table Structure

```mermaid
flowchart TB
    subgraph "TT Entry (Compact)"
        HASH[Zobrist Hash Key]
        DEPTH[Search Depth]
        SCORE[Evaluation Score]
        BOUND[Bound Type<br/>EXACT/LOWER/UPPER]
        MOVE[Best Move]
        AGE[Search Age]
    end
    
    subgraph "TT Operations"
        PROBE[Probe<br/>Check if position exists]
        STORE[Store<br/>Save search result]
        REPLACE[Replace<br/>Handle collisions]
        PREFETCH[Prefetch<br/>Hint next access]
    end
    
    subgraph "Hierarchical TT"
        L1[Level 1: Hot entries]
        L2[Level 2: Warm entries]
        L3[Level 3: Cold entries]
    end
    
    HASH --> PROBE
    PROBE --> L1
    L1 -->|Miss| L2
    L2 -->|Miss| L3
    
    STORE --> REPLACE
    REPLACE --> L1
```

## Parallel Search Architecture

```mermaid
flowchart TB
    subgraph "Main Thread"
        MAIN[Root Search]
        SPLIT[Split Point Detection]
        COORD[Work Coordination]
    end
    
    subgraph "Worker Threads"
        W1[Worker 1]
        W2[Worker 2]
        W3[Worker 3]
        WN[Worker N]
    end
    
    subgraph "Work Distribution"
        QUEUE[Work Stealing Queue]
        WORK[Work Units]
        RESULTS[Result Collection]
    end
    
    subgraph "Shared State"
        SHARED_TT[Shared TT]
        STOP_FLAG[Stop Flag]
        BEST_MOVE[Best Move]
    end
    
    MAIN --> SPLIT
    SPLIT --> QUEUE
    QUEUE --> WORK
    
    WORK --> W1
    WORK --> W2
    WORK --> W3
    WORK --> WN
    
    W1 --> RESULTS
    W2 --> RESULTS
    W3 --> RESULTS
    WN --> RESULTS
    
    RESULTS --> COORD
    
    W1 -.-> SHARED_TT
    W2 -.-> SHARED_TT
    W3 -.-> SHARED_TT
    WN -.-> SHARED_TT
    
    COORD -.-> STOP_FLAG
    COORD -.-> BEST_MOVE
```

## Key Data Structures

### Position Representation
```
BitboardBoard {
    pieces: [Bitboard; 28]     // 14 piece types x 2 players
    occupied: [Bitboard; 2]    // All pieces per player
    all_occupied: Bitboard     // All pieces combined
}
```

### Move Representation
```
Move {
    from: Option<Position>     // None for drops
    to: Position
    piece_type: PieceType
    player: Player
    is_promotion: bool
    is_capture: bool
    captured_piece: Option<PieceType>
}
```

### Search State
```
SearchEngine {
    evaluator: PositionEvaluator
    transposition_table: TranspositionTable
    killer_moves: [[Move; 2]; MAX_PLY]
    history_table: [[i32; 81]; 28]
    pv_table: PrincipalVariation
}
```

### NNUE Weights
```
NNUEWeights {
    input_weights: Vec<Vec<f32>>     // [768 inputs][256 hidden1]
    hidden1_biases: Vec<f32>         // [256]
    hidden1_to_hidden2: Vec<Vec<f32>>// [256][32]
    hidden2_biases: Vec<f32>         // [32]
    hidden2_to_output: Vec<f32>      // [32]
    output_bias: f32
}
```
