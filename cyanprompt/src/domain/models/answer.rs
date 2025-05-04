use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
pub enum Answer {
    String(String),
    StringArray(Vec<String>),
    Bool(bool),
}

impl fmt::Debug for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Answer::String(s) => write!(f, "String({:?})", s),
            Answer::StringArray(arr) => write!(f, "StringArray({:?})", arr),
            Answer::Bool(b) => write!(f, "Bool({:?})", b),
        }
    }
}
