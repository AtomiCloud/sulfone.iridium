use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

/// Trait for managing template state
pub trait StateManager {
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
