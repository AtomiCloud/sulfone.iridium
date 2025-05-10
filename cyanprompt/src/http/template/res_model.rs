use std::collections::HashMap;

use crate::http::core::cyan_res::CyanRes;
use crate::http::core::question_res::QuestionRes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateValidRes {
    pub valid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TemplateRes {
    #[serde(rename = "questionnaire")]
    Qna(TemplateQnARes),
    #[serde(rename = "final")]
    Cyan(TemplateFinalRes),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFinalRes {
    pub cyan: CyanRes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateQnARes {
    pub deterministic_state: HashMap<String, String>,
    pub question: QuestionRes,
}
