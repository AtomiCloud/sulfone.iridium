use rand::{Rng, distr::Alphanumeric};

/// Trait for generating session IDs
pub trait SessionIdGenerator {
    fn generate(&self) -> String;
}

/// Default implementation using alphanumeric characters
/// Creates a 10-character random alphanumeric string
#[derive(Default, Clone)]
pub struct DefaultSessionIdGenerator;

impl SessionIdGenerator for DefaultSessionIdGenerator {
    fn generate(&self) -> String {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect()
    }
}
