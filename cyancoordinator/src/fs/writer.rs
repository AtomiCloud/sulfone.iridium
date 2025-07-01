use std::error::Error;
use std::path::Path;

use super::VirtualFileSystem;
use super::traits::FileWriter;

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
