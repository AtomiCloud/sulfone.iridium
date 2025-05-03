use diffy::merge;
use std::error::Error;

use super::traits::FileMerger;
use super::VirtualFileSystem;

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
