use serde::{Deserialize, Serialize};

/// Resolver reference request for template push API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverRefReq {
    /// Resolver reference in format "username/name"
    pub resolver_reference: String,

    /// Resolver version (must be non-negative)
    pub resolver_version: u64,

    /// JSON config passed to resolver at runtime (defaults to empty object)
    #[serde(default = "default_config")]
    pub config: serde_json::Value,

    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}

fn default_config() -> serde_json::Value {
    serde_json::json!({})
}
