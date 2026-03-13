//! Test command subsystem for automated testing of CyanPrint artifacts.
//!
//! This module provides functionality for:
//! - Parsing test configuration from `test.cyan.yaml`
//! - Running validate commands and snapshot comparisons
//! - Generating human-readable and JUnit XML reports
//! - Executing template tests with non-interactive Q&A loops
//! - Executing processor, plugin, and resolver tests with Docker containers
//!
//! # Module Structure
//!
//! - [`config`]: Test configuration parsing from YAML files
//! - [`validation`]: Validate command execution and snapshot comparison
//! - [`report`]: Report formatting (human-readable and JUnit XML)
//! - [`template`]: Template test execution flow
//! - [`container`]: Docker container management for processor/plugin/resolver tests
//! - [`processor`]: Processor test execution flow
//! - [`plugin`]: Plugin test execution flow
//! - [`resolver`]: Resolver test execution flow
//! - [`init`]: Test initialization (placeholder for Plan 3)

pub mod config;
pub mod container;
pub mod init;
pub mod plugin;
pub mod processor;
pub mod report;
pub mod resolver;
pub mod semaphore;
pub mod template;
pub mod validation;

pub use config::{AnswerStateEntry, GlobEntry, TestCase, TestConfig, read_test_config};
pub use plugin::run_plugin_tests;
pub use processor::run_processor_tests;
pub use report::{TestResult, write_human_report, write_junit_report};
pub use resolver::run_resolver_tests;
pub use template::run_template_tests;
pub use validation::{
    ComparisonResult, FileComparisonResult, ValidateResult, compare_directories,
    run_validate_commands,
};
