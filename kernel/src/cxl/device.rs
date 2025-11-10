//! CXL device abstraction

use alloc::string::String;
use core::fmt;

/// CXL device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CXLDeviceType {
    /// Type 1: Device-only memory (e.g., GPU)
    Type1,
    /// Type 2: Device with host-managed memory (e.g., smart NIC)
    Type2,
    /// Type 3: Pure memory device (e.g., memory expansion)
    Type3,
}

/// CXL device capability flags
#[derive(Debug, Clone, Copy)]
pub struct CXLCapabilities {
    /// Supports cache coherency
    pub cache_coherent: bool,
    /// Supports memory pooling
    pub memory_pooling: bool,
    /// Supports hot-plug
    pub hot_plug: bool,
    /// Supports persistent memory
    pub persistent_memory: bool,
    /// Maximum memory bandwidth (MB/s)
    pub max_bandwidth: u64,
    /// Memory latency (nanoseconds)
    pub latency_ns: u32,
}

impl CXLCapabilities {
    pub fn new() -> Self {
        Self {
            cache_coherent: true,
            memory_pooling: false,
            hot_plug: false,
            persistent_memory: false,
            max_bandwidth: 64000, // 64 GB/s default
            latency_ns: 100,
        }
    }
}

/// CXL device trait
pub trait CXLDevice: Send + Sync {
    /// Get device type
    fn device_type(&self) -> CXLDeviceType;

    /// Get device ID
    fn device_id(&self) -> u64;

    /// Get device name
    fn name(&self) -> &str;

    /// Get device capabilities
    fn capabilities(&self) -> CXLCapabilities;

    /// Get base physical address of device memory
    fn base_address(&self) -> Option<usize>;

    /// Get size of device memory in bytes
    fn memory_size(&self) -> usize;

    /// Initialize the device
    fn init(&mut self) -> Result<(), &'static str>;

    /// Read device configuration
    fn read_config(&self, offset: usize, size: usize) -> Result<u64, &'static str>;

    /// Write device configuration
    fn write_config(&mut self, offset: usize, value: u64, size: usize) -> Result<(), &'static str>;
}

impl fmt::Debug for dyn CXLDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CXLDevice")
            .field("type", &self.device_type())
            .field("id", &self.device_id())
            .field("name", &self.name())
            .field("base_address", &self.base_address())
            .field("memory_size", &self.memory_size())
            .finish()
    }
}

/// Generic CXL memory device implementation
pub struct CXLMemoryDevice {
    device_id: u64,
    name: String,
    device_type: CXLDeviceType,
    capabilities: CXLCapabilities,
    base_address: Option<usize>,
    memory_size: usize,
}

impl CXLMemoryDevice {
    pub fn new(
        device_id: u64,
        name: String,
        device_type: CXLDeviceType,
        base_address: Option<usize>,
        memory_size: usize,
    ) -> Self {
        Self {
            device_id,
            name,
            device_type,
            capabilities: CXLCapabilities::new(),
            base_address,
            memory_size,
        }
    }

    pub fn with_capabilities(mut self, capabilities: CXLCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
}

impl CXLDevice for CXLMemoryDevice {
    fn device_type(&self) -> CXLDeviceType {
        self.device_type
    }

    fn device_id(&self) -> u64 {
        self.device_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> CXLCapabilities {
        self.capabilities
    }

    fn base_address(&self) -> Option<usize> {
        self.base_address
    }

    fn memory_size(&self) -> usize {
        self.memory_size
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // Placeholder initialization
        Ok(())
    }

    fn read_config(&self, _offset: usize, _size: usize) -> Result<u64, &'static str> {
        // Placeholder config read
        Ok(0)
    }

    fn write_config(&mut self, _offset: usize, _value: u64, _size: usize) -> Result<(), &'static str> {
        // Placeholder config write
        Ok(())
    }
}
