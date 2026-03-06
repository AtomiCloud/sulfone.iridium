use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::{DefaultStateManager, StateReader, StateWriter};
use cyanregistry::http::client::CyanRegistryClient;

use super::operator_factory::OperatorFactory;
use super::spec::{TemplateSpec, TemplateSpecManager, sort_specs};
use crate::run::batch_process;

/// Main orchestrator for the template update process
pub struct UpdateOrchestrator;

impl UpdateOrchestrator {
    /// Update all templates in a project to their latest versions with automatic composition detection
    /// Uses unified batch VFS processing: MAP -> LAYER -> MERGE+WRITE
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

        // Create the composition operator (clone coord_client since we also need it for batch_process)
        let mut composition_operator = OperatorFactory::create_composition_operator(
            session_id_generator,
            coord_client.clone(),
            registry_client.clone(),
            debug,
        );

        // PHASE 1: BUILD SPEC LISTS
        println!(
            "🔍 PHASE 1: Reading template state from: {:?}",
            target_dir.join(".cyan_state.yaml")
        );
        let state_file_path = target_dir.join(".cyan_state.yaml");
        let state_manager = DefaultStateManager::new();
        let mut cyan_state = state_manager
            .load_state_file(&state_file_path)
            .map_err(|e| {
                Box::new(std::io::Error::other(format!("Failed to load state: {e}")))
                    as Box<dyn Error + Send>
            })?;

        if cyan_state.templates.is_empty() {
            println!("⚠️ No templates found in state file");
            return Ok(Vec::new());
        }

        // Create the manager for composable spec operations
        let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

        // Build prev_specs from state
        let mut prev_specs = manager.get(&cyan_state);

        if prev_specs.is_empty() {
            println!("⚠️ No active templates to update");
            return Ok(Vec::new());
        }

        println!("📋 Found {} active templates", prev_specs.len());

        // Build curr_specs for update (with version upgrades)
        let mut curr_specs = manager.update(prev_specs.clone(), interactive)?;

        // Sort both lists by installation time for consistent LWW ordering
        sort_specs(&mut prev_specs);
        sort_specs(&mut curr_specs);

        // Find upgraded by comparing versions
        let upgraded: Vec<TemplateSpec> = curr_specs
            .iter()
            .filter(|c| {
                prev_specs
                    .iter()
                    .find(|p| p.key() == c.key())
                    .map(|p| p.version != c.version)
                    .unwrap_or(true) // New template
            })
            .cloned()
            .collect();

        println!(
            "📊 Template classification: {} total, {} being upgraded",
            curr_specs.len(),
            upgraded.len()
        );

        // Convert to references for batch_process
        let upgraded_refs: Vec<&TemplateSpec> = upgraded.iter().collect();

        // PHASE 2-4: BATCH PROCESS
        let (session_ids, file_conflicts) = batch_process(
            &prev_specs,
            &curr_specs,
            &upgraded_refs,
            target_dir,
            &registry_client,
            &coord_client,
            &mut composition_operator,
        )?;

        // Persist file conflicts to state file (always update to clear stale entries)
        let conflicts_count = file_conflicts.len();
        cyan_state.file_conflicts = file_conflicts;
        state_manager.save_state_file(&cyan_state, &state_file_path)?;
        if conflicts_count > 0 {
            println!("📝 Saved {conflicts_count} file conflict(s) to state");
        }

        println!("✅ Batch update complete");
        Ok(session_ids)
    }
}
