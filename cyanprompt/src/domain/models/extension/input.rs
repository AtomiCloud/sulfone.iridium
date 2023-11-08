use std::collections::HashMap;
use crate::domain::models::answer::Answer;
use crate::domain::models::cyan::Cyan;

pub struct ExtensionAnswerInput {
    pub answers: Vec<Answer>,
    pub deterministic_states: Vec<HashMap<String, String>>,
    pub prev_answers: Vec<Answer>,
    pub prev: Cyan,
}

pub struct ExtensionValidateInput {
    pub answers: Vec<Answer>,
    pub deterministic_states: Vec<HashMap<String, String>>,
    pub prev_answers: Vec<Answer>,
    pub prev: Cyan,
    pub validate: String,
}