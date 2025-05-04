use crate::errors::GenericError;
use chrono::{DateTime, Utc};
use cyanprompt::domain::models::answer::Answer;
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::path::Path;

pub fn generate_session_id() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

fn parse_ref_internal(s: String) -> Option<(String, String, Option<i64>)> {
    let mut parts = s.splitn(2, '/');
    let username = parts.next()?.to_string();
    let rest = parts.next()?;

    // Split the rest by ':'
    let mut parts = rest.splitn(2, ':');
    let name = parts.next()?.to_string();
    let version_str = parts.next();

    // Convert version string to u64 if present
    let version = match version_str {
        Some(v) => v.parse::<i64>().ok(),
        None => None,
    };
    Some((username, name, version))
}

pub fn parse_ref(s: String) -> Result<(String, String, Option<i64>), Box<dyn Error + Send>> {
    let err = s.clone();
    parse_ref_internal(s)
        .ok_or(Box::new(GenericError::FailedParsingReference(err)) as Box<dyn Error + Send>)
}

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

pub fn load_or_create_state_file(path: &Path) -> Result<CyanState, Box<dyn Error + Send>> {
    if path.exists() {
        let file = fs::File::open(path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let reader = BufReader::new(file);
        let state: CyanState =
            serde_yaml::from_reader(reader).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        Ok(state)
    } else {
        Ok(CyanState::default())
    }
}

pub fn save_state_file(state: &CyanState, path: &Path) -> Result<(), Box<dyn Error + Send>> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    }

    let file = fs::File::create(path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    serde_yaml::to_writer(file, state).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    Ok(())
}
