use chrono::Utc;
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::state::models::{CyanState, TemplateHistoryEntry, TemplateState as YamlTemplateState};
use crate::state::traits::{StateManager, StateReader, StateWriter};

/// Default implementation of StateManager
#[derive(Debug, Default)]
pub struct DefaultStateManager;

impl DefaultStateManager {
    pub fn new() -> Self {
        Self
    }
}

impl StateReader for DefaultStateManager {
    fn load_state_file(&self, path: &Path) -> Result<CyanState, Box<dyn Error + Send>> {
        if path.exists() {
            let file = fs::File::open(path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            let reader = BufReader::new(file);
            let state: CyanState = serde_yaml::from_reader(reader)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            Ok(state)
        } else {
            Ok(CyanState::default())
        }
    }
}

impl StateWriter for DefaultStateManager {
    fn save_state_file(&self, state: &CyanState, path: &Path) -> Result<(), Box<dyn Error + Send>> {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        }

        let file = fs::File::create(path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        serde_yaml::to_writer(file, state).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        Ok(())
    }

    fn save_template_metadata(
        &self,
        target_dir: &Path,
        template: &TemplateVersionRes,
        answers: &HashMap<String, Answer>,
        _template_state: &TemplateState,
        username: &str,
    ) -> Result<(), Box<dyn Error + Send>> {
        let state_file_path = target_dir.join(".cyan_state.yaml");

        let mut state = self.load_state_file(&state_file_path)?;

        let template_key = format!("{}/{}", username, template.template.name);

        let deterministic_states = HashMap::new();

        let history_entry = TemplateHistoryEntry {
            version: template.principal.version,
            time: Utc::now(),
            answers: answers.clone(),
            deterministic_states,
        };

        let template_state_entry =
            state
                .templates
                .entry(template_key)
                .or_insert(YamlTemplateState {
                    active: true,
                    history: Vec::new(),
                });

        template_state_entry.history.push(history_entry);

        self.save_state_file(&state, &state_file_path)?;

        Ok(())
    }
}

// Implement the combined trait
impl StateManager for DefaultStateManager {}
