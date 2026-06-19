//! Template test execution flow.
//!
//! This module provides functionality for:
//! - Template warm-up (Docker validation, building images, starting container)
//! - Non-interactive Q&A loop (bypassing TemplateEngine.start_with)
//! - Per-test-case execution with snapshot comparison
//! - Run-scoped container ownership via `RunGuard` (Drop-based cleanup)

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::test_cmd::semaphore::Semaphore;
use std::thread;
use std::time::{Duration, Instant};

use bollard::Docker;
use reqwest::blocking::Client;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{
    DefaultVfs, DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker,
};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::session::{DefaultSessionIdGenerator, SessionIdGenerator};
use cyancoordinator::template::DefaultTemplateExecutor;
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::models::question::QuestionTrait;
use cyanprompt::domain::models::template::{input::TemplateAnswerInput, output::TemplateOutput};
use cyanprompt::domain::services::repo::{CyanHttpRepo, CyanRepo};
use cyanprompt::http::client::CyanClient;
use cyanregistry::cli::mapper::read_build_config;
use cyanregistry::cli::models::template_config::CyanTemplateFileConfig;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use tokio::runtime::Builder;

use crate::command_executor::CommandExecutor;
use crate::docker::buildx::BuildxBuilder;
use crate::port::{TEMPLATE_TEST, TEMPLATE_TEST_END, allocate_port};
use crate::test_cmd::config::ExpectedOutput;
use crate::test_cmd::config::{AnswerStateEntry, TestCase, read_test_config};
use crate::test_cmd::container::RunGuard;
use crate::test_cmd::report::TestResult;
use crate::test_cmd::validation::{ValidateResult, compare_directories, run_validate_commands};
use crate::try_cmd::{ensure_daemon_running, split_image_ref};
use cyancoordinator::operations::composition::DependencyResolver;
use cyancoordinator::operations::composition::operator::CompositionOperator;
use cyancoordinator::operations::composition::resolver::DefaultDependencyResolver;

/// RAII guard that ensures coordinator session cleanup is always called.
///
/// This guard will call `try_cleanup()` on the coordinator client when dropped,
/// ensuring cleanup happens even on early returns or errors.
struct SessionCleanupGuard<'a> {
    coord_client: &'a CyanCoordinatorClient,
    session_id: String,
    armed: bool,
}

impl<'a> SessionCleanupGuard<'a> {
    /// Create a new cleanup guard.
    ///
    /// The guard will call `try_cleanup()` when dropped.
    fn new(coord_client: &'a CyanCoordinatorClient, session_id: String) -> Self {
        Self {
            coord_client,
            session_id,
            armed: true,
        }
    }
}

impl<'a> Drop for SessionCleanupGuard<'a> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.coord_client.try_cleanup(&self.session_id);
        }
    }
}

/// Template warm-up context.
///
/// Contains all resources created during warm-up phase.
///
/// Note: Clone is implemented with `docker: None` for parallel execution.
struct TemplateWarmup {
    /// Template version data
    template: TemplateVersionRes,

    /// Local template ID
    local_template_id: String,

    /// Run-scoped UUID for container labeling and cleanup
    run_id: String,

    /// Template container name
    container_name: Option<String>,

    /// Template image reference
    template_image_ref: Option<String>,

    /// Port template server is running on
    port: Option<u16>,

    /// Blob image reference (for cleanup)
    blob_image_ref: Option<String>,

    /// Docker client (not cloned for parallel execution)
    docker: Option<Docker>,

    /// Whether using dev mode (external template server)
    dev_mode: bool,
}

/// Custom Clone implementation for TemplateWarmup.
///
/// Docker client doesn't implement Clone, so we set it to None when cloning.
/// This is safe for parallel test execution since tests use `warmup.port` to
/// communicate with the template container, not the Docker client directly.
impl Clone for TemplateWarmup {
    fn clone(&self) -> Self {
        Self {
            template: self.template.clone(),
            local_template_id: self.local_template_id.clone(),
            run_id: self.run_id.clone(),
            container_name: self.container_name.clone(),
            template_image_ref: self.template_image_ref.clone(),
            port: self.port,
            blob_image_ref: self.blob_image_ref.clone(),
            docker: None,
            dev_mode: self.dev_mode,
        }
    }
}

