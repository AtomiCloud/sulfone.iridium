use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::rerun::RerunContext;
use cyancoordinator::operations::upgrade::UpgradeContext;
use cyancoordinator::operations::{create_new_template, rerun_template, upgrade_template};
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateHistory, TemplateUpdateType};

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
    match update_type {
        TemplateUpdateType::NewTemplate => {
            // Scenario 1: No previous template matching the current template
            create_new_template(
                session_id_generator,
                &template,
                target_dir,
                &template_executor,
                &template_history,
                &vfs,
                &username,
            )
        }
        TemplateUpdateType::UpgradeTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Scenario 2: Previous template matching the current template exists, but a different version
            let context = UpgradeContext {
                session_id_generator,
                template: &template,
                target_dir,
                template_executor: &template_executor,
                template_history: &template_history,
                vfs: &vfs,
                username: &username,
                previous_version,
                previous_answers,
                previous_states,
                get_previous_template: get_previous_template_ver,
            };

            upgrade_template(context)
        }
        TemplateUpdateType::RerunTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Scenario 3: Previous template matching the current template exists, with the same version
            let context = RerunContext {
                session_id_generator,
                template: &template,
                target_dir,
                template_executor: &template_executor,
                template_history: &template_history,
                vfs: &vfs,
                username: &username,
                previous_version,
                previous_answers,
                previous_states,
                get_previous_template: get_previous_template_ver,
            };

            rerun_template(context)
        }
    }
}
