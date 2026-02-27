use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::operations::composition::CompositionOperator;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;

use super::operator_factory::OperatorFactory;
use super::spec::{
    TemplateSpec, build_curr_specs_for_update, build_prev_specs, classify_specs_by_upgrade,
    sort_specs_by_time,
};

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

        // Create the composition operator
        let composition_operator = OperatorFactory::create_composition_operator(
            session_id_generator,
            coord_client,
            registry_client.clone(),
            debug,
        );

        // PHASE 1: BUILD SPEC LISTS
        println!(
            "🔍 PHASE 1: Reading template state from: {:?}",
            target_dir.join(".cyan_state.yaml")
        );
        let state_file_path = target_dir.join(".cyan_state.yaml");
        let cyan_state = DefaultStateManager::new()
            .load_state_file(&state_file_path)
            .map_err(|e| {
                Box::new(std::io::Error::other(format!("Failed to load state: {e}")))
                    as Box<dyn Error + Send>
            })?;

        if cyan_state.templates.is_empty() {
            println!("⚠️ No templates found in state file");
            return Ok(Vec::new());
        }

        // Build prev_specs from state
        let mut prev_specs = build_prev_specs(&cyan_state);

        if prev_specs.is_empty() {
            println!("⚠️ No active templates to update");
            return Ok(Vec::new());
        }

        println!("📋 Found {} active templates", prev_specs.len());

        // Build curr_specs for update (with version upgrades)
        let mut curr_specs =
            build_curr_specs_for_update(prev_specs.clone(), &registry_client, interactive)?;

        // Sort both lists by installation time for consistent LWW ordering
        sort_specs_by_time(&mut prev_specs);
        sort_specs_by_time(&mut curr_specs);

        // Classify specs to identify which ones are actually upgraded
        let (upgraded_specs, _) = classify_specs_by_upgrade(&prev_specs, &curr_specs);

        println!(
            "📊 Template classification: {} total, {} being upgraded",
            curr_specs.len(),
            upgraded_specs.len()
        );

        // PHASE 2-4: BATCH PROCESS
        let session_ids = batch_process(
            &prev_specs,
            &curr_specs,
            &upgraded_specs,
            target_dir,
            &registry_client,
            &composition_operator,
        )?;

        println!("✅ Batch update complete");
        Ok(session_ids)
    }
}

/// Unified batch processing for update commands.
/// Handles MAP, LAYER, and MERGE+WRITE phases.
fn batch_process(
    prev_specs: &[TemplateSpec],
    curr_specs: &[TemplateSpec],
    upgraded_specs: &[&TemplateSpec],
    target_dir: &Path,
    registry: &CyanRegistryClient,
    operator: &CompositionOperator,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // PHASE 2: MAP (execute each template spec → VFS)
    println!(
        "\n📦 PHASE 2: MAP - Executing {} prev + {} curr templates",
        prev_specs.len(),
        curr_specs.len()
    );

    // Execute prev_specs
    let mut prev_vfs_list = Vec::new();
    let mut prev_session_ids = Vec::new();

    for spec in prev_specs {
        println!(
            "  🔄 Executing prev: {} v{}",
            spec.template_key(),
            spec.version
        );
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, _final_state, session_ids) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        prev_vfs_list.push(vfs);
        prev_session_ids.extend(session_ids);
    }

    // Execute curr_specs and track final states for metadata
    let mut curr_vfs_list = Vec::new();
    let mut curr_session_ids = Vec::new();
    // Map template_key -> final answers for metadata persistence
    let mut final_answers_map: HashMap<String, HashMap<String, Answer>> = HashMap::new();

    for spec in curr_specs {
        println!(
            "  🔄 Executing curr: {} v{}",
            spec.template_key(),
            spec.version
        );
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, final_state, session_ids) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        curr_vfs_list.push(vfs);
        curr_session_ids.extend(session_ids);
        // Store the final answers for this template (includes Q&A answers)
        final_answers_map.insert(spec.template_key(), final_state.shared_answers);
    }

    // PHASE 3: LAYER (merge each list into ONE VFS)
    println!(
        "\n🔀 PHASE 3: LAYER - Merging {} prev + {} curr VFS outputs",
        prev_vfs_list.len(),
        curr_vfs_list.len()
    );

    let prev_vfs = operator.layer_merge(&prev_vfs_list)?;
    let curr_vfs = operator.layer_merge(&curr_vfs_list)?;

    // PHASE 4: MERGE + WRITE
    println!("\n📝 PHASE 4: MERGE+WRITE - 3-way merge with local files");

    let local_vfs = operator.load_local_files(target_dir)?;
    let merged_vfs = operator.merge(&prev_vfs, &local_vfs, &curr_vfs)?;

    operator.write_to_disk(target_dir, &merged_vfs)?;

    // Save metadata for upgraded templates only
    if !upgraded_specs.is_empty() {
        println!(
            "💾 Saving template metadata for {} upgraded templates",
            upgraded_specs.len()
        );

        for spec in upgraded_specs {
            let template_res = registry.get_template(
                spec.username.clone(),
                spec.template_name.clone(),
                Some(spec.version),
            )?;

            // Use final answers from execution (includes Q&A answers) if available,
            // otherwise fall back to spec.answers
            let final_answers = final_answers_map
                .get(&spec.template_key())
                .cloned()
                .unwrap_or_else(|| spec.answers.clone());

            let template_state = TemplateState::Complete(
                Cyan {
                    processors: Vec::new(),
                    plugins: Vec::new(),
                },
                final_answers,
            );

            operator.get_template_history().save_template_metadata(
                target_dir,
                &template_res,
                &template_state,
                &spec.username,
            )?;
        }
    }

    let mut all_session_ids = prev_session_ids;
    all_session_ids.extend(curr_session_ids);

    println!("✅ Batch process complete");
    Ok(all_session_ids)
}
