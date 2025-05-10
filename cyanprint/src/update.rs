use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::{TemplateOperations, TemplateOperator};
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::models::TemplateHistoryEntry;
use cyancoordinator::state::{DefaultStateManager, StateReader};
use cyancoordinator::template::{DefaultTemplateExecutor, DefaultTemplateHistory};
use cyanregistry::http::client::CyanRegistryClient;
use inquire::Select;

/// Custom error type for selection errors
#[derive(Debug)]
struct SelectionError(String);

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for SelectionError {}

/// Update all templates in a project to their latest versions
/// Returns all session IDs that were created and need to be cleaned up
pub fn cyan_update(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: String,
    coord_client: CyanCoordinatorClient,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
    interactive: bool,
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // 1. Read state
    let target_dir = PathBuf::from(&path).as_path().to_owned();
    println!("📁 Target directory: {:?}", target_dir);

    let state_file_path = target_dir.join(".cyan_state.yaml");
    println!("🔍 Reading template state from: {:?}", state_file_path);
    let state = DefaultStateManager::new().load_state_file(&state_file_path)?;

    if state.templates.is_empty() {
        println!("⚠️ No templates found in state file");
        return Ok(Vec::new());
    }

    // Initialize components
    let template_operator = create_template_operator(session_id_generator, coord_client, debug);

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
                let session_ids = process_template_upgrade(
                    &registry_client,
                    &template_operator,
                    &target_dir,
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

/// Create and configure a template operator with dependencies
fn create_template_operator(
    session_id_generator: Box<dyn SessionIdGenerator>,
    coord_client: CyanCoordinatorClient,
    debug: bool,
) -> TemplateOperator {
    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    let merger = Box::new(GitLikeMerger::new(debug, 50));
    let writer = Box::new(DiskFileWriter);

    let template_history = Box::new(DefaultTemplateHistory::new());
    let template_executor = Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

    TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
    )
}

/// Process a single template upgrade
fn process_template_upgrade(
    registry_client: &CyanRegistryClient,
    template_operator: &TemplateOperator,
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
        select_version_interactive(username, template_name, latest_entry.version, &all_versions)?
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

    perform_upgrade(
        registry_client,
        template_operator,
        target_dir,
        username,
        template_name,
        latest_entry,
        target_version_info,
    )
}

/// Let user select a version interactively
fn select_version_interactive(
    username: &str,
    template_name: &str,
    current_version: i64,
    versions: &[TemplateVersionInfo],
) -> Result<i64, Box<dyn Error + Send>> {
    println!(
        "\n📋 Available versions for {}/{}:",
        username, template_name
    );

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

    let prompt = format!(
        "Select version to upgrade to for {}/{} (ESC to skip)",
        username, template_name
    );

    Select::new(&prompt, version_options.clone())
        .with_help_message("↑↓ to move, enter to select, ESC to skip this template")
        .prompt()
        .map_err(|e| {
            Box::new(SelectionError(format!("Selection cancelled: {}", e))) as Box<dyn Error + Send>
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
fn format_friendly_date(date_str: &str) -> String {
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

/// Perform the actual upgrade
fn perform_upgrade(
    registry_client: &CyanRegistryClient,
    template_operator: &TemplateOperator,
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

    // Helper closure for fetching previous template version
    let get_previous_template = |previous_version: i64| {
        println!(
            "🔍 Fetching template '{}/{}:{}' from registry...",
            username, template_name, previous_version
        );
        let result = registry_client.get_template(
            username.to_string(),
            template_name.to_string(),
            Some(previous_version),
        );

        if result.is_ok() {
            println!("✅ Retrieved previous template version from registry");
        }

        result
    };

    // Perform upgrade
    template_operator
        .upgrade(
            &target_template,
            target_dir,
            username,
            latest_entry.version,
            latest_entry.answers.clone(),
            latest_entry.deterministic_states.clone(),
            get_previous_template,
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

/// Parse template key into username and template name
fn parse_template_key(template_key: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = template_key.split('/').collect();
    (parts.len() == 2).then(|| (parts[0].to_string(), parts[1].to_string()))
}

/// Template version information for display in interactive mode
#[derive(Clone)]
struct TemplateVersionInfo {
    version: i64,
    description: String,
    created_at: String,
    is_latest: bool,
}

/// Fetch all versions for a template in one go
fn fetch_all_template_versions(
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
            "No versions found for {}/{}",
            username, template_name
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
