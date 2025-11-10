//! CPU affinity management

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// Represents a set of CPUs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSet {
    mask: Vec<bool>,
}

impl CpuSet {
    /// Create a new CPU set with the given number of CPUs
    pub fn new(num_cpus: usize) -> Self {
        Self {
            mask: vec![false; num_cpus],
        }
    }

    /// Create a CPU set with all CPUs enabled
    pub fn all(num_cpus: usize) -> Self {
        Self {
            mask: vec![true; num_cpus],
        }
    }

    /// Create a CPU set with a single CPU
    pub fn single(num_cpus: usize, cpu_id: usize) -> Result<Self, &'static str> {
        if cpu_id >= num_cpus {
            return Err("CPU ID out of range");
        }

        let mut mask = vec![false; num_cpus];
        mask[cpu_id] = true;

        Ok(Self { mask })
    }

    /// Add a CPU to the set
    pub fn add(&mut self, cpu_id: usize) -> Result<(), &'static str> {
        if cpu_id >= self.mask.len() {
            return Err("CPU ID out of range");
        }
        self.mask[cpu_id] = true;
        Ok(())
    }

    /// Remove a CPU from the set
    pub fn remove(&mut self, cpu_id: usize) -> Result<(), &'static str> {
        if cpu_id >= self.mask.len() {
            return Err("CPU ID out of range");
        }
        self.mask[cpu_id] = false;
        Ok(())
    }

    /// Check if a CPU is in the set
    pub fn contains(&self, cpu_id: usize) -> bool {
        cpu_id < self.mask.len() && self.mask[cpu_id]
    }

    /// Get the number of CPUs in the set
    pub fn count(&self) -> usize {
        self.mask.iter().filter(|&&x| x).count()
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Get the first CPU in the set
    pub fn first(&self) -> Option<usize> {
        self.mask.iter().position(|&x| x)
    }

    /// Get all CPUs in the set
    pub fn cpus(&self) -> Vec<usize> {
        self.mask
            .iter()
            .enumerate()
            .filter_map(|(i, &enabled)| if enabled { Some(i) } else { None })
            .collect()
    }

    /// Clear all CPUs from the set
    pub fn clear(&mut self) {
        for cpu in &mut self.mask {
            *cpu = false;
        }
    }
}

impl fmt::Display for CpuSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cpus: Vec<_> = self.cpus();
        write!(f, "{{{}}}", cpus.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "))
    }
}

/// CPU affinity for a task
#[derive(Debug, Clone)]
pub struct CpuAffinity {
    allowed_cpus: CpuSet,
    preferred_cpu: Option<usize>,
}

impl CpuAffinity {
    /// Create a new CPU affinity with all CPUs allowed
    pub fn any(num_cpus: usize) -> Self {
        Self {
            allowed_cpus: CpuSet::all(num_cpus),
            preferred_cpu: None,
        }
    }

    /// Create a CPU affinity for a specific CPU
    pub fn single(num_cpus: usize, cpu_id: usize) -> Result<Self, &'static str> {
        Ok(Self {
            allowed_cpus: CpuSet::single(num_cpus, cpu_id)?,
            preferred_cpu: Some(cpu_id),
        })
    }

    /// Create a CPU affinity with a custom CPU set
    pub fn custom(allowed_cpus: CpuSet) -> Self {
        let preferred_cpu = allowed_cpus.first();
        Self {
            allowed_cpus,
            preferred_cpu,
        }
    }

    /// Get the allowed CPU set
    pub fn allowed_cpus(&self) -> &CpuSet {
        &self.allowed_cpus
    }

    /// Get the preferred CPU
    pub fn preferred_cpu(&self) -> Option<usize> {
        self.preferred_cpu
    }

    /// Set the preferred CPU
    pub fn set_preferred_cpu(&mut self, cpu_id: usize) -> Result<(), &'static str> {
        if !self.allowed_cpus.contains(cpu_id) {
            return Err("CPU not in allowed set");
        }
        self.preferred_cpu = Some(cpu_id);
        Ok(())
    }

    /// Check if a CPU is allowed
    pub fn is_allowed(&self, cpu_id: usize) -> bool {
        self.allowed_cpus.contains(cpu_id)
    }

    /// Select a CPU to run on (prefers the preferred CPU if set)
    pub fn select_cpu(&self) -> Option<usize> {
        if let Some(cpu) = self.preferred_cpu {
            if self.allowed_cpus.contains(cpu) {
                return Some(cpu);
            }
        }
        self.allowed_cpus.first()
    }
}

impl fmt::Display for CpuAffinity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "allowed={}", self.allowed_cpus)?;
        if let Some(cpu) = self.preferred_cpu {
            write!(f, ", preferred={}", cpu)?;
        }
        Ok(())
    }
}
