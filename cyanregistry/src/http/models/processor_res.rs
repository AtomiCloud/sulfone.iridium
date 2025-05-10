use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorVersionPrincipalRes {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub description: String,
    pub docker_reference: String,
    pub docker_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorVersionRes {
    pub principal: ProcessorVersionPrincipalRes,
    pub processor: ProcessorRes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorRes {
    pub id: String,
    pub name: String,
    pub project: String,
    pub source: String,
    pub email: String,
    pub tags: Vec<String>,
    pub description: String,
    pub readme: String,
    pub user_id: String,
}
