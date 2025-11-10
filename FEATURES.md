# k23 New Features

This document describes the newly added features to the k23 operating system kernel.

## 1. File System (FS) Support

### Overview
A complete Virtual File System (VFS) layer has been implemented, providing a unified interface for different file system implementations. The initial implementation includes an in-memory RAM-based file system (RamFS).

### Architecture

```
┌─────────────────────────────────────────┐
│          VFS Interface                   │
│  (mount, open, close, read, write, etc) │
└──────────────┬──────────────────────────┘
               │
               ├─────────┐
               │         │
         ┌─────▼─────┐  ┌▼──────────┐
         │  RamFS    │  │  Future   │
         │ (In-mem)  │  │  FS Types │
         └───────────┘  └───────────┘
```

### Components

#### VFS Core (`kernel/src/fs/vfs.rs`)
- Virtual file system layer
- Mount point management
- Path resolution
- File operations (open, close, read, write)
- Directory operations (mkdir, unlink, readdir)

#### Inode Abstraction (`kernel/src/fs/inode.rs`)
- Unified inode interface
- Metadata management (permissions, timestamps, size)
- Support for files, directories, symlinks, and devices

#### File Descriptors (`kernel/src/fs/file.rs`)
- File descriptor management
- Open flags (read, write, create, truncate, append)
- File offset tracking
- Read/write operations with proper flag checking

#### RamFS (`kernel/src/fs/ramfs.rs`)
- Complete in-memory file system
- Dynamic inode allocation
- Directory hierarchy support
- Zero disk I/O overhead

#### Syscalls (`kernel/src/fs/syscalls.rs`)
- WASM-compatible syscall interface
- `sys_open` - Open/create files
- `sys_close` - Close file descriptors
- `sys_read` - Read from files
- `sys_write` - Write to files
- `sys_mkdir` - Create directories
- `sys_unlink` - Remove files/directories

### Usage Example

```rust
use kernel::fs;

// Initialize VFS (called during kernel boot)
fs::init().expect("Failed to initialize VFS");

// Get VFS instance
let vfs = fs::get_vfs().expect("VFS not initialized");

// Create a directory
vfs.mkdir("/home").unwrap();

// Open a file (create if doesn't exist)
let flags = OpenFlags {
    read: true,
    write: true,
    create: true,
    truncate: false,
    append: false,
};
let fd = vfs.open("/home/test.txt", flags).unwrap();

// Write to the file
let data = b"Hello, k23 file system!";
vfs.write(fd, data).unwrap();

// Close the file
vfs.close(fd).unwrap();
```

---

## 2. CXL (Compute Express Link) Support

### Overview
CXL support provides hardware abstraction and memory management for CXL devices, enabling memory pooling, cache coherency, and hot-plug capabilities.

### Architecture

```
┌─────────────────────────────────────────┐
│        CXL Manager                       │
│  (Device Registration & Discovery)       │
└──────────────┬──────────────────────────┘
               │
        ┌──────┴────────┐
        │               │
  ┌─────▼─────┐   ┌────▼──────────┐
  │  Device   │   │    Memory      │
  │ Abstraction│  │   Manager      │
  └───────────┘   └────────────────┘
       │                  │
       │                  │
  ┌────▼─────┬──────┬────▼─────┐
  │  Type 1  │ Type 2│  Type 3  │
  │ Device   │Device │  Device  │
  └──────────┴───────┴──────────┘
```

### Components

#### CXL Manager (`kernel/src/cxl/mod.rs`)
- Device registration and discovery
- Global CXL manager instance
- Integration with memory manager

#### Device Abstraction (`kernel/src/cxl/device.rs`)
- CXL device types (Type 1, 2, 3)
- Device capabilities (cache coherency, memory pooling, hot-plug)
- Configuration space access
- Generic memory device implementation

#### Memory Manager (`kernel/src/cxl/memory.rs`)
- Memory region management
- Allocation policies (First-Fit, Best-Fit, NUMA-aware)
- Memory pooling support
- Overlap detection
- NUMA node awareness

#### Configuration (`kernel/src/cxl/config.rs`)
- CXL feature flags
- QoS settings
- Overcommit policies
- Device limits

### Device Types

- **Type 1**: Device-only memory (e.g., GPUs)
- **Type 2**: Device with host-managed memory (e.g., smart NICs)
- **Type 3**: Pure memory expansion devices

### Features

