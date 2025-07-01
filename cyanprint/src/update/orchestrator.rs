use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyanregistry::http::client::CyanRegistryClient;

use super::operator_factory::OperatorFactory;
use super::template_processor::TemplateProcessor;
use super::utils::parse_template_key;

/// Main orchestrator for the template update process
pub struct UpdateOrchestrator;

impl UpdateOrchestrator {
    /// Update all templates in a project to their latest versions with automatic composition detection
    /// Returns all session IDs that were created and need to be cleaned up
    pub fn update_templates(
        session_id_generator: Box<dyn SessionIdGenerator>,
        path: String,
        coord_client: CyanCoordinatorClient,
        registry_client: Rc<CyanRegistryClient>,
        debug: bool,
        interactive: bool,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        let target_dir = Path::new(&path);

        // Create the composition operator (handles both single templates and compositions)
        let composition_operator = OperatorFactory::create_composition_operator(
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
                    .map(|(username, template_name)| (username, template_name, latest_entry))
                    .or_else(|| {
                        println!("⚠️ Invalid template key format: {}", template_key);
                        None
                    })
            })
            .try_fold(
                Vec::new(),
                |mut acc, (username, template_name, latest_entry)| {
                    // For each template, process upgrade
                    let session_ids = TemplateProcessor::process_template_upgrade(
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
}
