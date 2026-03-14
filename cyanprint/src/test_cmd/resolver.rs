//! Resolver test execution flow.
//!
//! This module provides functionality for running resolver tests:
//! - Docker container management
//! - Reading input files from directories (one per template/layer)
//! - Grouping conflicting files by path
//! - API calls to resolver endpoints
//! - Snapshot comparison of resolved output

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use std::sync::{Arc, Mutex};

use crate::test_cmd::semaphore::Semaphore;
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;

use crate::test_cmd::config::{ExpectedOutput, read_test_config};
use crate::test_cmd::container::{build_and_start_container, cleanup_container};
use crate::test_cmd::report::TestResult;
use crate::test_cmd::validation::compare_directories;

/// Run resolver tests.
///
/// This function executes resolver tests by:
/// - Building and starting resolver container
/// - Reading input files from directories or inline config
/// - Grouping conflicting files by path
/// - Making API calls to the resolver endpoint
/// - Comparing output against expected snapshots
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
    _coordinator_endpoint: &str,
    _disable_daemon_autostart: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
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

    // Pre-flight validation: only check Docker connectivity (resolver tests don't need the coordinator)
    println!("Running pre-flight validation...");
    let _docker = bollard::Docker::connect_with_local_defaults()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    println!("  Docker daemon is reachable");

    // Warm up resolver
    println!("\nWarming up resolver...");
    let container = resolver_warmup(resolver_path)?;
    println!("Resolver warmed up successfully");

    // Run tests (parallel or sequential based on parallel count)
    // Force sequential when updating snapshots to avoid read-modify-write races
    println!("\nRunning tests...");
    let start_time = Instant::now();

    let results_result = if parallel > 1 && !update_snapshots {
        run_resolver_tests_parallel(
            test_cases,
            &container,
            resolver_path,
            output_dir,
            parallel,
            update_snapshots,
        )
    } else {
        if update_snapshots && parallel > 1 {
            println!("  Note: Running sequentially because --update-snapshots is enabled");
        }
        run_resolver_tests_sequential(
            test_cases,
            &container,
            resolver_path,
            output_dir,
            update_snapshots,
        )
    };

    let total_duration = start_time.elapsed();

    // Cleanup warm-up resources (always, even on test failure)
    println!("\nCleaning up resolver resources...");
    if let Err(e) = cleanup_container(&container) {
        eprintln!("Warning: resolver container cleanup failed: {e}");
    }
    println!("Cleanup complete");

    // Propagate any test execution error after cleanup
    let results = results_result?;

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
    output_dir: &str,
    update_snapshots: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    let mut results = Vec::new();

    for test_case in test_cases {
        let result = run_single_resolver_test_case(
            test_case,
            container,
            resolver_path,
            output_dir,
            update_snapshots,
        )?;
        results.push(result);
    }

    Ok(results)
}

