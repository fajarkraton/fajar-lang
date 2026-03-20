//! Virtual filesystem layer for FajarOS.
//!
//! Provides an in-memory filesystem with directory tree support,
//! file descriptors, and mount points. Simulated — no real disk I/O.

use std::collections::HashMap;

/// Filesystem errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum VfsError {
    /// Path not found.
    #[error("path not found: {0}")]
    NotFound(String),
    /// Path already exists.
    #[error("path already exists: {0}")]
    AlreadyExists(String),
    /// Not a directory.
    #[error("not a directory: {0}")]
    NotADirectory(String),
    /// Not a file.
    #[error("not a file: {0}")]
    NotAFile(String),
    /// Directory not empty.
    #[error("directory not empty: {0}")]
    DirectoryNotEmpty(String),
    /// Invalid file descriptor.
    #[error("invalid file descriptor: {0}")]
    InvalidFd(u64),
    /// File descriptor table full.
    #[error("too many open files")]
    TooManyOpenFiles,
}

/// Inode type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    /// Regular file.
    File,
    /// Directory.
    Directory,
}

/// Inode — metadata for a filesystem entry.
#[derive(Debug, Clone)]
pub struct Inode {
    /// Inode number.
    pub ino: u64,
    /// Entry type.
    pub kind: InodeType,
    /// File size in bytes.
    pub size: u64,
    /// File data (only for files).
    pub data: Vec<u8>,
    /// Children (only for directories): name -> inode number.
    pub children: HashMap<String, u64>,
}

/// File descriptor — tracks an open file.
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    /// Inode number.
    pub ino: u64,
    /// Current read/write offset.
    pub offset: u64,
    /// Whether writable.
    pub writable: bool,
}

/// Virtual filesystem.
#[derive(Debug)]
pub struct Vfs {
    /// All inodes by number.
    inodes: HashMap<u64, Inode>,
    /// Next inode number.
    next_ino: u64,
    /// Open file descriptors.
    fds: HashMap<u64, FileDescriptor>,
    /// Next file descriptor number.
    next_fd: u64,
    /// Maximum open files.
    max_open_files: usize,
}

impl Vfs {
    /// Creates a new VFS with a root directory.
    pub fn new() -> Self {
        let mut inodes = HashMap::new();
        inodes.insert(
            1,
            Inode {
                ino: 1,
                kind: InodeType::Directory,
                size: 0,
                data: Vec::new(),
                children: HashMap::new(),
            },
        );
        Self {
            inodes,
            next_ino: 2,
            fds: HashMap::new(),
            next_fd: 3, // 0=stdin, 1=stdout, 2=stderr
            max_open_files: 256,
        }
    }

    /// Resolves a path to an inode number.
    fn resolve_path(&self, path: &str) -> Option<u64> {
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "/" {
            return Some(1); // root
        }

        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let mut current = 1u64; // root inode

        for part in parts {
            if part.is_empty() {
                continue;
            }
            let inode = self.inodes.get(&current)?;
            if inode.kind != InodeType::Directory {
                return None;
            }
            current = *inode.children.get(part)?;
        }
        Some(current)
    }

    /// Resolves parent directory and returns (parent_ino, basename).
    fn resolve_parent(&self, path: &str) -> Option<(u64, String)> {
        let path = path.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return None; // root has no parent
        }

