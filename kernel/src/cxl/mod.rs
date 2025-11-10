//! CXL (Compute Express Link) support for k23
//!
//! This module provides support for CXL devices, including memory pooling,
//! device discovery, and memory-mapped access.

pub mod device;
pub mod memory;
pub mod config;

pub use device::{CXLDevice, CXLDeviceType};
pub use memory::{CXLMemoryRegion, CXLMemoryManager};
pub use config::CXLConfig;

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

/// Global CXL manager instance
static CXL_MANAGER: RwLock<Option<Arc<CXLManager>>> = RwLock::new(None);

/// CXL Manager - manages all CXL devices and memory regions
pub struct CXLManager {
    devices: RwLock<Vec<Arc<dyn CXLDevice>>>,
    memory_manager: Arc<CXLMemoryManager>,
}

impl CXLManager {
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(Vec::new()),
            memory_manager: Arc::new(CXLMemoryManager::new()),
        }
    }

    /// Register a CXL device
    pub fn register_device(&self, device: Arc<dyn CXLDevice>) -> Result<(), &'static str> {
        let mut devices = self.devices.write();
        devices.push(device);
        Ok(())
    }

    /// Get all registered CXL devices
    pub fn devices(&self) -> Vec<Arc<dyn CXLDevice>> {
        self.devices.read().clone()
    }

    /// Get the memory manager
    pub fn memory_manager(&self) -> &Arc<CXLMemoryManager> {
        &self.memory_manager
    }

    /// Discover and initialize CXL devices
    pub fn discover_devices(&self) -> Result<(), &'static str> {
        tracing::info!("Starting CXL device discovery...");

        // TODO: Implement full PCI enumeration for CXL devices
        // This requires:
        // 1. PCI configuration space access (via MMIO or I/O ports)
        // 2. Scanning PCI buses for CXL-capable devices (vendor/device IDs)
        // 3. Reading CXL DVSEC (Designated Vendor-Specific Extended Capability)
        // 4. Initializing CXL.cache and CXL.mem protocols
        // 5. Enumerating HDM (Host-managed Device Memory) decoders

        #[cfg(target_arch = "x86_64")]
        {
            // For x86_64, we would typically:
            // - Use MMCONFIG or legacy PCI config I/O ports (0xCF8/0xCFC)
            // - Scan bus 0-255, device 0-31, function 0-7
            // - Look for devices with CXL capabilities
            tracing::debug!("x86_64: PCI enumeration for CXL devices not yet implemented");
            tracing::debug!("Would scan PCI buses via MMCONFIG or I/O ports");
        }

        #[cfg(target_arch = "riscv64")]
        {
            // For RISC-V, we would typically:
            // - Use device tree or ACPI tables to find PCIe root complexes
            // - Enumerate devices via ECAM (Enhanced Configuration Access Mechanism)
            tracing::debug!("RISC-V: PCIe/CXL enumeration via device tree not yet implemented");
        }

        tracing::info!("CXL device discovery completed (0 devices found - enumeration not implemented)");
        Ok(())
    }
}

/// Initialize the CXL subsystem
pub fn init() -> Result<(), &'static str> {
    let mut manager = CXL_MANAGER.write();
    if manager.is_some() {
        return Err("CXL already initialized");
    }

    let cxl_manager = Arc::new(CXLManager::new());

    // Discover CXL devices
    cxl_manager.discover_devices()?;

    *manager = Some(cxl_manager);
    Ok(())
}

/// Get the global CXL manager instance
pub fn get_manager() -> Option<Arc<CXLManager>> {
    CXL_MANAGER.read().clone()
}
