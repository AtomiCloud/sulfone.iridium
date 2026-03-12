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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupRes {
    pub removed_containers: Vec<String>,
    pub removed_images: Vec<String>,
    pub removed_volumes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrySetupRes {
    pub session_id: String,
    pub blob_volume: DockerVolumeReference,
    pub session_volume: DockerVolumeReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerVolumeReference {
    pub cyan_id: String,
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_res_deserialization_empty() {
        let json = r#"{"removed_containers":[],"removed_images":[],"removed_volumes":[]}"#;
        let res: CleanupRes = serde_json::from_str(json).expect("Should deserialize");
        assert!(res.removed_containers.is_empty());
        assert!(res.removed_images.is_empty());
        assert!(res.removed_volumes.is_empty());
    }

    #[test]
    fn test_cleanup_res_deserialization_with_data() {
        let json = r#"{
            "removed_containers":["container1","container2"],
            "removed_images":["image1"],
            "removed_volumes":["volume1","volume2","volume3"]
        }"#;
        let res: CleanupRes = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(res.removed_containers, vec!["container1", "container2"]);
        assert_eq!(res.removed_images, vec!["image1"]);
        assert_eq!(res.removed_volumes, vec!["volume1", "volume2", "volume3"]);
    }

    #[test]
    fn test_cleanup_res_serialization() {
        let res = CleanupRes {
            removed_containers: vec!["c1".to_string()],
            removed_images: vec![],
            removed_volumes: vec!["v1".to_string(), "v2".to_string()],
        };
        let json = serde_json::to_string(&res).expect("Should serialize");
        assert!(json.contains("removed_containers"));
        assert!(json.contains("removed_images"));
        assert!(json.contains("removed_volumes"));
    }
}
