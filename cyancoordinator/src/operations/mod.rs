pub mod new_template;
pub mod rerun;
pub mod upgrade;

use cyanprompt::domain::models::answer::Answer;
pub use new_template::create_new_template;
pub use rerun::rerun_template;
pub use upgrade::upgrade_template;

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::fs::Vfs;
use crate::session::SessionIdGenerator;
use crate::template::{TemplateExecutor, TemplateHistory};
use cyanregistry::http::models::template_res::TemplateVersionRes;

/// Trait defining operations that can be performed on templates
pub trait TemplateOperations {
    /// Create a new project from a template
    fn create_new(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>;

    /// Rerun an existing template with the same version
    #[allow(clippy::too_many_arguments)]
    fn rerun<F>(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
        get_previous_template: F,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>
    where
        F: Fn(i64) -> Result<TemplateVersionRes, Box<dyn Error + Send>>;

    /// Upgrade a template to a new version
    #[allow(clippy::too_many_arguments)]
    fn upgrade<F>(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
        get_previous_template: F,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>
    where
        F: Fn(i64) -> Result<TemplateVersionRes, Box<dyn Error + Send>>;
}

/// Implementation of TemplateOperations that handles template operations
pub struct TemplateOperator {
    pub session_id_generator: Box<dyn SessionIdGenerator>,
    pub template_executor: Box<dyn TemplateExecutor>,
    pub template_history: Box<dyn TemplateHistory>,
    pub vfs: Box<dyn Vfs>,
}

impl TemplateOperator {
    /// Create a new TemplateOperator with the given dependencies
    pub fn new(
        session_id_generator: Box<dyn SessionIdGenerator>,
        template_executor: Box<dyn TemplateExecutor>,
        template_history: Box<dyn TemplateHistory>,
        vfs: Box<dyn Vfs>,
    ) -> Self {
        Self {
            session_id_generator,
            template_executor,
            template_history,
            vfs,
        }
    }
}

impl TemplateOperations for TemplateOperator {
    fn create_new(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        new_template::create_new_template(
            self.session_id_generator.as_ref(),
            template,
            target_dir,
            self.template_executor.as_ref(),
            self.template_history.as_ref(),
            self.vfs.as_ref(),
            username,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn rerun<F>(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
        get_previous_template: F,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>
    where
        F: Fn(i64) -> Result<TemplateVersionRes, Box<dyn Error + Send>>,
    {
        // Create a new context with our dependencies
        let context = rerun::RerunContext {
            session_id_generator: self.session_id_generator.as_ref(),
            template,
            target_dir,
            template_executor: self.template_executor.as_ref(),
            template_history: self.template_history.as_ref(),
            vfs: self.vfs.as_ref(),
            username,
            previous_version,
            previous_answers,
            previous_states,
            get_previous_template,
        };

        rerun_template(context)
    }

    #[allow(clippy::too_many_arguments)]
    fn upgrade<F>(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
        get_previous_template: F,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>
    where
        F: Fn(i64) -> Result<TemplateVersionRes, Box<dyn Error + Send>>,
    {
        // Create a new context with our dependencies
        let context = upgrade::UpgradeContext {
            session_id_generator: self.session_id_generator.as_ref(),
            template,
            target_dir,
            template_executor: self.template_executor.as_ref(),
            template_history: self.template_history.as_ref(),
            vfs: self.vfs.as_ref(),
            username,
            previous_version,
            previous_answers,
            previous_states,
            get_previous_template,
        };

        upgrade_template(context)
    }
}
