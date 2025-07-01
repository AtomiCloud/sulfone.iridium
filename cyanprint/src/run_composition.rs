use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::composition::{
    CompositionOperator, DefaultDependencyResolver, DefaultVfsLayerer,
};
use cyancoordinator::operations::TemplateOperator;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateHistory, TemplateUpdateType};

/// Run the cyan template generation process with composition support
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_run_composition(
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
    println!("📁 Target directory: {:?}", target_dir);
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

    // Create composition-specific components
    let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));
    let vfs_layerer = Box::new(DefaultVfsLayerer);

    // Create the CompositionOperator
    let composition_operator =
        CompositionOperator::new(template_operator, dependency_resolver, vfs_layerer);

    // Check template history to determine update scenario
    let update_type =
        DefaultTemplateHistory::new().check_template_history(target_dir, &template, &username)?;

    // Handle different update scenarios using composition-aware operations
    match update_type {
        TemplateUpdateType::NewTemplate => {
            // Scenario 1: No previous template matching the current template
            composition_operator.create_new_composition(&template, target_dir, &username)
        }
        TemplateUpdateType::UpgradeTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Scenario 2: Previous template matching the current template exists, but a different version
            composition_operator.upgrade_composition(
                &template,
                target_dir,
                &username,
                previous_version,
                previous_answers,
                previous_states,
            )
        }
        TemplateUpdateType::RerunTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Scenario 3: Previous template matching the current template exists, with the same version
            composition_operator.rerun_composition(
                &template,
                target_dir,
                &username,
                previous_version,
                previous_answers,
                previous_states,
            )
        }
    }
}

/// Check if a template has dependencies (composition support needed)
pub fn template_has_dependencies(template: &TemplateVersionRes) -> bool {
    !template.templates.is_empty()
}

/// Wrapper function that automatically chooses between single-template and composition execution
pub fn cyan_run_auto(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: Option<String>,
    template: TemplateVersionRes,
    coord_client: CyanCoordinatorClient,
    username: String,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    if template_has_dependencies(&template) {
        println!(
            "🔗 Template has {} dependencies - using composition execution",
            template.templates.len()
        );
        cyan_run_composition(
            session_id_generator,
            path,
            template,
            coord_client,
            username,
            registry_client,
            debug,
        )
    } else {
        println!("📦 Template has no dependencies - using single template execution");
        crate::run::cyan_run(
            session_id_generator,
            path,
            template,
            coord_client,
            username,
            registry_client,
            debug,
        )
    }
}
