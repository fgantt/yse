# SIMD Configuration Guide

This guide provides comprehensive documentation for configuring SIMD (Single Instruction, Multiple Data) optimizations in the Shogi engine.

## Overview

The engine supports SIMD optimizations for three main components:
1. **Evaluation**: Piece-square table (PST) evaluation
2. **Pattern Matching**: Tactical pattern detection (forks, pins, skewers)
3. **Move Generation**: Sliding piece move generation

SIMD optimizations provide significant performance improvements (2-4x speedup) for these operations, contributing to an overall 20%+ improvement in nodes per second (NPS).

## Prerequisites

### Compile-Time Feature Flag

SIMD optimizations require the `simd` feature to be enabled at compile time:

```bash
# Build with SIMD support
cargo build --release --features simd

# Run tests with SIMD support
cargo test --features simd

# Run benchmarks with SIMD support
cargo bench --features simd
```

**Important**: Runtime configuration flags have **no effect** if the `simd` feature is not enabled at compile time. The engine will always use scalar implementations in this case.

### Platform Support

SIMD optimizations are available on:
- **x86_64**: SSE (128-bit) and AVX2 (256-bit) support
- **ARM64**: NEON (128-bit) support

The engine automatically detects platform capabilities and selects the appropriate SIMD implementation.

## Configuration Structure

### SimdConfig

The `SimdConfig` structure is part of `EngineConfig` and contains three boolean flags:

```rust
pub struct SimdConfig {
    /// Enable SIMD-optimized evaluation (PST evaluation)
    pub enable_simd_evaluation: bool,
    
    /// Enable SIMD-optimized pattern matching (fork detection)
    pub enable_simd_pattern_matching: bool,
    
    /// Enable SIMD-optimized move generation (sliding pieces)
    pub enable_simd_move_generation: bool,
}
```

### Default Behavior

When the `simd` feature is **enabled** at compile time:
- All three flags default to `true`
- SIMD optimizations are used by default
- Runtime flags allow fine-grained control

When the `simd` feature is **disabled** at compile time:
- All flags default to `false`
- Flags have no effect (scalar implementations always used)

## Configuration Fields

### enable_simd_evaluation

**Type**: `bool`  
**Default**: `true` (when `simd` feature enabled)

**Description**: Controls SIMD-optimized piece-square table (PST) evaluation.

**What it affects**:
- `IntegratedEvaluator::evaluate_pst()` method
- Batch processing of piece-square table lookups
- Material evaluation using SIMD bitboards

**Performance Impact**:
- **Enabled**: 2-4x faster PST evaluation
- **Disabled**: Falls back to scalar evaluation (slower)

**When to disable**:
- Debugging evaluation issues
- Comparing SIMD vs scalar evaluation correctness
- Testing on platforms with evaluation compatibility issues

**Example**:
```rust
let mut config = EngineConfig::default();
config.simd.enable_simd_evaluation = false;
```

### enable_simd_pattern_matching

**Type**: `bool`  
**Default**: `true` (when `simd` feature enabled)

**Description**: Controls SIMD-optimized tactical pattern matching.

**What it affects**:
- `TacticalPatternRecognizer::detect_forks()` method
- Batch detection of forks, pins, skewers, and discovered attacks
- Pattern filtering using SIMD batch operations

**Performance Impact**:
- **Enabled**: 2-4x faster pattern detection
- **Disabled**: Falls back to scalar pattern matching (slower)

**When to disable**:
- Debugging pattern detection issues
- Comparing SIMD vs scalar pattern matching correctness
- Testing on platforms with pattern matching compatibility issues

**Example**:
```rust
let mut config = EngineConfig::default();
config.simd.enable_simd_pattern_matching = false;
```

### enable_simd_move_generation

**Type**: `bool`  
**Default**: `true` (when `simd` feature enabled)

**Description**: Controls SIMD-optimized move generation for sliding pieces.

**What it affects**:
- `MoveGenerator::generate_all_piece_moves()` for sliding pieces
- Batch processing of rook, bishop, and lance moves
- Vectorized magic table lookups

**Performance Impact**:
- **Enabled**: 2-4x faster sliding piece move generation
- **Disabled**: Falls back to scalar move generation (slower)

**When to disable**:
- Debugging move generation issues
- Comparing SIMD vs scalar move generation correctness
- Testing on platforms with move generation compatibility issues

**Example**:
```rust
let mut config = EngineConfig::default();
config.simd.enable_simd_move_generation = false;
```

## Configuration Methods

### Method 1: Programmatic Configuration (Rust)

#### Using Default Configuration

```rust
use shogi_engine::config::EngineConfig;

// Default: All SIMD features enabled (when simd feature is enabled)
let config = EngineConfig::default();
```

