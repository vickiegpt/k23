//! Virtual File System (VFS) implementation for k23
//!
//! This module provides a virtual file system layer that abstracts
//! different file system implementations.

pub mod vfs;
pub mod ramfs;
pub mod file;
pub mod inode;
pub mod syscalls;

pub use vfs::{VFS, FileSystem};
pub use file::{File, FileDescriptor, OpenFlags};
pub use inode::{Inode, InodeType};

use alloc::sync::Arc;
use spin::RwLock;

/// Global VFS instance
static VFS_INSTANCE: RwLock<Option<Arc<VFS>>> = RwLock::new(None);

/// Initialize the VFS with the root filesystem
pub fn init() -> Result<(), &'static str> {
    let mut vfs = VFS_INSTANCE.write();
    if vfs.is_some() {
        return Err("VFS already initialized");
    }

    let vfs_instance = Arc::new(VFS::new());

    // Mount root ramfs
    let root_fs = Arc::new(ramfs::RamFS::new());
    vfs_instance.mount("/", root_fs)?;

    *vfs = Some(vfs_instance);
    Ok(())
}

/// Get the global VFS instance
pub fn get_vfs() -> Option<Arc<VFS>> {
    VFS_INSTANCE.read().clone()
}
