use serde::{Deserialize, Serialize};

/// Resolver reference configuration from cyan.yaml
/// Used when a template declares resolvers it uses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyanResolverRefFileConfig {
    /// Resolver reference in format "username/name:version"
    pub resolver: String,

    /// JSON config passed to resolver at runtime
    pub config: Option<serde_json::Value>,

    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}
