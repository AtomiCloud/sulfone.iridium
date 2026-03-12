//! Processor test execution flow.
//!
//! This module provides functionality for running processor tests:
//! - Docker container management with volume mounts
//! - API calls to processor endpoints
//! - Snapshot comparison

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;

use crate::test_cmd::config::{ExpectedOutput, TestCase, read_test_config};
use crate::test_cmd::container::{ContainerHandle, build_and_start_container, cleanup_container};
use crate::test_cmd::report::TestResult;
use crate::test_cmd::validation::{compare_directories, run_validate_commands};
use crate::try_cmd::ensure_daemon_running;

/// Run processor tests.
///
/// This function executes processor tests by:
/// - Building and starting the processor container with volume mounts
/// - Making API calls to the processor endpoint
/// - Comparing output against expected snapshots
///
/// # Arguments
///
/// * `processor_path` - Path to processor directory
/// * `test_filter` - Optional test name to filter by
/// * `parallel` - Number of parallel test cases
/// * `update_snapshots` - Update snapshots with actual output
/// * `config` - Path to cyan.yaml
/// * `output_dir` - Output directory for test results
/// * `junit_path` - Optional path for JUnit XML report
/// * `coordinator_endpoint` - Coordinator endpoint
/// * `disable_daemon_autostart` - Skip automatic daemon start
///
/// # Returns
///
/// Returns a vector of [`TestResult`] with results for each test case.
///
/// # Errors
///
/// Returns an error if:
/// - Test configuration cannot be read
/// - Warm-up fails
/// - Test execution fails
#[allow(clippy::too_many_arguments)]
pub fn run_processor_tests(
    processor_path: &str,
    test_filter: Option<&str>,
    parallel: usize,
    update_snapshots: bool,
    _config: &str,
    output_dir: &str,
    junit_path: Option<&str>,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Create output directory
    fs::create_dir_all(output_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create output directory {output_dir}: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Read test configuration
    println!("Loading test configuration from test.cyan.yaml...");
    let test_config_path = PathBuf::from(processor_path).join("test.cyan.yaml");
    let test_config = read_test_config(test_config_path.to_string_lossy().to_string())?;

    // Filter test cases by name if specified
    let test_cases: Vec<&TestCase> = if let Some(filter) = test_filter {
        test_config
            .tests
            .iter()
            .filter(|t| t.name == filter)
            .collect()
    } else {
        test_config.tests.iter().collect()
    };

    if test_cases.is_empty() {
        if test_filter.is_some() {
            return Err(Box::new(std::io::Error::other(format!(
                "Test case '{}' not found",
                test_filter.unwrap()
            ))) as Box<dyn Error + Send>);
        } else {
            println!("No tests found");
            return Ok(Vec::new());
        }
    }

    println!("Found {} test case(s) to run", test_cases.len());

    // Pre-flight validation
    println!("Running pre-flight validation...");
    let docker = bollard::Docker::connect_with_local_defaults()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    ensure_daemon_running(&docker, disable_daemon_autostart, coordinator_endpoint)?;

    // Collect all input directories for bind mounts
    let mut test_inputs = std::collections::HashMap::new();
    for test_case in &test_cases {
        if let Some(ref input) = test_case.input {
            if let Some(input_str) = input.as_str() {
                test_inputs.insert(test_case.name.clone(), input_str.to_string());
            }
        }
    }

    // Create temporary output directory
    let tmp_output_dir = PathBuf::from(output_dir).join("tmp");
    fs::create_dir_all(&tmp_output_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create temp output directory {}: {}",
            tmp_output_dir.display(),
            e
        ))) as Box<dyn Error + Send>
    })?;

    // Prepare bind mounts: mount all input dirs (read-only) and tmp output dir (read-write)
    let mut bind_mounts = Vec::new();
    let processor_path_abs = PathBuf::from(processor_path).canonicalize().map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to resolve processor path: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Mount input directories - keyed by test name
    for (test_name, input_dir) in &test_inputs {
        let input_path = processor_path_abs.join(input_dir);
        if !input_path.exists() {
            return Err(Box::new(std::io::Error::other(format!(
                "Input directory does not exist: {}",
                input_path.display()
            ))) as Box<dyn Error + Send>);
        }
        let input_path_abs = input_path.canonicalize().map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to resolve input path: {e}"
            ))) as Box<dyn Error + Send>
        })?;

        // Mount to /workspace/cyanprint/{test_name}
        let container_path = format!("/workspace/cyanprint/{test_name}");
        bind_mounts.push((
            input_path_abs.to_string_lossy().to_string(),
            container_path,
            true,
        ));
    }

    // Mount output directory (read-write)
    let tmp_output_abs = tmp_output_dir.canonicalize().map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to resolve tmp output path: {e}"
        ))) as Box<dyn Error + Send>
    })?;
    bind_mounts.push((
        tmp_output_abs.to_string_lossy().to_string(),
        "/workspace/area".to_string(),
        false,
    ));

    // Warm up processor
    println!("\nWarming up processor...");
    let container = processor_warmup(processor_path, bind_mounts)?;
    println!("Processor warmed up successfully");

    // Run tests, ensuring cleanup happens even on failure
    let start_time = Instant::now();
    let test_result = (|| -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
        // Run tests (parallel or sequential based on parallel count)
        println!("\nRunning tests...");

        let results = if parallel > 1 {
            run_processor_tests_parallel(
                test_cases,
                &container,
                processor_path,
                &tmp_output_dir,
                update_snapshots,
                parallel,
            )?
        } else {
            run_processor_tests_sequential(
                test_cases,
                &container,
                processor_path,
                &tmp_output_dir,
                update_snapshots,
            )?
        };

        Ok(results)
    })();

    let total_duration = start_time.elapsed();

    // Cleanup warm-up resources (always runs, even on error)
    println!("\nCleaning up processor resources...");
    let _ = cleanup_container(&container);

    // Cleanup tmp output directory (always runs, even on error)
    if tmp_output_dir.exists() {
        let _ = fs::remove_dir_all(&tmp_output_dir);
    }
    println!("Cleanup complete");

    let results = test_result?;

    // Write JUnit report if requested
    if let Some(junit_path) = junit_path {
        println!("Writing JUnit report to {junit_path}");
        crate::test_cmd::report::write_junit_report(&results, junit_path)?;
    }

    println!(
        "\nCompleted {} test(s) in {:.2}s",
        results.len(),
        total_duration.as_secs_f64()
    );

    Ok(results)
}

