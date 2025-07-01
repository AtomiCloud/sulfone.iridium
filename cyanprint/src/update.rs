use std::error::Error;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::session::SessionIdGenerator;
use cyanregistry::http::client::CyanRegistryClient;

// Re-export the modular update system
mod operator_factory;
mod orchestrator;
mod template_processor;
mod upgrade_executor;
mod utils;
mod version_manager;

use orchestrator::UpdateOrchestrator;

// Re-export public interface
pub use utils::{parse_template_key, SelectionError};
pub use version_manager::{format_friendly_date, select_version_interactive, TemplateVersionInfo};

/// Update all templates in a project to their latest versions with automatic composition detection
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_update(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: String,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
    interactive: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    UpdateOrchestrator::update_templates(
        session_id_generator,
        path,
        coord_client,
        registry_client,
        debug,
        interactive,
    )
}