/// Run resolver test cases in parallel.
fn run_resolver_tests_parallel(
    test_cases: Vec<&crate::test_cmd::config::TestCase>,
    container: &crate::test_cmd::container::ContainerHandle,
    resolver_path: &str,
    output_dir: &str,
    parallel_count: usize,
    update_snapshots: bool,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    let semaphore = Arc::new(Semaphore::new(parallel_count));
    let results_mutex = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for test_case in test_cases {
        let test_case = test_case.clone();
        let container = container.clone();
        let resolver_path = resolver_path.to_string();
        let output_dir = output_dir.to_string();
        let semaphore = Arc::clone(&semaphore);
        let results_mutex = Arc::clone(&results_mutex);

        let handle = thread::spawn(move || {
            let _permit = semaphore.acquire();

            let result = run_single_resolver_test_case(
                &test_case,
                &container,
                &resolver_path,
                &output_dir,
                update_snapshots,
            );

            if let Ok(test_result) = result {
                let mut results = results_mutex.lock().unwrap();
                results.push(test_result);
            } else {
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

/// Collected file entry from an input directory.
struct CollectedFile {
    /// Relative path within the input directory
    relative_path: String,
    /// File content
    content: String,
    /// Origin metadata
    origin: crate::test_cmd::config::ResolverFileOrigin,
}

/// Read all files from an input directory and tag them with origin metadata.
fn collect_files_from_dir(
    dir_path: &Path,
    origin: &crate::test_cmd::config::ResolverFileOrigin,
) -> Result<Vec<CollectedFile>, Box<dyn Error + Send>> {
    let mut files = Vec::new();
    collect_files_recursive(dir_path, dir_path, origin, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    base: &Path,
    current: &Path,
    origin: &crate::test_cmd::config::ResolverFileOrigin,
    files: &mut Vec<CollectedFile>,
) -> Result<(), Box<dyn Error + Send>> {
    if !current.exists() {
        return Err(Box::new(std::io::Error::other(format!(
            "Input directory does not exist: {}",
            current.display()
        ))) as Box<dyn Error + Send>);
    }

    for entry in fs::read_dir(current).map_err(|e| Box::new(e) as Box<dyn Error + Send>)? {
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(base, &path, origin, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| {
                    Box::new(std::io::Error::other(format!(
                        "Failed to compute relative path: {e}"
                    ))) as Box<dyn Error + Send>
                })?
                .to_string_lossy()
                .replace('\\', "/");

            let content = fs::read_to_string(&path).map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "Failed to read file {}: {e}",
                    path.display()
                ))) as Box<dyn Error + Send>
            })?;

            files.push(CollectedFile {
                relative_path: relative,
                content,
                origin: origin.clone(),
            });
        }
    }

    Ok(())
}

/// Run a single resolver test case.
///
/// Reads files from input directories, groups by path, calls resolver,
/// writes resolved output, and compares against expected snapshot.
fn run_single_resolver_test_case(
    test_case: &crate::test_cmd::config::TestCase,
    container: &crate::test_cmd::container::ContainerHandle,
    resolver_path: &str,
    output_dir: &str,
    update_snapshots: bool,
) -> Result<TestResult, Box<dyn Error + Send>> {
    let start_time = Instant::now();
    println!("Running test case: {}", test_case.name);

    if test_case.resolver_inputs.is_none() {
        return Err(Box::new(std::io::Error::other(format!(
            "Test case '{}' missing resolver_inputs field",
            test_case.name
        ))) as Box<dyn Error + Send>);
    }

    run_resolver_test_directory_mode(
        test_case,
        container,
        resolver_path,
        output_dir,
        update_snapshots,
        start_time,
    )
}

