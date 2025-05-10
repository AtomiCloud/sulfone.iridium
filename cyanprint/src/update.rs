use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::fs::{
    DiskFileLoader, DiskFileWriter, FileLoader, FileMerger, FileUnpacker, FileWriter,
    GitLikeMerger, TarGzUnpacker,
};
use cyancoordinator::operations::{TemplateOperations, TemplateOperator};
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyancoordinator::template::{DefaultTemplateExecutor, DefaultTemplateHistory};
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use inquire::Select;

/// Update all templates in a project to their latest versions
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_update(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: String,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
    interactive: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // Handle the target directory
    let path_buf = PathBuf::from(&path);
    let target_dir = path_buf.as_path();
    println!("📁 Target directory: {:?}", target_dir);

    // Create a StateManager and use it to load the state file
    let state_file_path = target_dir.join(".cyan_state.yaml");
    let state_manager = DefaultStateManager::new();

    // Read the state file using the StateManager
    println!("🔍 Reading template state from: {:?}", state_file_path);
    let state = state_manager.load_state_file(&state_file_path)?;

    if state.templates.is_empty() {
        println!("⚠️ No templates found in state file");
        return Ok(Vec::new());
    }

    // Create all components for dependency injection at the highest level
    let unpacker: Box<dyn FileUnpacker> = Box::new(TarGzUnpacker);
    let loader: Box<dyn FileLoader> = Box::new(DiskFileLoader);
    let merger: Box<dyn FileMerger> = Box::new(GitLikeMerger::new(debug, 50));
    let writer: Box<dyn FileWriter> = Box::new(DiskFileWriter);

    // Setup services with explicit dependencies
    let template_history = Box::new(DefaultTemplateHistory::new());
    let template_executor = Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

    // Create the TemplateOperator with all dependencies
    let template_operator = TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
    );

    let mut all_session_ids = Vec::new();

    // Iterate through each template in the state file
    for (template_key, template_state) in state.templates.iter() {
        // Skip inactive templates
        if !template_state.active {
            println!("⏩ Skipping inactive template: {}", template_key);
            continue;
        }

        // Get the last entry in the history
        if let Some(latest_entry) = template_state.history.last() {
            // Parse the template key to get the username and template name
            let parts: Vec<&str> = template_key.split('/').collect();
            if parts.len() != 2 {
                println!("⚠️ Invalid template key format: {}", template_key);
                continue;
            }

            let username = parts[0].to_string();
            let template_name = parts[1].to_string();

            println!(
                "🔄 Processing template: {}/{} current version: {}",
                username, template_name, latest_entry.version
            );

            // Fetch the latest version to check if update is needed
            let latest_template =
                match registry_client.get_template(username.clone(), template_name.clone(), None) {
                    Ok(template) => template,
                    Err(e) => {
                        eprintln!(
                            "❌ Failed to fetch latest version of {}/{}: {}",
                            username, template_name, e
                        );
                        continue;
                    }
                };

            // If the latest version is the same as the current version and not in interactive mode, skip
            if latest_template.principal.version == latest_entry.version && !interactive {
                println!(
                    "✅ Template {}/{} is already at the latest version ({})",
                    username, template_name, latest_entry.version
                );
                continue;
            }

            let target_version: i64;

            if interactive {
                // Get available versions for this template
                println!(
                    "📚 Fetching available versions for {}/{}...",
                    username, template_name
                );
                let versions =
                    fetch_template_versions(&registry_client, &username, &template_name)?;

                if versions.is_empty() {
                    println!("⚠️ No versions found for {}/{}", username, template_name);
                    continue;
                }

                // Display a list of versions to choose from and get the selected version
                match select_template_version(
                    &versions,
                    &username,
                    &template_name,
                    latest_entry.version,
                )? {
                    Some(selection) => {
                        if selection.version == latest_entry.version {
                            println!(
                                "🔄 Keeping current version {} of {}/{}",
                                latest_entry.version, username, template_name
                            );
                            continue;
                        }

                        target_version = selection.version;
                        println!(
                            "🔄 Selected version {} of {}/{}",
                            target_version, username, template_name
                        );
                    }
                    None => {
                        println!("⏩ Skipping update for {}/{}", username, template_name);
                        continue;
                    }
                }
            } else {
                target_version = latest_template.principal.version;
            }

            // Skip if the target version is the same as the current version
            if target_version == latest_entry.version {
                println!(
                    "✅ Template {}/{} keeping version {}",
                    username, template_name, latest_entry.version
                );
                continue;
            }

            println!(
                "🔄 Upgrading {}/{} from version {} to {}",
                username, template_name, latest_entry.version, target_version
            );

            // Fetch the target template version
            let target_template = match registry_client.get_template(
                username.clone(),
                template_name.clone(),
                Some(target_version),
            ) {
                Ok(template) => template,
                Err(e) => {
                    eprintln!(
                        "❌ Failed to fetch version {} of {}/{}: {}",
                        target_version, username, template_name, e
                    );
                    continue;
                }
            };

            // Helper function to get previous template version
            let get_previous_template_ver =
                |previous_version: i64| -> Result<TemplateVersionRes, Box<dyn Error + Send>> {
                    println!(
                        "🔍 Fetching template '{}/{}:{}' from registry...",
                        username, template_name, previous_version
                    );
                    let prev_template = registry_client.get_template(
                        username.clone(),
                        template_name.clone(),
                        Some(previous_version),
                    )?;
                    println!("✅ Retrieved previous template version from registry");
                    Ok(prev_template)
                };

            // Perform the upgrade
            match template_operator.upgrade(
                &target_template,
                target_dir,
                &username,
                latest_entry.version,
                latest_entry.answers.clone(),
                latest_entry.deterministic_states.clone(),
                get_previous_template_ver,
            ) {
                Ok(session_ids) => {
                    println!(
                        "✅ Successfully upgraded {}/{} to version {}",
                        username, template_name, target_version
                    );
                    all_session_ids.extend(session_ids);
                }
                Err(e) => {
                    eprintln!("❌ Failed to upgrade {}/{}: {}", username, template_name, e);
                }
            }
        }
    }

    Ok(all_session_ids)
}

