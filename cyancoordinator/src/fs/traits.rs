use std::error::Error;
use std::path::{Path, PathBuf};

use super::VirtualFileSystem;

// FileUnpacker trait for unpacking archives
pub trait FileUnpacker {
    fn unpack(&self, archive_data: Vec<u8>) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
}

// FileLoader trait for loading existing files
pub trait FileLoader {
    fn load(
        &self,
        dir: &Path,
        paths: &[PathBuf],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
}

// FileMerger trait for merging files
pub trait FileMerger {
    fn merge(
        &self,
        base: &VirtualFileSystem,
        current: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
}

// FileWriter trait for writing files to disk
pub trait FileWriter {
    fn write(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>>;
}
