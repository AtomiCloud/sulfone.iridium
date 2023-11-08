use serde::{Deserialize, Serialize};
use crate::models::req::DockerVolumeReferenceReq;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardRes {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ExecutorWarmRes {
    pub session_id: String,
    pub vol_ref: DockerVolumeReferenceReq,
}