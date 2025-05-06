use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use crate::fs::Vfs;
use crate::session::SessionIdGenerator;
use crate::template::{TemplateExecutor, TemplateHistory};

/// Context for re-running a template
pub struct RerunContext<'a, F> {
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

/// Re-run a template with fresh answers
///
/// # Arguments
/// * `context` - Context containing all necessary parameters
///
/// # Returns
/// * `Vec<String>` - List of session IDs that were created and need to be cleaned up
pub fn rerun_template<F>(context: RerunContext<F>) -> Result<Vec<String>, Box<dyn Error + Send>>
where
    F: Fn(i64) -> Result<TemplateVersionRes, Box<dyn Error + Send>>,
{
    println!(
        "🔄 Re-running template (same version {})",
        context.previous_version
    );

    // Generate session IDs for both executions
    let prev_session_id = context.session_id_generator.generate();
    let curr_session_id = context.session_id_generator.generate();

    // Fetch the previous template version
    let prev_template = (context.get_previous_template)(context.previous_version)?;

    // First, recreate the previous template VFS state using saved answers and states
    println!("🏗️ Recreating previous template state");
    let (prev_archive_data, _, prev_actual_session_id) =
        context.template_executor.execute_template(
            &prev_template,
            &prev_session_id,
            Some(&context.previous_answers),
            Some(&context.previous_states),
        )?;

    // Second, execute the template with fresh Q&A
    println!("🏗️ Running template with new answers");
    let (curr_archive_data, template_state, curr_actual_session_id) =
        context.template_executor.execute_template(
            context.template,
            &curr_session_id,
            None, // No answers - user will provide fresh answers
            None,
        )?;

    // Unpack the archive into base VFS
    let base_vfs = context.vfs.unpack_archive(prev_archive_data)?;

    // Unpack the archive into incoming VFS
    let incoming_vfs = context.vfs.unpack_archive(curr_archive_data)?;

    // Load the current state of files from target directory
    let all_paths = Vec::new();
    let local_vfs = context
        .vfs
        .load_local_files(context.target_dir, &all_paths)?;

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

    println!("✅ Project recreated successfully with new answers");

    // Return both session IDs for cleanup
    Ok(vec![prev_actual_session_id, curr_actual_session_id])
}
