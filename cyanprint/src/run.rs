use std::error::Error;
use std::fs;
use std::path::PathBuf;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{
    DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker, VirtualFileSystem,
};
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::rc::Rc;

use crate::fs::{DefaultVfs, Vfs};
use crate::session::SessionIdGenerator;
use crate::template::{DefaultTemplateExecutor, TemplateExecutor};
use crate::template_history::{DefaultTemplateHistory, TemplateHistory, TemplateUpdateType};

/// Run the cyan template generation process
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_run(
    session_id_generator: &dyn SessionIdGenerator,
    path: Option<String>,
    template: TemplateVersionRes,
    coord_client: CyanCoordinatorClient,
    username: String,
    registry_client: Option<Rc<CyanRegistryClient>>,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // handle the target directory
    let path = path.unwrap_or(".".to_string());
    let path_buf = PathBuf::from(&path);
    let target_dir = path_buf.as_path();
    println!("📁 Target directory: {:?}", target_dir);
    fs::create_dir_all(target_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Create all components for dependency injection at the highest level
    let unpacker: Box<dyn cyancoordinator::fs::FileUnpacker> = Box::new(TarGzUnpacker);
    let loader: Box<dyn cyancoordinator::fs::FileLoader> = Box::new(DiskFileLoader);
    let merger: Box<dyn cyancoordinator::fs::FileMerger> = Box::new(GitLikeMerger::new(true, 50)); // Debug enabled
    let writer: Box<dyn cyancoordinator::fs::FileWriter> = Box::new(DiskFileWriter);

    // Setup services with explicit dependencies
    let template_history = DefaultTemplateHistory::new();
    let template_executor = DefaultTemplateExecutor::new(coord_client.endpoint.clone());
    let vfs = DefaultVfs::new(unpacker, loader, merger, writer);

    // Check template history to determine update scenario
    let update_type = template_history.check_template_history(target_dir, &template, &username)?;

    // Helper function to get previous template version
    let get_previous_template_ver =
        |previous_version: i64| -> Result<TemplateVersionRes, Box<dyn Error + Send>> {
            if let Some(registry) = &registry_client {
                // Fetch the actual previous version from registry
                let template_name = template.template.name.clone();
                println!(
                    "🔍 Fetching template '{}/{}:{}' from registry...",
                    username, template_name, previous_version
                );
                let prev_template = registry.get_template(
                    username.clone(),
                    template_name,
                    Some(previous_version),
                )?;
                println!("✅ Retrieved previous template version from registry");
                Ok(prev_template)
            } else {
                // Fallback to modifying the current template if registry client not available
                let mut prev_template_ver = template.clone();
                prev_template_ver.principal.version = previous_version;
                Ok(prev_template_ver)
            }
        };

    // Handle different update scenarios and collect all session IDs for cleanup
    let (session_ids, _) = match update_type {
        TemplateUpdateType::NewTemplate => {
            // Scenario 1: No previous template matching the current template
            println!("✨ Creating a new project from template");

            // Generate a new session ID
            let new_session_id = session_id_generator.generate();

            // Execute the template with fresh QA session
            let (archive_data, template_state, actual_session_id) =
                template_executor.execute_template(&template, &new_session_id, None, None)?;

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
            template_history.save_template_metadata(
                target_dir,
                &template,
                &template_state,
                &username,
            )?;

            println!("✅ Project created successfully");

            // Return the session ID for cleanup
            (vec![actual_session_id], ())
        }
        TemplateUpdateType::UpgradeTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Scenario 2: Previous template matching the current template exists, but a different version
            println!(
                "🔄 Upgrading from version {} to {}",
                previous_version, template.principal.version
            );

            // First, execute the old template version using saved answers to get the base VFS
            println!("🏗️ Recreating previous template version");

            // Generate session IDs for both executions
            let prev_session_id = session_id_generator.generate();
            let curr_session_id = session_id_generator.generate();

            // Fetch the previous template version
            let prev_template_ver = get_previous_template_ver(previous_version)?;

            let (prev_archive_data, _, prev_actual_session_id) = template_executor
                .execute_template(
                    &prev_template_ver,
                    &prev_session_id,
                    Some(&previous_answers),
                    Some(&previous_states),
                )?;

            // Unpack the archive into base VFS
            let base_vfs = vfs.unpack_archive(prev_archive_data)?;

            // Second, execute the new template version using saved answers where possible
            println!("🏗️ Creating new template version");
            let (curr_archive_data, template_state, curr_actual_session_id) = template_executor
                .execute_template(
                    &template,
                    &curr_session_id,
                    Some(&previous_answers),
                    Some(&previous_states),
                )?;

            // Unpack the archive into incoming VFS
            let incoming_vfs = vfs.unpack_archive(curr_archive_data)?;

            // Get all paths that should be considered for the local VFS (union of base and incoming)
            let all_paths = Vec::new();

            // Load the current state of files from target directory
            let local_vfs = vfs.load_local_files(target_dir, &all_paths)?;

            // Print the contents of all three VFS objects before merging
            println!("📂 Base VFS (Previous Template Version):");
            for path in base_vfs.get_paths() {
                if let Some(content) = base_vfs.get_file(&path) {
                    println!("  - {} ({} bytes)", path.display(), content.len());
                }
            }

            println!("📂 Local VFS (Current Files in Target Directory):");
            for path in local_vfs.get_paths() {
                if let Some(content) = local_vfs.get_file(&path) {
                    println!("  - {} ({} bytes)", path.display(), content.len());
                }
            }

            println!("📂 Incoming VFS (New Template Version):");
            for path in incoming_vfs.get_paths() {
                if let Some(content) = incoming_vfs.get_file(&path) {
                    println!("  - {} ({} bytes)", path.display(), content.len());
                }
            }

            // Perform 3-way merge with base=prev template, local=target folder, incoming=current template
            let merged_vfs = vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

            // Write the merged VFS to disk
            vfs.write_to_disk(target_dir, &merged_vfs)?;

            // Save updated template metadata
            template_history.save_template_metadata(
                target_dir,
                &template,
                &template_state,
                &username,
            )?;

            println!("✅ Project upgraded successfully");

            // Return both session IDs for cleanup
            (vec![prev_actual_session_id, curr_actual_session_id], ())
        }
        TemplateUpdateType::RerunTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Scenario 3: Previous template matching the current template exists, with the same version
            println!("🔄 Re-running template (same version {})", previous_version);

            // Generate session IDs for both executions
            let prev_session_id = session_id_generator.generate();
            let curr_session_id = session_id_generator.generate();

            // Fetch the previous template version
            let prev_template = get_previous_template_ver(previous_version)?;

            // First, recreate the previous template VFS state using saved answers and states
            println!("🏗️ Recreating previous template state");
            let (prev_archive_data, _, prev_actual_session_id) = template_executor
                .execute_template(
                    &prev_template,
                    &prev_session_id,
                    Some(&previous_answers),
                    Some(&previous_states),
                )?;

            // Second, execute the template with fresh Q&A
            println!("🏗️ Running template with new answers");
            let (curr_archive_data, template_state, curr_actual_session_id) = template_executor
                .execute_template(
                    &template,
                    &curr_session_id,
                    None, // No answers - user will provide fresh answers
                    None,
                )?;

            // Unpack the archive into base VFS
            let base_vfs = vfs.unpack_archive(prev_archive_data)?;

            // Unpack the archive into incoming VFS
            let incoming_vfs = vfs.unpack_archive(curr_archive_data)?;

            // Load the current state of files from target directory
            let all_paths = Vec::new();
            let local_vfs = vfs.load_local_files(target_dir, &all_paths)?;

            // Print the contents of all three VFS objects before merging
            println!("📂 Base VFS (Previous Template Version):");
            for path in base_vfs.get_paths() {
                if let Some(content) = base_vfs.get_file(&path) {
                    println!("  - {} ({} bytes)", path.display(), content.len());
                }
            }

            println!("📂 Local VFS (Current Files in Target Directory):");
            for path in local_vfs.get_paths() {
                if let Some(content) = local_vfs.get_file(&path) {
                    println!("  - {} ({} bytes)", path.display(), content.len());
                }
            }

            println!("📂 Incoming VFS (New Template Version):");
            for path in incoming_vfs.get_paths() {
                if let Some(content) = incoming_vfs.get_file(&path) {
                    println!("  - {} ({} bytes)", path.display(), content.len());
                }
            }

            // Perform 3-way merge with base=prev template, local=target folder, incoming=current template
            let merged_vfs = vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

            // Write the merged VFS to disk
            vfs.write_to_disk(target_dir, &merged_vfs)?;

            // Save updated template metadata
            template_history.save_template_metadata(
                target_dir,
                &template,
                &template_state,
                &username,
            )?;

            println!("✅ Project recreated successfully with new answers");

            // Return both session IDs for cleanup
            (vec![prev_actual_session_id, curr_actual_session_id], ())
        }
    };

    Ok(session_ids)
}