#### Custom Configuration

```rust
use shogi_engine::config::{EngineConfig, SimdConfig};

// Disable all SIMD features
let mut config = EngineConfig::default();
config.simd = SimdConfig {
    enable_simd_evaluation: false,
    enable_simd_pattern_matching: false,
    enable_simd_move_generation: false,
};

// Enable only move generation
config.simd = SimdConfig {
    enable_simd_evaluation: false,
    enable_simd_pattern_matching: false,
    enable_simd_move_generation: true,
};

// Selective configuration
config.simd.enable_simd_evaluation = false;  // Disable evaluation only
config.simd.enable_simd_pattern_matching = true;  // Keep pattern matching enabled
```

#### Applying Configuration to Engine

```rust
use shogi_engine::types::*;

let mut engine = ShogiEngine::new();

// Get current config
let mut config = engine.get_engine_config();

// Modify SIMD settings
config.simd.enable_simd_evaluation = false;

// Apply configuration
engine.update_engine_config(config)?;
```

### Method 2: JSON Configuration File

#### Creating a Configuration File

```json
{
  "search": {
    "max_depth": 20,
    "min_depth": 1
  },
  "simd": {
    "enable_simd_evaluation": true,
    "enable_simd_pattern_matching": true,
    "enable_simd_move_generation": false
  }
}
```

#### Loading Configuration from File

```rust
use shogi_engine::config::EngineConfig;

// Load from file
let config = EngineConfig::from_file("config.json")?;

// Validate configuration
config.validate()?;

// Use configuration
let engine = ShogiEngine::with_config(config);
```

#### Saving Configuration to File

```rust
use shogi_engine::config::EngineConfig;

let config = EngineConfig::default();
config.to_file("config.json")?;
```

### Method 3: Runtime Modification

#### Updating Configuration at Runtime

```rust
// Get current configuration
let mut config = engine.get_engine_config();

// Modify SIMD settings
config.simd.enable_simd_evaluation = false;

// Apply changes
engine.update_engine_config(config)?;
```

## Performance Implications

### Enabling SIMD (Default)

When all SIMD features are enabled (default behavior):

**Component-Level Performance**:
- **Evaluation**: 2-4x faster PST evaluation
- **Pattern Matching**: 2-4x faster fork/pin/skewer detection
- **Move Generation**: 2-4x faster sliding piece move generation

**Overall Engine Performance**:
- **20%+ improvement** in nodes per second (NPS)
- Better search depth for same time budget
- Improved time-to-depth for analysis

**Memory Impact**:
- Minimal additional memory usage
- SIMD operations use same data structures as scalar
- Batch operations may improve cache locality

### Disabling SIMD

When SIMD features are disabled:

**Performance Impact**:
- **2-4x slower** for affected operations
- Falls back to scalar implementations
- No functional differences (same results)

**Use Cases for Disabling**:
1. **Debugging**: Isolating SIMD-related issues
2. **Correctness Verification**: Comparing SIMD vs scalar results
3. **Platform Compatibility**: Testing on platforms with SIMD issues
4. **Performance Baseline**: Establishing scalar performance baseline

### Selective Disabling

You can disable individual components:

**Example: Disable Evaluation Only**
```rust
config.simd.enable_simd_evaluation = false;
// Pattern matching and move generation still use SIMD
```

**Performance Impact**:
- Only affected component uses scalar implementation
- Other components continue to benefit from SIMD
- Useful for isolating performance issues

## Monitoring SIMD Usage

### Telemetry

The engine provides telemetry to monitor SIMD vs scalar usage:

```rust
use shogi_engine::utils::telemetry;

// Get SIMD telemetry
let telemetry = telemetry::get_simd_telemetry();

println!("SIMD Evaluation Calls: {}", telemetry.simd_evaluation_calls);
println!("Scalar Evaluation Calls: {}", telemetry.scalar_evaluation_calls);
println!("SIMD Pattern Calls: {}", telemetry.simd_pattern_calls);
println!("Scalar Pattern Calls: {}", telemetry.scalar_pattern_calls);
println!("SIMD Move Gen Calls: {}", telemetry.simd_move_gen_calls);
println!("Scalar Move Gen Calls: {}", telemetry.scalar_move_gen_calls);
```

### From Search Engine

```rust
let search_engine = engine.search_engine.lock()?;
let telemetry = search_engine.get_simd_telemetry();

// Check if SIMD is being used
if telemetry.simd_evaluation_calls > 0 {
    println!("SIMD evaluation is active");
}
```

### Resetting Telemetry

```rust
use shogi_engine::utils::telemetry;

// Reset counters for new measurement session
telemetry::reset_simd_telemetry();
```

