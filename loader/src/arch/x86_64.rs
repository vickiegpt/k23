// Copyright 2025 Jonas Kruckenberg
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::arch::{asm, global_asm, naked_asm};
use core::fmt;
use core::num::NonZero;
use core::ptr::NonNull;

use bitflags::bitflags;

use crate::GlobalInitResult;
use crate::frame_alloc::FrameAllocator;
use crate::machine_info::MachineInfo;
use crate::mapping::Flags;

// PVH ELF Note to enable direct kernel loading like RISC-V
// This allows QEMU to boot our kernel directly without a traditional bootloader
global_asm!(
    r#"
    .pushsection .note.Xen, "a", @note
    .align 4
    .long 4                    /* name size */
    .long 4                    /* desc size */
    .long 0x12                 /* type = XEN_ELFNOTE_PHYS32_ENTRY */
    .asciz "Xen"              /* name */
    .long 0x100000             /* desc = entry point at 1MB physical */
    .popsection
    "#
);

pub const DEFAULT_ASID: u16 = 0;
pub const KERNEL_ASPACE_BASE: usize = 0xffffffc000000000;
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_TABLE_ENTRIES: usize = 512;
pub const PAGE_TABLE_LEVELS: usize = 4; // PML4, PDPT, PD, PT
pub const VIRT_ADDR_BITS: u32 = 48;

pub const PAGE_SHIFT: usize = (PAGE_SIZE - 1).count_ones() as usize;
pub const PAGE_ENTRY_SHIFT: usize = (PAGE_TABLE_ENTRIES - 1).count_ones() as usize;

/// Entry point for the boot CPU
/// PVH boot protocol starts us in 32-bit protected mode with:
/// - ebx = boot params address (we ignore for simplicity)
/// - All other registers undefined
/// We need to transition to 64-bit long mode before running 64-bit code
#[unsafe(link_section = ".text.start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        naked_asm! {
            // We start in 32-bit protected mode, need to transition to 64-bit
            ".code32",

            // Disable interrupts
            "cli",

            // Enable PAE (required for long mode)
            "mov eax, cr4",
            "or eax, 0x20",      // Set PAE bit
            "mov cr4, eax",

            // Set up initial page tables for identity mapping
            // We'll create a minimal identity map for the first 256MB
            // PML4 at 0x1000, PDPT at 0x2000, PD at 0x3000

            // Clear page table area
            "mov edi, 0x1000",
            "xor eax, eax",
            "mov ecx, 0x1000",   // Clear 16KB (4096 dwords = 16KB)
            "rep stosd",

            // Set up PML4[0] -> PDPT (for identity mapping low addresses)
            "mov dword ptr [0x1000], 0x2003",  // PDPT address | Present | Writable

            // Set up PDPT[0] -> PD (for identity mapping)
            "mov dword ptr [0x2000], 0x3003",  // PD address | Present | Writable

            // Set up PD entries for first 256MB (128 * 2MB pages)
            // Using 2MB pages (bit 7 = PS)
            "mov edi, 0x3000",
            "mov eax, 0x83",     // Present | Writable | PS (2MB pages)
            "mov ecx, 128",      // 128 entries for 256MB
            "2:",
            "mov [edi], eax",
            "add eax, 0x200000", // Next 2MB
            "add edi, 8",
            "loop 2b",

            // Load PML4 into CR3
            "mov eax, 0x1000",
            "mov cr3, eax",

            // Enable long mode in EFER MSR
            // WRMSR uses: ECX = MSR number, EDX:EAX = value
            "mov ecx, 0xC0000080",  // EFER MSR number
            "mov eax, 0x900",       // LME (bit 8) + NXE (bit 11)
            "xor edx, edx",         // Upper 32 bits = 0
            "wrmsr",

            // Enable paging and protected mode
            "mov eax, cr0",
            "or eax, 0x80000001",   // Set PG and PE bits
            "mov cr0, eax",

            // Load a temporary GDT with 64-bit code segment
            "lgdt [5f]",         // Load GDT descriptor at label 5

            // Far jump to 64-bit code
            // Use manual encoding for far jump
            ".byte 0xEA",        // Far jump opcode
            ".long 6f",          // Offset to label 6
            ".word 0x08",        // Code segment selector

            // GDT descriptor (label 5)
            ".align 4",
            "5:",
            ".word 23",          // GDT limit (3 entries * 8 - 1)
            ".long 4f",          // GDT base address (label 4)

            // Temporary GDT (label 4)
            ".align 16",
            "4:",
            ".quad 0",                          // Null descriptor
            ".quad 0x00af9a000000ffff",        // 64-bit code segment
            ".quad 0x00cf92000000ffff",        // Data segment

            // Now in 64-bit mode (label 6)
            ".code64",
            "6:",

            // Set up data segments for 64-bit mode
            "mov ax, 0x10",      // Data segment selector
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",

            // Set up a temporary stack at a known good location
            // Use 2MB as temporary stack location (well above our code at 1MB)
            "mov rsp, 0x200000",

            // Clear direction flag for string operations
            "cld",

            // Clear BSS section (uninitialized globals)
            "lea rdi, [rip + __bss_zero_start]",
            "lea rcx, [rip + __bss_end]",
            "sub rcx, rdi",      // rcx = size of BSS
            "jz 3f",             // Skip if BSS is empty
            "xor eax, eax",      // Zero to write
            "rep stosb",         // Clear BSS byte by byte
            "3:",


            // Set up arguments for main()
            "xor rdi, rdi",      // CPU ID = 0
            "xor rsi, rsi",      // No FDT on x86
            "xor rdx, rdx",      // boot_ticks = 0

            // Call Rust main
            "call {main}",

            // Should never return
            "2:",
            "hlt",
            "jmp 2b",

            main = sym crate::main,
        }
    }
}

