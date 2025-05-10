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
}