/// Warm up the processor for testing.
///
/// This function builds the processor image and starts the container
/// with bind mounts for input and output directories.
///
/// # Arguments
///
/// * `processor_path` - Path to processor directory
/// * `bind_mounts` - List of (host_path, container_path, read_only) tuples
///
/// # Returns
///
/// Returns a [`ContainerHandle`] with container details.
///
/// # Errors
///
/// Returns an error if:
/// - Build fails
/// - Container startup fails
/// - Health check fails
fn processor_warmup(
    processor_path: &str,
    bind_mounts: Vec<(String, String, bool)>,
) -> Result<ContainerHandle, Box<dyn Error + Send>> {
    // Build and start container with bind mounts
    // Processor listens on internal port 5551
    let container = build_and_start_container(
        processor_path,
        "processor",
        Some(bind_mounts),
        5551, // Internal port
    )?;

    println!(
        "Processor container started on port {}",
        container.host_port
    );

    Ok(container)
}

/// Run processor test cases sequentially.
fn run_processor_tests_sequential(
    test_cases: Vec<&TestCase>,
    container: &ContainerHandle,
    processor_path: &str,
    tmp_output_dir: &Path,
    update_snapshots: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    let mut results = Vec::new();

    for test_case in test_cases {
        let result = run_single_processor_test_case(
            test_case,
            container,
            processor_path,
            tmp_output_dir,
            update_snapshots,
        )?;
        results.push(result);
    }

    Ok(results)
}