- **Cache Coherency**: Automatic cache coherence with host memory
- **Memory Pooling**: Share memory across multiple devices
- **Hot-Plug**: Dynamic device insertion/removal
- **NUMA Awareness**: Optimal memory allocation based on NUMA topology
- **QoS**: Quality of Service support for bandwidth guarantees

### Usage Example

```rust
use kernel::cxl::{self, CXLMemoryDevice, CXLDeviceType, CXLMemoryRegion};
use alloc::sync::Arc;

// Initialize CXL subsystem (called during kernel boot)
cxl::init().expect("Failed to initialize CXL");

// Get CXL manager
let manager = cxl::get_manager().expect("CXL not initialized");

// Create a CXL Type 3 memory device
let device = Arc::new(CXLMemoryDevice::new(
    0x1000,                          // Device ID
    "CXL-Memory-0".to_string(),      // Device name
    CXLDeviceType::Type3,            // Device type
    Some(0x1_0000_0000),             // Base address (4GB)
    0x4000_0000,                      // Size (1GB)
));

// Register the device
manager.register_device(device).unwrap();

// Register a memory region
let region = CXLMemoryRegion::new(
    0x1_0000_0000..0x1_4000_0000,    // Physical address range
    0x1000,                           // Device ID
);
manager.memory_manager().register_region(region).unwrap();

// Allocate CXL memory
let mem_range = manager.memory_manager()
    .allocate(0x10_0000, None)        // 1MB, any NUMA node
    .unwrap();

println!("Allocated CXL memory at: {:?}", mem_range);

// Free the memory when done
manager.memory_manager().free(mem_range.start).unwrap();
```

---

## 3. Enhanced SMP (Symmetric Multi-Processing) Support

### Overview
Enhanced SMP support provides sophisticated multi-processor coordination, including CPU affinity management, cross-CPU task scheduling, and inter-processor communication.

### Architecture

```
┌──────────────────────────────────────────┐
│          SMP Manager                      │
│  (Coordination & IPI Broadcasting)        │
└───────────┬──────────────────────────────┘
            │
    ┌───────┼───────┐
    │       │       │
┌───▼──┐ ┌──▼──┐ ┌─▼───┐
│Sched │ │ IPC │ │Affi │
│uler  │ │     │ │nity │
└──────┘ └─────┘ └─────┘
    │        │       │
┌───┴────────┴───────┴────┐
│  Per-CPU Run Queues      │
│  ┌────┐ ┌────┐ ┌────┐   │
│  │CPU0│ │CPU1│ │CPUn│   │
│  └────┘ └────┘ └────┘   │
└──────────────────────────┘
```

### Components

#### SMP Manager (`kernel/src/smp/mod.rs`)
- Global SMP coordination
- IPI (Inter-Processor Interrupt) management
- Scheduler and IPC integration
- Broadcast operations

#### CPU Affinity (`kernel/src/smp/affinity.rs`)
- CPU set management
- Affinity policies
- Preferred CPU tracking
- CPU selection algorithms

#### Inter-Processor Communication (`kernel/src/smp/ipc.rs`)
- Message passing between CPUs
- Message types (TLB flush, cache invalidate, task scheduling, wake-up)
- Per-CPU message queues
- Non-blocking message delivery
- Broadcast support

#### SMP Scheduler (`kernel/src/smp/scheduler.rs`)
- Multi-level scheduling (per-CPU and global queues)
- Priority-based scheduling
- CPU affinity-aware task placement
- Work stealing for load balancing
- Task migration support
- Automatic load balancing

### Features

#### CPU Affinity Management
- Define which CPUs a task can run on
- Set preferred CPU for tasks
- Support for single-CPU pinning
- Dynamic affinity updates

#### Cross-CPU Scheduling
- Schedule tasks on remote CPUs
- Affinity-aware CPU selection
- Load-balanced task distribution
- Priority support (Idle, Low, Normal, High, Realtime)

#### Inter-Processor Communication
- Send messages to specific CPUs
- Broadcast to all CPUs
- Standard message types for system operations
- Custom message support

#### Load Balancing
- Automatic work stealing
- Manual load balancing API
- Queue length monitoring
- Migration with affinity constraints

### Usage Examples

#### CPU Affinity

```rust
use kernel::smp::{self, CpuSet, CpuAffinity};

let manager = smp::get_manager().expect("SMP not initialized");

// Create a CPU set for CPUs 0 and 1
let mut cpu_set = CpuSet::new(4);
cpu_set.add(0).unwrap();
cpu_set.add(1).unwrap();

// Create affinity with preferred CPU
let mut affinity = CpuAffinity::custom(cpu_set);
affinity.set_preferred_cpu(0).unwrap();

println!("Affinity: {}", affinity);
println!("Can run on CPU 0: {}", affinity.is_allowed(0));
println!("Selected CPU: {:?}", affinity.select_cpu());
```

