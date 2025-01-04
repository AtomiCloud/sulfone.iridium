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
    title: String,
    status: u16,
    #[serde(rename = "type")]
    t: String,
    trace_id: Option<String>,
    data: Option<serde_json::Value>,
}

impl fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenericError::ProblemDetails(pd) => write!(f, "{:#?}", pd),
        }
    }
}

impl Error for GenericError {}