        let parts: Vec<&str> = path.split('/').collect();
        let basename = parts.last()?.to_string();
        let parent_path = if parts.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", parts[..parts.len() - 1].join("/"))
        };

        let parent_ino = self.resolve_path(&parent_path)?;
        let parent = self.inodes.get(&parent_ino)?;
        if parent.kind != InodeType::Directory {
            return None;
        }

        Some((parent_ino, basename))
    }

    /// Creates a file at the given path.
    pub fn create_file(&mut self, path: &str) -> Result<u64, VfsError> {
        if self.resolve_path(path).is_some() {
            return Err(VfsError::AlreadyExists(path.to_string()));
        }

        let (parent_ino, basename) = self
            .resolve_parent(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        let ino = self.next_ino;
        self.next_ino += 1;

        self.inodes.insert(
            ino,
            Inode {
                ino,
                kind: InodeType::File,
                size: 0,
                data: Vec::new(),
                children: HashMap::new(),
            },
        );

        if let Some(parent) = self.inodes.get_mut(&parent_ino) {
            parent.children.insert(basename, ino);
        }

        Ok(ino)
    }

    /// Creates a directory at the given path.
    pub fn create_dir(&mut self, path: &str) -> Result<u64, VfsError> {
        if self.resolve_path(path).is_some() {
            return Err(VfsError::AlreadyExists(path.to_string()));
        }

        let (parent_ino, basename) = self
            .resolve_parent(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        let ino = self.next_ino;
        self.next_ino += 1;

        self.inodes.insert(
            ino,
            Inode {
                ino,
                kind: InodeType::Directory,
                size: 0,
                data: Vec::new(),
                children: HashMap::new(),
            },
        );

        if let Some(parent) = self.inodes.get_mut(&parent_ino) {
            parent.children.insert(basename, ino);
        }

        Ok(ino)
    }

    /// Deletes a file or empty directory.
    pub fn delete(&mut self, path: &str) -> Result<(), VfsError> {
        let ino = self
            .resolve_path(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if ino == 1 {
            return Err(VfsError::NotFound("cannot delete root".to_string()));
        }

        let inode = self
            .inodes
            .get(&ino)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if inode.kind == InodeType::Directory && !inode.children.is_empty() {
            return Err(VfsError::DirectoryNotEmpty(path.to_string()));
        }

        let (parent_ino, basename) = self
            .resolve_parent(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if let Some(parent) = self.inodes.get_mut(&parent_ino) {
            parent.children.remove(&basename);
        }
        self.inodes.remove(&ino);

        Ok(())
    }

    /// Renames/moves a file or directory.
    pub fn rename(&mut self, old_path: &str, new_path: &str) -> Result<(), VfsError> {
        let ino = self
            .resolve_path(old_path)
            .ok_or(VfsError::NotFound(old_path.to_string()))?;

        if self.resolve_path(new_path).is_some() {
            return Err(VfsError::AlreadyExists(new_path.to_string()));
        }

        // Remove from old parent
        let (old_parent, old_name) = self
            .resolve_parent(old_path)
            .ok_or(VfsError::NotFound(old_path.to_string()))?;
        if let Some(parent) = self.inodes.get_mut(&old_parent) {
            parent.children.remove(&old_name);
        }

        // Add to new parent
        let (new_parent, new_name) = self
            .resolve_parent(new_path)
            .ok_or(VfsError::NotFound(new_path.to_string()))?;
        if let Some(parent) = self.inodes.get_mut(&new_parent) {
            parent.children.insert(new_name, ino);
        }

        Ok(())
    }

    /// Lists directory contents.
    pub fn read_dir(&self, path: &str) -> Result<Vec<String>, VfsError> {
        let ino = self
            .resolve_path(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;
        let inode = self
            .inodes
            .get(&ino)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if inode.kind != InodeType::Directory {
            return Err(VfsError::NotADirectory(path.to_string()));
        }

        let mut entries: Vec<String> = inode.children.keys().cloned().collect();
        entries.sort();
        Ok(entries)
    }

    /// Gets file/directory metadata.
    pub fn stat(&self, path: &str) -> Result<(InodeType, u64), VfsError> {
        let ino = self
            .resolve_path(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;
        let inode = self
            .inodes
            .get(&ino)
            .ok_or(VfsError::NotFound(path.to_string()))?;
        Ok((inode.kind, inode.size))
    }

    /// Writes data to a file (overwrites).
    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        let ino = self
            .resolve_path(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;
        let inode = self
            .inodes
            .get_mut(&ino)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if inode.kind != InodeType::File {
            return Err(VfsError::NotAFile(path.to_string()));
        }

        inode.data = data.to_vec();
        inode.size = data.len() as u64;
        Ok(())
    }

    /// Reads data from a file.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        let ino = self
            .resolve_path(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;
        let inode = self
            .inodes
            .get(&ino)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if inode.kind != InodeType::File {
            return Err(VfsError::NotAFile(path.to_string()));
        }

        Ok(inode.data.clone())
    }

    /// Opens a file and returns a file descriptor.
    pub fn open(&mut self, path: &str, writable: bool) -> Result<u64, VfsError> {
        let ino = self
            .resolve_path(path)
            .ok_or(VfsError::NotFound(path.to_string()))?;
        let inode = self
            .inodes
            .get(&ino)
            .ok_or(VfsError::NotFound(path.to_string()))?;

        if inode.kind != InodeType::File {
            return Err(VfsError::NotAFile(path.to_string()));
        }

        if self.fds.len() >= self.max_open_files {
            return Err(VfsError::TooManyOpenFiles);
        }

        let fd = self.next_fd;
        self.next_fd += 1;
        self.fds.insert(
            fd,
            FileDescriptor {
                ino,
                offset: 0,
                writable,
            },
        );
        Ok(fd)
    }

    /// Closes a file descriptor.
    pub fn close(&mut self, fd: u64) -> Result<(), VfsError> {
        self.fds
            .remove(&fd)
            .map(|_| ())
            .ok_or(VfsError::InvalidFd(fd))
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s26_1_vfs_root_exists() {
        let vfs = Vfs::new();
        assert!(vfs.resolve_path("/").is_some());
        let entries = vfs.read_dir("/").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn s26_2_create_file() {
        let mut vfs = Vfs::new();
        let ino = vfs.create_file("/hello.txt").unwrap();
        assert!(ino > 1);
        let entries = vfs.read_dir("/").unwrap();
        assert_eq!(entries, vec!["hello.txt"]);
    }

    #[test]
    fn s26_3_create_directory() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/docs").unwrap();
        vfs.create_file("/docs/readme.md").unwrap();
        let entries = vfs.read_dir("/docs").unwrap();
        assert_eq!(entries, vec!["readme.md"]);
    }

    #[test]
    fn s26_4_write_and_read_file() {
        let mut vfs = Vfs::new();
        vfs.create_file("/data.bin").unwrap();
        vfs.write_file("/data.bin", b"Hello, FajarOS!").unwrap();
        let data = vfs.read_file("/data.bin").unwrap();
        assert_eq!(data, b"Hello, FajarOS!");
    }

    #[test]
    fn s26_5_delete_file() {
        let mut vfs = Vfs::new();
        vfs.create_file("/temp.txt").unwrap();
        assert!(vfs.resolve_path("/temp.txt").is_some());
        vfs.delete("/temp.txt").unwrap();
        assert!(vfs.resolve_path("/temp.txt").is_none());
    }

    #[test]
    fn s26_6_delete_nonempty_dir_fails() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/stuff").unwrap();
        vfs.create_file("/stuff/a.txt").unwrap();
        assert!(matches!(
            vfs.delete("/stuff"),
            Err(VfsError::DirectoryNotEmpty(_))
        ));
    }

    #[test]
    fn s26_7_rename() {
        let mut vfs = Vfs::new();
        vfs.create_file("/old.txt").unwrap();
        vfs.write_file("/old.txt", b"data").unwrap();
        vfs.rename("/old.txt", "/new.txt").unwrap();
        assert!(vfs.resolve_path("/old.txt").is_none());
        let data = vfs.read_file("/new.txt").unwrap();
        assert_eq!(data, b"data");
    }

    #[test]
    fn s26_8_stat() {
        let mut vfs = Vfs::new();
        vfs.create_file("/f.txt").unwrap();
        vfs.write_file("/f.txt", b"12345").unwrap();
        let (kind, size) = vfs.stat("/f.txt").unwrap();
        assert_eq!(kind, InodeType::File);
        assert_eq!(size, 5);

        vfs.create_dir("/d").unwrap();
        let (kind, _) = vfs.stat("/d").unwrap();
        assert_eq!(kind, InodeType::Directory);
    }

    #[test]
    fn s26_9_file_descriptors() {
        let mut vfs = Vfs::new();
        vfs.create_file("/test.txt").unwrap();
        let fd = vfs.open("/test.txt", true).unwrap();
        assert!(fd >= 3); // 0,1,2 reserved
        vfs.close(fd).unwrap();
        assert!(matches!(vfs.close(fd), Err(VfsError::InvalidFd(_))));
    }

    #[test]
    fn s26_10_nested_directories() {
        let mut vfs = Vfs::new();
        vfs.create_dir("/a").unwrap();
        vfs.create_dir("/a/b").unwrap();
        vfs.create_dir("/a/b/c").unwrap();
        vfs.create_file("/a/b/c/deep.txt").unwrap();
        vfs.write_file("/a/b/c/deep.txt", b"deep").unwrap();
        let data = vfs.read_file("/a/b/c/deep.txt").unwrap();
        assert_eq!(data, b"deep");
    }
}