## Troubleshooting

### Issue: SIMD Features Not Working

**Symptoms**:
- No performance improvement
- Telemetry shows zero SIMD calls
- Configuration changes have no effect

**Solutions**:

1. **Check Compile-Time Feature**:
   ```bash
   # Verify simd feature is enabled
   cargo build --release --features simd
   
   # Check if feature is available
   cargo check --features simd
   ```

2. **Verify Runtime Flags**:
   ```rust
   let config = engine.get_engine_config();
   assert!(config.simd.enable_simd_evaluation);  // Should be true
   ```

3. **Check Platform Support**:
   - x86_64: Requires SSE (always available) or AVX2 (optional)
   - ARM64: Requires NEON (always available)
   - The engine automatically detects and uses available SIMD features

4. **Verify Configuration is Applied**:
   ```rust
   // Ensure configuration is properly set
   engine.update_engine_config(config)?;
   
   // Restart engine if needed
   ```

### Issue: Performance Not Improving

**Symptoms**:
- SIMD is enabled but no speedup observed
- Benchmarks show similar performance

**Solutions**:

1. **Verify SIMD is Actually Used**:
   ```rust
   let telemetry = get_simd_telemetry();
   // Check that simd_*_calls > 0
   ```

2. **Check Hardware Support**:
   - Older CPUs may not support AVX2 (falls back to SSE)
   - SSE still provides speedup but less than AVX2
   - Check CPU capabilities: `lscpu` (Linux) or CPU-Z (Windows)

3. **Run Benchmarks**:
   ```bash
   cargo bench --features simd --bench simd_integration_benchmarks
   ```

4. **Check Workload**:
   - SIMD benefits are most apparent with batch operations
   - Single-piece operations may not show significant improvement
   - Try positions with many pieces for better SIMD utilization

### Issue: Unexpected Behavior with SIMD Enabled

**Symptoms**:
- Incorrect evaluation results
- Wrong move generation
- Pattern detection errors

**Solutions**:

1. **Disable SIMD for Debugging**:
   ```rust
   config.simd = SimdConfig {
       enable_simd_evaluation: false,
       enable_simd_pattern_matching: false,
       enable_simd_move_generation: false,
   };
   ```

2. **Compare Results**:
   ```rust
   // Run with SIMD enabled
   let result_simd = evaluator.evaluate_pst(&board);
   
   // Run with SIMD disabled
   config.simd.enable_simd_evaluation = false;
   let result_scalar = evaluator.evaluate_pst(&board);
   
   // Results should be identical
   assert_eq!(result_simd, result_scalar);
   ```

3. **Check Telemetry**:
   - Verify which paths are being used
   - Ensure SIMD paths are actually active
   - Check for fallback to scalar

4. **Report Issue**:
   - If results differ, this indicates a bug
   - Report with configuration and test case

### Issue: Configuration Not Taking Effect

**Symptoms**:
- Configuration changes don't affect behavior
- Settings revert to defaults
- Runtime flags ignored

**Solutions**:

1. **Verify Configuration is Applied**:
   ```rust
   // Check current configuration
   let config = engine.get_engine_config();
   println!("SIMD Evaluation: {}", config.simd.enable_simd_evaluation);
   ```

2. **Check Feature Flag**:
   - Runtime flags only work when `simd` feature is enabled at compile time
   - Verify with: `#[cfg(feature = "simd")]`

3. **Restart Engine**:
   - Some configuration changes may require engine restart
   - Create new engine instance with updated config

4. **Check Configuration Source**:
   - Ensure configuration is loaded from correct source
   - Verify JSON file is properly formatted
   - Check for configuration conflicts

### Issue: Compilation Errors

**Symptoms**:
- Build fails with SIMD feature enabled
- Platform-specific errors

**Solutions**:

1. **Check Rust Version**:
   - SIMD requires Rust 1.75+ for `std::simd`
   - Platform-specific intrinsics require stable Rust
   - Update Rust: `rustup update stable`

2. **Check Platform**:
   - SIMD is only available on x86_64 and ARM64
   - Other platforms will use scalar fallbacks
   - Verify target platform: `rustc --print target-list`

3. **Check Feature Flags**:
   ```bash
   # Ensure feature is properly enabled
   cargo build --features simd
   ```

## Best Practices

### For Production Use

1. **Enable All SIMD Features** (default):
   ```rust
   let config = EngineConfig::default();
   // All SIMD features enabled by default
   ```

2. **Monitor Performance**:
   - Use telemetry to verify SIMD usage
   - Benchmark regularly to catch regressions
   - Monitor NPS improvements

