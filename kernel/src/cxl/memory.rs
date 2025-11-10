//! CXL memory management

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;
use spin::RwLock;

/// Represents a CXL memory region
#[derive(Debug, Clone)]
pub struct CXLMemoryRegion {
    /// Physical address range
    pub physical_range: Range<usize>,
    /// Device ID that owns this region
    pub device_id: u64,
    /// Whether this region is currently allocated
    pub allocated: bool,
    /// NUMA node (if applicable)
    pub numa_node: Option<u32>,
}

impl CXLMemoryRegion {
    pub fn new(physical_range: Range<usize>, device_id: u64) -> Self {
        Self {
            physical_range,
            device_id,
            allocated: false,
            numa_node: None,
        }
    }

    pub fn size(&self) -> usize {
        self.physical_range.end - self.physical_range.start
    }
}

/// CXL memory allocation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationPolicy {
    /// First-fit allocation
    FirstFit,
    /// Best-fit allocation
    BestFit,
    /// NUMA-aware allocation
    NumaAware,
}

/// CXL Memory Manager - manages memory regions from CXL devices
pub struct CXLMemoryManager {
    regions: RwLock<BTreeMap<usize, CXLMemoryRegion>>,
    allocation_policy: RwLock<AllocationPolicy>,
}

impl CXLMemoryManager {
    pub fn new() -> Self {
        Self {
            regions: RwLock::new(BTreeMap::new()),
            allocation_policy: RwLock::new(AllocationPolicy::FirstFit),
        }
    }

    /// Register a CXL memory region
    pub fn register_region(&self, region: CXLMemoryRegion) -> Result<(), &'static str> {
        let base_addr = region.physical_range.start;
        let mut regions = self.regions.write();

        // Check for overlaps
        for (_, existing) in regions.iter() {
            if self.ranges_overlap(&region.physical_range, &existing.physical_range) {
                return Err("Memory region overlaps with existing region");
            }
        }

        regions.insert(base_addr, region);
        Ok(())
    }

    /// Allocate a CXL memory region of the specified size
    pub fn allocate(&self, size: usize, numa_node: Option<u32>) -> Result<Range<usize>, &'static str> {
        if size == 0 {
            return Err("Cannot allocate zero-sized region");
        }

        let mut regions = self.regions.write();
        let policy = *self.allocation_policy.read();

        let region = self.find_free_region(&regions, size, numa_node, policy)
            .ok_or("No suitable CXL memory region available")?;

        // Mark as allocated
        if let Some(r) = regions.get_mut(&region) {
            r.allocated = true;
            Ok(r.physical_range.clone())
        } else {
            Err("Region not found")
        }
    }

    /// Free a CXL memory region
    pub fn free(&self, address: usize) -> Result<(), &'static str> {
        let mut regions = self.regions.write();

        let region = regions.get_mut(&address)
            .ok_or("Region not found")?;

        if !region.allocated {
            return Err("Region not allocated");
        }

        region.allocated = false;
        Ok(())
    }

    /// Get all registered regions
    pub fn regions(&self) -> Vec<CXLMemoryRegion> {
        self.regions.read().values().cloned().collect()
    }

    /// Get total CXL memory size
    pub fn total_memory(&self) -> usize {
        self.regions.read()
            .values()
            .map(|r| r.size())
            .sum()
    }

    /// Get available CXL memory size
    pub fn available_memory(&self) -> usize {
        self.regions.read()
            .values()
            .filter(|r| !r.allocated)
            .map(|r| r.size())
            .sum()
    }

    /// Set allocation policy
    pub fn set_allocation_policy(&self, policy: AllocationPolicy) {
        *self.allocation_policy.write() = policy;
    }

    fn find_free_region(
        &self,
        regions: &BTreeMap<usize, CXLMemoryRegion>,
        size: usize,
        numa_node: Option<u32>,
        policy: AllocationPolicy,
    ) -> Option<usize> {
        let mut candidates: Vec<_> = regions
            .iter()
            .filter(|(_, r)| !r.allocated && r.size() >= size)
            .filter(|(_, r)| {
                if let Some(node) = numa_node {
                    r.numa_node == Some(node) || policy != AllocationPolicy::NumaAware
                } else {
                    true
                }
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        match policy {
            AllocationPolicy::FirstFit => {
                candidates.first().map(|(addr, _)| **addr)
            }
            AllocationPolicy::BestFit => {
                candidates.sort_by_key(|(_, r)| r.size());
                candidates.first().map(|(addr, _)| **addr)
            }
            AllocationPolicy::NumaAware => {
                // Prefer same NUMA node
                candidates.sort_by_key(|(_, r)| {
                    if r.numa_node == numa_node {
                        0
                    } else {
                        1
                    }
                });
                candidates.first().map(|(addr, _)| **addr)
            }
        }
    }

    fn ranges_overlap(&self, a: &Range<usize>, b: &Range<usize>) -> bool {
        a.start < b.end && b.start < a.end
    }
}