#### Task Scheduling

```rust
use kernel::smp::{self, TaskId, Priority};
use kernel::smp::scheduler::Task;

let manager = smp::get_manager().expect("SMP not initialized");
let scheduler = manager.scheduler();

// Create a task with specific priority and affinity
let task = Task::new(TaskId::new(1), 4)
    .with_priority(Priority::High)
    .with_affinity(CpuAffinity::any(4));

// Schedule the task (scheduler picks best CPU)
scheduler.schedule(task).unwrap();

// Get next task for current CPU
if let Some(task) = scheduler.next_task(0) {
    println!("Running task {:?} on CPU 0", task.id);
}

// Manual load balancing
scheduler.balance_load();
```

#### Inter-Processor Communication

```rust
use kernel::smp::{self, IPCMessage};

let manager = smp::get_manager().expect("SMP not initialized");

// Send a message to CPU 1
manager.send_ipi(1, IPCMessage::WakeUp).unwrap();

// Broadcast TLB flush to all other CPUs
let current_cpu = 0;
manager.broadcast_ipi(current_cpu, IPCMessage::TlbFlush).unwrap();

// Process messages on current CPU
use kernel::smp::ipc::process_messages;

process_messages(0, |msg| {
    match msg {
        IPCMessage::TlbFlush => {
            println!("Flushing TLB");
            // Perform TLB flush
        }
        IPCMessage::ScheduleTask { task_id } => {
            println!("Scheduling task {}", task_id);
            // Schedule the task
        }
        _ => {}
    }
}).unwrap();
```

#### Work Stealing

```rust
use kernel::smp;

let manager = smp::get_manager().expect("SMP not initialized");
let scheduler = manager.scheduler();

// Check queue lengths
for cpu in 0..4 {
    let len = scheduler.queue_length(cpu);
    println!("CPU {} queue length: {}", cpu, len);
}

// Get task for CPU (will steal from others if local queue is empty)
if let Some(task) = scheduler.next_task(2) {
    println!("CPU 2 got task {:?} (possibly stolen)", task.id);
}
```

---

## Kernel Integration

All three features are initialized during kernel boot in `kernel/src/main.rs`:

```rust
// Initialize the file system
if let Err(e) = fs::init() {
    tracing::error!("Failed to initialize VFS: {}", e);
} else {
    tracing::info!("VFS initialized successfully");
}

// Initialize CXL support
if let Err(e) = cxl::init() {
    tracing::error!("Failed to initialize CXL: {}", e);
} else {
    tracing::info!("CXL subsystem initialized successfully");
}

// Initialize SMP support
let num_cpus = boot_info.cpu_mask.count_ones() as usize;
if let Err(e) = smp::init(num_cpus) {
    tracing::error!("Failed to initialize SMP: {}", e);
} else {
    tracing::info!("SMP subsystem initialized for {} CPUs", num_cpus);
}
```

## Future Enhancements

### File System
- [ ] Add persistent storage backends (ext4, FAT32)
- [ ] Implement file permissions and ownership
- [ ] Add symbolic links support
- [ ] Implement memory-mapped files
- [ ] Add file locking mechanisms

### CXL
- [ ] PCI enumeration for CXL device discovery
- [ ] CXL.cache and CXL.mem protocol implementation
- [ ] Switch fabric support for multi-device topologies
- [ ] Performance monitoring and telemetry
- [ ] Error handling and recovery

### SMP
- [ ] Real IPI triggering via hardware (APIC, SBI)
- [ ] CPU hotplug support
- [ ] NUMA-aware memory allocation integration
- [ ] CPU frequency scaling coordination
- [ ] Power management (C-states, P-states)
- [ ] Lock-free data structures for better scalability

## Testing

To test these features, you can build and run the kernel:

```bash
# Build the kernel
cargo build --package kernel

# Run in QEMU (requires appropriate setup)
cargo run --package kernel

# Run tests
cargo test --package kernel
```

## Contributing

When contributing to these features, please ensure:

1. Code follows the existing style and conventions
2. All unsafe code is properly documented
3. Tests are added for new functionality
4. Documentation is updated
5. Commits are well-formatted with clear messages

## License

This code is dual-licensed under Apache 2.0 and MIT licenses, matching the rest of the k23 project.
