use cyanprompt::http::core::cyan_req::CyanReq;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerVolumeReferenceReq {
    pub cyan_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReq {
    pub template: TemplateVersionRes,
    pub cyan: CyanReq,
    pub merger_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergerReq {
    pub merger_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartExecutorReq {
    pub session_id: String,
    pub template: TemplateVersionRes,
    pub write_vol_reference: DockerVolumeReferenceReq,
    pub merger: MergerReq,
}

/// Docker image reference for try command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerImageReference {
    #[serde(rename = "reference")]
    pub reference: String,
    pub tag: String,
}

/// Request for setting up a try environment (blob volume, images, resolvers).
///
/// This prepares the template environment (once-per-template work).
/// Session volumes are created separately via `/executor/:sessionId/warm`.
///
/// Note: `session_id` and `merger_id` are kept for backward compatibility
/// with Boron until it is updated to not require them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrySetupReq {
    pub session_id: String,
    pub local_template_id: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_ref: Option<DockerImageReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub template: TemplateVersionRes,
    pub merger_id: String,
}
