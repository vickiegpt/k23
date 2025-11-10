//! CXL configuration and feature flags

/// CXL configuration
#[derive(Debug, Clone)]
pub struct CXLConfig {
    /// Enable CXL cache coherency
    pub cache_coherency: bool,
    /// Enable CXL memory pooling
    pub memory_pooling: bool,
    /// Enable hot-plug support
    pub hot_plug: bool,
    /// Enable Quality of Service (QoS)
    pub qos_enabled: bool,
    /// Maximum number of CXL devices
    pub max_devices: usize,
    /// Memory overcommit ratio (1.0 = no overcommit)
    pub overcommit_ratio: f32,
}

impl Default for CXLConfig {
    fn default() -> Self {
        Self {
            cache_coherency: true,
            memory_pooling: true,
            hot_plug: false,
            qos_enabled: false,
            max_devices: 16,
            overcommit_ratio: 1.0,
        }
    }
}

impl CXLConfig {
    /// Create a new CXL configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable all features
    pub fn all_features() -> Self {
        Self {
            cache_coherency: true,
            memory_pooling: true,
            hot_plug: true,
            qos_enabled: true,
            max_devices: 32,
            overcommit_ratio: 1.0,
        }
    }

    /// Minimal configuration (only cache coherency)
    pub fn minimal() -> Self {
        Self {
            cache_coherency: true,
            memory_pooling: false,
            hot_plug: false,
            qos_enabled: false,
            max_devices: 4,
            overcommit_ratio: 1.0,
        }
    }
}
