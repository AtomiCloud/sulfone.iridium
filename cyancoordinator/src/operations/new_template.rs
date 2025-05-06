use std::error::Error;
use std::path::Path;

use cyanregistry::http::models::template_res::TemplateVersionRes;

use crate::fs::{Vfs, VirtualFileSystem};
use crate::session::SessionIdGenerator;
use crate::template::{TemplateExecutor, TemplateHistory};

/// Create a new project from a template
///
/// # Arguments
/// * `session_id_generator` - Generator for session IDs
/// * `template` - Template version to use
/// * `target_dir` - Directory to create the project in
/// * `template_executor` - Executor to run the template
/// * `template_history` - History manager for template metadata
/// * `vfs` - Virtual file system implementation
/// * `username` - Username for template history
///
/// # Returns
/// * `Vec<String>` - List of session IDs that were created and need to be cleaned up
pub fn create_new_template(
    session_id_generator: &dyn SessionIdGenerator,
    template: &TemplateVersionRes,
    target_dir: &Path,
    template_executor: &dyn TemplateExecutor,
    template_history: &dyn TemplateHistory,
    vfs: &dyn Vfs,
    username: &str,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    println!("✨ Creating a new project from template");

    // Generate a new session ID
    let new_session_id = session_id_generator.generate();

    // Execute the template with fresh QA session
    let (archive_data, template_state, actual_session_id) =
        template_executor.execute_template(template, &new_session_id, None, None)?;

    // Unpack the archive into VFS
    let incoming_vfs = vfs.unpack_archive(archive_data)?;

    // Create an empty base VFS for comparison
    let base_vfs = VirtualFileSystem::new();

    // Load any existing files that match paths in incoming_vfs
    let paths = incoming_vfs.get_paths();
    let local_vfs = vfs.load_local_files(target_dir, &paths)?;

    // Merge with base=empty, local=target folder, incoming=VFS
    let merged_vfs = vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

    // Write the merged VFS to disk
    vfs.write_to_disk(target_dir, &merged_vfs)?;

    // Save template metadata
    template_history.save_template_metadata(target_dir, template, &template_state, username)?;

    println!("✅ Project created successfully");

    // Return the session ID for cleanup
    Ok(vec![actual_session_id])
}
