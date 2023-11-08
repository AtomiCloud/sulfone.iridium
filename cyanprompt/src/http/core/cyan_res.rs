use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanGlobRes {
    pub glob: String,
    pub exclude: Vec<String>,
    #[serde(rename = "type")]
    pub glob_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanPluginRes {
    pub name: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanProcessorRes {
    pub name: String,
    pub config: Value,
    pub files: Vec<CyanGlobRes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CyanRes {
    pub processors: Vec<CyanProcessorRes>,
    pub plugins: Vec<CyanPluginRes>,
}
