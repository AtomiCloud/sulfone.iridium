mod loader;
mod merger;
mod traits;
mod unpacker;
mod vfs;
mod writer;

pub use loader::DiskFileLoader;
pub use merger::GitLikeMerger;
pub use traits::*;
pub use unpacker::TarGzUnpacker;
pub use vfs::VirtualFileSystem;
pub use writer::DiskFileWriter;

use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};

/// Trait for virtual file system operations
pub trait Vfs {
    /// Unpack an archive into a virtual file system
    fn unpack_archive(
        &self,
        archive_data: Vec<u8>,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;

    /// Load local files that match paths into a virtual file system
    fn load_local_files(
        &self,
        target_dir: &Path,
        paths: &[PathBuf],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;

    /// Merge virtual file systems using a three-way merge
    fn merge(
        &self,
        base: &VirtualFileSystem,
        local: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;

    /// Write a virtual file system to disk
    fn write_to_disk(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>>;

    /// Delete files that were present before merge but absent after merge
    fn cleanup_deleted_files(
        &self,
        target_dir: &Path,
        local_vfs: &VirtualFileSystem,
        merged_vfs: &VirtualFileSystem,
    ) -> Result<Vec<PathBuf>, Box<dyn Error + Send>>;
}

/// Default implementation of Vfs trait
pub struct DefaultVfs {
    unpacker: Box<dyn FileUnpacker>,
    loader: Box<dyn FileLoader>,
    merger: Box<dyn FileMerger>,
    writer: Box<dyn FileWriter>,
}

impl DefaultVfs {
    pub fn new(
        unpacker: Box<dyn FileUnpacker>,
        loader: Box<dyn FileLoader>,
        merger: Box<dyn FileMerger>,
        writer: Box<dyn FileWriter>,
    ) -> Self {
        Self {
            unpacker,
            loader,
            merger,
            writer,
        }
    }
}

impl Vfs for DefaultVfs {
    fn unpack_archive(
        &self,
        archive_data: Vec<u8>,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.unpacker.unpack(archive_data)
    }

    fn load_local_files(
        &self,
        target_dir: &Path,
        paths: &[PathBuf],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.loader.load(target_dir, paths)
    }

    fn merge(
        &self,
        base: &VirtualFileSystem,
        local: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.merger.merge(base, local, incoming)
    }

    fn write_to_disk(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.writer.write(target_dir, vfs)
    }

    fn cleanup_deleted_files(
        &self,
        target_dir: &Path,
        local_vfs: &VirtualFileSystem,
        merged_vfs: &VirtualFileSystem,
    ) -> Result<Vec<PathBuf>, Box<dyn Error + Send>> {
        let local_paths: HashSet<PathBuf> = local_vfs.get_paths().into_iter().collect();
        let merged_paths: HashSet<PathBuf> = merged_vfs.get_paths().into_iter().collect();

        let files_to_delete: Vec<PathBuf> =
            local_paths.difference(&merged_paths).cloned().collect();

        if !files_to_delete.is_empty() {
            self.writer.cleanup(target_dir, &files_to_delete)?;
        }

        Ok(files_to_delete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_cleanup_deleted_files_computes_diff() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        // Create files on disk
        std::fs::write(target.join("keep.txt"), b"keep").unwrap();
        std::fs::write(target.join("delete_me.txt"), b"delete").unwrap();

        // local_vfs has both files
        let mut local_vfs = VirtualFileSystem::new();
        local_vfs.add_file(PathBuf::from("keep.txt"), b"keep".to_vec());
        local_vfs.add_file(PathBuf::from("delete_me.txt"), b"delete".to_vec());

        // merged_vfs only has keep.txt
        let mut merged_vfs = VirtualFileSystem::new();
        merged_vfs.add_file(PathBuf::from("keep.txt"), b"keep".to_vec());

        let vfs = DefaultVfs::new(
            Box::new(TarGzUnpacker),
            Box::new(DiskFileLoader),
            Box::new(GitLikeMerger::new(false, 50)),
            Box::new(DiskFileWriter),
        );

        let deleted = vfs
            .cleanup_deleted_files(target, &local_vfs, &merged_vfs)
            .unwrap();

        assert_eq!(deleted.len(), 1);
        assert!(deleted.contains(&PathBuf::from("delete_me.txt")));
        assert!(!target.join("delete_me.txt").exists());
        assert!(target.join("keep.txt").exists());
    }

    #[test]
    fn test_cleanup_deleted_files_no_deletions() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        std::fs::write(target.join("file.txt"), b"content").unwrap();

        let mut local_vfs = VirtualFileSystem::new();
        local_vfs.add_file(PathBuf::from("file.txt"), b"content".to_vec());

        let merged_vfs = local_vfs.clone();

        let vfs = DefaultVfs::new(
            Box::new(TarGzUnpacker),
            Box::new(DiskFileLoader),
            Box::new(GitLikeMerger::new(false, 50)),
            Box::new(DiskFileWriter),
        );

        let deleted = vfs
            .cleanup_deleted_files(target, &local_vfs, &merged_vfs)
            .unwrap();

        assert!(deleted.is_empty());
        assert!(target.join("file.txt").exists());
    }

    #[test]
    fn test_cleanup_before_write_handles_file_to_directory_transition() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        fs::write(target.join("path"), b"old file").unwrap();

        let mut local_vfs = VirtualFileSystem::new();
        local_vfs.add_file(PathBuf::from("path"), b"old file".to_vec());

        let mut merged_vfs = VirtualFileSystem::new();
        merged_vfs.add_file(PathBuf::from("path/child.txt"), b"new child".to_vec());

        let vfs = DefaultVfs::new(
            Box::new(TarGzUnpacker),
            Box::new(DiskFileLoader),
            Box::new(GitLikeMerger::new(false, 50)),
            Box::new(DiskFileWriter),
        );

        let deleted = vfs
            .cleanup_deleted_files(target, &local_vfs, &merged_vfs)
            .unwrap();
        assert_eq!(deleted, vec![PathBuf::from("path")]);

        vfs.write_to_disk(target, &merged_vfs).unwrap();

        assert!(!target.join("path").is_file());
        assert_eq!(
            fs::read(target.join("path/child.txt")).unwrap(),
            b"new child".to_vec()
        );
    }

    #[test]
    fn test_cleanup_before_write_handles_directory_to_file_transition() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        fs::create_dir_all(target.join("path")).unwrap();
        fs::write(target.join("path/child.txt"), b"old child").unwrap();

        let mut local_vfs = VirtualFileSystem::new();
        local_vfs.add_file(PathBuf::from("path/child.txt"), b"old child".to_vec());

        let mut merged_vfs = VirtualFileSystem::new();
        merged_vfs.add_file(PathBuf::from("path"), b"new file".to_vec());

        let vfs = DefaultVfs::new(
            Box::new(TarGzUnpacker),
            Box::new(DiskFileLoader),
            Box::new(GitLikeMerger::new(false, 50)),
            Box::new(DiskFileWriter),
        );

        let deleted = vfs
            .cleanup_deleted_files(target, &local_vfs, &merged_vfs)
            .unwrap();
        assert_eq!(deleted, vec![PathBuf::from("path/child.txt")]);

        vfs.write_to_disk(target, &merged_vfs).unwrap();

        assert!(!target.join("path").is_dir());
        assert_eq!(fs::read(target.join("path")).unwrap(), b"new file".to_vec());
    }
}
