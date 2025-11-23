# Shogi Engine

This is the Shogi game engine, implementing the USI (Universal Shogi Interface) protocol.

## Building

```bash
# Standard build
cargo build --release --bin usi-engine

# Build with SIMD optimizations (native platforms only: x86_64, ARM64)
cargo build --release --bin usi-engine --features simd
```

## Running

The engine runs as a USI-compatible process, communicating via stdin/stdout.

## Features

- **SIMD Optimizations**: Enable with `--features simd` for native platform performance improvements (x86_64 with AVX2/SSE, ARM64 with NEON). Note: WebAssembly support has been removed - SIMD is native-only.

## Documentation

See the `docs/` directory for detailed documentation.
