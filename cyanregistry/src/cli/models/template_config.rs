use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyanTemplateFileConfig {
    pub username: String,

    pub name: String,

    pub description: String,

    pub project: String,

    pub source: String,

    pub email: String,

    pub tags: Vec<String>,

    pub readme: String,

    pub processors: Vec<String>,

    pub plugins: Vec<String>,

    pub templates: Vec<String>,

    #[serde(default)]
    pub resolvers: Vec<CyanResolverRefFileConfig>,
}

/// Resolver reference configuration from cyan.yaml
/// Used when a template declares resolvers it uses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyanResolverRefFileConfig {
    /// Resolver reference in format "username/name:version"
    pub resolver: String,

    /// JSON config passed to resolver at runtime (defaults to empty object)
    #[serde(default = "default_config")]
    pub config: serde_json::Value,

    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}

fn default_config() -> serde_json::Value {
    serde_json::json!({})
}