/// Entry point for secondary CPUs (not implemented yet)
#[unsafe(naked)]
unsafe extern "C" fn _start_secondary() -> ! {
    unsafe {
        naked_asm! {
            // For now, just halt secondary CPUs
            "cli",
            "2:",
            "hlt",
            "jmp 2b"
        }
    }
}

/// Fill stack with canary pattern
/// rdi = bottom of stack, rsi = top of stack
#[unsafe(naked)]
unsafe extern "C" fn fill_stack() {
    unsafe {
        naked_asm! {
            "mov rax, 0xACE0BACE",
            "2:",
            "mov [rdi], rax",
            "add rdi, 8",
            "cmp rdi, rsi",
            "jb 2b",
            "ret"
        }
    }
}

/// Hand off control to the kernel
pub unsafe fn handoff_to_kernel(cpuid: usize, boot_ticks: u64, init: &GlobalInitResult) -> ! {
    let stack = init.stacks_alloc.region_for_cpu(cpuid);
    let tls = init
        .maybe_tls_alloc
        .as_ref()
        .map(|tls| tls.region_for_hart(cpuid))
        .unwrap_or_default();

    log::debug!("CPU {cpuid} Jumping to kernel...");
    log::trace!(
        "CPU {cpuid} entry: {:#x}, arguments: rdi={cpuid} rsi={:?} stack={stack:#x?} tls={tls:#x?}",
        init.kernel_entry,
        init.boot_info
    );

    init.barrier.wait();

    unsafe {
        asm! {
            // Set up stack first
            "mov rsp, {stack_top}",

            // Save kernel arguments on the new stack (they might be in registers we'll clobber)
            "push {cpuid}",
            "push {boot_info}",
            "push {boot_ticks}",

            // Set up TLS (FS base)
            "mov rcx, 0xc0000100",  // FS_BASE MSR
            "mov rax, {tls_start}",
            "mov rdx, {tls_start}",
            "shr rdx, 32",
            "wrmsr",

            // Fill stack with canary
            "mov rdi, {stack_bottom}",
            "mov rsi, {stack_top}",
            "sub rsi, 24",  // Account for the 3 values we pushed
            "call {fill_stack}",

            // Restore and set up kernel arguments (System V ABI)
            "pop rdx",  // boot_ticks
            "pop rsi",  // boot_info
            "pop rdi",  // cpuid

            // Clear return address
            "xor rax, rax",

            // Jump to kernel
            "jmp {kernel_entry}",

            cpuid = in(reg) cpuid,
            boot_info = in(reg) init.boot_info as usize,
            boot_ticks = in(reg) boot_ticks,
            stack_bottom = in(reg) stack.start,
            stack_top = in(reg) stack.end,
            tls_start = in(reg) tls.start,
            kernel_entry = in(reg) init.kernel_entry,
            fill_stack = sym fill_stack,
            options(noreturn)
        }
    }
}

