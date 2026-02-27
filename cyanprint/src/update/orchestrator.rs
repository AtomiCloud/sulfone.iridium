use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyanregistry::http::client::CyanRegistryClient;

use super::batch_processor::{BatchProcessor, TemplateNonUpgradeInfo, TemplateProcessInfo};
use super::operator_factory::OperatorFactory;
use super::utils::parse_template_key;

/// Main orchestrator for the template update process
pub struct UpdateOrchestrator;

impl UpdateOrchestrator {
    /// Update all templates in a project to their latest versions with automatic composition detection
    /// Uses batch VFS layering: collects all VFS outputs first, then does ONE merge and write
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
        println!("🔍 Reading template state from: {state_file_path:?}");
        let state = DefaultStateManager::new().load_state_file(&state_file_path)?;

        if state.templates.is_empty() {
            println!("⚠️ No templates found in state file");
            return Ok(Vec::new());
        }

        // 2. Collect all templates with their history entries, sorted by time for LWW semantics
        // CRITICAL: We keep ONE sorted list throughout to preserve LWW ordering
        let mut templates_to_process: Vec<_> = state
            .templates
            .iter()
            .filter(|(_, state)| state.active)
            .filter_map(|(template_key, template_state)| {
                template_state
                    .history
                    .last()
                    .map(|entry| (template_key.clone(), entry.clone()))
            })
            .filter_map(|(template_key, latest_entry)| {
                let key_ref = &template_key;
                parse_template_key(key_ref)
                    .map(|(username, template_name)| {
                        (template_key.clone(), username, template_name, latest_entry)
                    })
                    .or_else(|| {
                        println!("⚠️ Invalid template key format: {key_ref}");
                        None
                    })
            })
            .collect();

        // Sort by time (oldest first) for LWW semantics - later templates overwrite earlier ones
        templates_to_process.sort_by(|a, b| a.3.time.cmp(&b.3.time));

        println!(
            "📋 Processing {} templates in order (sorted by time for LWW semantics)",
            templates_to_process.len()
        );

        // 3. RESOLVE phase: Determine which templates need upgrading vs staying at current version
        // We maintain a SINGLE list with time preserved to keep LWW ordering correct
        let mut process_list: Vec<(chrono::DateTime<chrono::Utc>, TemplateProcessInfo)> =
            Vec::new();

        for (template_key, username, template_name, latest_entry) in templates_to_process {
            match BatchProcessor::collect_template_upgrades(
                &registry_client,
                target_dir,
                &username,
                &template_name,
                &latest_entry,
                interactive,
            ) {
                Ok(Some(upgrade_info)) => {
                    println!("  ✅ {} -> v{}", template_key, upgrade_info.target_version);
                    // Preserve the original time for correct LWW ordering
                    let time = upgrade_info.latest_entry.time;
                    process_list.push((time, TemplateProcessInfo::Upgrade(upgrade_info)));
                }
                Ok(None) => {
                    // Template doesn't need upgrading - but we still need to collect its VFS for LWW layering
                    println!("  📌 {} staying at v{}", template_key, latest_entry.version);

                    // Fetch the current template from registry
                    // CRITICAL: Abort on fetch failure to maintain VFS consistency
                    let current_template = registry_client
                        .get_template(
                            username.clone(),
                            template_name.clone(),
                            Some(latest_entry.version),
                        )
                        .map_err(|e| {
                            Box::new(std::io::Error::other(format!(
                                "Failed to fetch template {template_key}: {e}"
                            ))) as Box<dyn Error + Send>
                        })?;

                    // Preserve the original time for correct LWW ordering
                    let time = latest_entry.time;
                    process_list.push((
                        time,
                        TemplateProcessInfo::NonUpgrade(TemplateNonUpgradeInfo {
                            username,
                            template_name,
                            current_version: latest_entry.version,
                            current_template,
                            answers: latest_entry.answers.clone(),
                            deterministic_states: latest_entry.deterministic_states.clone(),
                        }),
                    ));
                }
                Err(e) => {
                    eprintln!("  ❌ Error processing {template_key}: {e}");
                    return Err(e);
                }
            }
        }

        // 4. Re-sort the process list by time to ensure correct LWW ordering
        // (The list should already be sorted, but this guarantees correctness)
        process_list.sort_by(|a, b| a.0.cmp(&b.0));

        if process_list.is_empty() {
            println!("✅ No templates to process");
            return Ok(Vec::new());
        }

        let upgrade_count = process_list.iter().filter(|(_, p)| p.is_upgrade()).count();
        let non_upgrade_count = process_list.len() - upgrade_count;

        println!(
            "\n📦 COLLECT phase: Collecting VFS outputs for {} templates in LWW order ({} upgrading, {} at current version)",
            process_list.len(),
            upgrade_count,
            non_upgrade_count
        );

        // 5. COLLECT phase: Collect VFS outputs for ALL templates in LWW order
        // CRITICAL: We iterate in sorted time order, not separately by upgrade status
        let mut vfs_collections = Vec::new();
        let mut upgraded_vfs_collections = Vec::new(); // Only for templates being upgraded (for metadata save)
        let mut upgrades_for_metadata = Vec::new(); // Track upgrade info for metadata save

        for (_time, process_info) in &process_list {
            let collection = match process_info {
                TemplateProcessInfo::Upgrade(upgrade_info) => {
                    let collection =
                        BatchProcessor::collect_template_vfs(&composition_operator, upgrade_info)?;
                    upgraded_vfs_collections.push(collection.clone());
                    upgrades_for_metadata.push(upgrade_info.clone());
                    collection
                }
                TemplateProcessInfo::NonUpgrade(non_upgrade_info) => {
                    BatchProcessor::collect_non_upgrade_vfs(
                        &composition_operator,
                        non_upgrade_info,
                    )?
                }
            };
            vfs_collections.push(collection);
        }

        // 6. MERGE phase: Layer all VFS outputs and do ONE 3-way merge
        println!("\n🔀 MERGE phase: Layering VFS outputs and performing 3-way merge");
        let (merged_vfs, all_session_ids) = composition_operator.layer_and_merge_vfs(
            &vfs_collections,
            target_dir,
            upgrade_count > 0, // is_upgrade only if there are upgrades
        )?;

        // 7. WRITE phase: Write merged VFS to disk ONCE
        println!("\n📝 WRITE phase: Writing merged VFS to disk");
        composition_operator
            .get_vfs()
            .write_to_disk(target_dir, &merged_vfs)?;

        // 8. Save metadata ONLY for upgraded templates (not for non-upgraded ones)
        if !upgrades_for_metadata.is_empty() {
            println!(
                "💾 Saving template metadata for {} upgraded templates",
                upgrades_for_metadata.len()
            );
            BatchProcessor::save_batch_metadata(
                &composition_operator,
                target_dir,
                &upgraded_vfs_collections,
                &upgrades_for_metadata,
            )?;
        }

        if upgrade_count == 0 {
            println!("\n✅ All templates verified - no upgrades needed");
        } else {
            println!("\n✅ Batch upgrade complete: {upgrade_count} templates upgraded");
        }
        Ok(all_session_ids)
    }
}
