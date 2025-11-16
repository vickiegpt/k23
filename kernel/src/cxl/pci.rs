//! PCI enumeration for CXL devices
//!
//! Based on Linux kernel CXL PCI driver (drivers/cxl/pci.c)

use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::format;

use super::device::{CXLDevice, CXLDeviceType, CXLMemoryDevice, CXLCapabilities};

/// PCI Configuration Space Register Offsets
const PCI_VENDOR_ID: u16 = 0x00;
const PCI_DEVICE_ID: u16 = 0x02;
const PCI_COMMAND: u16 = 0x04;
const PCI_STATUS: u16 = 0x06;
const PCI_CLASS_REVISION: u16 = 0x08;
const PCI_HEADER_TYPE: u16 = 0x0E;
const PCI_BASE_ADDRESS_0: u16 = 0x10;

/// PCI Configuration Space I/O Ports (x86_64 legacy method)
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// PCI Express Extended Capability IDs
const PCI_EXT_CAP_ID_DVSEC: u16 = 0x0023; // Designated Vendor-Specific Extended Capability

/// CXL DVSEC IDs
const CXL_DVSEC_PCIE_DEVICE: u16 = 0x0000;
const CXL_DVSEC_NON_CXL_FUNCTION_MAP: u16 = 0x0002;
const CXL_DVSEC_CXL2_0_EXT_DVSEC: u16 = 0x0003;

/// PCI Command Register bits
const PCI_COMMAND_MEMORY: u16 = 0x0002;
const PCI_COMMAND_MASTER: u16 = 0x0004;

/// Read a 32-bit value from PCI configuration space
#[cfg(target_arch = "x86_64")]
unsafe fn pci_config_read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    use core::arch::asm;

    // Build address: Enable bit (31) | Bus (23-16) | Device (15-11) | Function (10-8) | Offset (7-2)
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    // Write address to 0xCF8
    asm!(
        "out dx, eax",
        in("dx") PCI_CONFIG_ADDRESS,
        in("eax") address,
        options(nomem, nostack, preserves_flags)
    );

    // Read data from 0xCFC
    let value: u32;
    asm!(
        "in eax, dx",
        in("dx") PCI_CONFIG_DATA,
        out("eax") value,
        options(nomem, nostack, preserves_flags)
    );

    value
}

