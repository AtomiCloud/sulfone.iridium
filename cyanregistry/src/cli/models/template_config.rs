use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Template reference in cyan.yaml — accepts both plain strings and extended objects
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CyanTemplateFileRef {
    /// Simple reference: "username/name" or "username/name:version"
    Simple(String),
    /// Extended reference with preset answers
    Extended {
        template: String,
        #[serde(default)]
        preset_answers: HashMap<String, serde_json::Value>,
    },
}

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

    pub templates: Vec<CyanTemplateFileRef>,

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
