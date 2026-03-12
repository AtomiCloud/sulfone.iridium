//! Test command subsystem for automated testing of CyanPrint artifacts.
//!
//! This module provides functionality for:
//! - Parsing test configuration from `test.cyan.yaml`
//! - Running validate commands and snapshot comparisons
//! - Generating human-readable and JUnit XML reports
//! - Executing template tests with non-interactive Q&A loops
//!
//! # Module Structure
//!
//! - [`config`]: Test configuration parsing from YAML files
//! - [`validation`]: Validate command execution and snapshot comparison
//! - [`report`]: Report formatting (human-readable and JUnit XML)
//! - [`template`]: Template test execution flow
//! - [`init`]: Test initialization (placeholder for Plan 3)

pub mod config;
pub mod init;
pub mod report;
pub mod template;
pub mod validation;

pub use config::{
    AnswerStateEntry, GlobEntry, ResolverExpected, ResolverInput, TestCase, TestConfig,
    read_test_config,
};
pub use report::{TestResult, write_human_report, write_junit_report};
pub use template::run_template_tests;
pub use validation::{
    ComparisonResult, FileComparisonResult, ValidateResult, compare_directories,
    run_validate_commands,
};
