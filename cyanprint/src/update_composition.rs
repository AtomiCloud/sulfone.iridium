use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::composition::{
    CompositionOperator, DefaultDependencyResolver, DefaultVfsLayerer,
};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::state::models::TemplateHistoryEntry;
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyancoordinator::template::{DefaultTemplateExecutor, DefaultTemplateHistory};
use cyancoordinator::{fs::DefaultVfs, session::SessionIdGenerator};
use cyanregistry::http::client::CyanRegistryClient;

use crate::update::{
    fetch_all_template_versions, parse_template_key, select_version_interactive,
    TemplateVersionInfo,
};

/// Update all templates in a project with composition support
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_update_composition(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: String,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
    interactive: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    let target_dir = Path::new(&path);

    // Create the composition operator
    let composition_operator = create_composition_operator(
        session_id_generator,
        coord_client,
        registry_client.clone(),
        debug,
    );

    // 1. Read state
    let state_file_path = target_dir.join(".cyan_state.yaml");
    println!("🔍 Reading template state from: {:?}", state_file_path);
    let state = DefaultStateManager::new().load_state_file(&state_file_path)?;

    if state.templates.is_empty() {
        println!("⚠️ No templates found in state file");
        return Ok(Vec::new());
    }

    // 2. Process each template
    state
        .templates
        .iter()
        .filter(|(_, state)| state.active)
        .filter_map(|(template_key, template_state)| {
            template_state
                .history
                .last()
                .map(|entry| (template_key, entry))
        })
        .filter_map(|(template_key, latest_entry)| {
            parse_template_key(template_key)
                .map(|(username, template_name)| (username.clone(), template_name.clone(), latest_entry))
                .or_else(|| {
                    println!("⚠️ Invalid template key format: {}", template_key);
                    None
                })
        })
        .try_fold(
            Vec::new(),
            |mut acc, (username, template_name, latest_entry): (String, String, &TemplateHistoryEntry)| {
                // For each template, process upgrade with composition support
                let session_ids = process_template_upgrade_composition(
                    &registry_client,
                    &composition_operator,
                    target_dir,
                    &username,
                    &template_name,
                    latest_entry,
                    interactive,
                )?;

                acc.extend(session_ids);
                Ok(acc)
            },
        )
}

/// Create a composition operator with the given dependencies
fn create_composition_operator(
    session_id_generator: Box<dyn SessionIdGenerator>,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
) -> CompositionOperator {
    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    let merger = Box::new(GitLikeMerger::new(debug, 50));
    let writer = Box::new(DiskFileWriter);

    let template_history = Box::new(DefaultTemplateHistory::new());
    let template_executor = Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

    let template_operator = TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
        registry_client.clone(),
    );

    let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));
    let vfs_layerer = Box::new(DefaultVfsLayerer);

    CompositionOperator::new(template_operator, dependency_resolver, vfs_layerer)
}

/// Process a single template upgrade with composition support
fn process_template_upgrade_composition(
    registry_client: &CyanRegistryClient,
    composition_operator: &CompositionOperator,
    target_dir: &Path,
    username: &str,
    template_name: &str,
    latest_entry: &TemplateHistoryEntry,
    interactive: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    println!(
        "🔄 Processing template composition: {}/{} current version: {}",
        username, template_name, latest_entry.version
    );

    // a. Fetch all versions
    let all_versions = fetch_all_template_versions(registry_client, username, template_name)?;

    if all_versions.is_empty() {
        println!("⚠️ No versions found for {}/{}", username, template_name);
        return Ok(Vec::new());
    }

    // Get the latest version
    let latest_version = all_versions
        .iter()
        .max_by_key(|v| v.version)
        .expect("Should have at least one version");

    // c. If non-interactive and already at latest version, return early
    if !interactive && latest_version.version == latest_entry.version {
        println!(
            "✅ Template {}/{} is already at latest version ({})",
            username, template_name, latest_entry.version
        );
        return Ok(Vec::new());
    }

    // d. Determine target version
    let target_version = if interactive {
        select_version_interactive(username, template_name, latest_entry.version, &all_versions)?
    } else {
        latest_version.version
    };

    // Skip if version is the same
    if target_version == latest_entry.version {
        println!(
            "✅ Template {}/{} keeping version {}",
            username, template_name, latest_entry.version
        );
        return Ok(Vec::new());
    }

    // e. Perform the upgrade with composition support
    let target_version_info = all_versions
        .iter()
        .find(|v| v.version == target_version)
        .expect("Target version should exist in fetched versions");

    perform_upgrade_composition(
        registry_client,
        composition_operator,
        target_dir,
        username,
        template_name,
        latest_entry,
        target_version_info,
    )
}

/// Perform the actual upgrade with composition support
fn perform_upgrade_composition(
    registry_client: &CyanRegistryClient,
    composition_operator: &CompositionOperator,
    target_dir: &Path,
    username: &str,
    template_name: &str,
    latest_entry: &TemplateHistoryEntry,
    target_version_info: &TemplateVersionInfo,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    println!(
        "🔄 Upgrading template composition {}/{} from version {} to {}",
        username, template_name, latest_entry.version, target_version_info.version
    );

    // Fetch target template version
    let target_template = registry_client
        .get_template(
            username.to_string(),
            template_name.to_string(),
            Some(target_version_info.version),
        )
        .map_err(|e| {
            eprintln!(
                "❌ Failed to fetch version {} of {}/{}: {}",
                target_version_info.version, username, template_name, e
            );
            e
        })?;

    // Check if template has dependencies
    if target_template.templates.is_empty() {
        println!("📦 Template has no dependencies - using single template upgrade");
        // Fall back to regular template operator for non-composition templates
        // This would require access to template_operator, but for now we can use composition_operator
        // which should handle single templates correctly
    } else {
        println!(
            "🔗 Template has {} dependencies - using composition upgrade",
            target_template.templates.len()
        );
    }

    // Perform upgrade using composition operator (handles both single and composition templates)
    composition_operator
        .upgrade_composition(
            &target_template,
            target_dir,
            username,
            latest_entry.version,
            latest_entry.answers.clone(),
            latest_entry.deterministic_states.clone(),
        )
        .inspect(|session_ids| {
            println!(
                "✅ Successfully upgraded template composition {}/{} to version {} ({} sessions)",
                username,
                template_name,
                target_version_info.version,
                session_ids.len()
            );
        })
        .map_err(|e| {
            eprintln!("❌ Failed to upgrade {}/{}: {}", username, template_name, e);
            e
        })
}

/// Wrapper function that automatically chooses between single-template and composition updates
pub fn cyan_update_auto(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: String,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
    interactive: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // Always use composition-aware update since it handles both single templates and compositions
    println!("🔄 Using composition-aware update (handles both single templates and compositions)");
    cyan_update_composition(
        session_id_generator,
        path,
        coord_client,
        registry_client,
        debug,
        interactive,
    )
}
