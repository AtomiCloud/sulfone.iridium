use std::error::Error;
use std::fmt;

/// Custom error type for selection errors
#[derive(Debug)]
pub struct SelectionError(pub String);

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for SelectionError {}

/// Parse template key into username and template name
pub fn parse_template_key(template_key: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = template_key.split('/').collect();
    (parts.len() == 2).then(|| (parts[0].to_string(), parts[1].to_string()))
}
