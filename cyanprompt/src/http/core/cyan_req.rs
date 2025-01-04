use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanGlobReq {
    pub root: Option<String>,
    pub glob: String,
    pub exclude: Vec<String>,
    #[serde(rename = "type")]
    pub glob_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanPluginReq {
    pub name: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanProcessorReq {
    pub name: String,
    pub config: Value,
    pub files: Vec<CyanGlobReq>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanReq {
    pub processors: Vec<CyanProcessorReq>,
    pub plugins: Vec<CyanPluginReq>,
}
