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
