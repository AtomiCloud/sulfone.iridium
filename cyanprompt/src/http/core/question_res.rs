use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QuestionRes {
    #[serde(rename = "confirm")]
    Confirm(ConfirmQuestionRes),
    #[serde(rename = "date")]
    Date(DateQuestionRes),
    #[serde(rename = "checkbox")]
    Checkbox(CheckboxQuestionRes),
    #[serde(rename = "password")]
    Password(PasswordQuestionRes),
    #[serde(rename = "text")]
    Text(TextQuestionRes),
    #[serde(rename = "select")]
    Select(SelectQuestionRes),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmQuestionRes {
    pub message: String,
    pub desc: Option<String>,
    pub default: Option<bool>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateQuestionRes {
    pub message: String,
    pub desc: Option<String>,
    pub default: Option<String>,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckboxQuestionRes {
    pub message: String,
    pub options: Vec<String>,
    pub desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordQuestionRes {
    pub message: String,
    pub desc: Option<String>,
    pub confirmation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextQuestionRes {
    pub message: String,
    pub default: Option<String>,
    pub desc: Option<String>,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectQuestionRes {
    pub message: String,
    pub desc: Option<String>,
    pub options: Vec<String>,
}
