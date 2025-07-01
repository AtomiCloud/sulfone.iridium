use std::error::Error;

use chrono;
use cyanregistry::http::client::CyanRegistryClient;
use inquire::Select;

use super::utils::SelectionError;

/// Template version information for display in interactive mode
#[derive(Clone)]
pub struct TemplateVersionInfo {
    pub version: i64,
    pub description: String,
    pub created_at: String,
    pub is_latest: bool,
}

/// Fetch all versions for a template in one go
pub fn fetch_all_template_versions(
    registry_client: &CyanRegistryClient,
    username: &str,
    template_name: &str,
) -> Result<Vec<TemplateVersionInfo>, Box<dyn Error + Send>> {
    // Fetch versions in batches of 100
    let batch_size: i64 = 100;
    let mut all_versions = Vec::new();
    let mut skip = 0;

    loop {
        let versions = registry_client.get_template_versions(
            username.to_string(),
            template_name.to_string(),
            skip,
            batch_size,
        )?;

        if versions.is_empty() {
            break;
        }

        // Process this batch
        let batch_versions: Vec<TemplateVersionInfo> = versions
            .iter()
            .map(|v| TemplateVersionInfo {
                version: v.version,
                description: v.description.clone(),
                created_at: v.created_at.clone(),
                is_latest: false, // We'll set this later
            })
            .collect();

        all_versions.extend(batch_versions);

        // Prepare for next batch
        skip += batch_size;

        // If we got fewer results than the batch size, we're done
        if versions.len() < batch_size as usize {
            break;
        }
    }

    if all_versions.is_empty() {
        return Err(Box::new(SelectionError(format!(
            "No versions found for {username}/{template_name}"
        ))));
    }

    // Set is_latest flag on the highest version
    if let Some(max_version) = all_versions.iter().map(|v| v.version).max() {
        for version in all_versions.iter_mut() {
            version.is_latest = version.version == max_version;
        }
    }

    // Sort by version descending (newest first)
    all_versions.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(all_versions)
}

/// Let user select a version interactively
pub fn select_version_interactive(
    username: &str,
    template_name: &str,
    current_version: i64,
    versions: &[TemplateVersionInfo],
) -> Result<i64, Box<dyn Error + Send>> {
    println!("\n📋 Available versions for {username}/{template_name}:");

    let version_options = versions
        .iter()
        .map(|v| {
            let status = if v.version == current_version {
                " [CURRENT]"
            } else if v.is_latest {
                " [LATEST]"
            } else {
                ""
            };

            format!(
                "({}) - Version {}: {}{}",
                format_friendly_date(&v.created_at),
                v.version,
                v.description,
                status
            )
        })
        .collect::<Vec<_>>();

    let prompt =
        format!("Select version to upgrade to for {username}/{template_name} (ESC to skip)");

    Select::new(&prompt, version_options.clone())
        .with_help_message("↑↓ to move, enter to select, ESC to skip this template")
        .prompt()
        .map_err(|e| {
            Box::new(SelectionError(format!("Selection cancelled: {e}"))) as Box<dyn Error + Send>
        })
        .and_then(
            |selected| match version_options.iter().position(|item| item == &selected) {
                Some(idx) => Ok(versions[idx].version),
                None => Err(Box::new(SelectionError(String::from(
                    "Failed to find selected version",
                ))) as Box<dyn Error + Send>),
            },
        )
}

/// Format date string into a more friendly format with local timezone
pub fn format_friendly_date(date_str: &str) -> String {
    // Try to parse the date string
    // Assuming format like "2023-04-25T15:30:45Z" or similar ISO format
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(date_str) {
        // Convert to local time
        let local_time = datetime.with_timezone(&chrono::Local);

        // Format as a friendly date with time in local timezone
        return local_time.format("%b %d, %Y at %H:%M:%S %Z").to_string();
    }

    // Fallback if parsing fails
    date_str.to_string()
}
