use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringArrayAnswerRes {
    pub answer: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringAnswerRes {
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoolAnswerRes {
    pub answer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnswerRes {
    #[serde(rename = "str_array")]
    StringArray(StringArrayAnswerRes),
    #[serde(rename = "string")]
    String(StringAnswerRes),

    #[serde(rename = "boolean")]
    Bool(BoolAnswerRes),
}