/// Run template tests.
///
/// # Arguments
///
/// * `template_path` - Path to template directory
/// * `test_filter` - Optional test name to filter by
/// * `parallel` - Number of parallel test cases
/// * `update_snapshots` - Update snapshots with actual output
/// * `config` - Path to cyan.yaml
/// * `output_dir` - Output directory for test results
/// * `junit_path` - Optional path for JUnit XML report
/// * `coordinator_endpoint` - Coordinator endpoint
/// * `disable_daemon_autostart` - Skip automatic daemon start
/// * `skip_deps` - Skip template dependencies and test the root template in
///   isolation. When `false` (the default), template dependencies are composed
///   in so the final merged state is tested.
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
pub fn run_template_tests(
    template_path: &str,
    test_filter: Option<&str>,
    parallel: usize,
    update_snapshots: bool,
    config: &str,
    output_dir: &str,
    junit_path: Option<&str>,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    skip_deps: bool,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Create output directory
    fs::create_dir_all(output_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create output directory {output_dir}: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Read test configuration
    println!("Loading test configuration from test.cyan.yaml...");
    let test_config_path = PathBuf::from(template_path).join("test.cyan.yaml");
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

    // Decide whether to compose template dependencies into the run.
    // By default we include them so the final merged state is tested; `--skip-deps`
    // (skip_deps=true) tests the root template in isolation.
    let has_template_deps = template_has_dependencies(template_path, config)?;
    let include_deps = !skip_deps && has_template_deps;

    if skip_deps && has_template_deps {
        println!(
            "Skipping template dependencies (--skip-deps): testing the root template in isolation"
        );
    } else if include_deps {
        println!("Composing template dependencies in: testing the final merged state");
    }

    // Generate a run-scoped UUID for container ownership and cleanup.
    // All containers created during this run will be labeled with this UUID,
    // and the RunGuard will clean them up when it drops (even on panic).
    let run_id = uuid::Uuid::new_v4().to_string();
    let _run_guard = RunGuard::new(run_id.clone());
    println!("Run ID: {run_id}");

    let start_time = Instant::now();
    let results_result = if include_deps {
        run_composition_tests(
            test_cases,
            template_path,
            config,
            output_dir,
            update_snapshots,
            coordinator_endpoint,
            disable_daemon_autostart,
            parallel,
            registry_client,
        )
    } else {
        run_isolated_tests(
            test_cases,
            template_path,
            config,
            output_dir,
            update_snapshots,
            coordinator_endpoint,
            disable_daemon_autostart,
            parallel,
            registry_client,
            &run_id,
        )
    };

    let total_duration = start_time.elapsed();

    // Propagate any test execution error
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

    // Clean up temporary output directory
    if PathBuf::from(output_dir).exists() {
        if let Err(e) = fs::remove_dir_all(output_dir) {
            eprintln!("Warning: failed to clean up output directory {output_dir}: {e}");
        }
    }

    Ok(results)
}

/// Read the template's `cyan.yaml` and report whether it declares any template
/// dependencies (i.e. it is a composition / group with child templates).
fn template_has_dependencies(
    template_path: &str,
    config_path: &str,
) -> Result<bool, Box<dyn Error + Send>> {
    let full_config_path = PathBuf::from(template_path).join(config_path);
    let content = fs::read_to_string(&full_config_path).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to read config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;
    let template_config: CyanTemplateFileConfig = serde_yaml::from_str(&content).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to parse config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;
    Ok(!template_config.templates.is_empty())
}

/// Run the isolated (root-only) test path: warm a single template container and
/// execute each test case against it without composing template dependencies.
///
/// This is the path used when `--skip-deps` is set or when the template has no
/// dependencies.
#[allow(clippy::too_many_arguments)]
fn run_isolated_tests(
    test_cases: Vec<&TestCase>,
    template_path: &str,
    config: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    parallel: usize,
    registry_client: &CyanRegistryClient,
    run_id: &str,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Warm up template (build images, start container, health check)
    println!("\nWarming up template...");
    let warmup = template_warmup(
        template_path,
        config,
        coordinator_endpoint,
        disable_daemon_autostart,
        registry_client,
        run_id,
    )?;

    println!("Template warmed up successfully");

    // Run tests (parallel or sequential based on parallel count)
    println!("\nRunning tests...");
    let results_result = if parallel > 1 {
        run_tests_parallel(
            test_cases,
            &warmup,
            template_path,
            output_dir,
            update_snapshots,
            coordinator_endpoint,
            parallel,
            registry_client,
        )
    } else {
        run_tests_sequential(
            test_cases,
            &warmup,
            template_path,
            output_dir,
            update_snapshots,
            coordinator_endpoint,
            registry_client,
        )
    };

    // Cleanup warm-up resources (always, even on test failure)
    println!("\nCleaning up template resources...");
    if let Err(e) = cleanup_warmup(&warmup) {
        eprintln!("Warning: template warmup cleanup failed: {e}");
    }
    println!("Cleanup complete");

    results_result
}

/// Warm-up artifacts for the composition (dependency-inclusive) test path.
///
/// Unlike [`TemplateWarmup`], this does not start a long-lived template
/// container — the [`CompositionOperator`] warms the root and each dependency
/// itself, per execution. We only need the locally-built images (referenced by
/// the synthetic template's properties) to exist in the local Docker daemon.
struct CompositionWarmup {
    /// Synthetic root template with locally-built image properties.
    synthetic_template: TemplateVersionRes,
    /// Built template image reference (for cleanup).
    template_image_ref: String,
    /// Built blob image reference (for cleanup).
    blob_image_ref: String,
    /// Docker client (for image cleanup).
    docker: Docker,
}

/// Build the images and synthetic template needed for composition execution.
///
/// Mirrors the build steps of [`template_warmup`] but stops short of starting a
/// template container, since composition execution warms templates on demand.
fn composition_warmup(
    template_path: &str,
    config_path: &str,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    registry_client: &CyanRegistryClient,
) -> Result<CompositionWarmup, Box<dyn Error + Send>> {
    // Read configuration
    let full_config_path = PathBuf::from(template_path).join(config_path);
    let content = fs::read_to_string(full_config_path).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to read config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    let template_config: CyanTemplateFileConfig = serde_yaml::from_str(&content).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to parse config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Pre-flight validation (tests always build, so dev_mode=false)
    println!("Running pre-flight validation...");
    crate::try_cmd::pre_flight_validation(template_path, false)?;

    // Ensure daemon is running
    println!("Ensuring daemon is running...");
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    ensure_daemon_running(&docker, disable_daemon_autostart, coordinator_endpoint)?;

    // Resolve and pin dependencies (root images are local; deps come from registry)
    println!("Resolving and pinning dependencies...");
    let pinned = crate::try_cmd::resolve_and_pin_dependencies(registry_client, &template_config)?;

    // Build images
    println!("Building template and blob images...");
    let build_config_path = PathBuf::from(template_path).join(config_path);
    let build_config = read_build_config(build_config_path.to_string_lossy().to_string())?;

    let registry = build_config.registry.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No registry configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let images = build_config.images.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No images configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let (blob_docker_ref, template_docker_ref) =
        build_template_images(registry, images, template_path)?;

    println!("Images built successfully");

    // Create synthetic root template with locally-built image properties.
    println!("Creating synthetic template object...");
    let local_template_id = uuid::Uuid::new_v4().to_string();
    let build_result = Some((
        Some(blob_docker_ref.clone()),
        Some(template_docker_ref.clone()),
    ));

    let synthetic_template = crate::try_cmd::build_synthetic_template(
        &local_template_id,
        &template_config,
        &pinned,
        false, // dev_mode=false — root has real, locally-built images
        build_result.as_ref(),
    )?;

    println!("Synthetic template created");

    Ok(CompositionWarmup {
        synthetic_template,
        template_image_ref: template_docker_ref,
        blob_image_ref: blob_docker_ref,
        docker,
    })
}

/// Remove the images built for the composition warmup.
fn cleanup_composition_warmup(warmup: &CompositionWarmup) -> Result<(), Box<dyn Error + Send>> {
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    for image_ref in [&warmup.template_image_ref, &warmup.blob_image_ref] {
        println!("  Removing image: {image_ref}");
        if let Err(e) = runtime.block_on(async {
            warmup
                .docker
                .remove_image(
                    image_ref,
                    None::<bollard::query_parameters::RemoveImageOptions>,
                    None::<bollard::auth::DockerCredentials>,
                )
                .await
                .map(|_| ())
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        }) {
            eprintln!("  Warning: failed to remove image {image_ref}: {e}");
        }
    }

    Ok(())
}

/// Run the composition (dependency-inclusive) test path.
///
/// Builds the root template's images once, then for each test case executes the
/// full dependency tree (root + dependencies) via the [`CompositionOperator`]
/// and compares the layered, final state against the snapshot.
#[allow(clippy::too_many_arguments)]
fn run_composition_tests(
    test_cases: Vec<&TestCase>,
    template_path: &str,
    config: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    parallel: usize,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    println!("\nWarming up template (composition mode)...");
    let warmup = composition_warmup(
        template_path,
        config,
        coordinator_endpoint,
        disable_daemon_autostart,
        registry_client,
    )?;
    println!("Template warmed up successfully");

    println!("\nRunning tests...");
    let results_result = if parallel > 1 {
        run_composition_tests_parallel(
            test_cases,
            &warmup.synthetic_template,
            template_path,
            output_dir,
            update_snapshots,
            coordinator_endpoint,
            parallel,
            registry_client,
        )
    } else {
        let mut results = Vec::new();
        for test_case in test_cases {
            let result = run_single_composition_test_case(
                test_case,
                &warmup.synthetic_template,
                template_path,
                output_dir,
                update_snapshots,
                coordinator_endpoint,
                registry_client,
            )?;
            results.push(result);
        }
        Ok(results)
    };

    // Cleanup built images (always, even on failure)
    println!("\nCleaning up template resources...");
    if let Err(e) = cleanup_composition_warmup(&warmup) {
        eprintln!("Warning: composition warmup cleanup failed: {e}");
    }
    println!("Cleanup complete");

    results_result
}

/// Run composition test cases in parallel, bounded by `parallel_count`.
///
/// Each test case is fully independent (own coordinator sessions and
/// composition operator), so it is safe to run concurrently.
#[allow(clippy::too_many_arguments)]
fn run_composition_tests_parallel(
    test_cases: Vec<&TestCase>,
    synthetic_template: &TemplateVersionRes,
    template_path: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    parallel_count: usize,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    let semaphore = Arc::new(Semaphore::new(parallel_count));
    let results_mutex = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    let registry_endpoint = Arc::new(registry_client.endpoint.clone());
    let registry_version = Arc::new(registry_client.version.clone());

    for test_case in test_cases {
        let test_case = test_case.clone();
        let synthetic_template = synthetic_template.clone();
        let template_path = template_path.to_string();
        let output_dir = output_dir.to_string();
        let coordinator_endpoint = coordinator_endpoint.to_string();
        let semaphore = Arc::clone(&semaphore);
        let results_mutex = Arc::clone(&results_mutex);
        let registry_endpoint = Arc::clone(&registry_endpoint);
        let registry_version = Arc::clone(&registry_version);

        let handle = thread::spawn(move || {
            let _permit = semaphore.acquire();

            // Create a per-thread registry client (Rc is not Send, so build in-thread)
            let thread_registry = CyanRegistryClient {
                endpoint: (*registry_endpoint).clone(),
                version: (*registry_version).clone(),
                client: Rc::new(reqwest::blocking::Client::builder().build().unwrap()),
            };

            let result = run_single_composition_test_case(
                &test_case,
                &synthetic_template,
                &template_path,
                &output_dir,
                update_snapshots,
                &coordinator_endpoint,
                &thread_registry,
            );

            let mut results = results_mutex.lock().unwrap();
            match result {
                Ok(test_result) => results.push(test_result),
                Err(e) => results.push(TestResult {
                    name: test_case.name.clone(),
                    passed: false,
                    duration: Duration::from_secs(0),
                    failure_message: Some(format!("Test failed: {e:?}")),
                }),
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

/// Run a single composition test case: execute the full dependency tree and
/// compare the layered output against the expected snapshot.
fn run_single_composition_test_case(
    test_case: &TestCase,
    synthetic_template: &TemplateVersionRes,
    template_path: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    registry_client: &CyanRegistryClient,
) -> Result<TestResult, Box<dyn Error + Send>> {
    let start_time = Instant::now();
    println!("Running test case (with dependencies): {}", test_case.name);

    // Build answers + deterministic state from the test case
    let mut answers: HashMap<String, Answer> = HashMap::new();
    let mut deterministic_state: HashMap<String, String> = HashMap::new();

    for (key, value) in &test_case.deterministic_state {
        deterministic_state.insert(key.clone(), value.clone());
    }
    for (question_id, answer_entry) in &test_case.answer_state {
        match answer_entry {
            AnswerStateEntry::String(s) => {
                answers.insert(question_id.clone(), Answer::String(s.clone()));
            }
            AnswerStateEntry::StringArray(arr) => {
                answers.insert(question_id.clone(), Answer::StringArray(arr.clone()));
            }
            AnswerStateEntry::Bool(b) => {
                answers.insert(question_id.clone(), Answer::Bool(*b));
            }
        }
    }

    // Build the composition operator (same wiring as `try group` / `run`)
    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.to_string());
    let rc_registry = Rc::new(CyanRegistryClient {
        endpoint: registry_client.endpoint.clone(),
        version: registry_client.version.clone(),
        client: Rc::clone(&registry_client.client),
    });

    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    let merger = Box::new(GitLikeMerger::new(false, 50));
    let writer = Box::new(DiskFileWriter);
    let template_executor = Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));
    let session_id_generator: Box<dyn SessionIdGenerator> = Box::new(DefaultSessionIdGenerator);
    let template_history = Box::new(cyancoordinator::template::DefaultTemplateHistory::new());

    let template_operator = TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
        rc_registry.clone(),
    );
    let dependency_resolver = Box::new(DefaultDependencyResolver::new(rc_registry));
    let mut composition_operator = CompositionOperator::with_client(
        template_operator,
        dependency_resolver,
        coord_client.clone(),
    );

    // Execute composition (resolves deps, warms each, runs non-interactive Q&A, layers)
    println!("  Executing composition for {}...", test_case.name);
    let (vfs_output, _final_state, session_ids, resolved_commands) = composition_operator
        .execute_template(synthetic_template, &answers, &deterministic_state)?;

    // Clean up coordinator sessions created during composition (before any
    // further fallible step so sessions are never leaked on a later error)
    for sid in &session_ids {
        if let Err(e) = coord_client.clean(sid.clone()) {
            eprintln!("  Warning: failed to cleanup session {sid}: {e}");
        }
    }

    // Prepare clean output directory
    let test_output_dir = PathBuf::from(output_dir).join(&test_case.name);
    if test_output_dir.exists() {
        fs::remove_dir_all(&test_output_dir).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to clear test output directory {}: {}",
                test_output_dir.display(),
                e
            ))) as Box<dyn Error + Send>
        })?;
    }
    fs::create_dir_all(&test_output_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Write the layered (final) state to disk
    composition_operator
        .get_vfs()
        .write_to_disk(&test_output_dir, &vfs_output)?;

    println!("  Output written successfully");

    // Execute post-template commands (collected from the full dependency tree)
    if !resolved_commands.is_empty() {
        println!(
            "  Executing {} post-template command(s)...",
            resolved_commands.len()
        );
        let exec_result = CommandExecutor::execute_commands_non_interactive(
            &resolved_commands,
            &test_output_dir,
        )?;
        if !exec_result.all_succeeded() {
            let cmd_msg = format!(
                "Command execution failed: {}/{} succeeded, {}/{} failed",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            );
            return Ok(TestResult {
                name: test_case.name.clone(),
                passed: false,
                duration: start_time.elapsed(),
                failure_message: Some(cmd_msg),
            });
        }
    }

    // Compare with expected snapshot, then run validate commands
    let failure_message =
        compare_and_validate(test_case, &test_output_dir, template_path, update_snapshots)?;

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

/// Warm up the template for testing.
///
/// This function:
/// - Validates Docker + cyan.yaml
/// - Resolves and pins dependencies
/// - Builds template and blob images
/// - Creates synthetic template object
/// - Starts template container
/// - Health checks template container
///
/// Note: Template tests always use build mode (dev_mode=false).
fn template_warmup(
    template_path: &str,
    config_path: &str,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    registry_client: &CyanRegistryClient,
    run_id: &str,
) -> Result<TemplateWarmup, Box<dyn Error + Send>> {
    // Template tests always use build mode (dev_mode=false)
    let _dev_mode = false;

    // Read configuration
    let full_config_path = PathBuf::from(template_path).join(config_path);
    let content = fs::read_to_string(full_config_path).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to read config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    let template_config: CyanTemplateFileConfig = serde_yaml::from_str(&content).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to parse config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Pre-flight validation (pass dev_mode=false since tests always build)
    println!("Running pre-flight validation...");
    crate::try_cmd::pre_flight_validation(template_path, false)?;

    // Ensure daemon is running (always check, even with --disable-daemon-autostart)
    println!("Ensuring daemon is running...");
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    ensure_daemon_running(&docker, disable_daemon_autostart, coordinator_endpoint)?;

    // Resolve and pin dependencies
    println!("Resolving and pinning dependencies...");
    let pinned = crate::try_cmd::resolve_and_pin_dependencies(registry_client, &template_config)?;

    // Build images
    println!("Building template and blob images...");
    let build_config_path = PathBuf::from(template_path).join(config_path);
    let build_config = read_build_config(build_config_path.to_string_lossy().to_string())?;

    let registry = build_config.registry.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No registry configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let images = build_config.images.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No images configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let (blob_docker_ref, template_docker_ref) =
        build_template_images(registry, images, template_path)?;

    println!("Images built successfully");

    // Create synthetic template
    println!("Creating synthetic template object...");
    let local_template_id = uuid::Uuid::new_v4().to_string();
    let build_result = Some((
        Some(blob_docker_ref.clone()),
        Some(template_docker_ref.clone()),
    ));

    let template = crate::try_cmd::build_synthetic_template(
        &local_template_id,
        &template_config,
        &pinned,
        false, // dev_mode=false
        build_result.as_ref(),
    )?;

    println!("Synthetic template created");

    // Find available port and start template container
    println!("Starting template container...");
    let image_ref = template_docker_ref;
    let mut port: u16 = 0;
    let mut last_err: Option<Box<dyn Error + Send>> = None;
    let mut container_name = String::new();

    for _ in 0..3 {
        container_name = format!(
            "cyan-template-{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        );
        let Some(port_alloc) = allocate_port(TEMPLATE_TEST, TEMPLATE_TEST_END) else {
            last_err = Some(Box::new(std::io::Error::other(format!(
                "No available port found in range {TEMPLATE_TEST}-{TEMPLATE_TEST_END} after 3 retries"
            ))) as Box<dyn Error + Send>);
            continue;
        };
        port = port_alloc.release();

        match crate::try_cmd::start_template_container(
            &docker,
            &container_name,
            &image_ref,
            port,
            coordinator_endpoint,
            "cyanprint.test",
            Some(run_id),
        ) {
            Ok(()) => {
                last_err = None;
                break;
            }
            Err(e) => {
                // Clean up any partially created container before retrying
                crate::try_cmd::stop_and_remove_container(&docker, &container_name);
                last_err = Some(e);
            }
        }
    }
    if let Some(e) = last_err {
        return Err(e);
    }

    println!("Template container started on port {port}");

    // Health check template container
    println!("Health checking template container...");
    crate::try_cmd::health_check_template_container(port, 30, 2)?;

    Ok(TemplateWarmup {
        template,
        local_template_id,
        run_id: run_id.to_string(),
        container_name: Some(container_name),
        template_image_ref: Some(image_ref),
        port: Some(port),
        blob_image_ref: Some(blob_docker_ref),
        docker: Some(docker),
        dev_mode: false,
    })
}

/// Build template and blob images.
///
/// Returns the built image references.
fn build_template_images(
    registry: &str,
    images: &cyanregistry::cli::models::build_config::ImagesConfig,
    template_path: &str,
) -> Result<(String, String), Box<dyn Error + Send>> {
    let template_path_abs = PathBuf::from(template_path);

    // Build blob image
    let blob_config = images.blob.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No blob image configured")) as Box<dyn Error + Send>
    })?;

    let blob_name = blob_config.image.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(
            "blob image name not specified in build config",
        )) as Box<dyn Error + Send>
    })?;

    println!("  Building blob image...");
    let blob_dockerfile_path = template_path_abs.join(&blob_config.dockerfile);
    let blob_context_path = template_path_abs.join(&blob_config.context);

    crate::try_cmd::build_image(
        &BuildxBuilder::new(),
        registry,
        blob_name,
        "latest",
        blob_dockerfile_path.to_string_lossy().as_ref(),
        blob_context_path.to_string_lossy().as_ref(),
        &[],
    )?;

    let blob_ref = format!("{registry}/{blob_name}:latest");

    // Build template image
    let template_config = images.template.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No template image configured")) as Box<dyn Error + Send>
    })?;

    let template_name = template_config.image.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(
            "template image name not specified in build config",
        )) as Box<dyn Error + Send>
    })?;

    println!("  Building template image...");
    let template_dockerfile_path = template_path_abs.join(&template_config.dockerfile);
    let template_context_path = template_path_abs.join(&template_config.context);

    crate::try_cmd::build_image(
        &BuildxBuilder::new(),
        registry,
        template_name,
        "latest",
        template_dockerfile_path.to_string_lossy().as_ref(),
        template_context_path.to_string_lossy().as_ref(),
        &[],
    )?;

    let template_ref = format!("{registry}/{template_name}:latest");

    Ok((blob_ref, template_ref))
}
/// Run test cases sequentially.
fn run_tests_sequential(
    test_cases: Vec<&TestCase>,
    warmup: &TemplateWarmup,
    template_path: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    let mut results = Vec::new();

    for test_case in test_cases {
        let result = run_single_test_case(
            test_case,
            warmup,
            template_path,
            output_dir,
            update_snapshots,
            coordinator_endpoint,
            registry_client,
        )?;
        results.push(result);
    }

    Ok(results)
}

