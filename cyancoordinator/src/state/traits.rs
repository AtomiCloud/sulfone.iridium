use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::state::models::CyanState;

/// Trait for managing template state - reading operations
pub trait StateReader: Send + Sync {
    /// Load state file or create a new one if it doesn't exist
    fn load_state_file(&self, path: &Path) -> Result<CyanState, Box<dyn Error + Send>>;
}

/// Trait for managing template state - writing operations
pub trait StateWriter: Send + Sync {
    /// Save state to file
    fn save_state_file(&self, state: &CyanState, path: &Path) -> Result<(), Box<dyn Error + Send>>;

    /// Save template metadata after generation
    fn save_template_metadata(
        &self,
        target_dir: &Path,
        template: &TemplateVersionRes,
        answers: &HashMap<String, Answer>,
        template_state: &TemplateState,
        username: &str,
    ) -> Result<(), Box<dyn Error + Send>>;
}

/// Combined trait for full state management (both read and write)
pub trait StateManager: StateReader + StateWriter {}
