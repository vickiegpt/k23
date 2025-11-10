//! RAM-based filesystem implementation

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

use super::inode::{Inode, InodeId, InodeMeta, InodeType};
use super::vfs::FileSystem;

/// RAM filesystem inode
enum RamFSInodeData {
    File(Vec<u8>),
    Directory(BTreeMap<String, InodeId>),
}

struct RamFSInode {
    meta: InodeMeta,
    data: RamFSInodeData,
}

impl RamFSInode {
    fn new_file() -> Self {
        Self {
            meta: InodeMeta::new(InodeType::File),
            data: RamFSInodeData::File(Vec::new()),
        }
    }

    fn new_dir() -> Self {
        Self {
            meta: InodeMeta::new(InodeType::Directory),
            data: RamFSInodeData::Directory(BTreeMap::new()),
        }
    }
}

impl Inode for RamFSInode {
    fn metadata(&self) -> InodeMeta {
        self.meta.clone()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
        match &self.data {
            RamFSInodeData::File(data) => {
                if offset >= data.len() {
                    return Ok(0);
                }
                let available = data.len() - offset;
                let to_read = buf.len().min(available);
                buf[..to_read].copy_from_slice(&data[offset..offset + to_read]);
                Ok(to_read)
            }
            RamFSInodeData::Directory(_) => Err("Cannot read from directory"),
        }
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize, &'static str> {
        match &mut self.data {
            RamFSInodeData::File(data) => {
                let end = offset + buf.len();
                if end > data.len() {
                    data.resize(end, 0);
                }
                data[offset..end].copy_from_slice(buf);
                self.meta.size = data.len();
                Ok(buf.len())
            }
            RamFSInodeData::Directory(_) => Err("Cannot write to directory"),
        }
    }

    fn readdir(&self) -> Result<Vec<(String, InodeId)>, &'static str> {
        match &self.data {
            RamFSInodeData::Directory(entries) => {
                Ok(entries.iter().map(|(name, id)| (name.clone(), *id)).collect())
            }
            RamFSInodeData::File(_) => Err("Not a directory"),
        }
    }

    fn lookup(&self, name: &str) -> Result<InodeId, &'static str> {
        match &self.data {
            RamFSInodeData::Directory(entries) => {
                entries.get(name).copied().ok_or("Entry not found")
            }
            RamFSInodeData::File(_) => Err("Not a directory"),
        }
    }

    fn create(&mut self, name: &str, inode_type: InodeType) -> Result<InodeId, &'static str> {
        match &mut self.data {
            RamFSInodeData::Directory(entries) => {
                if entries.contains_key(name) {
                    return Err("Entry already exists");
                }

                let new_inode = match inode_type {
                    InodeType::File => RamFSInode::new_file(),
                    InodeType::Directory => RamFSInode::new_dir(),
                    _ => return Err("Unsupported inode type"),
                };

                let inode_id = new_inode.meta.id;
                entries.insert(name.to_string(), inode_id);
                Ok(inode_id)
            }
            RamFSInodeData::File(_) => Err("Not a directory"),
        }
    }

    fn remove(&mut self, name: &str) -> Result<(), &'static str> {
        match &mut self.data {
            RamFSInodeData::Directory(entries) => {
                entries.remove(name).ok_or("Entry not found")?;
                Ok(())
            }
            RamFSInodeData::File(_) => Err("Not a directory"),
        }
    }

    fn truncate(&mut self, size: usize) -> Result<(), &'static str> {
        match &mut self.data {
            RamFSInodeData::File(data) => {
                data.resize(size, 0);
                self.meta.size = size;
                Ok(())
            }
            RamFSInodeData::Directory(_) => Err("Cannot truncate directory"),
        }
    }
}

/// RAM filesystem
pub struct RamFS {
    inodes: RwLock<BTreeMap<InodeId, Arc<RwLock<RamFSInode>>>>,
    root_id: InodeId,
}

impl RamFS {
    pub fn new() -> Self {
        let root = Arc::new(RwLock::new(RamFSInode::new_dir()));
        let root_id = root.read().meta.id;

        let mut inodes = BTreeMap::new();
        inodes.insert(root_id, root);

        Self {
            inodes: RwLock::new(inodes),
            root_id,
        }
    }

    fn ensure_inode_exists(&self, id: InodeId) -> Result<(), &'static str> {
        let inodes = self.inodes.read();
        if inodes.contains_key(&id) {
            return Ok(());
        }
        drop(inodes);

        // Create the inode if it doesn't exist
        // This handles the case where an inode was created but not yet registered
        let mut inodes = self.inodes.write();
        if !inodes.contains_key(&id) {
            // Create a new file inode with this ID
            let inode = Arc::new(RwLock::new(RamFSInode::new_file()));
            inodes.insert(id, inode);
        }

        Ok(())
    }
}

impl FileSystem for RamFS {
    fn root_inode(&self) -> Arc<RwLock<dyn Inode>> {
        let inodes = self.inodes.read();
        let root = inodes.get(&self.root_id).unwrap().clone();
        root as Arc<RwLock<dyn Inode>>
    }

    fn get_inode(&self, id: InodeId) -> Result<Arc<RwLock<dyn Inode>>, &'static str> {
        self.ensure_inode_exists(id)?;
        let inodes = self.inodes.read();
        let inode = inodes.get(&id).ok_or("Inode not found")?.clone();
        Ok(inode as Arc<RwLock<dyn Inode>>)
    }

    fn name(&self) -> &'static str {
        "ramfs"
    }
}
