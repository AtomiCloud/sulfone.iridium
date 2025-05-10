use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::http::core::answer_req::AnswerReq;
use crate::http::core::cyan_req::CyanReq;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionAnswerReq {
    pub answers: Vec<AnswerReq>,

    pub deterministic_states: Vec<HashMap<String, String>>,

    pub prev_answers: Vec<AnswerReq>,

    pub prev_cyan: CyanReq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionValidateReq {
    pub answers: Vec<AnswerReq>,

    pub deterministic_states: Vec<HashMap<String, String>>,

    pub prev_answers: Vec<AnswerReq>,

    pub prev_cyan: CyanReq,

    pub validate: String,
}