#[allow(clippy::too_many_arguments)]
/// Run test cases in parallel.
fn run_tests_parallel(
    test_cases: Vec<&TestCase>,
    warmup: &TemplateWarmup,
    template_path: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    parallel_count: usize,
    registry_client: &CyanRegistryClient,
) -> Result<Vec<TestResult>, Box<dyn Error + Send>> {
    // Use a semaphore to limit concurrency
    let semaphore = Arc::new(Semaphore::new(parallel_count));
    let results_mutex = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    let registry_endpoint = Arc::new(registry_client.endpoint.clone());
    let registry_version = Arc::new(registry_client.version.clone());

    for test_case in test_cases {
        let test_case = test_case.clone();
        let warmup = warmup.clone();
        let template_path = template_path.to_string();
        let output_dir = output_dir.to_string();
        let coordinator_endpoint = coordinator_endpoint.to_string();
        let semaphore = Arc::clone(&semaphore);
        let results_mutex = Arc::clone(&results_mutex);
        let registry_endpoint = Arc::clone(&registry_endpoint);
        let registry_version = Arc::clone(&registry_version);

        let handle = thread::spawn(move || {
            // Acquire semaphore
            let _permit = semaphore.acquire();

            // Create a per-thread registry client
            let thread_registry = CyanRegistryClient {
                endpoint: (*registry_endpoint).clone(),
                version: (*registry_version).clone(),
                client: Rc::new(reqwest::blocking::Client::builder().build().unwrap()),
            };

            let result = run_single_test_case(
                &test_case,
                &warmup,
                &template_path,
                &output_dir,
                update_snapshots,
                &coordinator_endpoint,
                &thread_registry,
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

/// Run a single test case.
fn run_single_test_case(
    test_case: &TestCase,
    warmup: &TemplateWarmup,
    template_path: &str,
    output_dir: &str,
    update_snapshots: bool,
    coordinator_endpoint: &str,
    registry_client: &CyanRegistryClient,
) -> Result<TestResult, Box<dyn Error + Send>> {
    let start_time = Instant::now();
    println!("Running test case: {}", test_case.name);

    // Generate unique IDs
    let id_gen = DefaultSessionIdGenerator;
    let session_id = id_gen.generate();
    let merger_id = uuid::Uuid::new_v4().to_string();

    // Setup try environment with Boron (blob volume, images, resolvers)
    println!("  Setting up try environment...");
    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.to_string());

    let image_ref = warmup.template_image_ref.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("Template image reference required"))
            as Box<dyn Error + Send>
    })?;
    let (reference, tag) = split_image_ref(image_ref);
    let try_setup_req = cyancoordinator::models::req::TrySetupReq {
        session_id: session_id.clone(),
        local_template_id: warmup.local_template_id.clone(),
        source: "image".to_string(),
        image_ref: Some(cyancoordinator::models::req::DockerImageReference { reference, tag }),
        path: None,
        template: warmup.template.clone(),
        merger_id: merger_id.clone(),
    };
    coord_client.try_setup(&try_setup_req)?;

    println!("  Try environment ready");

    // Warm executor session (creates session volume)
    println!("  Warming executor session...");
    let warm_res = coord_client.warn_executor(session_id.clone(), &warmup.template)?;

    println!("  Executor session warmed");

    // Create cleanup guard to ensure session is always cleaned up, even on error
    let _cleanup_guard = SessionCleanupGuard::new(&coord_client, session_id.clone());

    // Convert answer state to HashMap
    let mut answers: HashMap<String, Answer> = HashMap::new();
    let mut deterministic_state: HashMap<String, String> = HashMap::new();

    // Add deterministic state from test case
    for (key, value) in &test_case.deterministic_state {
        deterministic_state.insert(key.clone(), value.clone());
    }

    // Add answers from test case (keyed by question ID)
    for (question_id, answer_entry) in &test_case.answer_state {
        match answer_entry {
            AnswerStateEntry::String(s) => {
                answers.insert(question_id.clone(), Answer::String(s.clone()));
            }
            AnswerStateEntry::StringArray(arr) => {
                answers.insert(question_id.clone(), Answer::StringArray(arr.clone()));
            }
            AnswerStateEntry::Bool(b) => {
                answers.insert(question_id.clone(), Answer::Bool(*b));
            }
        }
    }

    // Run non-interactive Q&A loop (AFTER warm, BEFORE bootstrap)
    // Note: Template tests always use build mode (dev_mode=false)
    println!("  Running Q&A loop for {}...", test_case.name);
    let port = warmup.port.ok_or_else(|| {
        Box::new(std::io::Error::other("Template port not available")) as Box<dyn Error + Send>
    })?;

    let template_endpoint = format!("http://localhost:{port}");

    let (cyan, _final_answers, final_states) = run_non_interactive_qa_loop(
        &template_endpoint,
        answers,
        deterministic_state.clone(),
        &test_case.name,
    )?;

    // Update deterministic state with final states from Q&A loop
    for (key, value) in &final_states {
        deterministic_state.insert(key.clone(), value.clone());
    }

    // Bootstrap executor session (AFTER Q&A loop, BEFORE execution)
    println!("  Bootstrapping executor session...");
    let start_executor_req =
        crate::try_cmd::build_bootstrap_req(&session_id, &warmup.template, &warm_res, &merger_id);
    coord_client.bootstrap(&start_executor_req)?;

    // Execute template and unpack output
    println!("  Executing template and capturing output...");
    let test_output_dir = PathBuf::from(output_dir).join(&test_case.name);

    // Clear any previous output first to avoid stale files contaminating reruns
    if test_output_dir.exists() {
        fs::remove_dir_all(&test_output_dir).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to clear test output directory {}: {}",
                test_output_dir.display(),
                e
            ))) as Box<dyn Error + Send>
        })?;
    }

    crate::try_cmd::execute_and_unpack(
        coordinator_endpoint,
        &session_id,
        test_output_dir.to_str().unwrap(),
        &warmup.template,
        cyan,
        &merger_id,
    )?;

    println!("  Output unpacked successfully");

    // Execute post-template commands (resolved from dependency tree)
    let rc_registry = Rc::new(CyanRegistryClient {
        endpoint: registry_client.endpoint.clone(),
        version: registry_client.version.clone(),
        client: Rc::clone(&registry_client.client),
    });
    let resolver = DefaultDependencyResolver::new(rc_registry);
    let resolved_commands: Vec<String> = match resolver.resolve_dependencies(&warmup.template) {
        Ok(deps) => CompositionOperator::collect_commands(&deps),
        Err(e) => {
            eprintln!("  Warning: dependency resolution failed, using root commands only: {e}");
            warmup.template.commands.clone()
        }
    };
    if !resolved_commands.is_empty() {
        println!(
            "  Executing {} post-template command(s)...",
            resolved_commands.len()
        );
        let exec_result = CommandExecutor::execute_commands_non_interactive(
            &resolved_commands,
            &test_output_dir,
        )?;
        if !exec_result.all_succeeded() {
            let cmd_msg = format!(
                "Command execution failed: {}/{} succeeded, {}/{} failed",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            );
            return Ok(TestResult {
                name: test_case.name.clone(),
                passed: false,
                duration: start_time.elapsed(),
                failure_message: Some(cmd_msg),
            });
        }
    }

    // Compare with expected snapshot, then run validate commands
    let failure_message =
        compare_and_validate(test_case, &test_output_dir, template_path, update_snapshots)?;

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