/// Run processor test cases in parallel.
fn run_processor_tests_parallel(
    test_cases: Vec<&TestCase>,
    container: &ContainerHandle,
    processor_path: &str,
    tmp_output_dir: &Path,
    update_snapshots: bool,
    parallel_count: usize,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Use a semaphore to limit concurrency
    let semaphore = Arc::new(Semaphore::new(parallel_count));
    let results_mutex = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for test_case in test_cases {
        let test_case = test_case.clone();
        let container = container.clone();
        let processor_path = processor_path.to_string();
        let tmp_output_dir = tmp_output_dir.to_path_buf();
        let semaphore = Arc::clone(&semaphore);
        let results_mutex = Arc::clone(&results_mutex);

        let handle = thread::spawn(move || {
            // Acquire semaphore
            let _permit = semaphore.acquire();

            let result = run_single_processor_test_case(
                &test_case,
                &container,
                &processor_path,
                &tmp_output_dir,
                update_snapshots,
            );

            // Store result
            if let Ok(test_result) = result {
                let mut results = results_mutex.lock().unwrap();
                results.push(test_result);
            } else {
                // Handle error case
                let mut results = results_mutex.lock().unwrap();
                results.push(TestResult {
                    name: test_case.name.clone(),
                    passed: false,
                    duration: Duration::from_secs(0),
                    failure_message: Some(format!("Test failed: {:?}", result.unwrap_err())),
                });
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().map_err(|e| {
            Box::new(std::io::Error::other(format!("Thread panicked: {e:?}")))
                as Box<dyn Error + Send>
        })?;
    }

    let results = Arc::try_unwrap(results_mutex)
        .map_err(|_| {
            Box::new(std::io::Error::other("Failed to unwrap Arc")) as Box<dyn Error + Send>
        })?
        .into_inner()
        .map_err(|e| {
            Box::new(std::io::Error::other(format!("Mutex poisoned: {e}"))) as Box<dyn Error + Send>
        })?;

    Ok(results)
}

/// Run a single processor test case.
///
/// Makes a POST request to the processor API and compares output
/// against expected snapshots.
fn run_single_processor_test_case(
    test_case: &TestCase,
    container: &ContainerHandle,
    processor_path: &str,
    tmp_output_dir: &Path,
    update_snapshots: bool,
) -> Result<TestResult, Box<dyn Error + Send>> {
    let start_time = Instant::now();
    println!("Running test case: {}", test_case.name);

    let mut failure_message: Option<String> = None;

    // Build request body for processor API
    let mut request_body = serde_json::Map::new();

    // Read directory maps to /workspace/cyanprint/{test_name}
    request_body.insert(
        "readDir".to_string(),
        serde_json::json!(format!("/workspace/cyanprint/{}", test_case.name)),
    );

    // Write directory maps to /workspace/area/{test_name}
    request_body.insert(
        "writeDir".to_string(),
        serde_json::json!(format!("/workspace/area/{}", test_case.name)),
    );

    // Add globs if specified
    if let Some(ref globs) = test_case.globs {
        let glob_array: Vec<serde_json::Value> = globs
            .iter()
            .map(|g| {
                serde_json::json!({
                    "pattern": g.pattern,
                    "type": g.glob_type
                })
            })
            .collect();
        request_body.insert("globs".to_string(), serde_json::json!(glob_array));
    }

    // Add config if specified
    if let Some(ref config) = test_case.config {
        request_body.insert("config".to_string(), config.clone());
    }

    // Make API call to processor
    let http_client = Client::builder()
        .timeout(Duration::from_secs(300)) // 5 minute timeout for processing
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let processor_url = format!("http://localhost:{}/api/process", container.host_port);

    let response = http_client.post(&processor_url).json(&request_body).send();

    match response {
        Ok(resp) if resp.status().is_success() => {
            // Output is written to the bind-mounted tmp output directory
            println!("  Processing complete, output written to tmp directory");
        }
        Ok(resp) => {
            let status = resp.status();
            let error_text = resp.text().unwrap_or_else(|_| "Unknown error".to_string());
            failure_message = Some(format!(
                "Processor API returned error status {status}: {error_text}"
            ));
        }
        Err(e) => {
            failure_message = Some(format!("Failed to call processor API: {e}"));
        }
    }

    // If no API error, run validation and compare snapshots
    if failure_message.is_none() {
        let test_output_dir = tmp_output_dir.join(&test_case.name);

        // Run validate commands if specified
        if !test_case.validate.is_empty() {
            println!("  Running validate commands...");
            if test_output_dir.exists() {
                let validate_results =
                    run_validate_commands(test_output_dir.to_str().unwrap(), &test_case.validate)?;

                let validate_failures: Vec<&crate::test_cmd::validation::ValidateResult> =
                    validate_results.iter().filter(|r| !r.passed).collect();

                if !validate_failures.is_empty() {
                    let mut messages = Vec::new();
                    for result in &validate_failures {
                        messages.push(format!(
                            "Command '{}' failed: {}",
                            result.command, result.stderr
                        ));
                    }
                    failure_message = Some(format!(
                        "Validate commands failed:\n{}",
                        messages.join("\n")
                    ));
                }
            }
        }

        // Compare with expected snapshot if no validate failures
        if let ExpectedOutput::Snapshot { ref path } = test_case.expected {
            let expected_path = if path.starts_with('/') {
                // Absolute path
                PathBuf::from(path)
            } else {
                // Relative to processor directory
                PathBuf::from(processor_path).join(path)
            };

            if test_output_dir.exists() {
                println!(
                    "  Comparing with expected snapshot at {}...",
                    expected_path.display()
                );

                let comparison = compare_directories(
                    test_output_dir.to_str().unwrap(),
                    expected_path.to_str().unwrap(),
                )?;

                if !comparison.matched {
                    let mut messages = Vec::new();

                    for file in &comparison.mismatched_files {
                        messages.push(format!(
                            "File '{}' mismatched: {}",
                            file.path,
                            file.details.as_deref().unwrap_or("unknown error")
                        ));
                    }

                    for file in &comparison.extra_files {
                        messages.push(format!("Extra file: {file}"));
                    }

                    for file in &comparison.missing_files {
                        messages.push(format!("Missing file: {file}"));
                    }

                    if !comparison.skipped_binary_files.is_empty() {
                        messages.push(format!(
                            "Skipped {} binary files",
                            comparison.skipped_binary_files.len()
                        ));
                    }

                    failure_message = Some(format!(
                        "Snapshot comparison failed:\n{}",
                        messages.join("\n")
                    ));

                    // Update snapshots if requested
                    if update_snapshots {
                        println!("  Updating snapshot...");
                        copy_to_snapshot(&test_output_dir, &expected_path)?;
                        println!("  Snapshot updated");
                    }
                } else {
                    println!("  Snapshot matched");
                }
            } else {
                failure_message = Some(format!(
                    "Output directory not found: {}",
                    test_output_dir.display()
                ));
            }
        }
    }

    let duration = start_time.elapsed();

    println!(
        "  Test case '{}' completed in {:.2}s",
        test_case.name,
        duration.as_secs_f64()
    );

    Ok(TestResult {
        name: test_case.name.clone(),
        passed: failure_message.is_none(),
        duration,
        failure_message,
    })
}

/// Copy actual output to expected snapshot directory.
///
/// Used for --update-snapshots flag.
fn copy_to_snapshot(actual_dir: &Path, expected_dir: &Path) -> Result<(), Box<dyn Error + Send>> {
    // Remove expected directory if it exists
    if expected_dir.exists() {
        fs::remove_dir_all(expected_dir).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to remove expected snapshot directory {}: {}",
                expected_dir.display(),
                e
            ))) as Box<dyn Error + Send>
        })?;
    }

    // Create expected directory
    fs::create_dir_all(expected_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create expected snapshot directory {}: {}",
            expected_dir.display(),
            e
        ))) as Box<dyn Error + Send>
    })?;

    // Copy all files from actual to expected
    copy_recursive(actual_dir, expected_dir)
}

