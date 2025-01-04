use crate::http::models::plugin_res::PluginVersionPrincipalRes;
use crate::http::models::processor_res::ProcessorVersionPrincipalRes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersionPrincipalRes {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub description: String,
    pub blob_docker_reference: String,
    pub blob_docker_tag: String,
    pub template_docker_reference: String,
    pub template_docker_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionRes {
    pub principal: TemplateVersionPrincipalRes,
    pub template: TemplatePrincipalRes,
    pub plugins: Vec<PluginVersionPrincipalRes>,
    pub processors: Vec<ProcessorVersionPrincipalRes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePrincipalRes {
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
