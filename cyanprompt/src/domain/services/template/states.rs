use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::Cyan;
use crate::domain::models::question::Question;
use std::collections::HashMap;

#[derive(Clone)]
pub enum TemplateState {
    QnA(),
    Complete(Cyan, HashMap<String, Answer>),
    /// Headless replay reached a question that has no supplied answer yet.
    ///
    /// Carries the unanswered `Question` and the deterministic state accumulated
    /// from the coordinator up to that point. This is a terminal state for the
    /// headless driver: the caller emits the question and stops, expecting the
    /// next invocation to supply the answer (stateless replay).
    NeedInput(Question, HashMap<String, String>),
    Err(String),
}

impl TemplateState {
    pub fn cont(&self) -> bool {
        match self {
            TemplateState::QnA() => true,
            TemplateState::Complete(_, _) => false,
            TemplateState::NeedInput(_, _) => false,
            TemplateState::Err(_) => false,
        }
    }
}
