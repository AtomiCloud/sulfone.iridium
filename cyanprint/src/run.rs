use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::iter;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::conflict_file_resolver::FileConflictEntry;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::operations::composition::{CompositionOperator, DefaultDependencyResolver};
use cyancoordinator::state::{DefaultStateManager, StateReader, StateWriter};
use cyancoordinator::template::TemplateHistory;
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateUpdateType};

use crate::command_executor::CommandExecutor;
use crate::update::spec::{TemplateSpec, TemplateSpecManager, sort_specs};

/// Check if a template has execution artifacts (Docker properties)
fn has_execution_artifacts(template: &TemplateVersionRes) -> bool {
    template.principal.properties.is_some()
}

/// Check if a template has dependencies
fn has_dependencies(template: &TemplateVersionRes) -> bool {
    !template.templates.is_empty()
}

/// Unified batch processing for both create and update commands.
/// Handles MAP, LAYER, and MERGE+WRITE phases.
/// Returns session IDs for cleanup, file conflicts for state persistence, and commands for execution.
#[allow(clippy::type_complexity)]
pub fn batch_process(
    prev_specs: &[TemplateSpec],
    curr_specs: &[TemplateSpec],
    upgraded_specs: &[&TemplateSpec], // Templates that need metadata saved
    target_dir: &Path,
    registry: &CyanRegistryClient,
    coord_client: &CyanCoordinatorClient,
    operator: &mut CompositionOperator,
) -> Result<(Vec<String>, Vec<FileConflictEntry>, Vec<String>), Box<dyn Error + Send>> {
    // PHASE 2: MAP (execute each template spec → VFS)
    println!(
        "\n📦 PHASE 2: MAP - Executing {} prev + {} curr templates",
        prev_specs.len(),
        curr_specs.len()
    );

    // Execute prev_specs and collect template responses for horizontal layering
    let mut prev_vfs_list = Vec::new();
    let mut prev_session_ids = Vec::new();
    let mut prev_template_res_list = Vec::new();

    for spec in prev_specs {
        println!("  🔄 Executing prev: {} v{}", spec.key(), spec.version);
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, _final_state, session_ids) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        prev_vfs_list.push(vfs);
        prev_session_ids.extend(session_ids);
        prev_template_res_list.push(template_res);
    }

    // Execute curr_specs and track final states for metadata
    let mut curr_vfs_list = Vec::new();
    let mut curr_session_ids = Vec::new();
    // Map template_key -> final answers for metadata persistence
    let mut final_answers_map: HashMap<String, HashMap<String, Answer>> = HashMap::new();
    let mut curr_template_res_list = Vec::new();

    for spec in curr_specs {
        println!("  🔄 Executing curr: {} v{}", spec.key(), spec.version);
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, final_state, session_ids) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        curr_vfs_list.push(vfs);
        curr_session_ids.extend(session_ids);
        // Store the final answers for this template (includes Q&A answers)
        final_answers_map.insert(spec.key(), final_state.shared_answers);
        curr_template_res_list.push(template_res);
    }

    // PHASE 3: LAYER (merge each list into ONE VFS)
    // Horizontal layering: collect resolvers from ONLY root templates (not dependencies)
    println!(
        "\n🔀 PHASE 3: LAYER - Merging {} prev + {} curr VFS outputs",
        prev_vfs_list.len(),
        curr_vfs_list.len()
    );

    let prev_vfs = if prev_vfs_list.is_empty() {
        cyancoordinator::fs::VirtualFileSystem::new()
    } else if prev_vfs_list.len() == 1 {
        prev_vfs_list.into_iter().next().unwrap()
    } else {
        // Use resolver-aware horizontal layering
        operator.layer_merge_with_resolvers(
            &prev_vfs_list,
            &prev_template_res_list,
            coord_client,
        )?
    };

    let curr_vfs = if curr_vfs_list.is_empty() {
        cyancoordinator::fs::VirtualFileSystem::new()
    } else if curr_vfs_list.len() == 1 {
        curr_vfs_list.into_iter().next().unwrap()
    } else {
        // Use resolver-aware horizontal layering
        operator.layer_merge_with_resolvers(
            &curr_vfs_list,
            &curr_template_res_list,
            coord_client,
        )?
    };

    // PHASE 4: MERGE + WRITE
    println!("\n📝 PHASE 4: MERGE+WRITE - 3-way merge with local files");

    let local_vfs = operator.load_local_files(target_dir)?;
    let merged_vfs = operator.merge(&prev_vfs, &local_vfs, &curr_vfs)?;

    // Clean up files that were deleted during merge
    let deleted = operator.cleanup_deleted_files(target_dir, &local_vfs, &merged_vfs)?;
    if !deleted.is_empty() {
        println!("🗑️ Removed {} file(s) no longer in template", deleted.len());
    }

    operator.write_to_disk(target_dir, &merged_vfs)?;

    // Save metadata for upgraded templates only
    if !upgraded_specs.is_empty() {
        println!(
            "💾 Saving template metadata for {} upgraded templates",
            upgraded_specs.len()
        );

        for spec in upgraded_specs {
            let template_res = registry.get_template(
                spec.username.clone(),
                spec.template_name.clone(),
                Some(spec.version),
            )?;

            // Use final answers from execution (includes Q&A answers) if available,
            // otherwise fall back to spec.answers
            let final_answers = final_answers_map
                .get(&spec.key())
                .cloned()
                .unwrap_or_else(|| spec.answers.clone());

            let template_state = TemplateState::Complete(
                Cyan {
                    processors: Vec::new(),
                    plugins: Vec::new(),
                },
                final_answers,
            );

            operator.get_template_history().save_template_metadata(
                target_dir,
                &template_res,
                &template_state,
                &spec.username,
            )?;
        }
    }

    let mut all_session_ids = prev_session_ids;
    all_session_ids.extend(curr_session_ids);

    // Collect file conflicts from operator for state persistence
    let file_conflicts = operator.get_file_conflicts().to_vec();

    // Collect commands from curr template result list only
    // (prev is just the 3-way-merge baseline; its commands would be duplicates or stale)
    let all_commands =
        CompositionOperator::collect_commands_from_templates(&curr_template_res_list);

    println!("✅ Batch process complete");
    Ok((all_session_ids, file_conflicts, all_commands))
}

