use flate2::read::GzDecoder;
use std::error::Error;
use std::io::Read;
use std::path::Path;
use tar::Archive;

use super::VirtualFileSystem;
use super::traits::FileUnpacker;

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

            // Skip any .git directory files
            if is_git_path(&path) {
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

// Helper function to determine if a path is in the .git directory
fn is_git_path(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".git")
}
