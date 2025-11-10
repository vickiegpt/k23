//! File system syscalls for WASM host functions

use alloc::vec::Vec;
use alloc::string::String;

use super::{get_vfs, OpenFlags, FileDescriptor};

/// Open a file
pub fn sys_open(path_ptr: u32, path_len: u32, flags: u32, memory: &[u8]) -> i32 {
    // Extract path from WASM memory
    let path_bytes = &memory[path_ptr as usize..(path_ptr + path_len) as usize];
    let path = match core::str::from_utf8(path_bytes) {
        Ok(p) => p,
        Err(_) => return -1, // EINVAL
    };

    let vfs = match get_vfs() {
        Some(v) => v,
        None => return -1,
    };

    let open_flags = parse_open_flags(flags);

    match vfs.open(path, open_flags) {
        Ok(fd) => fd.as_u32() as i32,
        Err(_) => -1,
    }
}

/// Close a file descriptor
pub fn sys_close(fd: u32) -> i32 {
    let vfs = match get_vfs() {
        Some(v) => v,
        None => return -1,
    };

    let fd = FileDescriptor::from(fd);
    match vfs.close(fd) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Read from a file descriptor
pub fn sys_read(fd: u32, buf_ptr: u32, buf_len: u32, memory: &mut [u8]) -> i32 {
    let vfs = match get_vfs() {
        Some(v) => v,
        None => return -1,
    };

    let fd = FileDescriptor::from(fd);
    let buf = &mut memory[buf_ptr as usize..(buf_ptr + buf_len) as usize];

    match vfs.read(fd, buf) {
        Ok(bytes_read) => bytes_read as i32,
        Err(_) => -1,
    }
}

/// Write to a file descriptor
pub fn sys_write(fd: u32, buf_ptr: u32, buf_len: u32, memory: &[u8]) -> i32 {
    let vfs = match get_vfs() {
        Some(v) => v,
        None => return -1,
    };

    let fd = FileDescriptor::from(fd);
    let buf = &memory[buf_ptr as usize..(buf_ptr + buf_len) as usize];

    match vfs.write(fd, buf) {
        Ok(bytes_written) => bytes_written as i32,
        Err(_) => -1,
    }
}

/// Create a directory
pub fn sys_mkdir(path_ptr: u32, path_len: u32, memory: &[u8]) -> i32 {
    let path_bytes = &memory[path_ptr as usize..(path_ptr + path_len) as usize];
    let path = match core::str::from_utf8(path_bytes) {
        Ok(p) => p,
        Err(_) => return -1,
    };

    let vfs = match get_vfs() {
        Some(v) => v,
        None => return -1,
    };

    match vfs.mkdir(path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Remove a file or directory
pub fn sys_unlink(path_ptr: u32, path_len: u32, memory: &[u8]) -> i32 {
    let path_bytes = &memory[path_ptr as usize..(path_ptr + path_len) as usize];
    let path = match core::str::from_utf8(path_bytes) {
        Ok(p) => p,
        Err(_) => return -1,
    };

    let vfs = match get_vfs() {
        Some(v) => v,
        None => return -1,
    };

    match vfs.unlink(path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

fn parse_open_flags(flags: u32) -> OpenFlags {
    const O_RDONLY: u32 = 0x0000;
    const O_WRONLY: u32 = 0x0001;
    const O_RDWR: u32 = 0x0002;
    const O_CREAT: u32 = 0x0040;
    const O_TRUNC: u32 = 0x0200;
    const O_APPEND: u32 = 0x0400;

    let access_mode = flags & 0x0003;

    OpenFlags {
        read: access_mode == O_RDONLY || access_mode == O_RDWR,
        write: access_mode == O_WRONLY || access_mode == O_RDWR,
        create: flags & O_CREAT != 0,
        truncate: flags & O_TRUNC != 0,
        append: flags & O_APPEND != 0,
    }
}

impl From<u32> for FileDescriptor {
    fn from(fd: u32) -> Self {
        FileDescriptor(fd)
    }
}
