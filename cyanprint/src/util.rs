use crate::errors::GenericError;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::error::Error;

pub fn generate_session_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

fn parse_ref_internal(s: String) -> Option<(String, String, Option<i64>)> {
    let mut parts = s.splitn(2, '/');
    let username = parts.next()?.to_string();
    let rest = parts.next()?;

    // Split the rest by ':'
    let mut parts = rest.splitn(2, ':');
    let name = parts.next()?.to_string();
    let version_str = parts.next();

    // Convert version string to u64 if present
    let version = match version_str {
        Some(v) => v.parse::<i64>().ok(),
        None => None,
    };
    Some((username, name, version))
}

pub fn parse_ref(s: String) -> Result<(String, String, Option<i64>), Box<dyn Error + Send>> {
    let err = s.clone();
    parse_ref_internal(s)
        .ok_or(Box::new(GenericError::FailedParsingReference(err)) as Box<dyn Error + Send>)
}