/// APIC registers (memory-mapped)
const APIC_BASE: usize = 0xFEE00000;
const APIC_ID: usize = 0x020;
const APIC_ICR_LOW: usize = 0x300;
const APIC_ICR_HIGH: usize = 0x310;

/// IPI delivery modes
const APIC_DELMODE_INIT: u32 = 0x500;
const APIC_DELMODE_STARTUP: u32 = 0x600;
const APIC_LEVEL_ASSERT: u32 = 0x4000;

/// Secondary CPU entry point address (must be below 1MB)
const AP_STARTUP_ADDR: usize = 0x8000;

/// Read from APIC register
unsafe fn apic_read(reg: usize) -> u32 {
    let addr = (APIC_BASE + reg) as *const u32;
    unsafe { core::ptr::read_volatile(addr) }
}

/// Write to APIC register
unsafe fn apic_write(reg: usize, value: u32) {
    let addr = (APIC_BASE + reg) as *mut u32;
    unsafe { core::ptr::write_volatile(addr, value) }
}

/// Detect number of CPUs via CPUID
fn detect_cpu_count() -> usize {
    unsafe {
        // CPUID leaf 0x1, EBX[23:16] contains max APIC IDs
        // Note: We can't use ebx directly as it's reserved by LLVM
        // We need to save/restore it manually
        let ebx: u32;

        core::arch::asm!(
            // Save rbx (LLVM uses it)
            "push rbx",
            // Call cpuid with eax=1
            "mov eax, 1",
            "cpuid",
            // Save ebx to a temporary (r10 is caller-saved)
            "mov r10d, ebx",
            // Restore rbx
            "pop rbx",
            // Move result from r10 to output
            "mov {0:e}, r10d",
            out(reg) ebx,
            out("eax") _,
            out("ecx") _,
            out("edx") _,
            out("r10") _,
        );

        let max_logical = ((ebx >> 16) & 0xFF) as usize;

        // Cap at 32 CPUs for safety
        if max_logical > 1 && max_logical <= 32 {
            log::info!("Detected {} logical CPUs via CPUID", max_logical);
            max_logical
        } else {
            1
        }
    }
}

/// Send INIT IPI to a specific CPU
unsafe fn send_init_ipi(apic_id: u32) {
    // Write target APIC ID to ICR high
    unsafe {
        apic_write(APIC_ICR_HIGH, apic_id << 24);
        // Send INIT IPI
        apic_write(APIC_ICR_LOW, APIC_DELMODE_INIT | APIC_LEVEL_ASSERT);
    }

    // Wait for delivery
    busy_wait_ms(10);
}

/// Send STARTUP IPI to a specific CPU
unsafe fn send_startup_ipi(apic_id: u32, start_vector: u8) {
    // Write target APIC ID to ICR high
    unsafe {
        apic_write(APIC_ICR_HIGH, apic_id << 24);
        // Send STARTUP IPI with start vector
        apic_write(APIC_ICR_LOW, APIC_DELMODE_STARTUP | (start_vector as u32));
    }

    // Wait for delivery
    busy_wait_ms(1);
}

/// Busy wait for approximately N milliseconds
fn busy_wait_ms(ms: u32) {
    // Rough busy loop - not precise but good enough for initialization
    for _ in 0..(ms * 1000) {
        unsafe {
            core::arch::asm!("pause", options(nomem, nostack));
        }
    }
}

/// Copy AP startup code to low memory
unsafe fn setup_ap_trampoline() -> crate::Result<()> {
    // Copy the AP startup code from _start_secondary to AP_STARTUP_ADDR
    let trampoline_code = AP_TRAMPOLINE_CODE;
    let dest = AP_STARTUP_ADDR as *mut u8;

    log::debug!("Copying AP trampoline code to {:#x} ({} bytes)", AP_STARTUP_ADDR, trampoline_code.len());

    unsafe {
        core::ptr::copy_nonoverlapping(
            trampoline_code.as_ptr(),
            dest,
            trampoline_code.len(),
        );
    }

    Ok(())
}

