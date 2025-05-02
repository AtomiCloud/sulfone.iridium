use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::Cyan;
use std::collections::HashMap;

pub enum TemplateState {
    QnA(),
    Complete(Cyan, HashMap<String, Answer>),
    Err(String),
}

impl TemplateState {
    pub fn cont(&self) -> bool {
        match self {
            TemplateState::QnA() => true,
            TemplateState::Complete(_, _) => false,
            TemplateState::Err(_) => false,
        }
    }
}
