//! Inode abstraction for the VFS

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// Unique inode identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InodeId(u64);

impl InodeId {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Inode type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    File,
    Directory,
    Symlink,
    Device,
}

/// Inode metadata
#[derive(Debug, Clone)]
pub struct InodeMeta {
    pub id: InodeId,
    pub inode_type: InodeType,
    pub size: usize,
    pub permissions: u32,
    pub uid: u32,
    pub gid: u32,
    pub created_at: u64,
    pub modified_at: u64,
    pub accessed_at: u64,
}

impl InodeMeta {
    pub fn new(inode_type: InodeType) -> Self {
        let now = 0; // TODO: get current timestamp
        Self {
            id: InodeId::new(),
            inode_type,
            size: 0,
            permissions: 0o644,
            uid: 0,
            gid: 0,
            created_at: now,
            modified_at: now,
            accessed_at: now,
        }
    }
}

/// Inode trait that all filesystem implementations must provide
pub trait Inode: Send + Sync {
    /// Get inode metadata
    fn metadata(&self) -> InodeMeta;

    /// Read data from the inode
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize, &'static str>;

    /// Write data to the inode
    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize, &'static str>;

    /// List directory entries (only for directories)
    fn readdir(&self) -> Result<Vec<(String, InodeId)>, &'static str>;

    /// Lookup a child inode by name (only for directories)
    fn lookup(&self, name: &str) -> Result<InodeId, &'static str>;

    /// Create a new child inode (only for directories)
    fn create(&mut self, name: &str, inode_type: InodeType) -> Result<InodeId, &'static str>;

    /// Remove a child inode (only for directories)
    fn remove(&mut self, name: &str) -> Result<(), &'static str>;

    /// Truncate file to specified size
    fn truncate(&mut self, size: usize) -> Result<(), &'static str>;
}