/// Compare the produced output against the expected snapshot (updating it if
/// requested) and run any validate commands.
///
/// Returns `Some(message)` describing the failure(s), or `None` if everything
/// passed. Shared by the isolated and composition test paths.
fn compare_and_validate(
    test_case: &TestCase,
    test_output_dir: &Path,
    template_path: &str,
    update_snapshots: bool,
) -> Result<Option<String>, Box<dyn Error + Send>> {
    // Compare with expected snapshot first
    let mut failure_message: Option<String> = None;

    if let ExpectedOutput::Snapshot { ref path } = test_case.expected {
        let expected_path = if path.starts_with('/') {
            // Absolute path
            PathBuf::from(path)
        } else {
            // Relative to template directory
            PathBuf::from(template_path).join(path)
        };

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

            // Update snapshots if requested
            if update_snapshots {
                println!("  Updating snapshot...");
                copy_to_snapshot(test_output_dir, &expected_path)?;
                println!("  Snapshot updated");
            } else {
                failure_message = Some(format!(
                    "Snapshot comparison failed:\n{}",
                    messages.join("\n")
                ));
            }
        } else {
            println!("  Snapshot matched");
        }
    }

    // Run validate commands if specified (always run, regardless of snapshot result)
    if !test_case.validate.is_empty() {
        println!("  Running validate commands...");
        let validate_results =
            run_validate_commands(test_output_dir.to_str().unwrap(), &test_case.validate)?;

        let validate_failures: Vec<&ValidateResult> =
            validate_results.iter().filter(|r| !r.passed).collect();

        if !validate_failures.is_empty() {
            let mut messages = Vec::new();
            for result in &validate_failures {
                let exit_info = result
                    .exit_code
                    .map(|c| format!(" (exit code {c})"))
                    .unwrap_or_default();
                messages.push(format!(
                    "Command '{}' failed{}: {}",
                    result.command, exit_info, result.stderr
                ));
            }
            let validate_msg = format!("Validate commands failed:\n{}", messages.join("\n"));
            failure_message = Some(match failure_message {
                Some(existing) => format!("{existing}\n{validate_msg}"),
                None => validate_msg,
            });
        }
    }

    Ok(failure_message)
}

