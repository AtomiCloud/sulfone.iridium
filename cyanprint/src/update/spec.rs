use chrono::{DateTime, Utc};
use cyancoordinator::state::models::CyanState;
use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::client::CyanRegistryClient;
use std::collections::HashMap;
use std::error::Error;

use super::utils::parse_template_key;
use super::version_manager::{fetch_all_template_versions, select_version_interactive};

/// A simple data structure representing a template to execute.
/// Used for the unified batch processing flow.
#[derive(Clone)]
pub struct TemplateSpec {
    pub username: String,
    pub template_name: String,
    pub version: i64,
    pub answers: HashMap<String, Answer>,
    pub deterministic_states: HashMap<String, String>,
    /// Installation time for LWW ordering
    pub installed_at: DateTime<Utc>,
}

impl TemplateSpec {
    /// Create a new TemplateSpec with the given parameters
    pub fn new(
        username: String,
        template_name: String,
        version: i64,
        answers: HashMap<String, Answer>,
        deterministic_states: HashMap<String, String>,
        installed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            username,
            template_name,
            version,
            answers,
            deterministic_states,
            installed_at,
        }
    }

    /// Create a TemplateSpec for a new template (empty answers - will trigger Q&A)
    pub fn for_new_template(username: String, template_name: String, version: i64) -> Self {
        Self {
            username,
            template_name,
            version,
            answers: HashMap::new(),
            deterministic_states: HashMap::new(),
            installed_at: Utc::now(),
        }
    }

    /// Get the template key in the format "username/template_name"
    pub fn template_key(&self) -> String {
        format!("{}/{}", self.username, self.template_name)
    }
}

/// Build prev_specs from .cyan_state.yaml
/// Returns a list of TemplateSpec for all active templates, sorted by installation time
pub fn build_prev_specs(cyan_state: &CyanState) -> Vec<TemplateSpec> {
    let mut specs: Vec<TemplateSpec> = cyan_state
        .templates
        .iter()
        .filter(|(_, state)| state.active)
        .filter_map(|(key, state)| {
            let (username, template_name) = parse_template_key(key)?;
            let entry = state.history.last()?;
            Some(TemplateSpec::new(
                username,
                template_name,
                entry.version,
                entry.answers.clone(),
                entry.deterministic_states.clone(),
                entry.time,
            ))
        })
        .collect();

    // Sort by installation time (oldest first) for LWW semantics
    specs.sort_by(|a, b| a.installed_at.cmp(&b.installed_at));
    specs
}

/// Build curr_specs for create command
/// Appends a new template spec to the existing specs
pub fn build_curr_specs_for_create(
    prev_specs: Vec<TemplateSpec>,
    new_template: TemplateSpec,
) -> Vec<TemplateSpec> {
    let mut curr = prev_specs;
    curr.push(new_template);
    curr
}

/// Build curr_specs for update command
/// Upgrades templates to their latest versions while reusing stored answers
pub fn build_curr_specs_for_update(
    prev_specs: Vec<TemplateSpec>,
    registry: &CyanRegistryClient,
    interactive: bool,
) -> Result<Vec<TemplateSpec>, Box<dyn Error + Send>> {
    prev_specs
        .iter()
        .map(|spec| {
            // Fetch all versions
            let all_versions =
                fetch_all_template_versions(registry, &spec.username, &spec.template_name)?;

            // Get the latest version
            let latest = all_versions
                .iter()
                .max_by_key(|v| v.version)
                .ok_or_else(|| {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!(
                            "No versions found for {}/{}",
                            spec.username, spec.template_name
                        ),
                    )) as Box<dyn Error + Send>
                })?;

            // Determine target version
            let target_version = if interactive {
                select_version_interactive(
                    &spec.username,
                    &spec.template_name,
                    spec.version,
                    &all_versions,
                )?
            } else {
                latest.version
            };

            Ok(TemplateSpec::new(
                spec.username.clone(),
                spec.template_name.clone(),
                target_version,
                spec.answers.clone(),
                spec.deterministic_states.clone(),
                spec.installed_at, // Preserve original installation time for LWW ordering
            ))
        })
        .collect()
}

/// Sort specs by installation time for consistent LWW ordering
pub fn sort_specs_by_time(specs: &mut [TemplateSpec]) {
    specs.sort_by(|a, b| a.installed_at.cmp(&b.installed_at));
}

/// Filter specs to only include those that have a different version
/// Returns (upgraded_specs, unchanged_specs)
pub fn classify_specs_by_upgrade<'a>(
    prev_specs: &[TemplateSpec],
    curr_specs: &'a [TemplateSpec],
) -> (Vec<&'a TemplateSpec>, Vec<&'a TemplateSpec>) {
    let prev_versions: HashMap<(String, String), i64> = prev_specs
        .iter()
        .map(|s| ((s.username.clone(), s.template_name.clone()), s.version))
        .collect();

    let mut upgraded = Vec::new();
    let mut unchanged = Vec::new();

    for spec in curr_specs {
        let key = (spec.username.clone(), spec.template_name.clone());
        if let Some(&prev_version) = prev_versions.get(&key) {
            if spec.version != prev_version {
                upgraded.push(spec);
            } else {
                unchanged.push(spec);
            }
        } else {
            // New template - treat as upgraded for metadata purposes
            upgraded.push(spec);
        }
    }

    (upgraded, unchanged)
}
