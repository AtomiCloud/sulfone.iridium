use ignore::{DirEntry, WalkBuilder};
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

        // If paths are specified, load only those files
        if !paths.is_empty() {
            for path in paths {
                let full_path = dir.join(path);
                if full_path.exists()
                    && !is_git_path(&full_path)
                    && path.file_name() != Some(".cyan_state.yaml".as_ref())
                {
                    let content = std::fs::read(&full_path)
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                    vfs.add_file(path.clone(), content);
                }
            }
        } else {
            // Otherwise, walk the directory respecting .gitignore
            let walker = WalkBuilder::new(dir)
                .hidden(false) // Process hidden files (except .git which is excluded by default)
                .git_ignore(true) // Respect .gitignore files
                .git_exclude(true) // Respect .git/info/exclude
                .git_global(true) // Respect global gitignore
                .build();

            for result in walker {
                match result {
                    Ok(entry) => {
                        if should_process_entry(&entry, dir) {
                            let path = entry.path();
                            let rel_path = path.strip_prefix(dir).unwrap_or(path);

                            if path.is_file() {
                                let content = std::fs::read(path)
                                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                                vfs.add_file(rel_path.to_path_buf(), content);
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!("Error walking directory: {}", err);
                    }
                }
            }
        }

        Ok(vfs)
    }
}

// Helper function to determine if a path is in the .git directory
fn is_git_path(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".git")
}

// Helper function to determine if an entry should be processed
fn should_process_entry(entry: &DirEntry, _base_dir: &Path) -> bool {
    // Skip the .git directory and its contents
    if is_git_path(entry.path()) {
        return false;
    }

    // Skip .cyan_state.yaml files
    if entry.file_name() == ".cyan_state.yaml" {
        return false;
    }

    // Skip directories, we only want files
    if entry.file_type().is_some_and(|ft| ft.is_dir()) {
        return false;
    }

    true
}
