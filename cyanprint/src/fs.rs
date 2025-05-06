use std::error::Error;
use std::path::{Path, PathBuf};

use cyancoordinator::fs::{FileLoader, FileMerger, FileUnpacker, FileWriter, VirtualFileSystem};

pub trait Vfs {
    fn unpack_archive(
        &self,
        archive_data: Vec<u8>,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
    fn load_local_files(
        &self,
        dir: &Path,
        paths: &[PathBuf],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
    fn merge(
        &self,
        base: &VirtualFileSystem,
        local: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
    fn write_to_disk(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>>;
}

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
        dir: &Path,
        paths: &[PathBuf],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.loader.load(dir, paths)
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
