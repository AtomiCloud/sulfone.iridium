
#[derive(Clone)]
pub enum Answer {
    String(String),
    StringArray(Vec<String>),
    Bool(bool),
}