/// Template version information for display in interactive mode
struct TemplateVersionInfo {
    version: i64,
    description: String,
    created_at: String,
    is_latest: bool,
}

/// Fetch available versions for a template
fn fetch_template_versions(
    registry_client: &CyanRegistryClient,
    username: &str,
    template_name: &str,
) -> Result<Vec<TemplateVersionInfo>, Box<dyn Error + Send>> {
    // In a real implementation, this would fetch the list of available versions
    // For now, let's make a simplified version that gets the latest and a few previous versions

    let mut versions = Vec::new();

    // Get the latest version first
    let latest =
        registry_client.get_template(username.to_string(), template_name.to_string(), None)?;

    let latest_version = latest.principal.version;

    // Add the latest version
    versions.push(TemplateVersionInfo {
        version: latest_version,
        description: latest.principal.description.clone(),
        created_at: latest.principal.created_at.clone(),
        is_latest: true,
    });

    // Try to get a few previous versions
    for i in 1..5 {
        let prev_version = latest_version - i;
        if prev_version <= 0 {
            break;
        }

        match registry_client.get_template(
            username.to_string(),
            template_name.to_string(),
            Some(prev_version),
        ) {
            Ok(template) => {
                versions.push(TemplateVersionInfo {
                    version: template.principal.version,
                    description: template.principal.description.clone(),
                    created_at: template.principal.created_at.clone(),
                    is_latest: false,
                });
            }
            Err(_) => {
                // Skip versions that don't exist
                continue;
            }
        }
    }

    // Sort by version descending
    versions.sort_by(|a, b| b.version.cmp(&a.version));

    Ok(versions)
}

/// Display and allow selection of a template version
fn select_template_version<'a>(
    versions: &'a [TemplateVersionInfo],
    username: &str,
    template_name: &str,
    current_version: i64,
) -> Result<Option<&'a TemplateVersionInfo>, Box<dyn Error + Send>> {
    println!(
        "\n📋 Available versions for {}/{}:",
        username, template_name
    );

    let items: Vec<String> = versions
        .iter()
        .map(|v| {
            let latest_tag = if v.is_latest { " (LATEST)" } else { "" };
            let current_tag = if v.version == current_version {
                " (CURRENT)"
            } else {
                ""
            };
            format!(
                "Version {}{}{}: {}  ({})",
                v.version, latest_tag, current_tag, v.description, v.created_at
            )
        })
        .collect();

    let prompt = format!(
        "Select version to upgrade to for {}/{} (ESC to skip)",
        username, template_name
    );

    match Select::new(&prompt, items.clone())
        .with_help_message("↑↓ to move, enter to select, ESC to skip this template")
        .prompt()
    {
        Ok(selected) => {
            // Find the index of the selected item
            let index = items.iter().position(|item| item == &selected).unwrap_or(0);
            Ok(Some(&versions[index]))
        }
        Err(_) => {
            // User cancelled, skip this template
            Ok(None)
        }
    }
}
