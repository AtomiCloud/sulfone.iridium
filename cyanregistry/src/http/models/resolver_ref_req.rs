use serde::{Deserialize, Serialize};

/// Resolver reference request for template push API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverRefReq {
    /// Resolver reference in format "username/name"
    pub resolver_reference: String,

    /// Resolver version (must be non-negative)
    pub resolver_version: u64,

    /// JSON config passed to resolver at runtime
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,

    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}
