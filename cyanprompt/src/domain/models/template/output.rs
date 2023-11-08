use std::collections::HashMap;

use crate::domain::models::cyan::Cyan;
use crate::domain::models::question::Question;

pub enum TemplateOutput {
    QnA(TemplateQnAOutput),
    Final(TemplateFinalOutput),
}

pub struct TemplateQnAOutput {
    pub deterministic_state: Vec<HashMap<String, String>>,
    pub question: Question,
}

pub struct TemplateFinalOutput {
    pub cyan: Cyan,
}