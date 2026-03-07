use serde::{Deserialize, Serialize};

/// Resolver reference request for template push API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverRefReq {
    pub username: String,

    pub name: String,

    #[serde(default)]
    pub version: i64,

    /// JSON config passed to resolver at runtime (defaults to empty object)
    #[serde(default = "default_config")]
    pub config: serde_json::Value,

    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}

fn default_config() -> serde_json::Value {
    serde_json::json!({})
}