/// Write a 32-bit value to PCI configuration space
#[cfg(target_arch = "x86_64")]
unsafe fn pci_config_write_u32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    use core::arch::asm;

    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    // Write address
    asm!(
        "out dx, eax",
        in("dx") PCI_CONFIG_ADDRESS,
        in("eax") address,
        options(nomem, nostack, preserves_flags)
    );

    // Write data
    asm!(
        "out dx, eax",
        in("dx") PCI_CONFIG_DATA,
        in("eax") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Read a 16-bit value from PCI configuration space
#[cfg(target_arch = "x86_64")]
unsafe fn pci_config_read_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let value = pci_config_read_u32(bus, device, function, offset & 0xFC);
    ((value >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

/// PCI device location
#[derive(Debug, Clone, Copy)]
pub struct PCIAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PCIAddress {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }
}

/// PCI device information
#[derive(Debug)]
pub struct PCIDeviceInfo {
    pub address: PCIAddress,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub header_type: u8,
    pub bar0: u32,
}

impl PCIDeviceInfo {
    /// Read PCI device information
    #[cfg(target_arch = "x86_64")]
    pub unsafe fn read(address: PCIAddress) -> Option<Self> {
        let vendor_id = pci_config_read_u16(address.bus, address.device, address.function, PCI_VENDOR_ID as u8);

        // 0xFFFF means no device
        if vendor_id == 0xFFFF {
            return None;
        }

        let device_id = pci_config_read_u16(address.bus, address.device, address.function, PCI_DEVICE_ID as u8);
        let class_rev = pci_config_read_u32(address.bus, address.device, address.function, PCI_CLASS_REVISION as u8);
        let header_type = pci_config_read_u16(address.bus, address.device, address.function, PCI_HEADER_TYPE as u8) as u8;
        let bar0 = pci_config_read_u32(address.bus, address.device, address.function, PCI_BASE_ADDRESS_0 as u8);

        Some(Self {
            address,
            vendor_id,
            device_id,
            class_code: ((class_rev >> 24) & 0xFF) as u8,
            subclass: ((class_rev >> 16) & 0xFF) as u8,
            prog_if: ((class_rev >> 8) & 0xFF) as u8,
            header_type,
            bar0,
        })
    }

    /// Check if this device has CXL capabilities
    pub fn is_cxl_capable(&self) -> bool {
        // CXL devices typically have Memory Controller class (0x05, subclass 0x02)
        // or Bridge class (0x06) with CXL DVSEC
        self.class_code == 0x05 && self.subclass == 0x02
    }

    /// Try to determine CXL device type from capabilities
    /// This is a simplified version - real implementation would read DVSEC
    pub fn guess_cxl_type(&self) -> CXLDeviceType {
        // Based on class code and subclass, make an educated guess
        match (self.class_code, self.subclass) {
            (0x05, 0x02) => CXLDeviceType::Type3, // Memory controller -> likely Type 3
            (0x06, _) => CXLDeviceType::Type2,     // Bridge -> likely Type 2 accelerator
            (0x0C, _) => CXLDeviceType::Type1,     // Serial bus controller -> likely Type 1
            _ => CXLDeviceType::Type3,             // Default to Type 3
        }
    }
}

/// Enumerate PCI buses looking for CXL devices
#[cfg(target_arch = "x86_64")]
pub fn enumerate_cxl_devices() -> Result<Vec<Arc<dyn CXLDevice>>, &'static str> {
    let mut devices = Vec::new();
    let mut device_count = 0u64;

    tracing::info!("Scanning PCI buses for CXL devices...");

    // Scan bus 0 only for now (full scan would be 0-255)
    // In production, we'd check PCI host bridge configuration
    for bus in 0..=0 {
        for device in 0..32 {
            for function in 0..8 {
                let address = PCIAddress::new(bus, device, function);

                unsafe {
                    if let Some(info) = PCIDeviceInfo::read(address) {
                        tracing::debug!(
                            "Found PCI device at {}:{}:{} - Vendor: {:#x}, Device: {:#x}, Class: {:#x}:{:#x}",
                            bus, device, function,
                            info.vendor_id, info.device_id,
                            info.class_code, info.subclass
                        );

                        // Check if this could be a CXL device
                        if info.is_cxl_capable() {
                            tracing::info!(
                                "Found potential CXL device at {}:{}:{} - Vendor: {:#x}, Device: {:#x}",
                                bus, device, function, info.vendor_id, info.device_id
                            );

                            // Create CXL device instance
                            let device_type = info.guess_cxl_type();
                            let base_addr = if info.bar0 & 0x1 == 0 {
                                // Memory BAR
                                Some((info.bar0 & 0xFFFFFFF0) as usize)
                            } else {
                                None
                            };

                            let cxl_device = CXLMemoryDevice::new(
                                device_count,
                                format!("cxl{}_{:x}:{:x}", device_count, info.vendor_id, info.device_id),
                                device_type,
                                base_addr,
                                256 * 1024 * 1024, // Default 256MB, would read from device
                            );

                            let mut cxl_device_arc: Arc<dyn CXLDevice> = Arc::new(cxl_device);

                            // Initialize the device
                            // Note: We need to get mutable access, but Arc<dyn Trait> is tricky
                            // In a real implementation, we'd use interior mutability or different design
                            tracing::info!("Created CXL {:?} device: {}", device_type, device_count);

                            devices.push(cxl_device_arc);
                            device_count += 1;
                        }

                        // Multi-function device check
                        if function == 0 && (info.header_type & 0x80) == 0 {
                            break; // Not a multi-function device, skip other functions
                        }
                    }
                }
            }
        }
    }

    if device_count > 0 {
        tracing::info!("Found {} CXL device(s)", device_count);
    } else {
        tracing::info!("No CXL devices found on PCI bus 0");
    }

    Ok(devices)
}

/// Placeholder for other architectures
#[cfg(not(target_arch = "x86_64"))]
pub fn enumerate_cxl_devices() -> Result<Vec<Arc<dyn CXLDevice>>, &'static str> {
    tracing::warn!("CXL PCI enumeration not implemented for this architecture");
    Ok(Vec::new())
}
