//! File descriptor abstraction

use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use spin::RwLock;

use super::inode::{Inode, InodeId};

/// File descriptor identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileDescriptor(pub u32);

impl FileDescriptor {
    pub fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(3); // Start from 3 (after stdin/stdout/stderr)
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Standard input file descriptor
    pub const STDIN: Self = Self(0);
    /// Standard output file descriptor
    pub const STDOUT: Self = Self(1);
    /// Standard error file descriptor
    pub const STDERR: Self = Self(2);
}

/// File open flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
    pub append: bool,
}

impl OpenFlags {
    pub const fn new() -> Self {
        Self {
            read: false,
            write: false,
            create: false,
            truncate: false,
            append: false,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            create: false,
            truncate: false,
            append: false,
        }
    }

    pub const fn write_only() -> Self {
        Self {
            read: false,
            write: true,
            create: false,
            truncate: false,
            append: false,
        }
    }

    pub const fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            create: false,
            truncate: false,
            append: false,
        }
    }
}

/// Represents an open file
pub struct File {
    pub inode_id: InodeId,
    pub flags: OpenFlags,
    offset: AtomicUsize,
    inode: Arc<RwLock<dyn Inode>>,
}

impl File {
    pub fn new(inode_id: InodeId, flags: OpenFlags, inode: Arc<RwLock<dyn Inode>>) -> Self {
        Self {
            inode_id,
            flags,
            offset: AtomicUsize::new(0),
            inode,
        }
    }

    /// Get current file offset
    pub fn offset(&self) -> usize {
        self.offset.load(Ordering::SeqCst)
    }

    /// Set file offset
    pub fn seek(&self, offset: usize) -> Result<usize, &'static str> {
        self.offset.store(offset, Ordering::SeqCst);
        Ok(offset)
    }

    /// Read from file at current offset
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if !self.flags.read {
            return Err("File not open for reading");
        }

        let offset = self.offset.load(Ordering::SeqCst);
        let inode = self.inode.read();
        let bytes_read = inode.read(offset, buf)?;
        self.offset.fetch_add(bytes_read, Ordering::SeqCst);
        Ok(bytes_read)
    }

    /// Write to file at current offset
    pub fn write(&self, buf: &[u8]) -> Result<usize, &'static str> {
        if !self.flags.write {
            return Err("File not open for writing");
        }

        let offset = if self.flags.append {
            let inode = self.inode.read();
            inode.metadata().size
        } else {
            self.offset.load(Ordering::SeqCst)
        };

        let mut inode = self.inode.write();
        let bytes_written = inode.write(offset, buf)?;

        if !self.flags.append {
            self.offset.fetch_add(bytes_written, Ordering::SeqCst);
        }

        Ok(bytes_written)
    }
}
