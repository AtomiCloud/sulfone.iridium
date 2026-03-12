//! Test configuration parsing from `test.cyan.yaml`.
//!
//! This module provides types and functions for parsing test configuration
//! from YAML files into strongly-typed Rust structs.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Top-level test configuration parsed from `test.cyan.yaml`.
///
/// Contains a list of test cases to execute, along with optional global configuration.
///
/// # Example
///
/// ```yaml
/// tests:
///   - name: basic_template
///     expected:
///       path: ./snapshots/basic_template
///     answer_state:
///       - type: String
///         value: "my-project"
///     deterministic_state:
///       projectName: "my-project"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    /// List of test cases to execute
    #[serde(default)]
    pub tests: Vec<TestCase>,
}

/// A single test case definition.
///
/// Test cases are defined in `test.cyan.yaml` and specify:
/// - The test name
/// - Expected output (snapshot path or inline data)
/// - Optional type-specific configuration
///
/// Different test types use different subsets of these fields:
/// - **Template tests**: use `answer_state`, `deterministic_state`, `expected`
/// - **Processor tests**: use `input`, `expected`, `config`
/// - **Plugin tests**: use `input`, `expected`, `config`
/// - **Resolver tests**: use `resolver_input`, `resolver_expected`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Unique identifier for this test case
    pub name: String,

    /// Expected output for snapshot comparison
    pub expected: ExpectedOutput,

    /// Answer state for template Q&A (template tests only)
    ///
    /// Maps question IDs to their answers. The key is the question ID,
    /// and the value is the answer for that question.
    #[serde(default)]
    pub answer_state: std::collections::HashMap<String, AnswerStateEntry>,

    /// Deterministic state values (template tests only)
    #[serde(default)]
    pub deterministic_state: std::collections::HashMap<String, String>,

    /// Validate commands to run after test execution
    #[serde(default)]
    pub validate: Vec<String>,

    /// Input for processor/plugin tests
    #[serde(default)]
    pub input: Option<serde_json::Value>,

    /// File glob patterns for processor/plugin tests
    #[serde(default)]
    pub globs: Option<Vec<GlobEntry>>,

    /// Runtime configuration for processors/plugins (YAML in file, serde_json::Value in memory)
    #[serde(default)]
    pub config: Option<serde_json::Value>,

    /// Resolver input data (resolver tests only, used in Plan 2)
    #[serde(default)]
    pub resolver_input: Option<ResolverInput>,

    /// Resolver expected output (resolver tests only, used in Plan 2)
    #[serde(default)]
    pub resolver_expected: Option<ResolverExpected>,
}

/// Expected output for snapshot comparison.
///
/// Defines how to compare the actual output against expected results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ExpectedOutput {
    /// Compare against files in a directory path
    #[serde(rename = "snapshot")]
    Snapshot { path: String },

    /// Inline expected output (for simple test cases)
    #[serde(rename = "inline")]
    Inline(serde_json::Value),
}

/// Answer state entry for template Q&A.
///
/// Maps to [`cyanprompt::domain::models::answer::Answer`](../../cyanprompt/domain/models/answer/enum.Answer.html).
///
/// Used in template tests to pre-supply answers for Q&A questions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum AnswerStateEntry {
    /// String answer for text/date/password questions
    #[serde(rename = "String")]
    String(String),

    /// String array answer for checkbox/multiselect questions
    #[serde(rename = "StringArray")]
    StringArray(Vec<String>),

    /// Boolean answer for confirm questions
    #[serde(rename = "Bool")]
    Bool(bool),
}

/// File glob pattern entry.
///
/// Used in processor/plugin tests to specify which files to process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobEntry {
    /// Glob pattern (e.g., `"**/*.json"`, `"src/**/*.rs"`)
    pub pattern: String,

    /// File type identifier (processor/plugin specific)
    #[serde(rename = "type")]
    pub glob_type: String,
}

/// Resolver input data for resolver tests (defined now, used in Plan 2).
///
/// Contains input data to send to a conflict resolver for testing.
/// Matches Helium SDK ResolverInput structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverInput {
    /// Runtime configuration for resolver
    pub config: serde_json::Value,

    /// File variations to resolve
    pub files: Vec<ResolverFile>,
}

/// A single file variation in a resolver test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverFile {
    /// File path
    pub path: String,

    /// File content
    pub content: String,

    /// Origin metadata (template and layer)
    pub origin: ResolverFileOrigin,
}

/// Origin metadata for a file variation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverFileOrigin {
    /// Template ID that produced this file
    pub template: String,

    /// Layer index (order in composition)
    pub layer: i32,
}

