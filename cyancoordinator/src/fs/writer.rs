use std::error::Error;
use std::io::Read;
use std::path::Path;

use super::loader::DiskFileLoader;
use super::merger::DiffyMerger;
use super::traits::{FileLoader, FileMerger, FileUnpacker, FileWriter};
use super::unpacker::TarGzUnpacker;
use super::VirtualFileSystem;

// DiskFileWriter implementation for writing files to disk
pub struct DiskFileWriter;

impl FileWriter for DiskFileWriter {
    fn write(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>> {
        for (path, content) in &vfs.files {
            let full_path = target_dir.join(path);

            // Skip if target is an existing directory
            if full_path.is_dir() {
                continue;
            }

            // Create parent directories if they don't exist
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            }

            // Write file
            std::fs::write(&full_path, content)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        }

        Ok(())
    }
}

// FileSystemWriter that orchestrates the entire process
pub struct FileSystemWriter {
    unpacker: Box<dyn FileUnpacker>,
    loader: Box<dyn FileLoader>,
    merger: Box<dyn FileMerger>,
    writer: Box<dyn FileWriter>,
}

impl FileSystemWriter {
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

impl Default for FileSystemWriter {
    fn default() -> Self {
        Self {
            unpacker: Box::new(TarGzUnpacker),
            loader: Box::new(DiskFileLoader),
            merger: Box::new(DiffyMerger),
            writer: Box::new(DiskFileWriter),
        }
    }
}

impl FileSystemWriter {
    pub fn process<R: Read>(
        &self,
        archive_data: R,
        target_dir: &Path,
    ) -> Result<(), Box<dyn Error + Send>> {
        // Read all data into memory first
        let mut buffer = Vec::new();
        let mut reader = archive_data;
        reader
            .read_to_end(&mut buffer)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Step 1: Unpack archive into VFS
        let unpacked_vfs = self.unpacker.unpack(buffer)?;

        // Step 2: Get paths to load from target directory
        let paths = unpacked_vfs.get_paths();

        // Step 3: Load existing files into VFS
        let current_vfs = self.loader.load(target_dir, &paths)?;

        // Step 4: Create empty base VFS
        let base_vfs = VirtualFileSystem::new();

        // Step 5: Merge VFSs
        let merged_vfs = self.merger.merge(&base_vfs, &current_vfs, &unpacked_vfs)?;

        // Step 6: Write merged VFS to disk
        self.writer.write(target_dir, &merged_vfs)?;

        Ok(())
    }
}
