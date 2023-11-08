use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::http::core::cyan_res::CyanRes;
use crate::http::core::question_res::QuestionRes;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionValidRes {
    pub valid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExtensionRes {
    #[serde(rename = "questionnaire")]
    Qna(ExtensionQnARes),
    #[serde(rename = "final")]
    Cyan(ExtensionFinalRes),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionFinalRes {
    pub cyan: CyanRes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionQnARes {
    pub deterministic_state: Vec<HashMap<String, String>>,
    pub question: QuestionRes,
}

