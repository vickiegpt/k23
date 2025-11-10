//! Virtual File System core implementation

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

use super::file::{File, FileDescriptor, OpenFlags};
use super::inode::{Inode, InodeId, InodeType};

/// Filesystem trait that all filesystems must implement
pub trait FileSystem: Send + Sync {
    /// Get the root inode of this filesystem
    fn root_inode(&self) -> Arc<RwLock<dyn Inode>>;

    /// Get an inode by ID
    fn get_inode(&self, id: InodeId) -> Result<Arc<RwLock<dyn Inode>>, &'static str>;

    /// Get filesystem name
    fn name(&self) -> &'static str;
}

/// Mount point information
struct MountPoint {
    path: String,
    filesystem: Arc<dyn FileSystem>,
}

/// Virtual File System
pub struct VFS {
    mount_points: RwLock<Vec<MountPoint>>,
    open_files: RwLock<BTreeMap<FileDescriptor, Arc<File>>>,
}

impl VFS {
    pub fn new() -> Self {
        Self {
            mount_points: RwLock::new(Vec::new()),
            open_files: RwLock::new(BTreeMap::new()),
        }
    }

    /// Mount a filesystem at the given path
    pub fn mount(&self, path: &str, filesystem: Arc<dyn FileSystem>) -> Result<(), &'static str> {
        let mut mounts = self.mount_points.write();

        // Check if already mounted
        if mounts.iter().any(|m| m.path == path) {
            return Err("Path already mounted");
        }

        mounts.push(MountPoint {
            path: path.to_string(),
            filesystem,
        });

        // Sort by path length (longest first) for proper lookup
        mounts.sort_by(|a, b| b.path.len().cmp(&a.path.len()));

        Ok(())
    }

    /// Unmount a filesystem at the given path
    pub fn unmount(&self, path: &str) -> Result<(), &'static str> {
        let mut mounts = self.mount_points.write();
        let pos = mounts.iter().position(|m| m.path == path)
            .ok_or("Path not mounted")?;
        mounts.remove(pos);
        Ok(())
    }

    /// Find the filesystem for a given path and return owned relative path
    fn find_filesystem(&self, path: &str) -> Result<(Arc<dyn FileSystem>, String), &'static str> {
        let mounts = self.mount_points.read();

        for mount in mounts.iter() {
            if path.starts_with(&mount.path) {
                let relative_path = if mount.path == "/" {
                    path.to_string()
                } else {
                    path.strip_prefix(&mount.path).unwrap_or("/").to_string()
                };
                return Ok((mount.filesystem.clone(), relative_path));
            }
        }

        Err("No filesystem mounted for path")
    }

    /// Resolve a path to an inode
    fn resolve_path(&self, path: &str) -> Result<(Arc<dyn FileSystem>, InodeId), &'static str> {
        if path.is_empty() || !path.starts_with('/') {
            return Err("Path must be absolute");
        }

        let (fs, relative_path) = self.find_filesystem(path)?;
        let root = fs.root_inode();

        if relative_path == "/" {
            let root_guard = root.read();
            return Ok((fs, root_guard.metadata().id));
        }

        // Split path and traverse
        let components: Vec<&str> = relative_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current_inode = root;
        for component in components {
            let inode_guard = current_inode.read();
            let next_id = inode_guard.lookup(component)?;
            drop(inode_guard);
            current_inode = fs.get_inode(next_id)?;
        }

        let current_guard = current_inode.read();
        Ok((fs, current_guard.metadata().id))
    }

    /// Open a file and return a file descriptor
    pub fn open(&self, path: &str, flags: OpenFlags) -> Result<FileDescriptor, &'static str> {
        let (fs, inode_id) = match self.resolve_path(path) {
            Ok(result) => result,
            Err(_) if flags.create => {
                // Create new file
                self.create_file(path)?;
                self.resolve_path(path)?
            }
            Err(e) => return Err(e),
        };

        let inode = fs.get_inode(inode_id)?;

        // Truncate if requested
        if flags.truncate && flags.write {
            let mut inode_guard = inode.write();
            inode_guard.truncate(0)?;
        }

        let fd = FileDescriptor::new();
        let file = Arc::new(File::new(inode_id, flags, inode));

        self.open_files.write().insert(fd, file);

        Ok(fd)
    }

    /// Close a file descriptor
    pub fn close(&self, fd: FileDescriptor) -> Result<(), &'static str> {
        self.open_files.write()
            .remove(&fd)
            .ok_or("Invalid file descriptor")?;
        Ok(())
    }

    /// Read from a file descriptor
    pub fn read(&self, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize, &'static str> {
        let files = self.open_files.read();
        let file = files.get(&fd).ok_or("Invalid file descriptor")?;
        file.read(buf)
    }

    /// Write to a file descriptor
    pub fn write(&self, fd: FileDescriptor, buf: &[u8]) -> Result<usize, &'static str> {
        let files = self.open_files.read();
        let file = files.get(&fd).ok_or("Invalid file descriptor")?;
        file.write(buf)
    }

    /// Create a new file
    fn create_file(&self, path: &str) -> Result<(), &'static str> {
        let parent_path = path.rfind('/').map(|i| &path[..i]).unwrap_or("/");
        let name = path.rfind('/').map(|i| &path[i+1..]).unwrap_or(path);

        if name.is_empty() {
            return Err("Invalid filename");
        }

        let (fs, parent_id) = self.resolve_path(parent_path)?;
        let parent_inode = fs.get_inode(parent_id)?;
        let mut parent_guard = parent_inode.write();

        parent_guard.create(name, InodeType::File)?;
        Ok(())
    }

    /// Create a new directory
    pub fn mkdir(&self, path: &str) -> Result<(), &'static str> {
        let parent_path = path.rfind('/').map(|i| &path[..i]).unwrap_or("/");
        let name = path.rfind('/').map(|i| &path[i+1..]).unwrap_or(path);

        if name.is_empty() {
            return Err("Invalid directory name");
        }

        let (fs, parent_id) = self.resolve_path(parent_path)?;
        let parent_inode = fs.get_inode(parent_id)?;
        let mut parent_guard = parent_inode.write();

        parent_guard.create(name, InodeType::Directory)?;
        Ok(())
    }

    /// Remove a file or directory
    pub fn unlink(&self, path: &str) -> Result<(), &'static str> {
        let parent_path = path.rfind('/').map(|i| &path[..i]).unwrap_or("/");
        let name = path.rfind('/').map(|i| &path[i+1..]).unwrap_or(path);

        if name.is_empty() {
            return Err("Invalid filename");
        }

        let (fs, parent_id) = self.resolve_path(parent_path)?;
        let parent_inode = fs.get_inode(parent_id)?;
        let mut parent_guard = parent_inode.write();

        parent_guard.remove(name)
    }

    /// List directory contents
    pub fn readdir(&self, path: &str) -> Result<Vec<(String, InodeId)>, &'static str> {
        let (fs, inode_id) = self.resolve_path(path)?;
        let inode = fs.get_inode(inode_id)?;
        let inode_guard = inode.read();
        inode_guard.readdir()
    }
}
