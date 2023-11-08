use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum GenericError {
    FailedParsingReference(String),
}

impl Error for GenericError {}

impl fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenericError::FailedParsingReference(s) => write!(f, "Incorrect Reference: {}", s),
        }
    }
}
