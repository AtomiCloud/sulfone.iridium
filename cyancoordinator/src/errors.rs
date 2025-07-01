use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum GenericError {
    ProblemDetails(ProblemDetails),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
    pub title: String,
    pub status: u16,
    #[serde(rename = "type")]
    pub t: String,
    pub trace_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

impl fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenericError::ProblemDetails(pd) => write!(f, "{pd:#?}"),
        }
    }
}

impl Error for GenericError {}
