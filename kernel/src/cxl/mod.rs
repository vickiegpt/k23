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
        // TODO: Implement PCI enumeration for CXL devices
        // For now, this is a placeholder for future implementation
        crate::tracing::info!("CXL device discovery not yet implemented");
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
