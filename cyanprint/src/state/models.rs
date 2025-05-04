use chrono::{DateTime, Utc};
use cyanprompt::domain::models::answer::Answer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateHistoryEntry {
    pub version: i64,
    pub time: DateTime<Utc>,
    pub answers: HashMap<String, Answer>,
    pub deterministic_states: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateState {
    pub active: bool,
    pub history: Vec<TemplateHistoryEntry>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CyanState {
    #[serde(flatten)]
    pub templates: HashMap<String, TemplateState>,
}
