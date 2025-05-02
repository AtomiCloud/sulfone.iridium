use std::collections::HashMap;

use crate::http::core::answer_req::AnswerReq;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAnswerReq {
    pub answers: HashMap<String, AnswerReq>,
    pub deterministic_states: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateValidateReq {
    pub answers: HashMap<String, AnswerReq>,
    pub deterministic_states: HashMap<String, String>,
    pub validate: String,
}
