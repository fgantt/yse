# Search Algorithm Architecture

## Overview

The engine uses a **principal variation search** with **iterative deepening**, combining multiple optimization techniques for efficient game tree exploration.

## Search Flow

```mermaid
flowchart TD
    GO["go command"] --> ID_INIT
    ID_INIT --> ID_CHECK{"depth <= max?"}
    ID_CHECK -->|Yes| AB_ENTRY
    ID_CHECK -->|No| ID_DONE
    AB_ENTRY --> AB_TT
    AB_TT --> AB_MOVES
    AB_MOVES --> AB_ORDER
    AB_ORDER --> AB_TTMOVE{"TT move?"}
    AB_TTMOVE -->|Yes| AB_RECURSIVE
    AB_TTMOVE -->|No| AB_LOOP
    AB_LOOP --> AB_PRUNE{"Prune?"}
    AB_PRUNE -->|No| AB_RECURSIVE
    AB_PRUNE -->|Yes| AB_NEXT
    AB_RECURSIVE --> QS_ENTRY
    QS_ENTRY --> QS_CHECK{"In check?"}
    QS_CHECK -->|No| QS_GEN
    QS_GEN --> QS_SEE{"SEE >= 0?"}
    QS_SEE -->|Yes| QS_SEARCH
    QS_SEE -->|No| QS_SKIP
    QS_SEARCH --> QS_NEXT
    QS_SKIP --> QS_NEXT
    QS_NEXT --> QS_LOOPEND
    AB_NEXT --> ID_CHECK
    ID_CHECK --> ID_TIME{"Time up?"}
    ID_TIME -->|No| ID_INC
    ID_TIME -->|Yes| ID_DONE
    ID_INC --> ID_CHECK
    ID_DONE["Output bestmove"]
```

## Move Ordering

```mermaid
flowchart LR
    1["1. TT Move"] --> Done
    2["2. Captures MVV-LVA"] --> Done
    3["3. Killer Moves"] --> Done
    4["4. Counter Moves"] --> Done
    5["5. History Heuristic"] --> Done
    6["6. Quiet Moves"] --> Done
```

## Alpha-Beta Search Pseudocode

```
function alphaBeta(depth, alpha, beta, player):
    if depth == 0:
        return quiescence(alpha, beta)
    
    if terminal position:
        return evaluate()
    
    ttEntry = transpositionTable.probe()
    if ttEntry valid and ttEntry.depth >= depth:
        return ttEntry.score (with bound handling)
    
    moves = generateMoves()
    orderMoves(moves, ttEntry.move)
    
    bestMove = null
    for each move in moves:
        if shouldReduce(move, depth):
            score = -alphaBeta(depth - reduction - 1, alpha, beta)
        else:
            score = -alphaBeta(depth - 1, alpha, beta)
        
        if isPVSNode():
            score = -pvs(depth - 1, alpha, alpha + 1)
        
        if score > alpha:
            alpha = score
            bestMove = move
        
        if alpha >= beta:
            updateKiller(move)
            updateHistory(move)
            break
    
    transpositionTable.store(bestMove, alpha, depth)
    return alpha
```

## Transposition Table Structure

```mermaid
classDiagram
    class TTEntry {
        +u64 hash
        +Move move
        +Score score
        +u8 depth
        +Bound bound
        +u16 age
    }
    
    class Bound {
        <<enumeration>>
        EXACT
        LOWER
        UPPER
    }
    
    TTEntry --> Bound
```

## Replacement Policy

```mermaid
flowchart TD
    A["Depth > TT.depth and EXACT/LOWER"] --> REPLACE
    B["Depth == TT.depth and EXACT"] --> REPLACE
    C["Depth >= TT.depth and age matches"] --> REPLACE
    D["Any entry (fallback)"] --> REPLACE
```

## Parallel Search (YBWC)

```mermaid
flowchart TD
    MAIN["Main Thread Root"] --> SPLIT
    SPLIT --> W1["Worker 1"]
    SPLIT --> W2["Worker 2"]
    SPLIT --> W3["Worker N"]
    W1 --> WAIT
    W2 --> WAIT
    W3 --> WAIT
    WAIT --> COPY
    COPY --> MAIN
```

## Time Management

```mermaid
flowchart TD
    START --> INIT["Initialize budget"]
    INIT --> LOOP
    LOOP --> CHECK{"Time check"}
    CHECK -->|Stable| LESS["Use less time"]
    CHECK -->|Complex| MORE["Use more time"]
    LESS --> EXIT
    MORE --> EXIT
    LOOP -->|Max depth| EXIT
    CHECK -->|Stop cmd| EXIT
```

## Key Search Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `MaxDepth` | 0 (unlimited) | Maximum search depth |
| `USI_Hash` | 16 MB | Transposition table size |
| `USI_Threads` | CPU count | Parallel search threads |
| `AspirationWindowSize` | 25 cp | Initial window size |
| `LMR_min_depth` | 3 | Minimum depth for LMR |
| `LMR_base_reduction` | 1 | Base reduction value |
| `NullMove_min_depth` | 3 | Minimum depth for NMP |
| `NullMove_verification` | true | Use verification search |
| `YBWC_min_depth` | 4 | Minimum depth for split |

## Features by Module

| Module | Features |
|--------|----------|
| `transposition_table.rs` | Hierarchical TT, ML replacement, age-based eviction |
| `parallel_search.rs` | YBWC, work stealing, thread-safe TT |
| `iterative_deepening.rs` | Aspiration windows, time management |
| `quiescence.rs` | SEE pruning, delta pruning, check extensions |
| `reductions.rs` | LMR, history-based reduction, singular move |
| `null_move.rs` | R+1 search, verification, dynamic R |
| `pvs.rs` | Principal variation search, null window |
| `move_ordering/` | MVV-LVA, killers, history, counter moves |
