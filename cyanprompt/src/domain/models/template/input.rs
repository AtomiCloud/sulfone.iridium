use crate::domain::models::answer::Answer;
use std::collections::HashMap;

pub struct TemplateAnswerInput {
    pub answers: Vec<Answer>,
    pub deterministic_states: Vec<HashMap<String, String>>,
}

pub struct TemplateValidateInput {
    pub answers: Vec<Answer>,
    pub deterministic_states: Vec<HashMap<String, String>>,
    pub validate: String,
}
