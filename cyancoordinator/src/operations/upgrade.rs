use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use crate::fs::Vfs;
use crate::session::SessionIdGenerator;
use crate::template::{TemplateExecutor, TemplateHistory};

/// Context for upgrading a template
pub struct UpgradeContext<'a, F> {
    pub session_id_generator: &'a dyn SessionIdGenerator,
    pub template: &'a TemplateVersionRes,
    pub target_dir: &'a Path,
    pub template_executor: &'a dyn TemplateExecutor,
    pub template_history: &'a dyn TemplateHistory,
    pub vfs: &'a dyn Vfs,
    pub username: &'a str,
    pub previous_version: i64,
    pub previous_answers: HashMap<String, Answer>,
    pub previous_states: HashMap<String, String>,
    pub get_previous_template: F,
}

/// Upgrade a template to a new version
///
/// # Arguments
/// * `context` - Context containing all necessary parameters
///
/// # Returns
/// * `Vec<String>` - List of session IDs that were created and need to be cleaned up
pub fn upgrade_template<F>(context: UpgradeContext<F>) -> Result<Vec<String>, Box<dyn Error + Send>>
where
    F: Fn(i64) -> Result<TemplateVersionRes, Box<dyn Error + Send>>,
{
    println!(
        "🔄 Upgrading from version {} to {}",
        context.previous_version, context.template.principal.version
    );

    // First, execute the old template version using saved answers to get the base VFS
    println!("🏗️ Recreating previous template version");

    // Generate session IDs for both executions
    let prev_session_id = context.session_id_generator.generate();
    let curr_session_id = context.session_id_generator.generate();

    // Fetch the previous template version
    let prev_template_ver = (context.get_previous_template)(context.previous_version)?;

    let (prev_archive_data, _, prev_actual_session_id) =
        context.template_executor.execute_template(
            &prev_template_ver,
            &prev_session_id,
            Some(&context.previous_answers),
            Some(&context.previous_states),
        )?;

    // Unpack the archive into base VFS
    let base_vfs = context.vfs.unpack_archive(prev_archive_data)?;

    // Second, execute the new template version using saved answers where possible
    println!("🏗️ Creating new template version");
    let (curr_archive_data, template_state, curr_actual_session_id) =
        context.template_executor.execute_template(
            context.template,
            &curr_session_id,
            Some(&context.previous_answers),
            Some(&context.previous_states),
        )?;

    // Unpack the archive into incoming VFS
    let incoming_vfs = context.vfs.unpack_archive(curr_archive_data)?;

    // Get all paths that should be considered for the local VFS (union of base and incoming)
    let all_paths = Vec::new();

    // Load the current state of files from target directory
    let local_vfs = context
        .vfs
        .load_local_files(context.target_dir, &all_paths)?;

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
    let merged_vfs = context.vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

    // Write the merged VFS to disk
    context.vfs.write_to_disk(context.target_dir, &merged_vfs)?;

    // Save updated template metadata
    context.template_history.save_template_metadata(
        context.target_dir,
        context.template,
        &template_state,
        context.username,
    )?;

    println!("✅ Project upgraded successfully");

    // Return both session IDs for cleanup
    Ok(vec![prev_actual_session_id, curr_actual_session_id])
}