/// AP trampoline code (real mode entry point)
/// This code is position-independent and runs in real mode at AP_STARTUP_ADDR
/// It transitions the CPU from real mode -> protected mode -> long mode
static AP_TRAMPOLINE_CODE: &[u8] = &[
    // Real mode 16-bit code starts here (address 0x8000)
    0xFA,                           // cli - disable interrupts
    0x66, 0xB8, 0x10, 0x00,         // mov ax, 0x10 - data segment
    0x8E, 0xD8,                     // mov ds, ax
    0x8E, 0xC0,                     // mov es, ax
    0x8E, 0xD0,                     // mov ss, ax

    // Enable A20 line (fast method via port 0x92)
    0xE4, 0x92,                     // in al, 0x92
    0x0C, 0x02,                     // or al, 2
    0xE6, 0x92,                     // out 0x92, al

    // Enable PAE (required for long mode)
    0x0F, 0x20, 0xE0,               // mov eax, cr4
    0x0C, 0x20,                     // or al, 0x20 (PAE bit)
    0x0F, 0x22, 0xE0,               // mov cr4, eax

    // Load CR3 with PML4 address (reuse boot CPU's page tables at 0x1000)
    0xB8, 0x00, 0x10, 0x00, 0x00,   // mov eax, 0x1000
    0x0F, 0x22, 0xD8,               // mov cr3, eax

    // Enable long mode in EFER
    0xB9, 0x80, 0x00, 0x00, 0xC0,   // mov ecx, 0xC0000080 (EFER MSR)
    0x0F, 0x32,                     // rdmsr
    0x0C, 0x01,                     // or al, 1 (LME bit)
    0x0F, 0x30,                     // wrmsr

    // Enable paging and protected mode
    0x0F, 0x20, 0xC0,               // mov eax, cr0
    0x0D, 0x01, 0x00, 0x00, 0x80,   // or eax, 0x80000001 (PG + PE)
    0x0F, 0x22, 0xC0,               // mov cr0, eax

    // Now in 64-bit mode, jump to kernel
    // Load GDT and far jump would go here, but for simplicity
    // we'll just halt for now until proper 64-bit entry is implemented
    0xF4,                           // hlt
    0xEB, 0xFD,                     // jmp $ (infinite loop)
];

/// Start secondary CPUs
pub fn start_secondary_harts(boot_cpu: usize, minfo: &MachineInfo) -> crate::Result<()> {
    let cpu_count = detect_cpu_count();

    if cpu_count <= 1 {
        log::info!("Single CPU system detected, skipping SMP initialization");
        return Ok(());
    }

    log::info!("Starting {} secondary CPUs (boot CPU: {})", cpu_count - 1, boot_cpu);

    // Setup AP trampoline code in low memory
    unsafe {
        setup_ap_trampoline()?;
    }

    // Start each AP (Application Processor)
    for cpu_id in 0..cpu_count {
        if cpu_id == boot_cpu {
            continue; // Skip boot CPU
        }

        log::debug!("Starting CPU {}", cpu_id);

        unsafe {
            // Send INIT IPI
            send_init_ipi(cpu_id as u32);

            // Send STARTUP IPI (twice as per Intel MP spec)
            let start_vector = (AP_STARTUP_ADDR >> 12) as u8;
            send_startup_ipi(cpu_id as u32, start_vector);
            busy_wait_ms(1);
            send_startup_ipi(cpu_id as u32, start_vector);
        }

        // TODO: Wait for AP to signal readiness
        log::info!("CPU {} startup sequence sent", cpu_id);
    }

    log::info!("SMP initialization complete");
    Ok(())
}

