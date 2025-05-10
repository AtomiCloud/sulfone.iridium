use super::generator::SessionIdGenerator;

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
