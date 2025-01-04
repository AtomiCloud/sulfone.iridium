use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::Cyan;

pub enum TemplateState {
    QnA(),
    Complete(Cyan, Vec<Answer>),
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
