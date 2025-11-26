/// Tests for configuration system, including SIMD runtime flags
///
/// # Task 4.0 (Task 4.9)
use shogi_engine::config::{EngineConfig, SimdConfig};

#[test]
fn test_simd_config_default() {
    let config = SimdConfig::default();

    #[cfg(feature = "simd")]
    {
        // When SIMD feature is enabled, defaults should be true
        assert!(config.enable_simd_evaluation);
        assert!(config.enable_simd_pattern_matching);
        assert!(config.enable_simd_move_generation);
    }

    #[cfg(not(feature = "simd"))]
    {
        // When SIMD feature is disabled, defaults should be false
        assert!(!config.enable_simd_evaluation);
        assert!(!config.enable_simd_pattern_matching);
        assert!(!config.enable_simd_move_generation);
    }
}

#[test]
fn test_simd_config_custom() {
    let config = SimdConfig {
        enable_simd_evaluation: false,
        enable_simd_pattern_matching: true,
        enable_simd_move_generation: false,
    };

    assert!(!config.enable_simd_evaluation);
    assert!(config.enable_simd_pattern_matching);
    assert!(!config.enable_simd_move_generation);
}

#[test]
fn test_simd_config_validate() {
    let config = SimdConfig::default();
    // SIMD config should always validate (boolean flags)
    assert!(config.validate().is_ok());
}

#[test]
fn test_engine_config_has_simd() {
    let config = EngineConfig::default();

    #[cfg(feature = "simd")]
    {
        assert!(config.simd.enable_simd_evaluation);
        assert!(config.simd.enable_simd_pattern_matching);
        assert!(config.simd.enable_simd_move_generation);
    }
}

#[test]
fn test_engine_config_simd_can_be_disabled() {
    let mut config = EngineConfig::default();

    #[cfg(feature = "simd")]
    {
        config.simd.enable_simd_evaluation = false;
        config.simd.enable_simd_pattern_matching = false;
        config.simd.enable_simd_move_generation = false;

        assert!(!config.simd.enable_simd_evaluation);
        assert!(!config.simd.enable_simd_pattern_matching);
        assert!(!config.simd.enable_simd_move_generation);

        // Config should still validate
        assert!(config.validate().is_ok());
    }
}

#[test]
fn test_simd_config_serialization() {
    use serde_json;

    let config = SimdConfig {
        enable_simd_evaluation: true,
        enable_simd_pattern_matching: false,
        enable_simd_move_generation: true,
    };

    // Test serialization
    let json = serde_json::to_string(&config).expect("Should serialize");
    assert!(json.contains("enable_simd_evaluation"));
    assert!(json.contains("enable_simd_pattern_matching"));
    assert!(json.contains("enable_simd_move_generation"));

    // Test deserialization
    let deserialized: SimdConfig = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(deserialized.enable_simd_evaluation, config.enable_simd_evaluation);
    assert_eq!(deserialized.enable_simd_pattern_matching, config.enable_simd_pattern_matching);
    assert_eq!(deserialized.enable_simd_move_generation, config.enable_simd_move_generation);
}

#[test]
fn test_engine_config_simd_serialization() {
    use serde_json;

    let config = EngineConfig::default();

    // Test that SIMD config is included in serialization
    let json = serde_json::to_string(&config).expect("Should serialize");
    assert!(json.contains("simd"));

    // Test deserialization
    let deserialized: EngineConfig = serde_json::from_str(&json).expect("Should deserialize");

    #[cfg(feature = "simd")]
    {
        assert_eq!(deserialized.simd.enable_simd_evaluation, config.simd.enable_simd_evaluation);
    }
}
