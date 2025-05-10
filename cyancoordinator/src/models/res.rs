use crate::models::req::DockerVolumeReferenceReq;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardRes {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ExecutorWarmRes {
    pub session_id: String,
    pub vol_ref: DockerVolumeReferenceReq,
}