/// Copy directory recursively.
fn copy_recursive(from: &Path, to: &Path) -> Result<(), Box<dyn Error + Send>> {
    if !from.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(from).map_err(|e| Box::new(e) as Box<dyn Error + Send>)? {
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());

        if from_path.is_dir() {
            // Create target subdirectory before recursing
            fs::create_dir_all(&to_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            copy_recursive(&from_path, &to_path)?;
        } else {
            fs::copy(&from_path, &to_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        }
    }

    Ok(())
}

/// Simple semaphore for limiting parallel test execution.
struct Semaphore {
    permits: Arc<Mutex<usize>>,
    condvar: Arc<Condvar>,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Semaphore {
            permits: Arc::new(Mutex::new(permits)),
            condvar: Arc::new(Condvar::new()),
        }
    }

    fn acquire(&self) -> SemaphorePermit<'_> {
        let mut available = self.permits.lock().unwrap();
        while *available == 0 {
            available = self.condvar.wait(available).unwrap();
        }
        *available -= 1;
        SemaphorePermit { semaphore: self }
    }
}

struct SemaphorePermit<'a> {
    semaphore: &'a Semaphore,
}

impl<'a> Drop for SemaphorePermit<'a> {
    fn drop(&mut self) {
        let mut available = self.semaphore.permits.lock().unwrap();
        *available += 1;
        self.semaphore.condvar.notify_one();
    }
}
