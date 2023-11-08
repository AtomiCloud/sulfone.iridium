use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorReq {
    pub name: String,
    pub project: String,
    pub source: String,
    pub email: String,
    pub tags: Vec<String>,
    pub description: String,
    pub readme: String,
    pub version_description: String,

    pub docker_reference: String,
    pub docker_sha: String,
}