/// Run the cyan template generation process with automatic composition detection
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_run(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: Option<String>,
    template: TemplateVersionRes,
    coord_client: CyanCoordinatorClient,
    username: String,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // Handle the target directory
    let path = path.unwrap_or(".".to_string());
    let path_buf = PathBuf::from(&path);
    let target_dir = path_buf.as_path();
    println!("📁 Target directory: {target_dir:?}");
    fs::create_dir_all(target_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Create all components for dependency injection at the highest level
    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    let merger = Box::new(GitLikeMerger::new(debug, 50));
    let writer = Box::new(DiskFileWriter);

    // Setup services with explicit dependencies
    let template_history = Box::new(DefaultTemplateHistory::new());
    let template_executor = Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

    // Create the TemplateOperator
    let template_operator = TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
        registry_client.clone(),
    );

    // Check template history to determine update scenario
    let update_type =
        DefaultTemplateHistory::new().check_template_history(target_dir, &template, &username)?;

    // Create composition-specific components (needed for both single and composition templates
    // to support batch processing when adding to existing projects)
    let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));

    // Create the CompositionOperator with client for resolver-aware layering
    // Clone the client since we also need it for batch_process
    let mut composition_operator = CompositionOperator::with_client(
        template_operator,
        dependency_resolver,
        coord_client.clone(),
    );

    // Check if this is a composition template (has dependencies)
    let is_composition = has_dependencies(&template);

    // Log template type
    if is_composition {
        if has_execution_artifacts(&template) {
            println!(
                "🔗 Template with {} dependencies and execution artifacts - using composition execution",
                template.templates.len()
            );
        } else {
            println!(
                "🔗 Template group with {} dependencies (no execution artifacts) - using composition execution",
                template.templates.len()
            );
        }
    } else if has_execution_artifacts(&template) {
        println!("📦 Single template with execution artifacts - using single template execution");
    } else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid template: no dependencies and no execution artifacts. Templates must have either dependencies or execution artifacts.",
        )) as Box<dyn Error + Send>);
    }

    // Create the manager for composable spec operations
    let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

    // Load the current state (may be empty for new projects)
    let state_file_path = target_dir.join(".cyan_state.yaml");
    let state = DefaultStateManager::new()
        .load_state_file(&state_file_path)
        .unwrap_or_default();

    // Build specs using composable primitives
    let mut prev_specs = manager.get(&state);
    sort_specs(&mut prev_specs);

    let (prev_specs, curr_specs, upgraded_specs): (
        Vec<TemplateSpec>,
        Vec<TemplateSpec>,
        Vec<TemplateSpec>,
    ) = match update_type {
        TemplateUpdateType::NewTemplate => {
            // New template being added
            let new_spec = TemplateSpec::new_template(
                username.clone(),
                template.template.name.clone(),
                template.principal.version,
            );

            let curr: Vec<_> = prev_specs
                .iter()
                .cloned()
                .chain(iter::once(new_spec.clone()))
                .collect();

            // The new template is the only one that needs metadata saved
            (prev_specs, curr, vec![new_spec])
        }
        TemplateUpdateType::UpgradeTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Upgrade: prev uses old version, curr uses new version
            let target_key = format!("{}/{}", username, template.template.name);

            // Build prev_specs with the OLD version for the target template
            let prev: Vec<_> = state
                .templates
                .iter()
                .filter(|(_, s)| s.active)
                .filter_map(|(key, s)| {
                    let (u, t) = parse_template_key(key)?;
                    let entry = s.history.last()?;

                    if key == &target_key {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            previous_version,
                            previous_answers.clone(),
                            previous_states.clone(),
                            entry.time,
                        ))
                    } else {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            entry.version,
                            entry.answers.clone(),
                            entry.deterministic_states.clone(),
                            entry.time,
                        ))
                    }
                })
                .collect();

            // Build curr_specs with the NEW version for the target template
            let curr: Vec<_> = state
                .templates
                .iter()
                .filter(|(_, s)| s.active)
                .filter_map(|(key, s)| {
                    let (u, t) = parse_template_key(key)?;
                    let entry = s.history.last()?;

                    if key == &target_key {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            template.principal.version,
                            previous_answers.clone(),
                            previous_states.clone(),
                            entry.time,
                        ))
                    } else {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            entry.version,
                            entry.answers.clone(),
                            entry.deterministic_states.clone(),
                            entry.time,
                        ))
                    }
                })
                .collect();

            // Explicitly track what changed
            let upgraded: Vec<_> = curr
                .iter()
                .filter(|s| s.key() == target_key)
                .cloned()
                .collect();

            (prev, curr, upgraded)
        }
        TemplateUpdateType::RerunTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Rerun: prev uses old answers, curr uses empty answers (triggers Q&A)
            let target_key = format!("{}/{}", username, template.template.name);

            // Build prev_specs with the OLD answers for the target template
            let prev: Vec<_> = state
                .templates
                .iter()
                .filter(|(_, s)| s.active)
                .filter_map(|(key, s)| {
                    let (u, t) = parse_template_key(key)?;
                    let entry = s.history.last()?;

                    if key == &target_key {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            previous_version,
                            previous_answers.clone(),
                            previous_states.clone(),
                            entry.time,
                        ))
                    } else {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            entry.version,
                            entry.answers.clone(),
                            entry.deterministic_states.clone(),
                            entry.time,
                        ))
                    }
                })
                .collect();

            // Use manager.reset() to clear answers for curr_specs
            let curr = manager.reset(prev.clone());

            // Explicitly track what changed (the rerun target)
            let upgraded: Vec<_> = curr
                .iter()
                .filter(|s| s.key() == target_key)
                .cloned()
                .collect();

            (prev, curr, upgraded)
        }
    };

    // Convert upgraded_specs to references for batch_process
    let upgraded_refs: Vec<&TemplateSpec> = upgraded_specs.iter().collect();

    // Execute unified batch processing
    let (session_ids, file_conflicts, commands) = batch_process(
        &prev_specs,
        &curr_specs,
        &upgraded_refs,
        target_dir,
        &registry_client,
        &coord_client,
        &mut composition_operator,
    )?;

    // Persist file conflicts to state file (always update to clear stale entries)
    let state_manager = DefaultStateManager::new();
    let mut cyan_state = state_manager
        .load_state_file(&state_file_path)
        .unwrap_or_default();
    let conflicts_count = file_conflicts.len();
    cyan_state.file_conflicts = file_conflicts;
    state_manager.save_state_file(&cyan_state, &state_file_path)?;
    if conflicts_count > 0 {
        println!("📝 Saved {conflicts_count} file conflict(s) to state");
    }

    // Execute commands if any were collected
    if !commands.is_empty() {
        println!(
            "\n⚡ Executing {} post-template command(s)...",
            commands.len()
        );
        let exec_result = CommandExecutor::execute_commands(&commands, target_dir)?;
        if exec_result.aborted {
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution aborted: {}/{} succeeded, {}/{} failed before abort",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
    }

    Ok(session_ids)
}

/// Parse template key from the update module
fn parse_template_key(template_key: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = template_key.split('/').collect();
    (parts.len() == 2).then(|| (parts[0].to_string(), parts[1].to_string()))
}
