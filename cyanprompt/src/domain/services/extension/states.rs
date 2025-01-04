use crate::domain::models::cyan::Cyan;

pub enum ExtensionState {
    QnA(),
    Complete(Cyan),
    Err(String),
}

impl ExtensionState {
    pub fn cont(&self) -> bool {
        match self {
            ExtensionState::QnA() => true,
            ExtensionState::Complete(_) => false,
            ExtensionState::Err(_) => false,
        }
    }
}
