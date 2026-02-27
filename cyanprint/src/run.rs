use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::operations::composition::{
    CompositionOperator, CompositionState, DefaultDependencyResolver, DefaultVfsLayerer,
};
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyancoordinator::template::TemplateHistory;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateUpdateType};

use crate::update::spec::{
    TemplateSpec, build_curr_specs_for_create, build_prev_specs, sort_specs_by_time,
};

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
/// Returns session IDs for cleanup.
fn batch_process(
    prev_specs: &[TemplateSpec],
    curr_specs: &[TemplateSpec],
    new_spec: Option<&TemplateSpec>, // The new template being added (for metadata save)
    target_dir: &Path,
    registry: &CyanRegistryClient,
    operator: &CompositionOperator,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // PHASE 2: MAP (execute each template spec → VFS)
    println!(
        "\n📦 PHASE 2: MAP - Executing {} prev + {} curr templates",
        prev_specs.len(),
        curr_specs.len()
    );

    // Execute prev_specs
    let mut prev_vfs_list = Vec::new();
    let mut prev_session_ids = Vec::new();

    for spec in prev_specs {
        println!(
            "  🔄 Executing prev: {} v{}",
            spec.template_key(),
            spec.version
        );
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, _final_state, session_ids) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        prev_vfs_list.push(vfs);
        prev_session_ids.extend(session_ids);
    }

    // Execute curr_specs
    let mut curr_vfs_list = Vec::new();
    let mut curr_session_ids = Vec::new();
    // Track the last final state for metadata persistence
    let mut last_final_state: Option<CompositionState> = None;

    for spec in curr_specs {
        println!(
            "  🔄 Executing curr: {} v{}",
            spec.template_key(),
            spec.version
        );
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;

        // Each TemplateSpec carries its own answers - pass them directly
        let (vfs, final_state, session_ids) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        curr_vfs_list.push(vfs);
        curr_session_ids.extend(session_ids);
        last_final_state = Some(final_state);
    }

    // PHASE 3: LAYER (merge each list into ONE VFS)
    println!(
        "\n🔀 PHASE 3: LAYER - Merging {} prev + {} curr VFS outputs",
        prev_vfs_list.len(),
        curr_vfs_list.len()
    );

    let prev_vfs = if prev_vfs_list.is_empty() {
        cyancoordinator::fs::VirtualFileSystem::new()
    } else {
        operator.layer_merge(&prev_vfs_list)?
    };

    let curr_vfs = if curr_vfs_list.is_empty() {
        cyancoordinator::fs::VirtualFileSystem::new()
    } else {
        operator.layer_merge(&curr_vfs_list)?
    };

    // PHASE 4: MERGE + WRITE
    println!("\n📝 PHASE 4: MERGE+WRITE - 3-way merge with local files");

    let local_vfs = operator.load_local_files(target_dir)?;
    let merged_vfs = operator.merge(&prev_vfs, &local_vfs, &curr_vfs)?;

    operator.write_to_disk(target_dir, &merged_vfs)?;

    // Save metadata for the new template only (existing templates already have their metadata)
    if let Some(new_spec) = new_spec {
        println!("💾 Saving template metadata for new template");
        let template_res = registry.get_template(
            new_spec.username.clone(),
            new_spec.template_name.clone(),
            Some(new_spec.version),
        )?;

        // Use the final answers from the last executed template (includes Q&A answers)
        let final_answers = if let Some(ref final_state) = last_final_state {
            final_state.shared_answers.clone()
        } else {
            new_spec.answers.clone()
        };

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
            &new_spec.username,
        )?;
    }

    let mut all_session_ids = prev_session_ids;
    all_session_ids.extend(curr_session_ids);

    println!("✅ Batch process complete");
    Ok(all_session_ids)
}

/// Handle batch creation for existing projects
/// Re-runs all existing templates with stored answers + adds new template
/// Uses unified batch VFS processing: MAP -> LAYER -> MERGE+WRITE
fn batch_create_for_existing_project(
    composition_operator: &CompositionOperator,
    target_dir: &Path,
    new_template: &TemplateVersionRes,
    username: &str,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    println!("🔄 Existing project detected - using unified batch processing");

    // PHASE 1: BUILD SPEC LISTS
    let state_file_path = target_dir.join(".cyan_state.yaml");
    let state = DefaultStateManager::new().load_state_file(&state_file_path)?;

    if state.templates.is_empty() {
        // No existing templates - just run the new template normally
        println!("📦 No existing templates - running as new project");
        return composition_operator.create_new_composition(new_template, target_dir, username);
    }

    // Build prev_specs from existing state
    let mut prev_specs = build_prev_specs(&state);

    // Sort by installation time for LWW semantics
    sort_specs_by_time(&mut prev_specs);

    println!(
        "📋 Re-running {} existing templates in order (sorted by time for LWW semantics)",
        prev_specs.len()
    );

    // Build new template spec (answers empty - will trigger Q&A during execute)
    let new_spec = TemplateSpec::for_new_template(
        username.to_string(),
        new_template.template.name.clone(),
        new_template.principal.version,
    );

    // Build curr_specs for create
    let mut curr_specs = build_curr_specs_for_create(prev_specs.clone(), new_spec.clone());

    // Sort curr_specs by time for consistent LWW ordering
    sort_specs_by_time(&mut curr_specs);

    println!(
        "📦 Batch create: {} existing + 1 new template",
        prev_specs.len()
    );

    // PHASE 2-4: BATCH PROCESS
    batch_process(
        &prev_specs,
        &curr_specs,
        Some(&new_spec),
        target_dir,
        registry_client,
        composition_operator,
    )
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
    let vfs_layerer = Box::new(DefaultVfsLayerer);

    // Create the CompositionOperator (handles both single templates and compositions)
    let composition_operator =
        CompositionOperator::new(template_operator, dependency_resolver, vfs_layerer);

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

    // Handle different update scenarios
    // All templates (single or composition) use CompositionOperator for batch processing support
    match update_type {
        TemplateUpdateType::NewTemplate => {
            // Check if there are existing templates in the project
            let state_file_path = target_dir.join(".cyan_state.yaml");
            let state = DefaultStateManager::new().load_state_file(&state_file_path)?;

            if !state.templates.is_empty() {
                // Existing project - use batch processing to re-run all templates
                // This works for both single templates and compositions
                batch_create_for_existing_project(
                    &composition_operator,
                    target_dir,
                    &template,
                    &username,
                    &registry_client,
                )
            } else {
                // Truly new project - use unified batch processing flow
                let new_spec = TemplateSpec::for_new_template(
                    username.clone(),
                    template.template.name.clone(),
                    template.principal.version,
                );

                let prev_specs: Vec<TemplateSpec> = vec![];
                let curr_specs = vec![new_spec.clone()];

                batch_process(
                    &prev_specs,
                    &curr_specs,
                    Some(&new_spec),
                    target_dir,
                    &registry_client,
                    &composition_operator,
                )
            }
        }
        TemplateUpdateType::UpgradeTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => composition_operator.upgrade_composition(
            &template,
            target_dir,
            &username,
            previous_version,
            previous_answers,
            previous_states,
        ),
        TemplateUpdateType::RerunTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => composition_operator.rerun_composition(
            &template,
            target_dir,
            &username,
            previous_version,
            previous_answers,
            previous_states,
        ),
    }
}
