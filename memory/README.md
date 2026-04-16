# Memory Folder

This folder contains documentation for AI agents to understand and work on the Shogi Engine project.

## Files

| File | Description |
|------|-------------|
| [PROJECT.md](./PROJECT.md) | Complete project overview with architecture diagrams |
| [ENTRY_POINTS.md](./ENTRY_POINTS.md) | Entry points, binaries, and USI command reference |
| [MODULES.md](./MODULES.md) | Module responsibilities and data flow |
| [SEARCH.md](./SEARCH.md) | Search algorithm architecture and diagrams |
| [CONTEXT.md](./CONTEXT.md) | Quick reference context for AI agents |

## Quick Start for AI Agents

1. **Start here**: Read [CONTEXT.md](./CONTEXT.md) for the essential facts
2. **Understand architecture**: See [PROJECT.md](./PROJECT.md) architecture diagrams
3. **Find code locations**: See [MODULES.md](./MODULES.md) for where things live
4. **Understand search**: See [SEARCH.md](./SEARCH.md) for algorithm details

## Architecture Summary

```
USI Protocol (stdin/stdout)
    ↓
ShogiEngine (src/lib.rs)
    ↓
┌───┴───┐
│       │
Search  Board/Moves
│       │
├── Transposition Table
├── Parallel Search
├── Iterative Deepening
├── Quiescence
└── Evaluation
    ├── Material
    ├── Piece Square Tables
    ├── King Safety
    └── Patterns
```

## Common Development Patterns

### Adding a New Search Feature
1. Add to `src/search/` module
2. Update `src/search/mod.rs` exports
3. Integrate in `src/search/search_engine.rs`
4. Add tests in `src/search/*_tests.rs`

### Adding Evaluation Component
1. Add to `src/evaluation/` module
2. Update `src/evaluation/mod.rs` exports
3. Integrate in `src/evaluation/evaluation.rs`
4. Add to tapered evaluation if needed

### Adding USI Option
1. Add option definition in `src/usi.rs:handle_usi()`
2. Add handler in `src/lib.rs:handle_setoption()`
3. Store in appropriate struct
4. Document in USI spec

## Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| Board size | 9x9 = 81 | `src/types/core.rs` |
| Piece types | 14 (7 base + 7 promoted) | `src/types/core.rs` |
| Max depth | 100 | Various search modules |
| Default hash | 16 MB | `src/lib.rs` |
| Pawn value | 100 cp | `src/types/core.rs` |

## Code Conventions

- **Naming**: snake_case for functions/variables, PascalCase for types
- **Error handling**: Use `thiserror`, return `Result<T, E>`
- **Documentation**: Module-level docs, inline comments for complex logic
- **Testing**: Unit tests in `_tests.rs` files, integration tests in `tests/`

## Testing Commands

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run library tests only
cargo test --lib

# Run with coverage
cargo tarpaulin --exclude-tests
```

## Linting

```bash
# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --all-targets --all-features -- -W clippy::all
```

## Benchmarking

```bash
# Run all benchmarks
cargo bench --features legacy-tests,simd

# Run specific benchmark
cargo bench --features legacy-tests,simd benchmark_name
```
