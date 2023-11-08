use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use crate::http::core::answer_req::AnswerReq;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAnswerReq {
    pub answers: Vec<AnswerReq>,

    pub deterministic_states: Vec<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateValidateReq {
    pub answers: Vec<AnswerReq>,

    pub deterministic_states: Vec<HashMap<String, String>>,

    pub validate: String,
}

