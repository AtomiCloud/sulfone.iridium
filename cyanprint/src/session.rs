use rand::{distr::Alphanumeric, Rng};

/// Trait for generating session IDs
pub trait SessionIdGenerator {
    fn generate(&self) -> String;
}

/// Default implementation using alphanumeric characters
/// Creates a 10-character random alphanumeric string
#[derive(Default)]
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

#[cfg(test)]
pub mod test {
    use super::*;

    /// Test implementation that returns predictable session IDs
    pub struct TestSessionIdGenerator {
        prefix: String,
        counter: std::cell::RefCell<usize>,
    }

    impl TestSessionIdGenerator {
        pub fn new(prefix: &str) -> Self {
            Self {
                prefix: prefix.to_string(),
                counter: std::cell::RefCell::new(0),
            }
        }
    }

    impl SessionIdGenerator for TestSessionIdGenerator {
        fn generate(&self) -> String {
            let current = self.counter.replace_with(|&mut c| c + 1);
            format!("{}-{}", self.prefix, current)
        }
    }
}
