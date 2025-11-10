//! SMP-aware task scheduler

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

use super::affinity::CpuAffinity;

/// Task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Realtime = 4,
}

/// Task information
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub priority: Priority,
    pub affinity: CpuAffinity,
    pub current_cpu: Option<usize>,
}

impl Task {
    pub fn new(id: TaskId, num_cpus: usize) -> Self {
        Self {
            id,
            priority: Priority::Normal,
            affinity: CpuAffinity::any(num_cpus),
            current_cpu: None,
        }
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_affinity(mut self, affinity: CpuAffinity) -> Self {
        self.affinity = affinity;
        self
    }
}

/// Per-CPU run queue
struct RunQueue {
    tasks: RwLock<VecDeque<Task>>,
}

impl RunQueue {
    fn new() -> Self {
        Self {
            tasks: RwLock::new(VecDeque::new()),
        }
    }

    fn push(&self, task: Task) {
        let mut tasks = self.tasks.write();
        tasks.push_back(task);
    }

    fn pop(&self) -> Option<Task> {
        let mut tasks = self.tasks.write();
        tasks.pop_front()
    }

    fn len(&self) -> usize {
        self.tasks.read().len()
    }

    fn is_empty(&self) -> bool {
        self.tasks.read().is_empty()
    }
}

/// SMP-aware scheduler
pub struct SmpScheduler {
    num_cpus: usize,
    run_queues: Vec<Arc<RunQueue>>,
    global_queue: Arc<RunQueue>,
}

impl SmpScheduler {
    pub fn new(num_cpus: usize) -> Self {
        let run_queues: Vec<_> = (0..num_cpus)
            .map(|_| Arc::new(RunQueue::new()))
            .collect();

        Self {
            num_cpus,
            run_queues,
            global_queue: Arc::new(RunQueue::new()),
        }
    }

    /// Schedule a task on a specific CPU or let the scheduler decide
    pub fn schedule(&self, mut task: Task) -> Result<(), &'static str> {
        // Select the best CPU for this task
        let target_cpu = self.select_cpu(&task)?;

        task.current_cpu = Some(target_cpu);

        // Add to the run queue
        self.run_queues[target_cpu].push(task);

        Ok(())
    }

    /// Schedule a task on the global queue
    pub fn schedule_global(&self, task: Task) -> Result<(), &'static str> {
        self.global_queue.push(task);
        Ok(())
    }

    /// Get the next task for a CPU
    pub fn next_task(&self, cpu_id: usize) -> Option<Task> {
        if cpu_id >= self.num_cpus {
            return None;
        }

        // First try the CPU's local queue
        if let Some(task) = self.run_queues[cpu_id].pop() {
            return Some(task);
        }

        // Then try the global queue
        if let Some(task) = self.global_queue.pop() {
            // Check if this CPU is allowed
            if task.affinity.is_allowed(cpu_id) {
                return Some(task);
            } else {
                // Put it back in the global queue
                self.global_queue.push(task);
            }
        }

        // Finally, try work stealing from other CPUs
        self.steal_task(cpu_id)
    }

    /// Get the number of tasks waiting on a CPU
    pub fn queue_length(&self, cpu_id: usize) -> usize {
        if cpu_id >= self.num_cpus {
            return 0;
        }

        self.run_queues[cpu_id].len()
    }

    /// Get the total number of tasks in the system
    pub fn total_tasks(&self) -> usize {
        let local_tasks: usize = self.run_queues.iter().map(|q| q.len()).sum();
        let global_tasks = self.global_queue.len();
        local_tasks + global_tasks
    }

    /// Select the best CPU for a task based on affinity and load
    fn select_cpu(&self, task: &Task) -> Result<usize, &'static str> {
        // If there's a preferred CPU, try to use it
        if let Some(cpu) = task.affinity.preferred_cpu() {
            if cpu < self.num_cpus {
                return Ok(cpu);
            }
        }

        // Otherwise, find the least loaded CPU that's allowed
        let allowed_cpus = task.affinity.allowed_cpus().cpus();

        if allowed_cpus.is_empty() {
            return Err("No allowed CPUs");
        }

        // Find the CPU with the shortest queue
        let best_cpu = allowed_cpus
            .iter()
            .min_by_key(|&&cpu| self.run_queues[cpu].len())
            .copied()
            .unwrap();

        Ok(best_cpu)
    }

    /// Steal a task from another CPU's queue
    fn steal_task(&self, cpu_id: usize) -> Option<Task> {
        // Try to steal from the CPU with the longest queue
        let mut max_len = 0;
        let mut victim_cpu = None;

        for (i, queue) in self.run_queues.iter().enumerate() {
            if i != cpu_id {
                let len = queue.len();
                if len > max_len {
                    max_len = len;
                    victim_cpu = Some(i);
                }
            }
        }

        if let Some(victim) = victim_cpu {
            if let Some(task) = self.run_queues[victim].pop() {
                // Check if the current CPU is allowed
                if task.affinity.is_allowed(cpu_id) {
                    return Some(task);
                } else {
                    // Put it back
                    self.run_queues[victim].push(task);
                }
            }
        }

        None
    }

    /// Migrate a task from one CPU to another
    pub fn migrate_task(&self, task: Task, target_cpu: usize) -> Result<(), &'static str> {
        if target_cpu >= self.num_cpus {
            return Err("Invalid target CPU");
        }

        if !task.affinity.is_allowed(target_cpu) {
            return Err("Target CPU not allowed by affinity");
        }

        let mut migrated_task = task;
        migrated_task.current_cpu = Some(target_cpu);

        self.run_queues[target_cpu].push(migrated_task);

        Ok(())
    }

    /// Balance load across all CPUs
    pub fn balance_load(&self) {
        let avg_load = self.total_tasks() / self.num_cpus;

        for cpu_id in 0..self.num_cpus {
            let load = self.queue_length(cpu_id);

            // If this CPU is overloaded, try to migrate tasks
            if load > avg_load + 1 {
                while self.queue_length(cpu_id) > avg_load {
                    if let Some(task) = self.run_queues[cpu_id].pop() {
                        // Find the least loaded CPU that's allowed
                        if let Ok(target) = self.select_cpu(&task) {
                            if target != cpu_id {
                                let _ = self.migrate_task(task, target);
                            } else {
                                // Put it back
                                self.run_queues[cpu_id].push(task);
                                break;
                            }
                        } else {
                            // Put it back
                            self.run_queues[cpu_id].push(task);
                            break;
                        }
                    }
                }
            }
        }
    }
}
