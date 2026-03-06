/// Resolver reference in domain layer
/// Parsed from CLI config, used for business logic
#[derive(Debug, Clone)]
pub struct CyanResolverRef {
    pub username: String,

    pub name: String,

    pub version: Option<i64>,

    /// JSON config passed to resolver at runtime
    pub config: Option<serde_json::Value>,

    /// Glob patterns for which files this resolver handles
    pub files: Vec<String>,
}