/// Directory-based resolver test mode.
///
/// Reads files from input directories, groups by relative path,
/// calls the resolver for each path group, writes resolved files
/// to a tmp directory, and compares against expected snapshot.
fn run_resolver_test_directory_mode(
    test_case: &crate::test_cmd::config::TestCase,
    container: &crate::test_cmd::container::ContainerHandle,
    resolver_path: &str,
    output_dir: &str,
    update_snapshots: bool,
    start_time: Instant,
) -> Result<TestResult, Box<dyn Error + Send>> {
    let mut failure_message: Option<String> = None;

    let resolver_inputs = test_case.resolver_inputs.as_ref().unwrap();
    let config = test_case
        .config
        .as_ref()
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let resolver_base = PathBuf::from(resolver_path);

    // Collect all files from all input directories
    let mut all_files: Vec<CollectedFile> = Vec::new();
    for input_entry in resolver_inputs {
        let dir_path = resolver_base.join(&input_entry.path);
        let files = collect_files_from_dir(&dir_path, &input_entry.origin)?;
        if files.is_empty() {
            eprintln!(
                "  Warning: no files found in input directory {}",
                dir_path.display()
            );
        }
        all_files.extend(files);
    }

    if all_files.is_empty() {
        return Err(Box::new(std::io::Error::other(format!(
            "Test case '{}': no input files found across all resolver_inputs directories",
            test_case.name
        ))) as Box<dyn Error + Send>);
    }

    // Group files by relative path
    let mut path_groups: HashMap<String, Vec<&CollectedFile>> = HashMap::new();
    for file in &all_files {
        path_groups
            .entry(file.relative_path.clone())
            .or_default()
            .push(file);
    }

    println!(
        "  Found {} file(s) across {} unique path(s)",
        all_files.len(),
        path_groups.len()
    );

    // Create tmp output directory for this test case
    let test_output_dir = PathBuf::from(output_dir).join(&test_case.name);
    if test_output_dir.exists() {
        fs::remove_dir_all(&test_output_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    }
    fs::create_dir_all(&test_output_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Call resolver for each path group
    let http_client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let resolver_url = format!("http://localhost:{}/api/resolve", container.host_port);

    for (rel_path, files) in &path_groups {
        // Build resolver request: { config, files: [{path, content, origin: {template, layer}}] }
        let files_json: Vec<serde_json::Value> = files
            .iter()
            .map(|f| {
                serde_json::json!({
                    "path": f.relative_path,
                    "content": f.content,
                    "origin": {
                        "template": f.origin.template,
                        "layer": f.origin.layer
                    }
                })
            })
            .collect();

        let request_body = serde_json::json!({
            "config": config,
            "files": files_json
        });

        let response = http_client.post(&resolver_url).json(&request_body).send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                let response_text = resp
                    .text()
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

                let response_json: serde_json::Value =
                    serde_json::from_str(&response_text).map_err(|e| {
                        Box::new(std::io::Error::other(format!(
                            "Failed to parse resolver response for path '{rel_path}': {e}\nResponse: {response_text}"
                        ))) as Box<dyn Error + Send>
                    })?;

                // Resolver returns {path, content} — write content to the output dir
                let resolved_path = response_json
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(rel_path.as_str());
                let resolved_content = response_json
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let output_file = test_output_dir.join(resolved_path);
                if let Some(parent) = output_file.parent() {
                    fs::create_dir_all(parent).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                }
                fs::write(&output_file, resolved_content)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            }
            Ok(resp) => {
                let status = resp.status();
                let error_text = resp.text().unwrap_or_else(|_| "Unknown error".to_string());
                failure_message = Some(format!(
                    "Resolver API returned error for path '{rel_path}': {status}: {error_text}"
                ));
                break;
            }
            Err(e) => {
                failure_message = Some(format!(
                    "Failed to call resolver API for path '{rel_path}': {e}"
                ));
                break;
            }
        }
    }

    // Compare output against expected snapshot (if no API error)
    if failure_message.is_none() {
        let expected_dir = match &test_case.expected {
            ExpectedOutput::Snapshot { path } => {
                let p = resolver_base.join(path);
                p.to_string_lossy().to_string()
            }
            _ => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Test case '{}': directory-mode resolver tests require expected.type: snapshot",
                    test_case.name
                ))) as Box<dyn Error + Send>);
            }
        };

        if update_snapshots {
            // Copy output to snapshot directory
            println!("  Updating snapshot...");
            let snapshot_path = PathBuf::from(&expected_dir);
            if snapshot_path.exists() {
                fs::remove_dir_all(&snapshot_path)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            }
            copy_dir_recursive(&test_output_dir, &snapshot_path)?;
            println!("  Snapshot updated");
        } else {
            let comparison = compare_directories(test_output_dir.to_str().unwrap(), &expected_dir)?;

            if !comparison.matched {
                let mut details = Vec::new();
                for f in &comparison.missing_files {
                    details.push(format!("  Missing: {f}"));
                }
                for f in &comparison.extra_files {
                    details.push(format!("  Extra: {f}"));
                }
                for f in &comparison.mismatched_files {
                    details.push(format!(
                        "  Mismatch: {} ({})",
                        f.path,
                        f.details.as_deref().unwrap_or("unknown")
                    ));
                }
                failure_message = Some(format!(
                    "Resolver output does not match snapshot:\n{}",
                    details.join("\n")
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

/// Copy a directory recursively.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn Error + Send>> {
    fs::create_dir_all(dst).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    for entry in fs::read_dir(src).map_err(|e| Box::new(e) as Box<dyn Error + Send>)? {
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        }
    }

    Ok(())
}