pub unsafe fn map_contiguous(
    root_pgtable: usize,
    frame_alloc: &mut FrameAllocator,
    mut virt: usize,
    mut phys: usize,
    len: NonZero<usize>,
    flags: Flags,
    phys_off: usize,
) -> crate::Result<()> {
    let mut remaining_bytes = len.get();

    // Round up to page size if less than a page
    if remaining_bytes < PAGE_SIZE {
        remaining_bytes = PAGE_SIZE;
    }

    debug_assert!(
        virt % PAGE_SIZE == 0,
        "virtual address must be page-aligned: {:#x}",
        virt
    );
    debug_assert!(
        phys % PAGE_SIZE == 0,
        "physical address must be page-aligned: {:#x}",
        phys
    );

    'outer: while remaining_bytes > 0 {
        let mut pgtable: NonNull<PageTableEntry> = pgtable_ptr_from_phys(root_pgtable, phys_off);

        for lvl in (0..PAGE_TABLE_LEVELS).rev() {
            let index = pte_index_for_level(virt, lvl);
            let pte = unsafe { pgtable.add(index).as_mut() };

            if !pte.is_valid() {
                // For partial pages at the end, we still need to map a full page
                let effective_remaining = if remaining_bytes < PAGE_SIZE && lvl == 0 {
                    PAGE_SIZE
                } else {
                    remaining_bytes
                };

                if can_map_at_level(virt, phys, effective_remaining, lvl) {
                    let page_size = page_size_for_level(lvl);
                    let mut pte_flags = PTEFlags::VALID | PTEFlags::from(flags);

                    // For large pages (2MB at level 1, 1GB at level 2), set the PS bit
                    if lvl > 0 {
                        pte_flags |= PTEFlags::HUGE;
                    }

                    pte.replace_address_and_flags(phys, pte_flags);

                    virt = virt.checked_add(page_size).unwrap();
                    phys = phys.checked_add(page_size).unwrap();
                    remaining_bytes = remaining_bytes.saturating_sub(page_size);
                    continue 'outer;
                } else {
                    let frame = frame_alloc.allocate_one_zeroed(phys_off)?;
                    pte.replace_address_and_flags(frame, PTEFlags::VALID);
                    pgtable = pgtable_ptr_from_phys(frame, phys_off);
                }
            } else if pte.is_leaf(lvl) {
                unreachable!("Invalid state: {virt:#x} is already mapped");
            } else {
                // PTE is valid but not a leaf, follow to next level
                let next_table_phys = pte.get_address_and_flags().0;
                pgtable = pgtable_ptr_from_phys(next_table_phys, phys_off);
            }
        }
    }

    Ok(())
}

pub unsafe fn remap_contiguous(
    root_pgtable: usize,
    mut virt: usize,
    mut phys: usize,
    len: NonZero<usize>,
    phys_off: usize,
) {
    let mut remaining_bytes = len.get();
    debug_assert!(remaining_bytes >= PAGE_SIZE);
    debug_assert!(virt % PAGE_SIZE == 0);
    debug_assert!(phys % PAGE_SIZE == 0);

    'outer: while remaining_bytes > 0 {
        let mut pgtable = pgtable_ptr_from_phys(root_pgtable, phys_off);

        for lvl in (0..PAGE_TABLE_LEVELS).rev() {
            let index = pte_index_for_level(virt, lvl);
            let pte = unsafe { pgtable.add(index).as_mut() };

            if pte.is_valid() && pte.is_leaf(lvl) {
                let page_size = page_size_for_level(lvl);
                debug_assert!(can_map_at_level(virt, phys, remaining_bytes, lvl));

                let (_old_phys, flags) = pte.get_address_and_flags();
                pte.replace_address_and_flags(phys, flags);

                virt = virt.checked_add(page_size).unwrap();
                phys = phys.checked_add(page_size).unwrap();
                remaining_bytes -= page_size;
                continue 'outer;
            } else if pte.is_valid() {
                pgtable = pgtable_ptr_from_phys(pte.get_address_and_flags().0, phys_off);
            } else {
                unreachable!("Invalid state");
            }
        }
    }
}

pub unsafe fn activate_aspace(pgtable: usize) {
    unsafe {
        asm!("mov cr3, {}", in(reg) pgtable);
    }
}

pub fn page_size_for_level(level: usize) -> usize {
    assert!(level < PAGE_TABLE_LEVELS);
    match level {
        0 => 1 << 12, // 4KB pages (PT level)
        1 => 1 << 21, // 2MB pages (PD level)
        2 => 1 << 30, // 1GB pages (PDPT level)
        3 => 1 << 39, // 512GB PML4 level
        _ => unreachable!(),
    }
}

