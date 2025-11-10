//! Inter-Processor Communication (IPC) for SMP

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

/// IPC message types
#[derive(Debug, Clone)]
pub enum IPCMessage {
    /// Request to flush TLB
    TlbFlush,
    /// Request to invalidate cache
    CacheInvalidate,
    /// Request to schedule a task
    ScheduleTask { task_id: u64 },
    /// Request to wake up a CPU
    WakeUp,
    /// Custom message with user-defined data
    Custom { message_type: u32, data: u64 },
}

/// IPC channel for a single CPU
struct IPCQueue {
    messages: RwLock<VecDeque<IPCMessage>>,
}

impl IPCQueue {
    fn new() -> Self {
        Self {
            messages: RwLock::new(VecDeque::new()),
        }
    }

    fn send(&self, message: IPCMessage) -> Result<(), &'static str> {
        let mut queue = self.messages.write();
        queue.push_back(message);
        Ok(())
    }

    fn receive(&self) -> Option<IPCMessage> {
        let mut queue = self.messages.write();
        queue.pop_front()
    }

    fn peek(&self) -> Option<IPCMessage> {
        let queue = self.messages.read();
        queue.front().cloned()
    }

    fn len(&self) -> usize {
        self.messages.read().len()
    }

    fn is_empty(&self) -> bool {
        self.messages.read().is_empty()
    }
}

/// IPC channel wrapper
pub struct IPCChannel {
    cpu_id: usize,
    queue: Arc<IPCQueue>,
}

impl IPCChannel {
    fn new(cpu_id: usize, queue: Arc<IPCQueue>) -> Self {
        Self { cpu_id, queue }
    }

    /// Get the CPU ID this channel is for
    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    /// Send a message to this CPU
    pub fn send(&self, message: IPCMessage) -> Result<(), &'static str> {
        self.queue.send(message)
    }

    /// Receive a message (non-blocking)
    pub fn receive(&self) -> Option<IPCMessage> {
        self.queue.receive()
    }

    /// Peek at the next message without removing it
    pub fn peek(&self) -> Option<IPCMessage> {
        self.queue.peek()
    }

    /// Check if there are pending messages
    pub fn has_messages(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Get the number of pending messages
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }
}

/// IPC Manager - manages IPC channels for all CPUs
pub struct IPCManager {
    num_cpus: usize,
    queues: Vec<Arc<IPCQueue>>,
}

impl IPCManager {
    pub fn new(num_cpus: usize) -> Self {
        let queues: Vec<_> = (0..num_cpus)
            .map(|_| Arc::new(IPCQueue::new()))
            .collect();

        Self { num_cpus, queues }
    }

    /// Send a message to a specific CPU
    pub fn send(&self, target_cpu: usize, message: IPCMessage) -> Result<(), &'static str> {
        if target_cpu >= self.num_cpus {
            return Err("Invalid CPU ID");
        }

        self.queues[target_cpu].send(message)?;

        // In a real implementation, we would also trigger an IPI here
        // to wake up the target CPU if it's idle
        self.trigger_ipi(target_cpu);

        Ok(())
    }

    /// Get an IPC channel for a specific CPU
    pub fn channel(&self, cpu_id: usize) -> Result<IPCChannel, &'static str> {
        if cpu_id >= self.num_cpus {
            return Err("Invalid CPU ID");
        }

        Ok(IPCChannel::new(cpu_id, self.queues[cpu_id].clone()))
    }

    /// Receive a message for the current CPU
    pub fn receive(&self, cpu_id: usize) -> Result<Option<IPCMessage>, &'static str> {
        if cpu_id >= self.num_cpus {
            return Err("Invalid CPU ID");
        }

        Ok(self.queues[cpu_id].receive())
    }

    /// Check if a CPU has pending messages
    pub fn has_pending(&self, cpu_id: usize) -> bool {
        if cpu_id >= self.num_cpus {
            return false;
        }

        !self.queues[cpu_id].is_empty()
    }

    /// Broadcast a message to all CPUs except the sender
    pub fn broadcast(&self, sender_cpu: usize, message: IPCMessage) -> Result<(), &'static str> {
        for cpu in 0..self.num_cpus {
            if cpu != sender_cpu {
                self.send(cpu, message.clone())?;
            }
        }
        Ok(())
    }

    /// Trigger an inter-processor interrupt
    fn trigger_ipi(&self, _target_cpu: usize) {
        // Placeholder: In a real implementation, this would use
        // architecture-specific IPI mechanisms (e.g., APIC on x86,
        // or SBI on RISC-V) to interrupt the target CPU
    }
}

/// Process pending IPC messages for the current CPU
pub fn process_messages<F>(cpu_id: usize, handler: F) -> Result<usize, &'static str>
where
    F: Fn(IPCMessage),
{
    let manager = crate::smp::get_manager()
        .ok_or("SMP not initialized")?;

    let channel = manager.ipc().channel(cpu_id)?;
    let mut processed = 0;

    while let Some(message) = channel.receive() {
        handler(message);
        processed += 1;
    }

    Ok(processed)
}
