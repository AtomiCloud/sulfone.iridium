use crate::domain::models::cyan::Cyan;
use crate::domain::models::question::Question;
use std::collections::HashMap;

pub enum ExtensionOutput {
    QnA(ExtensionQnAOutput),
    Final(ExtensionFinalOutput),
}

pub struct ExtensionQnAOutput {
    pub deterministic_state: Vec<HashMap<String, String>>,
    pub question: Question,
}

pub struct ExtensionFinalOutput {
    pub cyan: Cyan,
}
