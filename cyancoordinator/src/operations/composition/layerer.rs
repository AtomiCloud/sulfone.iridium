use std::error::Error;

use crate::fs::VirtualFileSystem;

/// Trait for VFS layering operations
pub trait VfsLayerer {
    fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;
}

/// Default implementation that overwrites in order (later templates win)
pub struct DefaultVfsLayerer;

impl VfsLayerer for DefaultVfsLayerer {
    fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        if vfs_list.is_empty() {
            return Ok(VirtualFileSystem::new());
        }

        // Start with first VFS
        let mut result = vfs_list[0].clone();

        // Layer each subsequent VFS (later ones overwrite earlier ones)
        for vfs in &vfs_list[1..] {
            for path in vfs.get_paths() {
                if let Some(content) = vfs.get_file(&path) {
                    result.add_file(path, content.clone());
                }
            }
        }

        println!(
            "🔄 Layered {} VFS outputs (later templates overwrite earlier ones)",
            vfs_list.len()
        );
        Ok(result)
    }
}
