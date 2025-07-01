use cyanprompt::domain::models::answer::Answer;
use std::collections::HashMap;
use std::mem::discriminant;

/// State accumulator for template composition execution
#[derive(Debug, Clone)]
pub struct CompositionState {
    pub shared_answers: HashMap<String, Answer>,
    pub shared_deterministic_states: HashMap<String, String>,
    pub execution_order: Vec<String>, // Template IDs in execution order
}

impl CompositionState {
    pub fn new() -> Self {
        Self {
            shared_answers: HashMap::new(),
            shared_deterministic_states: HashMap::new(),
            execution_order: Vec::new(),
        }
    }

    /// Update state with results from template execution
    pub fn update_from_template_state(
        &mut self,
        template_state: &cyanprompt::domain::services::template::states::TemplateState,
        template_id: String,
    ) {
        use cyanprompt::domain::services::template::states::TemplateState;

        if let TemplateState::Complete(_, answers) = template_state {
            // Merge answers into shared state
            for (key, value) in answers.iter() {
                if let Some(existing) = self.shared_answers.get(key) {
                    // Type conflict check - abort on mismatch
                    if discriminant(existing) != discriminant(value) {
                        panic!(
                            "Type conflict for key '{key}': existing type differs from new type"
                        );
                    }
                }
                self.shared_answers.insert(key.clone(), value.clone());
            }
        }

        self.execution_order.push(template_id);
    }
}

impl Default for CompositionState {
    fn default() -> Self {
        Self::new()
    }
}
