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
    #[serde(alias = "removed_containers")]
    pub containers_removed: Option<Vec<String>>,
    #[serde(alias = "removed_images")]
    pub images_removed: Option<Vec<String>>,
    #[serde(alias = "removed_volumes")]
    pub volumes_removed: Option<Vec<String>>,
    #[serde(default)]
    pub containers_count: Option<i64>,
    #[serde(default)]
    pub images_count: Option<i64>,
    #[serde(default)]
    pub volumes_count: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrySetupRes {
    #[serde(default)]
    pub session_id: Option<String>,
    pub blob_volume: DockerVolumeReference,
    /// Deprecated: session volume is now created via `/executor/:sessionId/warm`.
    /// Optional for backward compatibility during transition.
    #[serde(default)]
    pub session_volume: Option<DockerVolumeReference>,
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
    fn test_cleanup_res_deserialization_legacy_format() {
        let json = r#"{"removed_containers":[],"removed_images":[],"removed_volumes":[]}"#;
        let res: CleanupRes = serde_json::from_str(json).expect("Should deserialize");
        assert!(res.containers_removed.unwrap().is_empty());
        assert!(res.images_removed.unwrap().is_empty());
        assert!(res.volumes_removed.unwrap().is_empty());
    }

    #[test]
    fn test_cleanup_res_deserialization_actual_format() {
        let json = r#"{
            "containers_count":0,"containers_removed":null,
            "error":"","images_count":0,"images_removed":null,
            "status":"OK","volumes_count":0,"volumes_removed":null
        }"#;
        let res: CleanupRes = serde_json::from_str(json).expect("Should deserialize");
        assert!(res.containers_removed.is_none());
        assert_eq!(res.status.unwrap(), "OK");
    }

    #[test]
    fn test_cleanup_res_deserialization_with_data() {
        let json = r#"{
            "containers_count":2,"containers_removed":["container1","container2"],
            "images_count":1,"images_removed":["image1"],
            "volumes_count":3,"volumes_removed":["volume1","volume2","volume3"],
            "status":"OK","error":""
        }"#;
        let res: CleanupRes = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(
            res.containers_removed.unwrap(),
            vec!["container1", "container2"]
        );
        assert_eq!(res.images_removed.unwrap(), vec!["image1"]);
        assert_eq!(
            res.volumes_removed.unwrap(),
            vec!["volume1", "volume2", "volume3"]
        );
    }
}