pub fn pte_index_for_level(virt: usize, lvl: usize) -> usize {
    assert!(lvl < PAGE_TABLE_LEVELS);
    let index = (virt >> (PAGE_SHIFT + lvl * PAGE_ENTRY_SHIFT)) & (PAGE_TABLE_ENTRIES - 1);
    debug_assert!(index < PAGE_TABLE_ENTRIES);
    index
}

pub fn can_map_at_level(virt: usize, phys: usize, remaining_bytes: usize, lvl: usize) -> bool {
    // Don't allow leaf pages at PML4 level
    if lvl >= 3 {
        return false;
    }
    let page_size = page_size_for_level(lvl);
    virt % page_size == 0 && phys % page_size == 0 && remaining_bytes >= page_size
}

fn pgtable_ptr_from_phys(phys: usize, phys_off: usize) -> NonNull<PageTableEntry> {
    NonNull::new(phys_off.checked_add(phys).unwrap() as *mut PageTableEntry).unwrap()
}

#[repr(transparent)]
pub struct PageTableEntry {
    bits: usize,
}

impl PageTableEntry {
    pub fn is_valid(&self) -> bool {
        PTEFlags::from_bits_retain(self.bits).contains(PTEFlags::VALID)
    }

    pub fn is_leaf(&self, level: usize) -> bool {
        // At level 0 (PT), all valid entries are leaves (4KB pages)
        // At levels 1-2 (PD, PDPT), entries are leaves if PS bit is set
        // At level 3 (PML4), entries are never leaves
        match level {
            0 => self.is_valid(), // PT entries are always leaves if valid
            1 | 2 => {
                self.is_valid() && PTEFlags::from_bits_retain(self.bits).contains(PTEFlags::HUGE)
            }
            _ => false, // PML4 entries are never leaves
        }
    }

    pub fn replace_address_and_flags(&mut self, address: usize, flags: PTEFlags) {
        self.bits = 0;
        self.bits |= (address & !0xFFF) | flags.bits();
    }

    pub fn get_address_and_flags(&self) -> (usize, PTEFlags) {
        let addr = self.bits & !0xFFF;
        let flags = PTEFlags::from_bits_truncate(self.bits);
        (addr, flags)
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (addr, flags) = self.get_address_and_flags();
        f.debug_struct("PageTableEntry")
            .field("addr", &format_args!("{addr:#x}"))
            .field("flags", &flags)
            .finish()
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
    pub struct PTEFlags: usize {
        const VALID = 1 << 0;      // Present bit
        const WRITABLE = 1 << 1;   // Write permission
        const USER = 1 << 2;       // User accessible
        const PWT = 1 << 3;        // Write through
        const PCD = 1 << 4;        // Cache disable
        const ACCESSED = 1 << 5;   // Accessed bit
        const DIRTY = 1 << 6;      // Dirty bit
        const HUGE = 1 << 7;       // Page size bit
        const GLOBAL = 1 << 8;     // Global page
        const NX = 1 << 63;        // No execute
    }
}

impl From<Flags> for PTEFlags {
    fn from(flags: Flags) -> Self {
        let mut out = Self::VALID | Self::DIRTY | Self::ACCESSED;

        // x86_64 is always readable if present
        if flags.contains(Flags::WRITE) {
            out.insert(Self::WRITABLE);
        }

        // Note: x86_64 uses NX bit (inverse of execute)
        if !flags.contains(Flags::EXECUTE) {
            out.insert(Self::NX);
        }

        out
    }
}

impl From<PTEFlags> for Flags {
    fn from(arch_flags: PTEFlags) -> Self {
        let mut out = Flags::empty();

        // x86_64 pages are always readable if valid
        if arch_flags.contains(PTEFlags::VALID) {
            out.insert(Self::READ);
        }

        if arch_flags.contains(PTEFlags::WRITABLE) {
            out.insert(Self::WRITE);
        }

        // Note the inversion
        if !arch_flags.contains(PTEFlags::NX) {
            out.insert(Self::EXECUTE);
        }

        out
    }
}
