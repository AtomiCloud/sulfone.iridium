use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::composition::{
    CompositionOperator, DefaultDependencyResolver, DefaultVfsLayerer,
};
use cyancoordinator::operations::{TemplateOperations, TemplateOperator};
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateHistory, TemplateUpdateType};

/// Check if a template has execution artifacts (Docker properties)
fn has_execution_artifacts(template: &TemplateVersionRes) -> bool {
    template.principal.properties.is_some()
}

/// Check if a template has dependencies  
fn has_dependencies(template: &TemplateVersionRes) -> bool {
    !template.templates.is_empty()
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

    // Auto-detect template type and use appropriate execution path
    if !has_dependencies(&template) {
        // Single template (no dependencies)
        if has_execution_artifacts(&template) {
            println!(
                "📦 Single template with execution artifacts - using single template execution"
            );
        } else {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid template: no dependencies and no execution artifacts. Templates must have either dependencies or execution artifacts.",
            )) as Box<dyn Error + Send>);
        }

        // Handle different update scenarios for single templates
        match update_type {
            TemplateUpdateType::NewTemplate => {
                template_operator.create_new(&template, target_dir, &username)
            }
            TemplateUpdateType::UpgradeTemplate {
                previous_version,
                previous_answers,
                previous_states,
            } => template_operator.upgrade(
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
            } => template_operator.rerun(
                &template,
                target_dir,
                &username,
                previous_version,
                previous_answers,
                previous_states,
            ),
        }
    } else {
        // Template with dependencies (composition)
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

        // Create composition-specific components
        let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));
        let vfs_layerer = Box::new(DefaultVfsLayerer);

        // Create the CompositionOperator
        let composition_operator =
            CompositionOperator::new(template_operator, dependency_resolver, vfs_layerer);

        // Handle different update scenarios for compositions
        match update_type {
            TemplateUpdateType::NewTemplate => {
                composition_operator.create_new_composition(&template, target_dir, &username)
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
}
