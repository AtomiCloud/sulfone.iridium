use std::error::Error;
use std::path::{Path, PathBuf};

use super::traits::FileLoader;
use super::VirtualFileSystem;

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
