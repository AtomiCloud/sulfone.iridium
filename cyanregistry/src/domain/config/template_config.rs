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
