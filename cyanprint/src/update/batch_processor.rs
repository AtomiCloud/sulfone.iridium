use std::error::Error;
use std::path::Path;

use cyancoordinator::operations::composition::{CompositionOperator, TemplateVfsCollection};
use cyancoordinator::state::models::TemplateHistoryEntry;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use super::version_manager::{fetch_all_template_versions, select_version_interactive};

/// Information about a template upgrade to be performed
#[derive(Clone)]
pub struct TemplateUpgradeInfo {
    pub username: String,
    pub template_name: String,
    pub latest_entry: TemplateHistoryEntry,
    pub target_version: i64,
    pub target_template: TemplateVersionRes,
}

/// Information about a template that is NOT being upgraded (stays at current version)
/// but still needs to contribute to VFS layering
#[derive(Clone)]
pub struct TemplateNonUpgradeInfo {
    pub username: String,
    pub template_name: String,
    pub current_version: i64,
    pub current_template: TemplateVersionRes,
    pub answers: std::collections::HashMap<String, cyanprompt::domain::models::answer::Answer>,
    pub deterministic_states: std::collections::HashMap<String, String>,
}

/// Unified template processing info for maintaining LWW order
/// Using an enum allows us to keep a single sorted list of all templates
/// regardless of whether they need upgrading or not
pub enum TemplateProcessInfo {
    /// Template that needs to be upgraded to a new version
    Upgrade(TemplateUpgradeInfo),
    /// Template that stays at current version but still needs VFS collection
    NonUpgrade(TemplateNonUpgradeInfo),
}

impl TemplateProcessInfo {
    /// Check if this is an upgrade
    pub fn is_upgrade(&self) -> bool {
        matches!(self, TemplateProcessInfo::Upgrade(_))
    }
}

/// Processor for batch template upgrades
pub struct BatchProcessor;

impl BatchProcessor {
    /// Determine which templates need upgrading and fetch their target versions
    pub fn collect_template_upgrades(
        registry_client: &CyanRegistryClient,
        _target_dir: &Path,
        username: &str,
        template_name: &str,
        latest_entry: &TemplateHistoryEntry,
        interactive: bool,
    ) -> Result<Option<TemplateUpgradeInfo>, Box<dyn Error + Send>> {
        println!(
            "🔄 Processing template: {}/{} current version: {}",
            username, template_name, latest_entry.version
        );

        // Fetch all versions
        let all_versions = fetch_all_template_versions(registry_client, username, template_name)?;

        if all_versions.is_empty() {
            println!("⚠️ No versions found for {username}/{template_name}");
            return Ok(None);
        }

        // Get the latest version
        let latest_version = all_versions
            .iter()
            .max_by_key(|v| v.version)
            .expect("Should have at least one version");

        // If non-interactive and already at latest version, return early
        if !interactive && latest_version.version == latest_entry.version {
            println!(
                "✅ Template {}/{} is already at latest version ({})",
                username, template_name, latest_entry.version
            );
            return Ok(None);
        }

        // Determine target version
        let target_version = if interactive {
            select_version_interactive(
                username,
                template_name,
                latest_entry.version,
                &all_versions,
            )?
        } else {
            latest_version.version
        };

        // Skip if version is the same
        if target_version == latest_entry.version {
            println!(
                "✅ Template {}/{} keeping version {}",
                username, template_name, latest_entry.version
            );
            return Ok(None);
        }

        // Fetch target template
        let target_version_info = all_versions
            .iter()
            .find(|v| v.version == target_version)
            .expect("Target version should exist in fetched versions");

        let target_template = registry_client
            .get_template(
                username.to_string(),
                template_name.to_string(),
                Some(target_version_info.version),
            )
            .map_err(|e| {
                eprintln!(
                    "❌ Failed to fetch version {} of {}/{}: {}",
                    target_version_info.version, username, template_name, e
                );
                e
            })?;

        Ok(Some(TemplateUpgradeInfo {
            username: username.to_string(),
            template_name: template_name.to_string(),
            latest_entry: latest_entry.clone(),
            target_version,
            target_template,
        }))
    }

    /// Collect VFS outputs for a single template upgrade WITHOUT writing to disk
    pub fn collect_template_vfs(
        composition_operator: &CompositionOperator,
        upgrade_info: &TemplateUpgradeInfo,
    ) -> Result<TemplateVfsCollection, Box<dyn Error + Send>> {
        println!(
            "📦 Collecting VFS for {}/{} (v{} -> v{})",
            upgrade_info.username,
            upgrade_info.template_name,
            upgrade_info.latest_entry.version,
            upgrade_info.target_version
        );

        // Check if template has dependencies
        if upgrade_info.target_template.templates.is_empty() {
            println!("📦 Template has no dependencies - using single template collection");
        } else {
            println!(
                "🔗 Template has {} dependencies - using composition collection",
                upgrade_info.target_template.templates.len()
            );
        }

        // Collect VFS using the composition operator's batch method
        composition_operator.collect_upgrade_vfs(
            &upgrade_info.target_template,
            &upgrade_info.username,
            upgrade_info.latest_entry.version,
            upgrade_info.latest_entry.answers.clone(),
            upgrade_info.latest_entry.deterministic_states.clone(),
        )
    }

    /// Collect VFS outputs for a template that is NOT being upgraded.
    /// This template still needs to contribute to LWW layering.
    /// Uses prev_vfs=None since there's no version change - only current VFS is needed.
    pub fn collect_non_upgrade_vfs(
        composition_operator: &CompositionOperator,
        non_upgrade_info: &TemplateNonUpgradeInfo,
    ) -> Result<TemplateVfsCollection, Box<dyn Error + Send>> {
        println!(
            "📦 Collecting VFS for non-upgraded template {}/{} (v{})",
            non_upgrade_info.username,
            non_upgrade_info.template_name,
            non_upgrade_info.current_version
        );

        // Use collect_create_vfs with stored answers - this gives us only curr_vfs (no prev_vfs)
        // which is correct since this template isn't changing versions
        use cyancoordinator::operations::composition::CompositionState;

        let initial_state = CompositionState {
            shared_answers: non_upgrade_info.answers.clone(),
            shared_deterministic_states: non_upgrade_info.deterministic_states.clone(),
            execution_order: Vec::new(),
        };

        composition_operator
            .collect_create_vfs(&non_upgrade_info.current_template, Some(&initial_state))
    }

    /// Save template metadata for a batch of upgrades
    pub fn save_batch_metadata(
        composition_operator: &CompositionOperator,
        target_dir: &Path,
        collections: &[TemplateVfsCollection],
        upgrades: &[TemplateUpgradeInfo],
    ) -> Result<(), Box<dyn Error + Send>> {
        for (collection, upgrade) in collections.iter().zip(upgrades.iter()) {
            let template_state = TemplateState::Complete(
                Cyan {
                    processors: Vec::new(),
                    plugins: Vec::new(),
                },
                collection.final_state.shared_answers.clone(),
            );

            composition_operator
                .get_template_history()
                .save_template_metadata(
                    target_dir,
                    &upgrade.target_template,
                    &template_state,
                    &upgrade.username,
                )?;
        }
        Ok(())
    }
}
