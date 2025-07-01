use std::error::Error;
use std::path::Path;

use cyancoordinator::operations::composition::CompositionOperator;
use cyancoordinator::state::models::TemplateHistoryEntry;
use cyanregistry::http::client::CyanRegistryClient;

use super::upgrade_executor::UpgradeExecutor;
use super::version_manager::{fetch_all_template_versions, select_version_interactive};

/// Processor for handling individual template upgrades
pub struct TemplateProcessor;

impl TemplateProcessor {
    /// Process a single template upgrade with automatic composition detection
    pub fn process_template_upgrade(
        registry_client: &CyanRegistryClient,
        composition_operator: &CompositionOperator,
        target_dir: &Path,
        username: &str,
        template_name: &str,
        latest_entry: &TemplateHistoryEntry,
        interactive: bool,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!(
            "🔄 Processing template: {}/{} current version: {}",
            username, template_name, latest_entry.version
        );

        // a. Fetch all versions
        let all_versions = fetch_all_template_versions(registry_client, username, template_name)?;

        if all_versions.is_empty() {
            println!("⚠️ No versions found for {}/{}", username, template_name);
            return Ok(Vec::new());
        }

        // Get the latest version
        let latest_version = all_versions
            .iter()
            .max_by_key(|v| v.version)
            .expect("Should have at least one version");

        // c. If non-interactive and already at latest version, return early
        if !interactive && latest_version.version == latest_entry.version {
            println!(
                "✅ Template {}/{} is already at latest version ({})",
                username, template_name, latest_entry.version
            );
            return Ok(Vec::new());
        }

        // d. Determine target version
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
            return Ok(Vec::new());
        }

        // e. Perform the upgrade
        let target_version_info = all_versions
            .iter()
            .find(|v| v.version == target_version)
            .expect("Target version should exist in fetched versions");

        UpgradeExecutor::perform_upgrade(
            registry_client,
            composition_operator,
            target_dir,
            username,
            template_name,
            latest_entry,
            target_version_info,
        )
    }
}