/// Run non-interactive Q&A loop against template server.
///
/// This bypasses TemplateEngine.start_with and directly calls CyanHttpRepo.prompt_template.
#[allow(clippy::type_complexity)]
fn run_non_interactive_qa_loop(
    template_endpoint: &str,
    mut answers: HashMap<String, Answer>,
    mut deterministic_state: HashMap<String, String>,
    test_name: &str,
) -> Result<(Cyan, HashMap<String, Answer>, HashMap<String, String>), Box<dyn Error + Send>> {
    let http_client = Rc::new(
        Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?,
    );

    let repo = CyanHttpRepo {
        client: CyanClient {
            endpoint: template_endpoint.to_string(),
            client: http_client.clone(),
        },
    };

    loop {
        let input = TemplateAnswerInput {
            answers: answers.clone(),
            deterministic_state: deterministic_state.clone(),
        };

        let output = repo.prompt_template(input)?;

        match output {
            TemplateOutput::Final(final_output) => {
                // Q&A complete, return cyan object
                return Ok((final_output.cyan, answers, deterministic_state));
            }
            TemplateOutput::QnA(qna) => {
                // Look up answer for this question
                let question_id = qna.question.id();

                if let Some(answer) = answers.get(&question_id) {
                    // Answer found, continue loop
                    answers.insert(question_id, answer.clone());

                    // Update deterministic state
                    for (key, value) in &qna.deterministic_state {
                        deterministic_state.insert(key.clone(), value.clone());
                    }
                } else {
                    // Answer not found - fail the test
                    return Err(Box::new(std::io::Error::other(format!(
                        "Missing answer for question '{question_id}' in test case '{test_name}'"
                    ))) as Box<dyn Error + Send>);
                }
            }
        }
    }
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

/// Cleanup warm-up resources.
fn cleanup_warmup(warmup: &TemplateWarmup) -> Result<(), Box<dyn Error + Send>> {
    if warmup.dev_mode {
        // Dev mode - nothing to clean up
        return Ok(());
    }

    let docker = warmup.docker.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("Docker client not available")) as Box<dyn Error + Send>
    })?;

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Stop and remove container
    if let Some(ref container_name) = warmup.container_name {
        println!("  Removing container: {container_name}");
        runtime.block_on(async {
            let _ = docker.stop_container(container_name, None).await;

            docker
                .remove_container(container_name, None)
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })?;
    }

    // Remove images
    if let Some(ref image_ref) = warmup.template_image_ref {
        println!("  Removing template image: {image_ref}");
        runtime.block_on(async {
            docker
                .remove_image(
                    image_ref,
                    None::<bollard::query_parameters::RemoveImageOptions>,
                    None::<bollard::auth::DockerCredentials>,
                )
                .await
                .map(|_| ())
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })?;
    }

    if let Some(ref blob_ref) = warmup.blob_image_ref {
        println!("  Removing blob image: {blob_ref}");
        runtime.block_on(async {
            docker
                .remove_image(
                    blob_ref,
                    None::<bollard::query_parameters::RemoveImageOptions>,
                    None::<bollard::auth::DockerCredentials>,
                )
                .await
                .map(|_| ())
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })?;
    }

    Ok(())
}
