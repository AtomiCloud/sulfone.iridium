#[derive(Debug, Clone)]
pub struct CyanTemplateConfig {
    pub username: String,

    pub name: String,

    pub description: String,

    pub project: String,

    pub source: String,

    pub email: String,

    pub tags: Vec<String>,

    pub processors: Vec<CyanProcessorRef>,

    pub plugins: Vec<CyanPluginRef>,

    pub templates: Vec<CyanTemplateRef>,

    pub readme: String,

    pub resolvers: Vec<CyanResolverRef>,
}

#[derive(Debug, Clone)]
pub struct CyanProcessorRef {
    pub username: String,
    pub name: String,
    pub version: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CyanPluginRef {
    pub username: String,
    pub name: String,
    pub version: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CyanTemplateRef {
    pub username: String,
    pub name: String,
    pub version: Option<i64>,
}

/// Resolver reference in domain layer
/// Parsed from CLI config, used for business logic
#[derive(Debug, Clone)]
pub struct CyanResolverRef {
    pub username: String,
    pub name: String,
    pub version: Option<u64>,
    /// JSON config passed to resolver at runtime (defaults to empty object)
    pub config: serde_json::Value,
    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}
