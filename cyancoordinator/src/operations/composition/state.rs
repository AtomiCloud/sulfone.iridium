use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::question::Question;
use std::collections::HashMap;
use std::error::Error;
use std::mem::discriminant;

/// State accumulator for template composition execution
#[derive(Debug, Clone)]
pub struct CompositionState {
    pub shared_answers: HashMap<String, Answer>,
    pub shared_deterministic_states: HashMap<String, String>,
    pub execution_order: Vec<String>, // Template IDs in execution order
    /// Set in headless mode when a template's Q&A reached an unanswered question.
    /// Its presence signals the composition to stop and surface the question
    /// upward (no further templates are executed, no files are written).
    ///
    /// Carries the domain [`Question`] rather than the JSON wire DTO: composition is a
    /// service-layer concept and must not depend on the headless serialization contract.
    /// The CLI boundary converts this to the wire representation immediately before
    /// emission.
    pub need_input: Option<Question>,
}

impl CompositionState {
    pub fn new() -> Self {
        Self {
            shared_answers: HashMap::new(),
            shared_deterministic_states: HashMap::new(),
            execution_order: Vec::new(),
            need_input: None,
        }
    }

    /// Update state with results from template execution.
    ///
    /// Returns `Err` if a merged answer's type conflicts with an existing answer for
    /// the same key (an answer's discriminant differing across templates). This is a
    /// recoverable condition that surfaces as an `error` envelope rather than a crash:
    /// per-template id namespacing normally keeps keys from two templates separate, so
    /// a collision indicates a real inconsistency the caller should report cleanly.
    pub fn update_from_template_state(
        &mut self,
        template_state: &cyanprompt::domain::services::template::states::TemplateState,
        template_id: String,
    ) -> Result<(), Box<dyn Error + Send>> {
        use cyanprompt::domain::services::template::states::TemplateState;

        match template_state {
            TemplateState::Complete(_, answers) => {
                // Merge answers into shared state
                for (key, value) in answers.iter() {
                    if let Some(existing) = self.shared_answers.get(key) {
                        // Type conflict check - surface as a recoverable error rather
                        // than panicking on a merge path.
                        if discriminant(existing) != discriminant(value) {
                            return Err(Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!(
                                    "type conflict for key '{key}': existing answer type differs from new answer type"
                                ),
                            )) as Box<dyn Error + Send>);
                        }
                    }
                    self.shared_answers.insert(key.clone(), value.clone());
                }
            }
            TemplateState::NeedInput(question, _) => {
                // Headless: record the unanswered question (domain type) so the
                // composition can short-circuit and surface it to the CLI, which converts
                // it to the wire representation at the point of emission.
                self.need_input = Some(question.clone());
            }
            TemplateState::QnA() | TemplateState::Err(_) => {}
        }

        self.execution_order.push(template_id);
        Ok(())
    }
}

impl Default for CompositionState {
    fn default() -> Self {
        Self::new()
    }
}
