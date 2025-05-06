use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::collections::HashMap;

use crate::state::{DefaultStateManager, StateManager};

pub enum TemplateUpdateType {
    /// No previous template found
    NewTemplate,
    /// Previous template found with different version
    UpgradeTemplate {
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    },
    /// Previous template found with same version
    RerunTemplate {
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    },
}

pub trait TemplateHistory {
    fn check_template_history(
        &self,
        target_dir: &Path,
        template: &TemplateVersionRes,
        username: &str,
    ) -> Result<TemplateUpdateType, Box<dyn Error + Send>>;

    fn save_template_metadata(
        &self,
        target_dir: &Path,
        template: &TemplateVersionRes,
        template_state: &TemplateState,
        username: &str,
    ) -> Result<(), Box<dyn Error + Send>>;
}

pub struct DefaultTemplateHistory {
    state_manager: Arc<dyn StateManager + Send + Sync>,
}

impl DefaultTemplateHistory {
    pub fn new() -> Self {
        Self {
            state_manager: Arc::new(DefaultStateManager::new()),
        }
    }

    pub fn with_state_manager(state_manager: Arc<dyn StateManager + Send + Sync>) -> Self {
        Self { state_manager }
    }
}

impl Default for DefaultTemplateHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateHistory for DefaultTemplateHistory {
    fn check_template_history(
        &self,
        target_dir: &Path,
        template: &TemplateVersionRes,
        username: &str,
    ) -> Result<TemplateUpdateType, Box<dyn Error + Send>> {
        let state_file_path = target_dir.join(".cyan_state.yaml");

        // If state file doesn't exist, it's a new template
        if !state_file_path.exists() {
            return Ok(TemplateUpdateType::NewTemplate);
        }

        // Load the state file
        let state = self.state_manager.load_state_file(&state_file_path)?;

        // Check if this template exists in history
        let template_key = format!("{}/{}", username, template.template.name);

        if let Some(template_state) = state.templates.get(&template_key) {
            // We found a matching template, get the most recent entry
            if let Some(latest_entry) = template_state.history.last() {
                // Check if versions match
                if latest_entry.version == template.principal.version {
                    // Same version - user wants to re-run
                    Ok(TemplateUpdateType::RerunTemplate {
                        previous_version: latest_entry.version,
                        previous_answers: latest_entry.answers.clone(),
                        previous_states: latest_entry.deterministic_states.clone(),
                    })
                } else {
                    // Different version - upgrade flow
                    Ok(TemplateUpdateType::UpgradeTemplate {
                        previous_version: latest_entry.version,
                        previous_answers: latest_entry.answers.clone(),
                        previous_states: latest_entry.deterministic_states.clone(),
                    })
                }
            } else {
                // No history entries (shouldn't happen, but treat as new)
                Ok(TemplateUpdateType::NewTemplate)
            }
        } else {
            // Template not found in history
            Ok(TemplateUpdateType::NewTemplate)
        }
    }

    fn save_template_metadata(
        &self,
        target_dir: &Path,
        template: &TemplateVersionRes,
        template_state: &TemplateState,
        username: &str,
    ) -> Result<(), Box<dyn Error + Send>> {
        if let TemplateState::Complete(_, answers) = template_state {
            self.state_manager.save_template_metadata(
                target_dir,
                template,
                answers,
                template_state,
                username,
            )?;
        }

        Ok(())
    }
}
