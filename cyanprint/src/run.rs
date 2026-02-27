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

use crate::update::parse_template_key;

/// Check if a template has execution artifacts (Docker properties)
fn has_execution_artifacts(template: &TemplateVersionRes) -> bool {
    template.principal.properties.is_some()
}

/// Check if a template has dependencies
fn has_dependencies(template: &TemplateVersionRes) -> bool {
    !template.templates.is_empty()
}

/// Handle batch creation for existing projects
/// Re-runs all existing templates with stored answers + adds new template
/// Uses batch VFS layering: collects all VFS outputs first, then does ONE merge and write
fn batch_create_for_existing_project(
    composition_operator: &CompositionOperator,
    target_dir: &Path,
    new_template: &TemplateVersionRes,
    username: &str,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    println!("🔄 Existing project detected - re-running all templates with batch layering");

    // 1. Read existing state
    let state_file_path = target_dir.join(".cyan_state.yaml");
    let state = DefaultStateManager::new().load_state_file(&state_file_path)?;

    if state.templates.is_empty() {
        // No existing templates - just run the new template normally
        println!("📦 No existing templates - running as new project");
        return composition_operator.create_new_composition(new_template, target_dir, username);
    }

    // 2. Collect VFS for existing templates, sorted by time for LWW semantics
    let mut existing_templates: Vec<_> = state
        .templates
        .iter()
        .filter(|(_, s)| s.active)
        .filter_map(|(key, template_state)| {
            template_state
                .history
                .last()
                .map(|entry| (key.clone(), entry.clone()))
        })
        .filter_map(|(key, entry)| parse_template_key(&key).map(|(u, n)| (key, u, n, entry)))
        .collect();

    // Sort by time (oldest first) for LWW semantics
    existing_templates.sort_by(|a, b| a.3.time.cmp(&b.3.time));

    println!(
        "📋 Re-running {} existing templates in order (sorted by time for LWW semantics)",
        existing_templates.len()
    );

    // 3. COLLECT phase: Collect VFS for all existing templates
    let mut all_vfs_collections = Vec::new();

    for (template_key, tmpl_username, tmpl_name, latest_entry) in &existing_templates {
        println!("  🔄 Re-running existing template: {template_key}");

        // Fetch the template at the stored version
        let existing_template = registry_client.get_template(
            tmpl_username.clone(),
            tmpl_name.clone(),
            Some(latest_entry.version),
        )?;

        // Collect VFS using stored answers
        // Note: CompositionOperator.collect_create_vfs handles both single templates and compositions
        let collection = composition_operator.collect_create_vfs(
            &existing_template,
            Some(&CompositionState {
                shared_answers: latest_entry.answers.clone(),
                shared_deterministic_states: latest_entry.deterministic_states.clone(),
                execution_order: Vec::new(),
            }),
        )?;

        all_vfs_collections.push(collection);
    }

    // 4. COLLECT phase: Collect VFS for the new template
    println!(
        "📦 Collecting VFS for new template: {}",
        new_template.template.name
    );

    // Merge shared state from all existing templates
    let mut merged_state = CompositionState::new();
    for collection in &all_vfs_collections {
        for (key, value) in &collection.final_state.shared_answers {
            merged_state
                .shared_answers
                .insert(key.clone(), value.clone());
        }
        for (key, value) in &collection.final_state.shared_deterministic_states {
            merged_state
                .shared_deterministic_states
                .insert(key.clone(), value.clone());
        }
    }

    let new_collection =
        composition_operator.collect_create_vfs(new_template, Some(&merged_state))?;
    all_vfs_collections.push(new_collection);

    // 5. MERGE phase: Layer all VFS outputs and do ONE 3-way merge
    println!(
        "\n🔀 MERGE phase: Layering {} VFS outputs and performing 3-way merge",
        all_vfs_collections.len()
    );
    let (merged_vfs, all_session_ids) = composition_operator.layer_and_merge_vfs(
        &all_vfs_collections,
        target_dir,
        false, // is_upgrade = false (this is a create with base from existing templates)
    )?;

    // 6. WRITE phase: Write merged VFS to disk ONCE
    println!("\n📝 WRITE phase: Writing merged VFS to disk");
    composition_operator
        .get_vfs()
        .write_to_disk(target_dir, &merged_vfs)?;

    // 7. Save metadata for the new template only (existing templates already have their metadata)
    println!("💾 Saving template metadata");
    let template_state = TemplateState::Complete(
        Cyan {
            processors: Vec::new(),
            plugins: Vec::new(),
        },
        all_vfs_collections
            .last()
            .expect("Should have at least one collection")
            .final_state
            .shared_answers
            .clone(),
    );

    composition_operator
        .get_template_history()
        .save_template_metadata(target_dir, new_template, &template_state, username)?;

    println!(
        "\n✅ Batch create complete: {} templates processed",
        all_vfs_collections.len()
    );
    Ok(all_session_ids)
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
                // Truly new project
                composition_operator.create_new_composition(&template, target_dir, &username)
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