/// Resolver expected output for resolver tests (defined now, used in Plan 2).
///
/// Contains expected output from a conflict resolver for testing.
/// Expected is an array of {path, content} pairs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverExpected {
    /// Expected resolved files as an array of {path, content} pairs
    pub files: Vec<ExpectedResolverFile>,
}

/// An expected resolved file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedResolverFile {
    /// Expected file path
    pub path: String,

    /// Expected resolved file content
    pub content: String,
}

/// Error type for test configuration parsing.
#[derive(Debug)]
pub enum TestConfigError {
    /// File not found
    FileNotFound(String),

    /// Invalid YAML syntax
    InvalidYaml(String),

    /// Missing required field
    MissingField(String),

    /// Invalid value for a field
    InvalidValue(String),
}

impl fmt::Display for TestConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestConfigError::FileNotFound(path) => {
                write!(f, "Test configuration file not found: {path}")
            }
            TestConfigError::InvalidYaml(msg) => {
                write!(f, "Invalid YAML in test configuration: {msg}")
            }
            TestConfigError::MissingField(field) => {
                write!(f, "Missing required field in test configuration: {field}")
            }
            TestConfigError::InvalidValue(msg) => {
                write!(f, "Invalid value in test configuration: {msg}")
            }
        }
    }
}

impl Error for TestConfigError {}

