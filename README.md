# Shogi Engine

This is the Shogi game engine, implementing the USI (Universal Shogi Interface) protocol.

## Building

```bash
# Standard build (SIMD optimizations enabled by default)
cargo build --release --bin usi-engine

# Build without SIMD optimizations (if needed for compatibility)
cargo build --release --bin usi-engine --no-default-features --features hierarchical-tt
```

## Running

The engine runs as a USI-compatible process, communicating via stdin/stdout.

## Features

- **SIMD Optimizations**: Enabled by default for optimal performance (x86_64 with AVX2/SSE, ARM64 with NEON). Provides 2-4x speedup for bitwise operations and 20%+ overall NPS improvement. Note: WebAssembly support has been removed - SIMD is native-only. Disable with `--no-default-features` if needed.

## Documentation

See the `docs/` directory for detailed documentation.
