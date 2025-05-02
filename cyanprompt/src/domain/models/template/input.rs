use crate::domain::models::answer::Answer;
use std::collections::HashMap;

pub struct TemplateAnswerInput {
    pub answers: HashMap<String, Answer>,
    pub deterministic_state: HashMap<String, String>,
}

pub struct TemplateValidateInput {
    pub answers: HashMap<String, Answer>,
    pub deterministic_state: HashMap<String, String>,
    pub validate: String,
}
