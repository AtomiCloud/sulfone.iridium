use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateReq {
    pub name: String,

    pub project: String,

    pub source: String,

    pub email: String,

    pub tags: Vec<String>,

    pub description: String,

    pub readme: String,

    pub version_description: String,

    pub properties: Option<TemplatePropertyReq>,

    pub plugins: Vec<PluginRefReq>,

    pub processors: Vec<ProcessorRefReq>,

    pub templates: Vec<TemplateRefReq>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRefReq {
    pub username: String,

    pub name: String,

    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorRefReq {
    pub username: String,

    pub name: String,

    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRefReq {
    pub username: String,

    pub name: String,

    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePropertyReq {
    pub blob_docker_reference: String,

    pub blob_docker_tag: String,

    pub template_docker_reference: String,

    pub template_docker_tag: String,
}
