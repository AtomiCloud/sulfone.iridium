use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

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

    fn cleanup(
        &self,
        target_dir: &Path,
        files_to_delete: &[PathBuf],
    ) -> Result<(), Box<dyn Error + Send>> {
        let mut parent_dirs: BTreeSet<PathBuf> = BTreeSet::new();

        for path in files_to_delete {
            let full_path = target_dir.join(path);
            if full_path.is_file() {
                std::fs::remove_file(&full_path)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

                // Collect parent directories for cleanup
                if let Some(parent) = full_path.parent() {
                    parent_dirs.insert(parent.to_path_buf());
                }
            }
        }

        // Remove empty parent directories (deepest first)
        let mut dirs_to_check: Vec<PathBuf> = parent_dirs.into_iter().collect();
        dirs_to_check.sort_by_key(|b| std::cmp::Reverse(b.components().count()));

        for dir in dirs_to_check {
            let mut current = dir.as_path();
            while current.starts_with(target_dir) && current != target_dir {
                if current.is_dir()
                    && std::fs::read_dir(current).map_or(true, |mut d| d.next().is_none())
                {
                    let _ = std::fs::remove_dir(current);
                } else {
                    break;
                }
                current = match current.parent() {
                    Some(p) => p,
                    None => break,
                };
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cleanup_deletes_files() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        // Create files on disk
        std::fs::write(target.join("keep.txt"), b"keep").unwrap();
        std::fs::write(target.join("delete.txt"), b"delete").unwrap();

        let writer = DiskFileWriter;
        writer
            .cleanup(target, &[PathBuf::from("delete.txt")])
            .unwrap();

        assert!(target.join("keep.txt").exists());
        assert!(!target.join("delete.txt").exists());
    }

    #[test]
    fn test_cleanup_removes_empty_parent_dirs() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        // Create nested file
        std::fs::create_dir_all(target.join("subdir/nested")).unwrap();
        std::fs::write(target.join("subdir/nested/file.txt"), b"content").unwrap();

        let writer = DiskFileWriter;
        writer
            .cleanup(target, &[PathBuf::from("subdir/nested/file.txt")])
            .unwrap();

        assert!(!target.join("subdir/nested/file.txt").exists());
        assert!(!target.join("subdir/nested").exists());
        assert!(!target.join("subdir").exists());
    }

    #[test]
    fn test_cleanup_keeps_nonempty_parent_dirs() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        // Create two files in same dir
        std::fs::create_dir_all(target.join("subdir")).unwrap();
        std::fs::write(target.join("subdir/keep.txt"), b"keep").unwrap();
        std::fs::write(target.join("subdir/delete.txt"), b"delete").unwrap();

        let writer = DiskFileWriter;
        writer
            .cleanup(target, &[PathBuf::from("subdir/delete.txt")])
            .unwrap();

        assert!(!target.join("subdir/delete.txt").exists());
        assert!(target.join("subdir").exists());
        assert!(target.join("subdir/keep.txt").exists());
    }

    #[test]
    fn test_cleanup_ignores_nonexistent_files() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        let writer = DiskFileWriter;
        let result = writer.cleanup(target, &[PathBuf::from("nonexistent.txt")]);
        assert!(result.is_ok());
    }
}
