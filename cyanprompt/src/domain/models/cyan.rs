use serde_json::Value;

#[derive(Debug, Clone)]
pub enum GlobType {
    Template(),
    Copy(),
}

#[derive(Debug, Clone)]
pub struct CyanGlob {
    pub root: Option<String>,
    pub glob_type: GlobType,
    pub exclude: Vec<String>,
    pub glob: String,
}

#[derive(Debug, Clone)]
pub struct CyanPlugin {
    pub name: String,
    pub config: Value,
}

#[derive(Debug, Clone)]
pub struct CyanProcessor {
    pub name: String,
    pub config: Value,
    pub files: Vec<CyanGlob>,
}

#[derive(Debug, Clone)]
pub struct Cyan {
    pub processors: Vec<CyanProcessor>,
    pub plugins: Vec<CyanPlugin>,
}
