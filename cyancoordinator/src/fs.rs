use diffy::merge;
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

// VirtualFileSystem represents an in-memory file system
#[derive(Debug, Default)]
pub struct VirtualFileSystem {
    files: HashMap<PathBuf, Vec<u8>>,
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: PathBuf, content: Vec<u8>) {
        self.files.insert(path, content);
    }

    pub fn get_file(&self, path: &Path) -> Option<&Vec<u8>> {
        self.files.get(path)
    }

    pub fn get_paths(&self) -> Vec<PathBuf> {
        self.files.keys().cloned().collect()
    }
}

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

// TarGzUnpacker implementation for tar.gz archives
pub struct TarGzUnpacker;

impl FileUnpacker for TarGzUnpacker {
    fn unpack(&self, archive_data: Vec<u8>) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        let tar_gz = GzDecoder::new(&archive_data[..]);
        let mut archive = Archive::new(tar_gz);
        let mut vfs = VirtualFileSystem::new();

        for entry in archive
            .entries()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
        {
            let mut entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            let path = entry
                .path()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
                .to_path_buf();

            // Skip directories - we only want to process files
            if entry.header().entry_type().is_dir() {
                continue;
            }

            let mut buffer = Vec::new();
            entry
                .read_to_end(&mut buffer)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            vfs.add_file(path, buffer);
        }

        Ok(vfs)
    }
}

// DiskFileLoader implementation for loading files from disk
pub struct DiskFileLoader;

impl FileLoader for DiskFileLoader {
    fn load(
        &self,
        dir: &Path,
        paths: &[PathBuf],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        let mut vfs = VirtualFileSystem::new();

        for path in paths {
            let full_path = dir.join(path);
            if full_path.exists() {
                let content =
                    std::fs::read(&full_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                vfs.add_file(path.clone(), content);
            }
        }

        Ok(vfs)
    }
}

// DiffyMerger implementation using the diffy library
pub struct DiffyMerger;

impl FileMerger for DiffyMerger {
    fn merge(
        &self,
        base: &VirtualFileSystem,
        current: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        let mut result = VirtualFileSystem::new();

        // Get all unique paths from incoming VFS
        let incoming_paths = incoming.get_paths();

        for path in &incoming_paths {
            let incoming_content = incoming.get_file(path).unwrap();

            // If file exists in current VFS, perform 3-way merge
            if let Some(current_content) = current.get_file(path) {
                // Get base content (empty by default, can be changed later)
                // Create a longer-lived value for the empty base content
                let empty_vec = Vec::new();
                let base_content = base.get_file(path).unwrap_or(&empty_vec);

                // Convert to strings for diffy (assuming UTF-8 content)
                let base_str = String::from_utf8_lossy(base_content);
                let current_str = String::from_utf8_lossy(current_content);
                let incoming_str = String::from_utf8_lossy(incoming_content);

                // Perform 3-way merge
                let merged_result = merge(&base_str, &current_str, &incoming_str);

                match merged_result {
                    Ok(merged) => {
                        result.add_file(path.clone(), merged.into_bytes());
                    }
                    Err(e) => {
                        // Instead of using incoming content directly, create a file with Git merge conflict markers
                        let base_str_display = String::from_utf8_lossy(base_content);
                        let current_str_display = String::from_utf8_lossy(current_content);
                        let incoming_str_display = String::from_utf8_lossy(incoming_content);

                        // Format conflict with Git-style markers
                        let conflict_content = format!(
                            "<<<<<<< ours\n{}\n||||||| original\n{}\n=======\n{}\n>>>>>>> theirs\n",
                            current_str_display, base_str_display, incoming_str_display
                        );

                        // Add the file with conflict markers instead of just the incoming content
                        result.add_file(path.clone(), conflict_content.into_bytes());

                        // Log the merge conflict to tracing
                        tracing::warn!("Merge conflict for {}: {}", path.display(), e);
                    }
                }
            } else {
                // If file doesn't exist in current VFS, simply add from incoming
                result.add_file(path.clone(), incoming_content.clone());
            }
        }

        Ok(result)
    }
}

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
