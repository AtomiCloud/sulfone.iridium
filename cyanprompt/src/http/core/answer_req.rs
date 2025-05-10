use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringArrayAnswerReq {
    pub answer: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringAnswerReq {
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoolAnswerReq {
    pub answer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnswerReq {
    #[serde(rename = "str_array")]
    StringArray(StringArrayAnswerReq),
    #[serde(rename = "string")]
    String(StringAnswerReq),

    #[serde(rename = "boolean")]
    Bool(BoolAnswerReq),
}
