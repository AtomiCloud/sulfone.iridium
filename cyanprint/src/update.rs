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

/// Update all templates in a project to their latest versions
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_update(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: String,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
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
                "🔄 Updating template: {}/{} from version {}",
                username, template_name, latest_entry.version
            );

            // Fetch the latest version of the template
            match registry_client.get_template(username.clone(), template_name.clone(), None) {
                Ok(latest_template) => {
                    // If the latest version is the same as the current version, skip
                    if latest_template.principal.version == latest_entry.version {
                        println!(
                            "✅ Template {}/{} is already at the latest version ({})",
                            username, template_name, latest_entry.version
                        );
                        continue;
                    }

                    println!(
                        "🔄 Upgrading {}/{} from version {} to {}",
                        username,
                        template_name,
                        latest_entry.version,
                        latest_template.principal.version
                    );

                    // Helper function to get previous template version
                    let get_previous_template_ver = |previous_version: i64| -> Result<
                        TemplateVersionRes,
                        Box<dyn Error + Send>,
                    > {
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
                        &latest_template,
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
                                username, template_name, latest_template.principal.version
                            );
                            all_session_ids.extend(session_ids);
                        }
                        Err(e) => {
                            eprintln!("❌ Failed to upgrade {}/{}: {}", username, template_name, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "❌ Failed to fetch latest version of {}/{}: {}",
                        username, template_name, e
                    );
                }
            }
        }
    }

    Ok(all_session_ids)
}
