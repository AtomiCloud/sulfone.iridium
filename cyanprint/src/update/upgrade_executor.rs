use std::error::Error;
use std::path::Path;

use cyancoordinator::operations::composition::CompositionOperator;
use cyancoordinator::state::models::TemplateHistoryEntry;
use cyanregistry::http::client::CyanRegistryClient;

use super::version_manager::TemplateVersionInfo;

/// Executor for performing template upgrades
pub struct UpgradeExecutor;

impl UpgradeExecutor {
    /// Perform the actual upgrade with automatic composition detection
    pub fn perform_upgrade(
        registry_client: &CyanRegistryClient,
        composition_operator: &CompositionOperator,
        target_dir: &Path,
        username: &str,
        template_name: &str,
        latest_entry: &TemplateHistoryEntry,
        target_version_info: &TemplateVersionInfo,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!(
            "🔄 Upgrading {}/{} from version {} to {}",
            username, template_name, latest_entry.version, target_version_info.version
        );

        // Fetch target template version (we may already have it from the version list, but it only has metadata)
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

        // Check if template has dependencies and use appropriate upgrade method
        if target_template.templates.is_empty() {
            println!("📦 Template has no dependencies - using single template upgrade");
        } else {
            println!(
                "🔗 Template has {} dependencies - using composition upgrade",
                target_template.templates.len()
            );
        }

        // Perform upgrade using composition operator (handles both single and composition templates)
        composition_operator
            .upgrade_composition(
                &target_template,
                target_dir,
                username,
                latest_entry.version,
                latest_entry.answers.clone(),
                latest_entry.deterministic_states.clone(),
            )
            .inspect(|_session_ids| {
                println!(
                    "✅ Successfully upgraded {}/{} to version {}",
                    username, template_name, target_version_info.version
                );
            })
            .map_err(|e| {
                eprintln!("❌ Failed to upgrade {}/{}: {}", username, template_name, e);
                e
            })
    }
}
