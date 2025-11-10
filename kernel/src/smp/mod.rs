//! SMP (Symmetric Multi-Processing) support for k23
//!
//! This module provides enhanced SMP functionality including:
//! - Cross-CPU syscall scheduling
//! - CPU affinity management
//! - Inter-processor interrupts (IPI)
//! - Cross-CPU communication

pub mod affinity;
pub mod ipc;
pub mod scheduler;

pub use affinity::{CpuAffinity, CpuSet};
pub use ipc::{IPCMessage, IPCChannel};
pub use scheduler::SmpScheduler;

use alloc::sync::Arc;
use spin::RwLock;

/// Global SMP manager instance
static SMP_MANAGER: RwLock<Option<Arc<SmpManager>>> = RwLock::new(None);

/// SMP Manager - coordinates multi-processor operations
pub struct SmpManager {
    num_cpus: usize,
    scheduler: Arc<SmpScheduler>,
    ipc_channels: Arc<ipc::IPCManager>,
}

impl SmpManager {
    pub fn new(num_cpus: usize) -> Self {
        Self {
            num_cpus,
            scheduler: Arc::new(SmpScheduler::new(num_cpus)),
            ipc_channels: Arc::new(ipc::IPCManager::new(num_cpus)),
        }
    }

    /// Get the number of CPUs
    pub fn num_cpus(&self) -> usize {
        self.num_cpus
    }

    /// Get the SMP scheduler
    pub fn scheduler(&self) -> &Arc<SmpScheduler> {
        &self.scheduler
    }

    /// Get the IPC manager
    pub fn ipc(&self) -> &Arc<ipc::IPCManager> {
        &self.ipc_channels
    }

    /// Send an IPI to a specific CPU
    pub fn send_ipi(&self, target_cpu: usize, message: IPCMessage) -> Result<(), &'static str> {
        if target_cpu >= self.num_cpus {
            return Err("Invalid CPU ID");
        }

        self.ipc_channels.send(target_cpu, message)
    }

    /// Broadcast an IPI to all CPUs except the current one
    pub fn broadcast_ipi(&self, current_cpu: usize, message: IPCMessage) -> Result<(), &'static str> {
        for cpu in 0..self.num_cpus {
            if cpu != current_cpu {
                self.ipc_channels.send(cpu, message.clone())?;
            }
        }
        Ok(())
    }
}

/// Initialize the SMP subsystem
pub fn init(num_cpus: usize) -> Result<(), &'static str> {
    let mut manager = SMP_MANAGER.write();
    if manager.is_some() {
        return Err("SMP already initialized");
    }

    let smp_manager = Arc::new(SmpManager::new(num_cpus));

    *manager = Some(smp_manager);
    Ok(())
}

/// Get the global SMP manager instance
pub fn get_manager() -> Option<Arc<SmpManager>> {
    SMP_MANAGER.read().clone()
}