/// Read and parse test configuration from a YAML file.
///
/// # Arguments
///
/// * `path` - Path to `test.cyan.yaml` file
///
/// # Returns
///
/// Returns a [`TestConfig`] containing all test cases.
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The YAML is malformed
/// - Required fields are missing
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::config::read_test_config;
///
/// let config = read_test_config("test.cyan.yaml".to_string()).unwrap();
/// println!("Found {} test cases", config.tests.len());
/// ```
pub fn read_test_config(path: String) -> Result<TestConfig, Box<dyn Error + Send>> {
    // First check if file exists
    if !Path::new(&path).exists() {
        return Err(Box::new(TestConfigError::FileNotFound(path)) as Box<dyn Error + Send>);
    }

    // Try to read as full TestConfig first
    let f = File::open(&path).map_err(|e| {
        Box::new(TestConfigError::InvalidYaml(format!(
            "Failed to open file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    let config: TestConfig = serde_yaml::from_reader(f).map_err(|e| {
        Box::new(TestConfigError::InvalidYaml(format!(
            "YAML parsing error: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Validate uniqueness of test names
    let mut seen_names = std::collections::HashSet::new();
    for test_case in &config.tests {
        if !seen_names.insert(&test_case.name) {
            return Err(Box::new(TestConfigError::InvalidValue(format!(
                "Duplicate test name: {}",
                test_case.name
            ))) as Box<dyn Error + Send>);
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_config_minimal() {
        let yaml = r#"
tests:
  - name: basic_test
    expected:
      type: snapshot
      value:
        path: ./snapshots/basic
"#;
        let config: TestConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.tests.len(), 1);
        assert_eq!(config.tests[0].name, "basic_test");
    }

    #[test]
    fn test_parse_test_config_with_answer_state() {
        let yaml = r#"
tests:
  - name: template_with_answers
    expected:
      type: snapshot
      value:
        path: ./snapshots/template1
    answer_state:
      question_id_1:
        type: String
        value: "my-project"
      question_id_2:
        type: Bool
        value: true
    deterministic_state:
      projectName: "my-project"
      authorName: "John Doe"
"#;
        let config: TestConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.tests.len(), 1);
        let test = &config.tests[0];
        assert_eq!(test.name, "template_with_answers");
        assert_eq!(test.answer_state.len(), 2);
        assert_eq!(test.deterministic_state.len(), 2);

        // Check answer_state HashMap entries
        assert!(test.answer_state.contains_key("question_id_1"));
        assert!(test.answer_state.contains_key("question_id_2"));

        match &test.answer_state["question_id_1"] {
            AnswerStateEntry::String(s) => assert_eq!(s, "my-project"),
            _ => panic!("Expected String answer"),
        }

        match &test.answer_state["question_id_2"] {
            AnswerStateEntry::Bool(b) => assert!(*b),
            _ => panic!("Expected Bool answer"),
        }
    }

    #[test]
    fn test_parse_test_config_with_globs() {
        let yaml = r#"
tests:
  - name: processor_test
    expected:
      type: snapshot
      value:
        path: ./snapshots/processor
    globs:
      - pattern: "**/*.json"
        type: json
      - pattern: "src/**/*.rs"
        type: rust
    config:
      strategy: deep-merge
"#;
        let config: TestConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.tests.len(), 1);
        let test = &config.tests[0];
        assert_eq!(test.name, "processor_test");
        assert!(test.globs.is_some());
        let globs = test.globs.as_ref().unwrap();
        assert_eq!(globs.len(), 2);
        assert_eq!(globs[0].pattern, "**/*.json");
        assert_eq!(globs[0].glob_type, "json");
        assert!(test.config.is_some());
    }

    #[test]
    fn test_parse_test_config_with_resolver_input() {
        let yaml = r#"
tests:
  - name: resolver_basic_merge
    expected:
      type: snapshot
      value:
        path: ./snapshots/resolver1
    resolver_input:
      config:
        strategy: line-merge
      files:
        - path: config.json
          content: '{"key": "old"}'
          origin:
            template: template1
            layer: 0
        - path: config.json
          content: '{"key": "new"}'
          origin:
            template: template2
            layer: 1
    resolver_expected:
      files:
        - path: config.json
          content: '{"key": "new"}'
"#;
        let config: TestConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.tests.len(), 1);
        let test = &config.tests[0];
        assert_eq!(test.name, "resolver_basic_merge");
        assert!(test.resolver_input.is_some());
        assert!(test.resolver_expected.is_some());
        let input = test.resolver_input.as_ref().unwrap();
        assert_eq!(
            input.config.get("strategy").and_then(|v| v.as_str()),
            Some("line-merge")
        );
        assert_eq!(input.files.len(), 2);

        let expected = test.resolver_expected.as_ref().unwrap();
        assert_eq!(expected.files.len(), 1);
        assert_eq!(expected.files[0].path, "config.json");
    }
    #[test]
    fn test_parse_test_config_empty_tests() {
        let yaml = r#"
tests: []
"#;
        let config: TestConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.tests.len(), 0);
    }

    #[test]
    fn test_parse_test_config_with_validate_commands() {
        let yaml = r#"
tests:
  - name: test_with_validation
    expected:
      type: snapshot
      value:
        path: ./snapshots/validate
    validate:
      - test -f output/file.txt
      - grep "expected" output/file.txt
"#;
        let config: TestConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.tests.len(), 1);
        let test = &config.tests[0];
        assert_eq!(test.validate.len(), 2);
        assert_eq!(test.validate[0], "test -f output/file.txt");
        assert_eq!(test.validate[1], "grep \"expected\" output/file.txt");
    }

    #[test]
    fn test_parse_answer_state_entry_string() {
        let yaml = r#"
type: String
value: "hello world"
"#;
        let entry: AnswerStateEntry = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        match entry {
            AnswerStateEntry::String(s) => assert_eq!(s, "hello world"),
            _ => panic!("Expected String variant"),
        }
    }

    #[test]
    fn test_parse_answer_state_entry_string_array() {
        let yaml = r#"
type: StringArray
value:
  - item1
  - item2
  - item3
"#;
        let entry: AnswerStateEntry = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        match entry {
            AnswerStateEntry::StringArray(arr) => {
                assert_eq!(arr, vec!["item1", "item2", "item3"]);
            }
            _ => panic!("Expected StringArray variant"),
        }
    }

    #[test]
    fn test_parse_answer_state_entry_bool() {
        let yaml = r#"
type: Bool
value: true
"#;
        let entry: AnswerStateEntry = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        match entry {
            AnswerStateEntry::Bool(b) => assert!(b),
            _ => panic!("Expected Bool variant"),
        }
    }

    #[test]
    fn test_parse_expected_output_snapshot() {
        let yaml = r#"
type: snapshot
value:
  path: ./snapshots/test1
"#;
        let expected: ExpectedOutput = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        match expected {
            ExpectedOutput::Snapshot { path } => {
                assert_eq!(path, "./snapshots/test1");
            }
            _ => panic!("Expected Snapshot variant"),
        }
    }

    #[test]
    fn test_parse_expected_output_inline() {
        let yaml = r#"
type: inline
value:
  result: "success"
  count: 42
"#;
        let expected: ExpectedOutput = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        match expected {
            ExpectedOutput::Inline(value) => {
                assert_eq!(value.get("result").unwrap(), "success");
                assert_eq!(value.get("count").unwrap(), 42);
            }
            _ => panic!("Expected Inline variant"),
        }
    }

    #[test]
    fn test_read_test_config_not_found() {
        let result = read_test_config("nonexistent.yaml".to_string());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