3. **Platform-Specific Tuning**:
   - AVX2-capable CPUs get best performance
   - SSE fallback still provides speedup
   - ARM64 NEON provides good performance

### For Development

1. **Selective Disabling**:
   - Disable specific components for debugging
   - Isolate performance issues
   - Compare SIMD vs scalar correctness

2. **Telemetry Usage**:
   - Monitor SIMD vs scalar call counts
   - Verify expected paths are used
   - Track performance improvements

3. **Testing**:
   - Test with SIMD enabled and disabled
   - Verify identical results
   - Benchmark both configurations

### For Debugging

1. **Disable All SIMD**:
   ```rust
   config.simd = SimdConfig {
       enable_simd_evaluation: false,
       enable_simd_pattern_matching: false,
       enable_simd_move_generation: false,
   };
   ```

2. **Compare Results**:
   - Run with SIMD enabled
   - Run with SIMD disabled
   - Results should be identical

3. **Isolate Issues**:
   - Disable one component at a time
   - Identify which component causes issues
   - Report with specific component disabled

## Examples

### Example 1: Default Configuration

```rust
use shogi_engine::config::EngineConfig;
use shogi_engine::types::*;

// Create engine with default SIMD configuration
let config = EngineConfig::default();
let engine = ShogiEngine::with_config(config);

// All SIMD features enabled (when simd feature is enabled)
```

### Example 2: Disable SIMD for Debugging

```rust
use shogi_engine::config::{EngineConfig, SimdConfig};

let mut config = EngineConfig::default();

// Disable all SIMD features for debugging
config.simd = SimdConfig {
    enable_simd_evaluation: false,
    enable_simd_pattern_matching: false,
    enable_simd_move_generation: false,
};

let engine = ShogiEngine::with_config(config);
```

### Example 3: Selective Configuration

```rust
use shogi_engine::config::EngineConfig;

let mut config = EngineConfig::default();

// Disable evaluation, keep pattern matching and move generation
config.simd.enable_simd_evaluation = false;
config.simd.enable_simd_pattern_matching = true;
config.simd.enable_simd_move_generation = true;

let engine = ShogiEngine::with_config(config);
```

### Example 4: JSON Configuration

**config.json**:
```json
{
  "search": {
    "max_depth": 20
  },
  "simd": {
    "enable_simd_evaluation": true,
    "enable_simd_pattern_matching": false,
    "enable_simd_move_generation": true
  }
}
```

**Rust Code**:
```rust
use shogi_engine::config::EngineConfig;

let config = EngineConfig::from_file("config.json")?;
config.validate()?;

let engine = ShogiEngine::with_config(config);
```

### Example 5: Runtime Configuration Update

```rust
use shogi_engine::types::*;

let mut engine = ShogiEngine::new();

// Get current configuration
let mut config = engine.get_engine_config();

// Disable SIMD evaluation at runtime
config.simd.enable_simd_evaluation = false;

// Apply configuration
engine.update_engine_config(config)?;
```

### Example 6: Monitoring SIMD Usage

```rust
use shogi_engine::utils::telemetry;

// Reset telemetry
telemetry::reset_simd_telemetry();

// Run some operations
// ... engine operations ...

// Check telemetry
let stats = telemetry::get_simd_telemetry();
println!("SIMD Evaluation: {} calls", stats.simd_evaluation_calls);
println!("Scalar Evaluation: {} calls", stats.scalar_evaluation_calls);

// Calculate SIMD usage percentage
let total_eval = stats.simd_evaluation_calls + stats.scalar_evaluation_calls;
if total_eval > 0 {
    let simd_percentage = (stats.simd_evaluation_calls as f64 / total_eval as f64) * 100.0;
    println!("SIMD Usage: {:.1}%", simd_percentage);
}
```

## Related Documentation

- [Engine Configuration Guide](../../ENGINE_CONFIGURATION_GUIDE.md) - General engine configuration
- [SIMD Integration Status](SIMD_INTEGRATION_STATUS.md) - SIMD integration details
- [SIMD Performance Targets](PERFORMANCE_TARGETS.md) - Performance expectations
- [SIMD Implementation Evaluation](SIMD_IMPLEMENTATION_EVALUATION.md) - Implementation details

## Summary

SIMD configuration provides fine-grained control over performance optimizations:

- **Default**: All SIMD features enabled (when `simd` feature is enabled)
- **Runtime Control**: Enable/disable individual components
- **Performance**: 2-4x speedup per component, 20%+ overall NPS improvement
- **Compatibility**: Automatic fallback to scalar when SIMD unavailable
- **Monitoring**: Telemetry available to track SIMD usage

For most users, the default configuration (all SIMD features enabled) provides the best performance. Disable SIMD features only when debugging or when specific compatibility issues arise.




