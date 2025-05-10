#[derive(Debug, Clone)]
pub struct CyanPluginConfig {
    pub username: String,

    pub name: String,

    pub description: String,

    pub project: String,

    pub source: String,

    pub email: String,

    pub tags: Vec<String>,

    pub readme: String,
}
