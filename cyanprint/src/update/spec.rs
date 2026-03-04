use chrono::{DateTime, Utc};
use cyancoordinator::state::models::CyanState;
use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::client::CyanRegistryClient;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

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
    pub fn new_template(username: String, template_name: String, version: i64) -> Self {
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
    pub fn key(&self) -> String {
        format!("{}/{}", self.username, self.template_name)
    }
}

/// Stateless service for TemplateSpec operations.
/// Only holds registry as dependency - no internal state.
pub struct TemplateSpecManager {
    registry: Rc<CyanRegistryClient>,
}

impl TemplateSpecManager {
    pub fn new(registry: Rc<CyanRegistryClient>) -> Self {
        Self { registry }
    }

    /// Read specs from .cyan_state.yaml (pure function)
    pub fn get(&self, state: &CyanState) -> Vec<TemplateSpec> {
        state
            .templates
            .iter()
            .filter(|(_, s)| s.active)
            .filter_map(|(key, s)| {
                let (username, template_name) = parse_template_key(key)?;
                let entry = s.history.last()?;
                Some(TemplateSpec::new(
                    username,
                    template_name,
                    entry.version,
                    entry.answers.clone(),
                    entry.deterministic_states.clone(),
                    entry.time,
                ))
            })
            .collect()
    }

    /// Update specs to latest versions via registry lookup (pure function)
    /// If interactive=true, prompt user to select versions
    pub fn update(
        &self,
        specs: Vec<TemplateSpec>,
        interactive: bool,
    ) -> Result<Vec<TemplateSpec>, Box<dyn Error + Send>> {
        specs
            .iter()
            .map(|spec| {
                // Fetch all versions
                let all_versions = fetch_all_template_versions(
                    &self.registry,
                    &spec.username,
                    &spec.template_name,
                )?;

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

    /// Reset answers to empty HashMap (pure function)
    /// Used for rerun scenario - empty answers trigger fresh Q&A
    pub fn reset(&self, specs: Vec<TemplateSpec>) -> Vec<TemplateSpec> {
        specs
            .into_iter()
            .map(|s| TemplateSpec {
                answers: HashMap::new(),
                deterministic_states: HashMap::new(),
                ..s
            })
            .collect()
    }
}

/// Sort specs by installation time for consistent LWW ordering
pub fn sort_specs(specs: &mut [TemplateSpec]) {
    specs.sort_by(|a, b| a.installed_at.cmp(&b.installed_at));
}
