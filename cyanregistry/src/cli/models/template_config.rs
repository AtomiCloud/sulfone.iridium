use serde::{Deserialize, Serialize};

use super::resolver_ref_config::CyanResolverRefFileConfig;

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
