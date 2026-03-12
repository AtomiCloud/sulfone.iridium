//! Resolver test execution flow.
//!
//! This module provides functionality for running resolver tests:
//! - Docker container management
//! - API calls to resolver endpoints
//! - JSON response validation

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;

use crate::test_cmd::config::read_test_config;
use crate::test_cmd::container::{build_and_start_container, cleanup_container};
use crate::test_cmd::report::TestResult;
use crate::try_cmd::ensure_daemon_running;

/// Run resolver tests.
///
/// This function executes resolver tests by:
/// - Building and starting resolver container
/// - Making API calls to resolver endpoint
/// - Comparing JSON responses against expected output
///
/// # Arguments
///
/// * `resolver_path` - Path to resolver directory
/// * `test_filter` - Optional test name to filter by
/// * `parallel` - Number of parallel test cases
/// * `update_snapshots` - Update snapshots with actual output
/// * `config` - Path to cyan.yaml
/// * `output_dir` - Output directory for test results
/// * `junit_path` - Optional path for JUnit XML report
/// * `coordinator_endpoint` - Coordinator endpoint (not used for resolvers)
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
pub fn run_resolver_tests(
    resolver_path: &str,
    test_filter: Option<&str>,
    parallel: usize,
    update_snapshots: bool,
    _config: &str,
    output_dir: &str,
    junit_path: Option<&str>,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Note: update_snapshots is used for resolver tests
    // We accept the parameter for API consistency with other test types

    // Create output directory
    fs::create_dir_all(output_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create output directory {output_dir}: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Read test configuration
    println!("Loading test configuration from test.cyan.yaml...");
    let test_config_path = PathBuf::from(resolver_path).join("test.cyan.yaml");
    let test_config = read_test_config(test_config_path.to_string_lossy().to_string())?;

    // Filter test cases by name if specified
    let test_cases: Vec<&crate::test_cmd::config::TestCase> = if let Some(filter) = test_filter {
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

    // Warm up resolver
    println!("\nWarming up resolver...");
    let container = resolver_warmup(resolver_path)?;
    println!("Resolver warmed up successfully");

    // Run tests (parallel or sequential based on parallel count)
    println!("\nRunning tests...");
    let start_time = Instant::now();

    let results = if parallel > 1 {
        run_resolver_tests_parallel(
            test_cases,
            &container,
            resolver_path,
            parallel,
            update_snapshots,
        )?
    } else {
        run_resolver_tests_sequential(test_cases, &container, resolver_path, update_snapshots)?
    };

    let total_duration = start_time.elapsed();

    // Cleanup warm-up resources
    println!("\nCleaning up resolver resources...");
    cleanup_container(&container)?;
    println!("Cleanup complete");

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

/// Warm up the resolver for testing.
///
/// This function builds the resolver image and starts the container
/// with health checking. Resolver tests are API-only, so no file mounts are needed.
///
/// # Arguments
///
/// * `resolver_path` - Path to the resolver directory
///
/// # Returns
///
/// Returns a [`crate::test_cmd::container::ContainerHandle`] with container details.
///
/// # Errors
///
/// Returns an error if:
/// - Build fails
/// - Container startup fails
/// - Health check fails
fn resolver_warmup(
    resolver_path: &str,
) -> Result<crate::test_cmd::container::ContainerHandle, Box<dyn Error + Send>> {
    // Build and start container with no bind mounts (API-only)
    // Resolver listens on internal port 5553
    let container = build_and_start_container(
        resolver_path,
        "resolver",
        None, // No bind mounts needed for resolver
        5553, // Internal port
    )?;

    println!("Resolver container started on port {}", container.host_port);

    Ok(container)
}

/// Run resolver test cases sequentially.
fn run_resolver_tests_sequential(
    test_cases: Vec<&crate::test_cmd::config::TestCase>,
    container: &crate::test_cmd::container::ContainerHandle,
    resolver_path: &str,
    update_snapshots: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    let mut results = Vec::new();

    for test_case in test_cases {
        let result =
            run_single_resolver_test_case(test_case, container, resolver_path, update_snapshots)?;
        results.push(result);
    }

    Ok(results)
}

/// Run resolver test cases in parallel.
fn run_resolver_tests_parallel(
    test_cases: Vec<&crate::test_cmd::config::TestCase>,
    container: &crate::test_cmd::container::ContainerHandle,
    resolver_path: &str,
    parallel_count: usize,
    update_snapshots: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Use a semaphore to limit concurrency
    let semaphore = Arc::new(Semaphore::new(parallel_count));
    let results_mutex = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for test_case in test_cases {
        let test_case = test_case.clone();
        let container = container.clone();
        let resolver_path = resolver_path.to_string();
        let semaphore = Arc::clone(&semaphore);
        let results_mutex = Arc::clone(&results_mutex);

        let handle = thread::spawn(move || {
            // Acquire semaphore
            let _permit = semaphore.acquire();

            let result = run_single_resolver_test_case(
                &test_case,
                &container,
                &resolver_path,
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

/// Run a single resolver test case.
///
/// Makes a POST request to the resolver API with test input
/// and compares the response against expected output using JSON deep comparison.
fn run_single_resolver_test_case(
    test_case: &crate::test_cmd::config::TestCase,
    container: &crate::test_cmd::container::ContainerHandle,
    resolver_path: &str,
    update_snapshots: bool,
) -> Result<TestResult, Box<dyn Error + Send>> {
    let start_time = Instant::now();
    println!("Running test case: {}", test_case.name);

    let mut failure_message: Option<String> = None;

    // Get resolver input and expected from test case
    let resolver_input = test_case.resolver_input.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(format!(
            "Test case '{}' missing resolver_input field",
            test_case.name
        ))) as Box<dyn Error + Send>
    })?;

    let resolver_expected = test_case.resolver_expected.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(format!(
            "Test case '{}' missing resolver_expected field",
            test_case.name
        ))) as Box<dyn Error + Send>
    })?;

    // Build request body for resolver API
    // Format: { config: {...}, files: [{path, content, origin}, ...] }
    let request_body = serde_json::json!(resolver_input);

    // Make API call to resolver
    let http_client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let resolver_url = format!("http://localhost:{}/api/resolve", container.host_port);

    let response = http_client.post(&resolver_url).json(&request_body).send();

    match response {
        Ok(resp) if resp.status().is_success() => {
            let response_text = resp
                .text()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

            // Parse response as JSON
            let response_json: serde_json::Value =
                serde_json::from_str(&response_text).map_err(|e| {
                    Box::new(std::io::Error::other(format!(
                        "Failed to parse resolver response as JSON: {e}\nResponse: {response_text}"
                    ))) as Box<dyn Error + Send>
                })?;

            // Build expected JSON for comparison
            let expected_json: serde_json::Value =
                serde_json::to_value(resolver_expected.files.clone())
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

            // JSON deep comparison (field order doesn't matter)
            if response_json != expected_json {
                failure_message = Some(format!(
                    "Resolver output mismatch:\nExpected: {}\nActual:   {}",
                    serde_json::to_string_pretty(&expected_json)
                        .unwrap_or_else(|_| "Invalid JSON".to_string()),
                    serde_json::to_string_pretty(&response_json)
                        .unwrap_or_else(|_| "Invalid JSON".to_string())
                ));

                // Update snapshots if requested
                if update_snapshots {
                    println!("  Updating snapshot...");
                    update_resolver_snapshot(resolver_path, &test_case.name, &response_text)?;
                    println!("  Snapshot updated");
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let error_text = resp.text().unwrap_or_else(|_| "Unknown error".to_string());
            failure_message = Some(format!(
                "Resolver API returned error status {status}: {error_text}"
            ));
        }
        Err(e) => {
            failure_message = Some(format!("Failed to call resolver API: {e}"));
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

/// Update resolver snapshot in test.cyan.yaml.
///
/// Reads the test configuration file, finds the specified test case,
/// and updates its resolver_expected.files field with the actual response.
///
/// # Arguments
///
/// * `resolver_path` - Path to the resolver directory containing test.cyan.yaml
/// * `test_name` - Name of the test case to update
/// * `actual_response` - Actual response JSON from the resolver API
///
/// # Errors
///
/// Returns an error if:
/// - The test.cyan.yaml file cannot be read or written
/// - The YAML cannot be parsed or serialized
/// - The test case is not found in the test configuration
/// - The test case is missing the resolver_expected field
/// - The actual response is not a JSON array (single objects are rejected)
fn update_resolver_snapshot(
    resolver_path: &str,
    test_name: &str,
    actual_response: &str,
) -> Result<(), Box<dyn Error + Send>> {
    use serde_yaml::Value;

    let test_config_path = PathBuf::from(resolver_path).join("test.cyan.yaml");

    // Read current YAML file
    let yaml_content = fs::read_to_string(&test_config_path).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to read test.cyan.yaml: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Parse as YAML value to preserve structure
    let mut yaml_value: serde_yaml::Value = serde_yaml::from_str(&yaml_content).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to parse test.cyan.yaml: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Track whether we found the test case
    let mut test_found = false;

    // Find and update the test case
    if let Some(tests) = yaml_value
        .get_mut("tests")
        .and_then(|v| v.as_sequence_mut())
    {
        for test in tests.iter_mut() {
            if let Some(name) = test.get("name").and_then(|v| v.as_str()) {
                if name == test_name {
                    test_found = true;

                    // Check that resolver_expected exists
                    let resolver_expected = test
                        .get_mut("resolver_expected")
                        .and_then(|v| v.as_mapping_mut())
                        .ok_or_else(|| {
                            Box::new(std::io::Error::other(format!(
                                "Test case '{test_name}' missing resolver_expected field"
                            ))) as Box<dyn Error + Send>
                        })?;

                    // Parse actual response as JSON
                    let actual_json: serde_json::Value = serde_json::from_str(actual_response)
                        .map_err(|e| {
                            Box::new(std::io::Error::other(format!(
                                "Failed to parse actual response as JSON: {e}"
                            ))) as Box<dyn Error + Send>
                        })?;

                    // Reject non-array responses - the spec requires array format
                    if !actual_json.is_array() {
                        return Err(Box::new(std::io::Error::other(format!(
                            "Resolver must return an array of {{path, content}} pairs, got: {}",
                            if actual_json.is_object() {
                                "single object (resolver spec requires array format)".to_string()
                            } else {
                                format!("{actual_json:?}")
                            }
                        ))) as Box<dyn Error + Send>);
                    }

                    // Update resolver_expected.files with the array
                    resolver_expected.insert(
                        Value::String("files".to_string()),
                        serde_yaml::to_value(actual_json)
                            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?,
                    );

                    break;
                }
            }
        }
    }

    // Return error if test case was not found
    if !test_found {
        return Err(Box::new(std::io::Error::other(format!(
            "Test case '{test_name}' not found in test.cyan.yaml"
        ))) as Box<dyn Error + Send>);
    }

    // Write back to file
    let updated_yaml = serde_yaml::to_string(&yaml_value).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to serialize updated YAML: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    fs::write(&test_config_path, updated_yaml).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to write test.cyan.yaml: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    Ok(())
}

/// Simple semaphore for limiting parallel test execution.
struct Semaphore {
    permits: Arc<Mutex<usize>>,
    cond: Arc<Condvar>,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Semaphore {
            permits: Arc::new(Mutex::new(permits)),
            cond: Arc::new(Condvar::new()),
        }
    }

    fn acquire(&self) -> SemaphorePermit<'_> {
        let mut available = self.permits.lock().unwrap();
        while *available == 0 {
            available = self.cond.wait(available).unwrap();
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
        self.semaphore.cond.notify_one();
    }
}
