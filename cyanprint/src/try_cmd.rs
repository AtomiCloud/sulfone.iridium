use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use bollard::Docker;
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{CreateContainerOptions, ListContainersOptions};

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::models::req::{BuildReq, MergerReq, StartExecutorReq, TrySetupReq};
use cyancoordinator::session::{DefaultSessionIdGenerator, SessionIdGenerator};
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::repo::CyanHttpRepo;
use cyanprompt::domain::services::template::engine::TemplateEngine;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanprompt::http::client::CyanClient;
use cyanprompt::http::mapper::cyan_req_mapper;
use cyanregistry::cli::mapper::{read_build_config, read_dev_config};
use cyanregistry::cli::models::template_config::{CyanTemplateFileConfig, CyanTemplateFileRef};
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::plugin_res::PluginVersionPrincipalRes;
use cyanregistry::http::models::processor_res::ProcessorVersionPrincipalRes;
use cyanregistry::http::models::template_res::{
    TemplatePrincipalRes, TemplatePropertyRes, TemplateVersionPrincipalRes, TemplateVersionRes,
    TemplateVersionResolverRes, TemplateVersionTemplateRefRes,
};

use cyancoordinator::fs::{
    DefaultVfs, DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker,
};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::operations::composition::{
    CompositionOperator, DefaultDependencyResolver, DependencyResolver,
};
use cyancoordinator::template::DefaultTemplateExecutor;

use crate::command_executor::CommandExecutor;
use crate::coord::start_coordinator;
use crate::docker::buildx::{BuildOptions, BuildOutput, BuildxBuilder};
use crate::port::{TEMPLATE_TRY, TEMPLATE_TRY_END, allocate_port};
use crate::util::parse_ref;

/// Type alias for Q&A loop result to reduce type complexity
type QaLoopResult =
    Result<(Cyan, HashMap<String, Answer>, HashMap<String, String>), Box<dyn Error + Send>>;

/// Configuration struct for execute_try_command to reduce argument count
#[allow(dead_code)]
#[derive(Clone)]
struct TryConfig {
    pub template_path: String,
    pub output_path: String,
    pub dev_mode: bool,
    pub keep_containers: bool,
    pub disable_daemon_autostart: bool,
    pub coordinator_endpoint: String,
}

/// Outcome of a `try template` / `try group` run, returned to the CLI boundary.
///
/// In headless mode the boundary ([`crate::headless::finish_headless_try`]) maps this onto
/// the JSON envelope + exit code, so `execute_try_command` / `execute_try_group_command`
/// never print the envelope themselves — they hand the outcome up and the single CLI
/// boundary emits it. This mirrors the create/update flow, where `cyan_run` returns a
/// `CyanRunResult` and `finish_headless` emits at the boundary (no split emission, no hidden
/// "already printed elsewhere" contract). Errors are the `Err` arm of the enclosing `Result`.
pub enum TryHeadlessOutcome {
    /// The run completed; the boundary emits `done` (exit 0).
    Done,
    /// The headless Q&A walk stopped on an unanswered question; the boundary emits
    /// `need_input` (exit 2) carrying this question. Holds the DOMAIN [`Question`] — the
    /// conversion to the JSON wire DTO is owned solely by the CLI boundary
    /// ([`crate::headless::finish_headless_try`]), mirroring `create`/`update`'s
    /// `CyanRunResult`. Command execution must not depend on the headless serialization
    /// representation; only the emission point does.
    NeedInput(cyanprompt::domain::models::question::Question),
}

/// Pure tracker for the resources created during `try template` setup: the Docker
/// artifacts (blob image, template image, container) and the coordinator try session.
///
/// This holds only the bookkeeping — the slots populated as each resource is produced,
/// the `armed` flag, and the `keep_containers` gate — and the pure decision of what to
/// tear down. It deliberately has NO live handle to Docker or the coordinator client
/// (the coordinator session is stored as a plain `(endpoint, id)` pair), so its decision
/// logic is unit-testable without a daemon or a coordinator. `SetupArtifactsGuard` wraps
/// it with the Docker handle and performs the actual best-effort removal on `Drop`.
///
/// The setup phase of `execute_try_command` (normal mode) builds a uniquely-tagged
/// blob image and template image, and then starts a template container. Several
/// fallible steps run *before* the post-Q&A cleanup closures exist — the template
/// image build, the coordinator `try_setup`, the container start, and the health
/// check. A bare `?` (or the `last_err` return after three failed port attempts)
/// on any of them used to unwind the function and orphan those artifacts: a blob
/// image alone, or a still-running container holding a port plus both images.
///
/// Rather than patch each `?` site (the per-path approach that has repeatedly missed
/// siblings), the artifacts are tracked here and torn down on `Drop` while armed.
/// The tracker is created before the first artifact and each slot is noted as the
/// corresponding resource is produced; it is `disarm()`-ed once control reaches the
/// Q&A loop, at which point the `cleanup_setup_session` (Q&A stop) and
/// `cleanup_executor_session` (post-Q&A) closures own teardown from there. So no window
/// is left unguarded: creation → Q&A hand-off is the guard's responsibility, Q&A → finish
/// is the closures'.
///
/// Tearing down on a setup failure is correct in BOTH interactive and headless modes
/// (it only adds cleanup of artifacts that would otherwise leak; it changes no output
/// and no prompt), so — like the shared `cleanup` helper — the guard is gated
/// on `keep_containers`, not `headless`: when `--keep-containers` is set the artifacts
/// are intentionally preserved for debugging, otherwise they are best-effort removed.
/// All removals are best-effort (a failed cleanup never masks the original error).
struct SetupArtifactsTracker {
    keep_containers: bool,
    armed: bool,
    blob_image_ref: Option<String>,
    template_image_ref: Option<String>,
    container_name: Option<String>,
    /// The coordinator try session (its endpoint + id) allocated by `try_setup`. Tracked
    /// here so a setup failure AFTER `try_setup` succeeds (e.g. the container start /
    /// health-check loop) releases the Boron session via `DELETE /executor/{id}` on drop,
    /// not just the local Docker artifacts. `None` until `try_setup` succeeds.
    coordinator_session: Option<(String, String)>,
}

impl SetupArtifactsTracker {
    fn new(keep_containers: bool) -> Self {
        Self {
            keep_containers,
            armed: true,
            blob_image_ref: None,
            template_image_ref: None,
            container_name: None,
            coordinator_session: None,
        }
    }

    /// Record the coordinator try session created by `try_setup` so a later setup failure
    /// releases it instead of leaking it on Boron. Stored as `(endpoint, session_id)` so
    /// the drop can build a throwaway client without borrowing the in-flight one.
    fn note_coordinator_session(&mut self, endpoint: String, session_id: String) {
        self.coordinator_session = Some((endpoint, session_id));
    }

    /// Record the built blob image so it is removed on a setup failure.
    fn note_blob_image(&mut self, image_ref: String) {
        self.blob_image_ref = Some(image_ref);
    }

    /// Record the built template image so it is removed on a setup failure.
    fn note_template_image(&mut self, image_ref: String) {
        self.template_image_ref = Some(image_ref);
    }

    /// Record a successfully started container so it is removed on a setup failure
    /// (e.g. a subsequent health-check timeout) that leaves it running.
    fn note_container(&mut self, name: String) {
        self.container_name = Some(name);
    }

    /// Clear the tracked container after it has already been removed inline (e.g. a
    /// failed `start_template_container` that retried with a fresh name). Keeps the
    /// tracker from removing a name that no longer resolves.
    fn clear_container(&mut self) {
        self.container_name = None;
    }

    /// Mark the setup phase as complete so dropping the guard no longer removes the
    /// artifacts. Called once the Q&A loop is about to run and teardown is handed off
    /// to the post-Q&A cleanup closures.
    fn disarm(&mut self) {
        self.armed = false;
    }

    /// Pure decision: which artifacts would be removed on drop right now. Returns
    /// `None` when the guard is inert (`keep_containers` set, or disarmed once setup
    /// completed). This is what `Drop` acts on, so asserting it pins the leak-closing
    /// behavior without needing a Docker daemon.
    fn removal_targets(&self) -> Option<SetupRemoval<'_>> {
        if self.keep_containers || !self.armed {
            return None;
        }
        Some(SetupRemoval {
            container_name: self.container_name.as_deref(),
            template_image_ref: self.template_image_ref.as_deref(),
            blob_image_ref: self.blob_image_ref.as_deref(),
            coordinator_session: self
                .coordinator_session
                .as_ref()
                .map(|(e, s)| (e.as_str(), s.as_str())),
        })
    }
}

/// RAII guard for the resources created during `try template` setup (Docker artifacts +
/// the coordinator try session). Wraps a `SetupArtifactsTracker` with the Docker handle so
/// the best-effort removal happens automatically on `Drop` while armed; the coordinator
/// session is released by building a throwaway client from the stored endpoint. See
/// `SetupArtifactsTracker` for the lifecycle.
struct SetupArtifactsGuard<'a> {
    docker: &'a Docker,
    tracker: SetupArtifactsTracker,
}

impl<'a> SetupArtifactsGuard<'a> {
    fn new(docker: &'a Docker, keep_containers: bool) -> Self {
        Self {
            docker,
            tracker: SetupArtifactsTracker::new(keep_containers),
        }
    }

    fn note_blob_image(&mut self, image_ref: String) {
        self.tracker.note_blob_image(image_ref);
    }

    fn note_template_image(&mut self, image_ref: String) {
        self.tracker.note_template_image(image_ref);
    }

    fn note_container(&mut self, name: String) {
        self.tracker.note_container(name);
    }

    fn note_coordinator_session(&mut self, endpoint: String, session_id: String) {
        self.tracker.note_coordinator_session(endpoint, session_id);
    }

    fn clear_container(&mut self) {
        self.tracker.clear_container();
    }

    fn disarm(&mut self) {
        self.tracker.disarm();
    }
}

/// The resources a still-armed guard would tear down on drop: the local Docker artifacts
/// plus the coordinator try session (`endpoint`, `session_id`) if `try_setup` had run.
struct SetupRemoval<'a> {
    container_name: Option<&'a str>,
    template_image_ref: Option<&'a str>,
    blob_image_ref: Option<&'a str>,
    coordinator_session: Option<(&'a str, &'a str)>,
}

impl Drop for SetupArtifactsGuard<'_> {
    fn drop(&mut self) {
        let Some(targets) = self.tracker.removal_targets() else {
            return;
        };
        // Release the coordinator try session first (it lives on Boron, not locally), then
        // tear down the local Docker artifacts. A throwaway client is built from the stored
        // endpoint so the drop borrows nothing from the in-flight client. Best-effort: a
        // failed release is logged to stderr and never masks the original setup error.
        if let Some((endpoint, session_id)) = targets.coordinator_session {
            let client = CyanCoordinatorClient::new(endpoint.to_string());
            if let Err(e) = client.try_cleanup(session_id) {
                eprintln!("  ⚠️ Failed to release coordinator session on setup failure: {e}");
            }
        }
        if let Some(name) = targets.container_name {
            stop_and_remove_container(self.docker, name);
        }
        if let Some(image) = targets.template_image_ref {
            remove_image_best_effort(self.docker, image);
        }
        if let Some(image) = targets.blob_image_ref {
            remove_image_best_effort(self.docker, image);
        }
    }
}

/// Execute try command
#[allow(clippy::too_many_arguments)]
pub fn execute_try_command(
    template_path: String,
    output_path: String,
    dev_mode: bool,
    keep_containers: bool,
    disable_daemon_autostart: bool,
    _registry: String,
    coordinator_endpoint: String,
    registry_client: Rc<CyanRegistryClient>,
    headless: bool,
    headless_answers: HashMap<String, Answer>,
) -> Result<TryHeadlessOutcome, Box<dyn Error + Send>> {
    crate::hprogress!(headless, "🚀 Starting cyanprint try...");
    crate::hprogress!(headless, "  Template path: {template_path}");
    crate::hprogress!(headless, "  Output path: {output_path}");
    crate::hprogress!(
        headless,
        "  Mode: {}",
        if dev_mode { "dev" } else { "normal" }
    );

    // Step 1: Pre-flight validation (mode-aware)
    pre_flight_validation(&template_path, dev_mode, headless)?;

    // Step 3: Ensure daemon is running with health check
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    ensure_daemon_running(
        &docker,
        disable_daemon_autostart,
        &coordinator_endpoint,
        headless,
    )?;

    // Track the resources created during setup — the Docker artifacts (blob image,
    // template image, running template container) AND the coordinator try session — so a
    // failure on any fallible setup step (the template image build, the container start, or
    // the health check, all of which run before the post-Q&A cleanup closures exist) tears
    // them down instead of leaking them. The coordinator session is registered after
    // `try_setup` succeeds (below). The guard is disarmed once the Q&A loop takes over; from
    // there the post-Q&A closures own teardown.
    let mut setup_guard = SetupArtifactsGuard::new(&docker, keep_containers);

    // Step 4: Generate IDs — match cyan_run formats
    let id_gen = DefaultSessionIdGenerator;
    let uuid_str = uuid::Uuid::new_v4().to_string(); // xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    let local_template_id = format!("10ca1{}", &uuid_str[5..]); // "10ca1" = "local", valid UUID format
    let session_id = id_gen.generate(); // 10-char alphanumeric, same as DefaultSessionIdGenerator in cyan_run
    let merger_id = uuid::Uuid::new_v4().to_string(); // UUID, same as coordinator's executor.rs

    crate::hprogress!(headless, "  Session ID: {session_id}");
    crate::hprogress!(headless, "  Local template ID: {local_template_id}");

    // Step 5: Read and validate config (mode-aware)
    let template_path_abs = Path::new(&template_path)
        .canonicalize()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let cyan_yaml_path = template_path_abs.join("cyan.yaml");
    let config_file =
        fs::read_to_string(&cyan_yaml_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let template_config: CyanTemplateFileConfig =
        serde_yaml::from_str(&config_file).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Mode-specific config validation
    if dev_mode {
        // Validate dev section exists and template_url is reachable
        let _dev_config = read_dev_config(cyan_yaml_path.to_string_lossy().to_string())?;
        // Template URL reachability will be checked during pre-flight validation
    } else {
        // Validate build section exists in normal mode
        let _build_config = read_build_config(cyan_yaml_path.to_string_lossy().to_string())?;
    }

    // Step 6: Resolve and pin dependencies (including resolvers)
    crate::hprogress!(headless, "📦 Resolving and pinning dependencies...");
    let pinned_deps = resolve_and_pin_dependencies(&registry_client, &template_config)?;
    crate::hprogress!(
        headless,
        "  Pinned {} dependencies",
        pinned_deps.total_count()
    );

    // Step 7: Mode-specific setup and image building
    let build_result = if !dev_mode {
        crate::hprogress!(headless, "🔨 Building template images...");
        let build_config = read_build_config(cyan_yaml_path.to_string_lossy().to_string())?;
        let registry = build_config.registry.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::other(
                "registry not configured in cyan.yaml build section",
            )) as Box<dyn Error + Send>
        })?;
        let images = build_config.images.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::other(
                "images not configured in cyan.yaml build section",
            )) as Box<dyn Error + Send>
        })?;

        let tag = format!(
            "{}-try-{}",
            template_config.name.to_lowercase().replace(' ', "-"),
            uuid::Uuid::new_v4()
        );

        let mut template_ref = None;

        // Build blob image (required in normal mode)
        let blob = images.blob.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::other(
                "blob image not configured in cyan.yaml build section (required in normal mode)",
            )) as Box<dyn Error + Send>
        })?;
        crate::hprogress!(headless, "  Building blob image...");
        let blob_name = blob.image.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::other(
                "blob image name not specified in build config",
            )) as Box<dyn Error + Send>
        })?;
        let dockerfile_path = template_path_abs.join(&blob.dockerfile);
        let context_path = template_path_abs.join(&blob.context);
        build_image(
            &BuildxBuilder::new().with_headless(headless),
            registry,
            blob_name,
            &tag,
            dockerfile_path.to_string_lossy().as_ref(),
            context_path.to_string_lossy().as_ref(),
            &[],
        )?;
        let blob_ref = Some(format!("{registry}/{blob_name}:{tag}"));
        setup_guard.note_blob_image(blob_ref.clone().unwrap_or_default());

        // Build template image if specified
        if let Some(ref tmpl) = images.template {
            crate::hprogress!(headless, "  Building template image...");
            let template_name = tmpl.image.as_ref().ok_or_else(|| {
                Box::new(std::io::Error::other(
                    "template image name not specified in build config",
                )) as Box<dyn Error + Send>
            })?;
            let dockerfile_path = template_path_abs.join(&tmpl.dockerfile);
            let context_path = template_path_abs.join(&tmpl.context);
            build_image(
                &BuildxBuilder::new().with_headless(headless),
                registry,
                template_name,
                &tag,
                dockerfile_path.to_string_lossy().as_ref(),
                context_path.to_string_lossy().as_ref(),
                &[],
            )?;
            template_ref = Some(format!("{registry}/{template_name}:{tag}"));
            setup_guard.note_template_image(template_ref.clone().unwrap_or_default());
        }

        Some((blob_ref, template_ref))
    } else {
        crate::hprogress!(headless, "  Dev mode: skipping image build");
        None
    };

    // Step 8: Build synthetic template for Boron (with populated resolvers)
    let synthetic_template = build_synthetic_template(
        &local_template_id,
        &template_config,
        &pinned_deps,
        dev_mode,
        build_result.as_ref(),
    )?;

    // Step 8.5: Resolve dependency commands (collect from all templates in dep tree)
    let dependency_resolver = DefaultDependencyResolver::new(registry_client.clone());
    let resolved_commands: Vec<String> =
        match dependency_resolver.resolve_dependencies(&synthetic_template) {
            Ok(deps) => CompositionOperator::collect_commands(&deps),
            Err(e) => {
                // Non-fatal: log but continue with just the local template's commands
                eprintln!("  ⚠️ Failed to resolve dependency commands: {e}");
                synthetic_template.commands.clone()
            }
        };

    // Step 9: Setup try environment with Boron (blob volume, images, resolvers)
    crate::hprogress!(headless, "🔧 Setting up try environment with Boron...");
    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());

    // Use template image for image_ref in normal mode (not blob image)
    let image_ref = build_result
        .as_ref()
        .and_then(|(_, template_ref)| template_ref.clone());
    // The blob image is built with the SAME unique `{uuid}` tag as the template image
    // (normal mode only). It must be cleaned up alongside the template image on every
    // headless NeedInput round — headless `try template` is iterative, so each
    // unanswered round builds both images and leaking the blob image would accumulate one
    // orphaned image per question per session.
    let blob_ref = build_result
        .as_ref()
        .and_then(|(blob_ref, _)| blob_ref.clone());

    let try_setup_req = if dev_mode {
        let dev_config = read_dev_config(cyan_yaml_path.to_string_lossy().to_string())?;

        let blob_full_path = template_path_abs
            .join(&dev_config.blob_path)
            .canonicalize()
            .map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "Failed to resolve blob path '{}': {e}",
                    dev_config.blob_path
                ))) as Box<dyn Error + Send>
            })?;

        TrySetupReq {
            session_id: session_id.clone(),
            local_template_id: local_template_id.clone(),
            source: "path".to_string(),
            image_ref: None,
            path: Some(blob_full_path.to_string_lossy().to_string()),
            template: synthetic_template.clone(),
            merger_id: merger_id.clone(),
        }
    } else {
        let image_ref_value = image_ref.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::other(
                "Template image reference required for normal mode",
            )) as Box<dyn Error + Send>
        })?;

        // Parse image ref into reference and tag
        let (reference, tag) = split_image_ref(image_ref_value);

        TrySetupReq {
            session_id: session_id.clone(),
            local_template_id: local_template_id.clone(),
            source: "image".to_string(),
            image_ref: Some(cyancoordinator::models::req::DockerImageReference { reference, tag }),
            path: None,
            template: synthetic_template.clone(),
            merger_id: merger_id.clone(),
        }
    };

    coord_client.try_setup(&try_setup_req)?;

    // `try_setup` allocated a coordinator session — hand it to the setup guard so a failure
    // on a later setup step (container start, health check) releases it on Boron via the
    // guard's Drop, not just the local Docker artifacts. From the Q&A loop onward the
    // post-Q&A cleanup closures (which also call `try_cleanup`) own this once the guard is
    // disarmed.
    setup_guard.note_coordinator_session(coordinator_endpoint.clone(), session_id.clone());

    // Step 10: Start template container (normal mode only)
    let (template_container_name, allocated_port) = if !dev_mode {
        // Use template image (not blob) for container startup
        let container_image_ref = image_ref.as_ref().ok_or_else(|| {
            Box::new(std::io::Error::other(
                "Template image reference required for container startup",
            )) as Box<dyn Error + Send>
        })?;

        let mut last_err: Option<Box<dyn Error + Send>> = None;
        let mut bound_port: Option<u16> = None;
        let mut container_name = String::new();

        for _ in 0..3 {
            container_name = format!(
                "cyan-template-{}",
                uuid::Uuid::new_v4().to_string().replace('-', "")
            );
            let Some(port_alloc) = allocate_port(TEMPLATE_TRY, TEMPLATE_TRY_END) else {
                last_err = Some(Box::new(std::io::Error::other(format!(
                    "No available port found in range {TEMPLATE_TRY}-{TEMPLATE_TRY_END} after 3 retries"
                ))) as Box<dyn Error + Send>);
                continue;
            };
            let port = port_alloc.release();

            crate::hprogress!(headless, "🐳 Starting template container on port {port}...");
            match start_template_container(
                &docker,
                &container_name,
                container_image_ref,
                port,
                &coordinator_endpoint,
                "cyanprint.dev",
                None,
                headless,
            ) {
                Ok(()) => {
                    // The container is now running — track it so a health-check
                    // timeout (or any later setup failure) tears it down via the guard
                    // instead of leaving it running on an allocated port.
                    setup_guard.note_container(container_name.clone());
                    health_check_template_container(port, 60, 1, headless)?;
                    bound_port = Some(port);
                    last_err = None;
                    break;
                }
                Err(e) => {
                    // Clean up any partially created container before retrying. The
                    // name is gone, so drop it from the guard too (the next iteration
                    // mints a fresh name).
                    stop_and_remove_container(&docker, &container_name);
                    setup_guard.clear_container();
                    last_err = Some(e);
                }
            }
        }

        if let Some(e) = last_err {
            return Err(e);
        }
        (Some(container_name), bound_port)
    } else {
        (None, None)
    };

    // Setup is complete: the coordinator try session, blob image, template image, and (in
    // normal mode) the running template container all exist. From here the Q&A outcomes and
    // the post-Q&A steps own their own teardown (the `cleanup_setup_session` and
    // `cleanup_executor_session` closures below, both of which release the coordinator
    // session via the shared `cleanup` helper), so disarm the setup guard — otherwise it
    // would remove the artifacts the Q&A need_input path and the success path still rely on.
    // Any error return above already dropped it while armed and cleaned up.
    setup_guard.disarm();

    // Step 11: Run Q&A loop and collect cyan, answers, and states
    // Collect Q&A answers and deterministic states - these are preserved through
    // the execution flow and used to build the execution payload sent to Boron
    //
    // By this point `try_setup` (POST /executor/try) has allocated a coordinator try
    // session keyed by `session_id`, and the template image, blob image, and (in normal
    // mode) the running template container all exist. The executor session has not been
    // *warmed* yet, but the coordinator session already exists, so a Q&A that stops here
    // (need_input) or errors MUST release the coordinator session in addition to the
    // Docker artifacts — otherwise every headless need_input round (headless is iterative:
    // empty → q1 → … → done) leaks a Boron try session plus a uniquely-tagged template AND
    // blob image. Route both terminal outcomes through the same shared `cleanup` helper the
    // post-Q&A error/success paths use (it calls `try_cleanup` + tears down the container
    // and both images), so there is exactly one teardown implementation and image-removal
    // style. Best-effort: a teardown hiccup must not stop us emitting the need_input
    // envelope or mask the original Q&A error.
    // Best-effort teardown: swallow errors so a teardown hiccup does not stop the
    // need_input envelope from being emitted or mask the original Q&A error. Same teardown
    // call + argument list as `cleanup_executor_session` below; that one propagates the
    // `Result` (post-Q&A errors are real), this one swallows it (the Q&A-stop path must
    // emit regardless). Defined here — before the Q&A loop that uses it — rather than
    // delegating to `cleanup_executor_session` (which is defined later, after the loop).
    let cleanup_setup_session = || {
        if let Err(e) = cleanup(
            &coord_client,
            &session_id,
            keep_containers,
            &docker,
            &template_container_name,
            image_ref.as_deref(),
            blob_ref.as_deref(),
            headless,
        ) {
            eprintln!("  ⚠️ Failed to clean up after Q&A stop: {e}");
        }
    };
    let (cyan, answers, states) = if headless {
        match run_qa_loop_headless(dev_mode, &cyan_yaml_path, allocated_port, headless_answers) {
            Ok(HeadlessQaOutcome::Complete(cyan, answers, states)) => (cyan, answers, states),
            Ok(HeadlessQaOutcome::NeedInput(question)) => {
                // Stop before executing. Release the coordinator try session and Docker
                // artifacts allocated during setup, then hand the question up to the CLI
                // boundary, which emits the need_input envelope and exits 2. The envelope is
                // NOT printed here — emission lives solely at the boundary.
                cleanup_setup_session();
                return Ok(TryHeadlessOutcome::NeedInput(question));
            }
            Err(e) => {
                // A headless Q&A error (e.g. a malformed answer or a transport error
                // surfaced as TemplateState::Err). The coordinator try session, template/blob
                // images, and running container were already created — clean them all before
                // propagating so a failed Q&A leaks neither the Boron session nor Docker
                // artifacts, exactly as the need_input arm does.
                cleanup_setup_session();
                return Err(e);
            }
        }
    } else {
        println!("🤖 Starting interactive Q&A...");
        run_qa_loop(dev_mode, &template_config, &cyan_yaml_path, allocated_port)?
    };

    // Steps 12-13 warm the executor session, bootstrap it, and execute the template.
    // The coordinator session already exists (allocated by `try_setup`); the warm call
    // warms it (and these steps rely on the template container/images built during setup).
    // An error on ANY of them — and the success path and every post-command arm below —
    // must run the SAME teardown: release the coordinator session and tear down the Docker
    // artifacts. Otherwise the session and Docker artifacts leak on a post-Q&A failure
    // (bootstrap error, execution failure, non-2xx coordinator response, output-dir create
    // failure, or archive-unpack failure). This single shared closure is the one teardown
    // implementation for every terminal path from here on, so they cannot drift apart.
    let cleanup_executor_session = || {
        cleanup(
            &coord_client,
            &session_id,
            keep_containers,
            &docker,
            &template_container_name,
            image_ref.as_deref(),
            blob_ref.as_deref(),
            headless,
        )
    };

    // Step 12: Warm executor session (creates session volume)
    crate::hprogress!(headless, "🔧 Warming executor session...");
    let warm_res = match coord_client.warn_executor(session_id.clone(), &synthetic_template) {
        Ok(res) => res,
        Err(e) => {
            cleanup_executor_session()?;
            return Err(e);
        }
    };

    // Step 13: Bootstrap executor with StartExecutorReq
    crate::hprogress!(headless, "🚀 Bootstrapping executor...");
    let start_executor_req =
        build_bootstrap_req(&session_id, &synthetic_template, &warm_res, &merger_id);
    if let Err(e) = coord_client.bootstrap(&start_executor_req) {
        cleanup_executor_session()?;
        return Err(e);
    }

    // Step 13: Execute and stream output using BuildReq with Cyan from Q&A
    crate::hprogress!(headless, "🚀 Executing template and streaming output...");
    if let Err(e) = execute_and_stream_output(
        &coord_client,
        &session_id,
        &output_path,
        &synthetic_template,
        cyan,
        answers,
        states,
        merger_id,
        headless,
    ) {
        cleanup_executor_session()?;
        return Err(e);
    }

    // Step 13.5: Execute post-template commands (resolved from all dependency templates)
    if !resolved_commands.is_empty() {
        crate::hprogress!(
            headless,
            "\n⚡ Executing {} post-template command(s)...",
            resolved_commands.len()
        );
        let exec_result = match CommandExecutor::execute_commands_for_mode(
            &resolved_commands,
            Path::new(&output_path),
            headless,
        ) {
            Ok(result) => result,
            Err(err) => {
                // Clean up coordinator session before propagating the error
                cleanup_executor_session()?;
                return Err(err);
            }
        };
        if exec_result.aborted {
            // Clean up coordinator session before returning on abort
            cleanup_executor_session()?;
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution aborted: {}/{} succeeded, {}/{} failed before abort",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
        if headless && !exec_result.all_succeeded() {
            // The non-interactive path runs every command and records failures in
            // the result but returns Ok — it never sets `aborted`. Without this check a
            // failed post-template command (e.g. one exiting non-zero) would be silently
            // ignored and the try would report `done` / exit 0. In headless mode there is
            // no interactive "continue?" prompt to surface the failure, so treat any
            // partial failure as an error (clean up the session first, same as the abort
            // path) → error envelope / exit 1. Interactive mode keeps its existing
            // behavior (the user already chose whether to continue).
            cleanup_executor_session()?;
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution failed: {}/{} succeeded, {}/{} failed",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
    }

    // Step 14: Cleanup (best-effort, including built template and blob images)
    crate::hprogress!(headless, "🧹 Cleaning up...");
    cleanup_executor_session()?;

    crate::hprogress!(headless, "✅ Try completed successfully");
    crate::hprogress!(headless, "  Output written to: {output_path}");

    Ok(TryHeadlessOutcome::Done)
}

pub(crate) fn split_image_ref(image_ref: &str) -> (String, String) {
    if let Some(last_colon) = image_ref.rfind(':') {
        let potential_tag = &image_ref[last_colon + 1..];
        // Check if the colon is part of host:port (contains a slash in the potential tag)
        if potential_tag.contains('/') {
            // The colon is part of host:port, not a tag separator
            (image_ref.to_string(), "latest".to_string())
        } else {
            (
                image_ref[..last_colon].to_string(),
                potential_tag.to_string(),
            )
        }
    } else {
        (image_ref.to_string(), "latest".to_string())
    }
}

pub(crate) fn pre_flight_validation(
    template_path: &str,
    dev_mode: bool,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    crate::hprogress!(headless, "🔍 Running pre-flight checks...");
    BuildxBuilder::check_docker().map_err(|e| {
        Box::new(std::io::Error::other(format!("Docker check failed: {e}")))
            as Box<dyn Error + Send>
    })?;
    crate::hprogress!(headless, "  ✓ Docker daemon is running");

    let cyan_yaml_path = Path::new(template_path).join("cyan.yaml");
    if !cyan_yaml_path.exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("cyan.yaml not found at: {}", cyan_yaml_path.display()),
        )) as Box<dyn Error + Send>);
    }
    crate::hprogress!(
        headless,
        "  ✓ cyan.yaml found at: {}",
        cyan_yaml_path.display()
    );

    // Mode-specific validation
    if dev_mode {
        let dev_config =
            read_dev_config(cyan_yaml_path.to_string_lossy().to_string()).map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "failed to read dev config from cyan.yaml: {e}"
                ))) as Box<dyn Error + Send>
            })?;

        // Verify template_url is reachable during pre-flight
        crate::hprogress!(headless, "  Checking template URL reachability...");
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        let template_url = dev_config.template_url.trim_end_matches('/');
        let health_url = format!("{template_url}/");

        match http_client.get(&health_url).send() {
            Ok(resp) if resp.status().is_success() => {
                crate::hprogress!(headless, "  ✓ Template URL is reachable: {template_url}");
            }
            Ok(resp) => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Template URL returned non-success status: {}",
                    resp.status()
                ))) as Box<dyn Error + Send>);
            }
            Err(e) => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Template URL is not reachable: {e}"
                ))) as Box<dyn Error + Send>);
            }
        }

        crate::hprogress!(
            headless,
            "  ✓ dev section validated and template URL is reachable"
        );
    } else {
        let _build_config = read_build_config(cyan_yaml_path.to_string_lossy().to_string())
            .map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "failed to read build config from cyan.yaml: {e}"
                ))) as Box<dyn Error + Send>
            })?;
        crate::hprogress!(headless, "  ✓ build section validated");
    }

    Ok(())
}

pub(crate) fn ensure_daemon_running(
    docker: &Docker,
    disable_autostart: bool,
    coordinator_endpoint: &str,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    let coord_filter = "^cyanprint-coordinator$";

    let mut filters_map = HashMap::new();
    filters_map.insert("name".to_string(), vec![coord_filter.to_string()]);

    let list_options = ListContainersOptions {
        all: false,
        filters: Some(filters_map),
        ..Default::default()
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let coordinator_already_running = runtime.block_on(async {
        let containers = docker
            .list_containers(Some(list_options))
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        Ok(!containers.is_empty())
    })?;

    if coordinator_already_running {
        crate::hprogress!(headless, "  ✓ Coordinator daemon is already running");
        // Perform health check even if already running
        return health_check_daemon(coordinator_endpoint, headless);
    }

    if disable_autostart {
        return Err(Box::new(std::io::Error::other(
            "Coordinator daemon is not running. Run 'cyanprint daemon start' or omit --disable-daemon-autostart.",
        )) as Box<dyn Error + Send>);
    }

    crate::hprogress!(headless, "🚀 Starting coordinator daemon...");
    let img = "ghcr.io/atomicloud/sulfone.boron/sulfone-boron:latest".to_string();
    runtime
        .block_on(async { start_coordinator(docker.clone(), img, 9000, None, headless).await })?;

    // Health check after starting
    health_check_daemon(coordinator_endpoint, headless)?;

    crate::hprogress!(headless, "  ✓ Coordinator daemon started and ready");
    Ok(())
}

fn health_check_daemon(
    coordinator_endpoint: &str,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let health_url = format!("{}/", coordinator_endpoint.trim_end_matches('/'));

    crate::hprogress!(headless, "  Checking daemon health...");
    for attempt in 1..=60 {
        let resp = http_client.get(&health_url).send();
        match resp {
            Ok(r) if r.status().is_success() => {
                crate::hprogress!(headless, "  ✓ Daemon is healthy");
                return Ok(());
            }
            Ok(r) if attempt == 60 => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Daemon health check failed after 60 attempts (last status: {})",
                    r.status()
                ))) as Box<dyn Error + Send>);
            }
            Ok(_) => {
                // Continue retrying
            }
            Err(e) if attempt == 60 => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Daemon health check failed after 60 attempts: {e}"
                ))) as Box<dyn Error + Send>);
            }
            Err(_) => {
                // Continue retrying
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    unreachable!("Loop should have returned by attempt 60")
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PinnedDependencies {
    pub processors: Vec<ProcessorVersionPrincipalRes>,
    pub plugins: Vec<PluginVersionPrincipalRes>,
    pub templates: Vec<TemplateVersionTemplateRefRes>,
    pub resolvers: Vec<TemplateVersionResolverRes>,
}

impl PinnedDependencies {
    pub fn total_count(&self) -> usize {
        self.processors.len() + self.plugins.len() + self.templates.len() + self.resolvers.len()
    }
}

pub(crate) fn resolve_and_pin_dependencies(
    registry: &CyanRegistryClient,
    config: &CyanTemplateFileConfig,
) -> Result<PinnedDependencies, Box<dyn Error + Send>> {
    let mut processors = Vec::new();
    let mut plugins = Vec::new();
    let mut templates = Vec::new();
    let mut resolvers = Vec::new();

    for proc_ref in &config.processors {
        match parse_ref(proc_ref.clone()) {
            Ok((username, name, version)) => {
                let proc = registry.get_processor(username, name, version)?;
                processors.push(ProcessorVersionPrincipalRes {
                    id: proc.principal.id,
                    version: proc.principal.version,
                    created_at: proc.principal.created_at,
                    description: proc.principal.description,
                    docker_reference: proc.principal.docker_reference,
                    docker_tag: proc.principal.docker_tag,
                });
            }
            Err(e) => {
                eprintln!("  Warning: Failed to parse processor reference '{proc_ref}': {e}");
            }
        }
    }

    for plugin_ref in &config.plugins {
        match parse_ref(plugin_ref.clone()) {
            Ok((username, name, version)) => {
                let plugin = registry.get_plugin(username, name, version)?;
                plugins.push(PluginVersionPrincipalRes {
                    id: plugin.principal.id,
                    version: plugin.principal.version,
                    created_at: plugin.principal.created_at,
                    description: plugin.principal.description,
                    docker_reference: plugin.principal.docker_reference,
                    docker_tag: plugin.principal.docker_tag,
                });
            }
            Err(e) => {
                eprintln!("  Warning: Failed to parse plugin reference '{plugin_ref}': {e}");
            }
        }
    }

    for tmpl_ref in &config.templates {
        let ref_string = match tmpl_ref {
            CyanTemplateFileRef::Simple(s) => s.clone(),
            CyanTemplateFileRef::Extended { template, .. } => template.clone(),
        };
        let preset_answers = match tmpl_ref {
            CyanTemplateFileRef::Simple(_) => std::collections::HashMap::new(),
            CyanTemplateFileRef::Extended { preset_answers, .. } => preset_answers.clone(),
        };
        match parse_ref(ref_string.clone()) {
            Ok((username, name, version)) => {
                let tmpl = registry.get_template(username, name, version)?;
                templates.push(TemplateVersionTemplateRefRes {
                    id: tmpl.principal.id.clone(),
                    version: tmpl.principal.version,
                    preset_answers,
                });
            }
            Err(e) => {
                eprintln!("  Warning: Failed to parse template reference '{ref_string}': {e}");
            }
        }
    }

    // Pin resolvers - resolve first-layer resolvers with get_resolver()
    for resolver_ref in &config.resolvers {
        let parsed = cyanregistry::cli::mapper::resolver_reference_parse(&resolver_ref.resolver);
        match parsed {
            Some(Ok((username, name, version))) => {
                let resolver = registry.get_resolver(username, name, version)?;
                resolvers.push(TemplateVersionResolverRes {
                    id: resolver.principal.id,
                    version: resolver.principal.version,
                    created_at: resolver.principal.created_at,
                    description: Some(resolver.principal.description),
                    docker_reference: resolver.principal.docker_reference,
                    docker_tag: resolver.principal.docker_tag,
                    config: resolver_ref.config.clone(),
                    files: resolver_ref.files.clone(),
                });
            }
            Some(Err(e)) => {
                eprintln!(
                    "  Warning: Failed to parse resolver reference '{}': {e}",
                    resolver_ref.resolver
                );
            }
            None => {
                eprintln!(
                    "  Warning: Could not parse resolver reference '{}'",
                    resolver_ref.resolver
                );
            }
        }
    }

    Ok(PinnedDependencies {
        processors,
        plugins,
        templates,
        resolvers,
    })
}

pub(crate) fn build_synthetic_template(
    local_template_id: &str,
    config: &CyanTemplateFileConfig,
    pinned: &PinnedDependencies,
    dev_mode: bool,
    build_result: Option<&(Option<String>, Option<String>)>,
) -> Result<TemplateVersionRes, Box<dyn Error + Send>> {
    let principal = TemplateVersionPrincipalRes {
        id: local_template_id.to_string(),
        version: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
        description: config.description.clone(),
        properties: if dev_mode {
            None
        } else {
            // Populate synthetic template properties from actual build output
            if let Some((Some(blob_ref), Some(template_ref))) = build_result {
                let (blob_docker_ref, blob_tag) = split_image_ref(blob_ref);
                let (template_docker_ref, template_tag) = split_image_ref(template_ref);
                Some(TemplatePropertyRes {
                    blob_docker_reference: blob_docker_ref,
                    blob_docker_tag: blob_tag,
                    template_docker_reference: template_docker_ref,
                    template_docker_tag: template_tag,
                })
            } else {
                None
            }
        },
    };

    let template = TemplatePrincipalRes {
        id: config.name.clone(),
        name: config.name.clone(),
        project: config.project.clone(),
        source: config.source.clone(),
        email: config.email.clone(),
        tags: config.tags.clone(),
        description: config.description.clone(),
        readme: config.readme.clone(),
        user_id: "local".to_string(),
    };

    Ok(TemplateVersionRes {
        principal,
        template,
        processors: pinned.processors.clone(),
        plugins: pinned.plugins.clone(),
        templates: pinned.templates.clone(),
        resolvers: pinned.resolvers.clone(),
        commands: config.commands.clone(),
    })
}

pub(crate) fn build_image(
    builder: &BuildxBuilder,
    registry: &str,
    image_name: &str,
    tag: &str,
    dockerfile: &str,
    context: &str,
    platforms: &[String],
) -> Result<(), Box<dyn Error + Send>> {
    builder.build(BuildOptions {
        registry,
        image_name,
        tag,
        dockerfile,
        context,
        platforms,
        no_cache: false,
        dry_run: false,
        output: BuildOutput::Load,
    })
}

/// Best-effort stop and remove of a Docker container.
///
/// Used to clean up partially created containers before retrying.
/// Errors are silently ignored since this is a cleanup operation.
pub(crate) fn stop_and_remove_container(docker: &Docker, container_name: &str) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return,
    };
    rt.block_on(async {
        let _ = docker.stop_container(container_name, None).await;
        let _ = docker.remove_container(container_name, None).await;
    });
}

/// Best-effort removal of a built Docker image.
///
/// Used by the headless `try template` NeedInput path: because headless Q&A is
/// iterative, every unanswered round rebuilds a uniquely-tagged image, so leaving the
/// image behind leaks one image per question. Errors are silently ignored (cleanup).
/// Remove a Docker image best-effort using a caller-supplied Tokio runtime. This is the
/// ONE image-removal mechanism in the teardown subsystem: both the template image and the
/// blob image go through it, so there is a single code path (no second inline
/// `runtime.block_on(docker.remove_image(...))` style to drift from it). Errors are
/// swallowed on purpose — teardown is best-effort and a remove hiccup must not mask the
/// original error that triggered cleanup.
pub(crate) fn remove_image_with_runtime(
    runtime: &tokio::runtime::Runtime,
    docker: &Docker,
    image_ref: &str,
) {
    runtime.block_on(async {
        let _ = docker
            .remove_image(
                image_ref,
                None::<bollard::query_parameters::RemoveImageOptions>,
                None::<bollard::auth::DockerCredentials>,
            )
            .await;
    });
}

/// Remove a Docker image best-effort when no runtime is already in scope (e.g. from a
/// `Drop` implementation that cannot borrow one). Builds a throwaway runtime and delegates
/// to [`remove_image_with_runtime`] so the actual removal logic stays in one place.
pub(crate) fn remove_image_best_effort(docker: &Docker, image_ref: &str) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return,
    };
    remove_image_with_runtime(&rt, docker, image_ref);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_template_container(
    docker: &Docker,
    container_name: &str,
    image_ref: &str,
    host_port: u16,
    _coordinator_endpoint: &str,
    label: &str,
    run_id: Option<&str>,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "5550/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(host_port.to_string()),
        }]),
    );

    let host_config = HostConfig {
        port_bindings: Some(port_bindings),
        network_mode: Some("cyanprint".to_string()),
        ..Default::default()
    };

    let create_options = CreateContainerOptions {
        name: Some(container_name.to_string()),
        platform: String::new(),
    };

    let create_body = ContainerCreateBody {
        image: Some(image_ref.to_string()),
        host_config: Some(host_config),
        labels: Some({
            let mut labels = HashMap::new();
            labels.insert(label.to_string(), "true".to_string());
            if let Some(run_id) = run_id {
                labels.insert("cyanprint.test.run".to_string(), run_id.to_string());
            }
            labels
        }),
        exposed_ports: Some(vec!["5550/tcp".to_string()]),
        ..Default::default()
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        docker
            .create_container(Some(create_options), create_body)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        docker
            .start_container(
                container_name,
                None::<bollard::query_parameters::StartContainerOptions>,
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // progress goes to stderr in headless so stdout stays the sole JSON stream.
        crate::hprogress!(headless, "  ✓ Template container started: {container_name}");

        Ok(())
    })
}

pub(crate) fn health_check_template_container(
    port: u16,
    max_attempts: u32,
    interval_secs: u64,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let health_url = format!("http://localhost:{port}/");

    // progress goes to stderr in headless so stdout stays the sole JSON stream.
    crate::hprogress!(headless, "  Checking template container health...");
    for attempt in 1..=max_attempts {
        let resp = http_client.get(&health_url).send();
        match resp {
            Ok(r) if r.status().is_success() => {
                crate::hprogress!(headless, "  ✓ Template container is healthy");
                return Ok(());
            }
            Ok(r) if attempt == max_attempts => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Template container health check failed after {} attempts (last status: {})",
                    max_attempts,
                    r.status()
                ))) as Box<dyn Error + Send>);
            }
            Ok(_) => {
                // Continue retrying
            }
            Err(e) if attempt == max_attempts => {
                return Err(Box::new(std::io::Error::other(format!(
                    "Template container health check failed after {max_attempts} attempts: {e}"
                ))) as Box<dyn Error + Send>);
            }
            Err(_) => {
                // Continue retrying
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }

    unreachable!("Loop should have returned by max_attempts")
}

/// Resolve the template-service endpoint for the Q&A walk.
///
/// In dev mode the endpoint comes from the cyan.yaml `dev.template_url`; in normal
/// mode it is the locally-started template container's port. Shared by the
/// interactive and headless Q&A loops.
fn template_endpoint(
    dev_mode: bool,
    cyan_yaml_path: &Path,
    port: Option<u16>,
) -> Result<String, Box<dyn Error + Send>> {
    if dev_mode {
        let dev_config = read_dev_config(cyan_yaml_path.to_string_lossy().to_string())?;
        Ok(dev_config.template_url.trim_end_matches('/').to_string())
    } else {
        Ok(format!("http://localhost:{}", port.unwrap()))
    }
}

/// Build a [`TemplateEngine`] pointed at the template-service endpoint. Shared by
/// the interactive and headless Q&A loops (the only difference between them is
/// `start_with` vs `start_headless`).
fn build_template_prompter(endpoint: String) -> Result<TemplateEngine, Box<dyn Error + Send>> {
    // A stalled template service must not hang a scripted/CI Q&A walk forever, so the
    // blocking client carries a request timeout (matching the executor's 600s ceiling).
    let c = Rc::new(
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?,
    );
    Ok(TemplateEngine {
        client: Rc::new(CyanHttpRepo {
            client: CyanClient {
                endpoint,
                client: c,
            },
        }),
    })
}

/// Extract deterministic states from Q&A answers: each `String` answer value is
/// treated as a deterministic-state entry (matching the original loop behavior).
fn extract_deterministic_states(answers: &HashMap<String, Answer>) -> HashMap<String, String> {
    let mut states = HashMap::new();
    for (key, answer) in answers.iter() {
        if let Answer::String(s) = answer {
            states.insert(key.clone(), s.clone());
        }
    }
    states
}

fn run_qa_loop(
    dev_mode: bool,
    _config: &CyanTemplateFileConfig,
    cyan_yaml_path: &Path,
    port: Option<u16>,
) -> QaLoopResult {
    let prompter = build_template_prompter(template_endpoint(dev_mode, cyan_yaml_path, port)?)?;
    let state = prompter.start_with(None, None);

    match state {
        TemplateState::Complete(cyan, answers) => {
            let states = extract_deterministic_states(&answers);
            Ok((cyan, answers, states))
        }
        TemplateState::QnA() => Err(Box::new(std::io::Error::other(
            "Q&A terminated in QnA state".to_string(),
        )) as Box<dyn Error + Send>),
        TemplateState::NeedInput(_, _) => Err(Box::new(std::io::Error::other(
            "interactive Q&A unexpectedly produced a NeedInput state".to_string(),
        )) as Box<dyn Error + Send>),
        TemplateState::Err(e) => Err(Box::new(std::io::Error::other(e)) as Box<dyn Error + Send>),
    }
}

/// Outcome of the headless `try template` Q&A walk.
//
// `Complete` is intentionally the larger variant: clippy's `large_enum_variant` lint
// flags enums where one variant is much bigger than the others (the enum's size is
// bounded by its largest variant, so `HeadlessQaOutcome` is ~the size of `Complete`).
// This is fine here — the enum is a single-shot return value from
// `run_qa_loop_headless`, never stored in a collection or passed by value through a hot
// path, so the inflated size carries no performance or memory cost. Boxing `Complete`
// would just add an indirection for no benefit.
#[allow(clippy::large_enum_variant)]
enum HeadlessQaOutcome {
    /// All answers supplied — proceed to execution with the finalized payload.
    Complete(Cyan, HashMap<String, Answer>, HashMap<String, String>),
    /// A question is still unanswered — emit it and stop. Carries the DOMAIN question;
    /// conversion to the wire DTO happens at the CLI emission boundary, not here.
    NeedInput(cyanprompt::domain::models::question::Question),
}

/// Headless variant of [`run_qa_loop`]: replays `answers` against the template
/// server via the non-interactive driver instead of prompting. Shares
/// endpoint derivation and prompter construction with the interactive loop.
///
/// Returns the DOMAIN [`Question`] on `need_input` (not the wire DTO): the
/// serialization representation is the CLI boundary's concern, so the run layer
/// stays decoupled from it, matching `create`/`update`.
fn run_qa_loop_headless(
    dev_mode: bool,
    cyan_yaml_path: &Path,
    port: Option<u16>,
    answers: HashMap<String, Answer>,
) -> Result<HeadlessQaOutcome, Box<dyn Error + Send>> {
    let prompter = build_template_prompter(template_endpoint(dev_mode, cyan_yaml_path, port)?)?;

    match prompter.start_headless(Some(answers)) {
        TemplateState::Complete(cyan, answers) => {
            let states = extract_deterministic_states(&answers);
            Ok(HeadlessQaOutcome::Complete(cyan, answers, states))
        }
        TemplateState::NeedInput(question, _) => Ok(HeadlessQaOutcome::NeedInput(question)),
        TemplateState::QnA() => Err(Box::new(std::io::Error::other(
            "headless Q&A terminated in QnA state".to_string(),
        )) as Box<dyn Error + Send>),
        TemplateState::Err(e) => Err(Box::new(std::io::Error::other(e)) as Box<dyn Error + Send>),
    }
}

/// Build a StartExecutorReq from executor warm response data.
pub(crate) fn build_bootstrap_req(
    session_id: &str,
    template: &TemplateVersionRes,
    warm_res: &cyancoordinator::models::res::ExecutorWarmRes,
    merger_id: &str,
) -> StartExecutorReq {
    StartExecutorReq {
        session_id: session_id.to_string(),
        template: template.clone(),
        write_vol_reference: cyancoordinator::models::req::DockerVolumeReferenceReq {
            cyan_id: warm_res.vol_ref.cyan_id.clone(),
            session_id: warm_res.vol_ref.session_id.clone(),
        },
        merger: MergerReq {
            merger_id: merger_id.to_string(),
        },
    }
}

/// Execute template via coordinator and unpack tar.gz output to a directory.
pub(crate) fn execute_and_unpack(
    coordinator_endpoint: &str,
    session_id: &str,
    output_path: &str,
    template: &TemplateVersionRes,
    cyan: Cyan,
    merger_id: &str,
) -> Result<(), Box<dyn Error + Send>> {
    let cyan_req = cyan_req_mapper(cyan);

    let build_req = BuildReq {
        template: template.clone(),
        cyan: cyan_req,
        merger_id: merger_id.to_string(),
    };

    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let endpoint = format!(
        "{}/executor/{session_id}",
        coordinator_endpoint.trim_end_matches('/')
    );
    let response = http_client
        .post(endpoint)
        .json(&build_req)
        .send()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    if !response.status().is_success() {
        let err_text = response
            .text()
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Box::new(std::io::Error::other(format!(
            "Execution failed: {err_text}"
        ))) as Box<dyn Error + Send>);
    }

    fs::create_dir_all(output_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let tar = flate2::read::GzDecoder::new(response);
    let mut archive = tar::Archive::new(tar);
    archive
        .unpack(output_path)
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_and_stream_output(
    coord_client: &CyanCoordinatorClient,
    session_id: &str,
    output_path: &str,
    template: &TemplateVersionRes,
    cyan: Cyan,
    answers: HashMap<String, Answer>,
    states: HashMap<String, String>,
    merger_id: String,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    // progress goes to stderr in headless so stdout stays the sole JSON stream.
    crate::hprogress!(
        headless,
        "  Q&A data collected: {} answers and {} deterministic states",
        answers.len(),
        states.len()
    );

    crate::hprogress!(headless, "  Unpacking output to {output_path}...");
    execute_and_unpack(
        &coord_client.endpoint,
        session_id,
        output_path,
        template,
        cyan,
        &merger_id,
    )?;

    crate::hprogress!(headless, "  ✓ Output unpacked successfully");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cleanup(
    coord_client: &CyanCoordinatorClient,
    session_id: &str,
    keep_containers: bool,
    docker: &Docker,
    template_container_name: &Option<String>,
    template_image_ref: Option<&str>,
    blob_image_ref: Option<&str>,
    headless: bool,
) -> Result<(), Box<dyn Error + Send>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    // Stop and remove template container and cleanup session. progress goes to
    // stderr in headless so stdout stays the sole JSON stream. Warnings are always
    // stderr (never on the stdout contract).
    if !keep_containers {
        // Cleanup session with Boron (synchronous call, no runtime needed)
        if let Err(e) = coord_client.try_cleanup(session_id) {
            eprintln!("  ⚠️ Failed to cleanup session: {e}");
        } else {
            crate::hprogress!(headless, "  ✓ Session cleaned up with Boron");
        }

        if let Some(container_name) = template_container_name {
            crate::hprogress!(
                headless,
                "  Removing template container: {container_name}..."
            );
            if let Err(e) = runtime.block_on(async {
                docker
                    .stop_container(container_name, None)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

                docker
                    .remove_container(container_name, None)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
            }) {
                eprintln!("  ⚠️ Failed to remove container: {e}");
            } else {
                crate::hprogress!(headless, "  ✓ Template container removed");
            }
        }

        // Best-effort removal of built template image. Both image types go through the
        // same `remove_image_with_runtime` helper so there is one removal code path.
        if let Some(image_ref) = template_image_ref {
            crate::hprogress!(headless, "  Removing template image: {image_ref}...");
            remove_image_with_runtime(&runtime, docker, image_ref);
            crate::hprogress!(headless, "  ✓ Template image removed");
        }

        // Best-effort removal of the built blob image. In normal mode the blob image is
        // built with the SAME unique tag as the template image, so it is a per-try
        // artifact that must be torn down alongside the template image. Removing it here
        // keeps every post-Q&A path (success, post-command failure, abort, and the
        // warm/bootstrap/execute error arms that share this function) from leaking the
        // blob image — matching the Q&A teardown, which already removes both.
        if let Some(blob_ref) = blob_image_ref {
            crate::hprogress!(headless, "  Removing blob image: {blob_ref}...");
            remove_image_with_runtime(&runtime, docker, blob_ref);
            crate::hprogress!(headless, "  ✓ Blob image removed");
        }
    } else {
        crate::hprogress!(
            headless,
            "  Keeping containers and images (--keep-containers specified)"
        );
    }

    Ok(())
}

/// Execute try group command — for group templates (no build, dependencies from registry)
#[allow(clippy::too_many_arguments)]
pub fn execute_try_group_command(
    template_path: String,
    output_path: String,
    disable_daemon_autostart: bool,
    coordinator_endpoint: String,
    registry_client: Rc<CyanRegistryClient>,
    cache_config: cyancoordinator::cache::CacheConfig,
    headless: bool,
    headless_answers: HashMap<String, Answer>,
) -> Result<TryHeadlessOutcome, Box<dyn Error + Send>> {
    crate::hprogress!(headless, "🔗 Starting cyanprint try group...");
    crate::hprogress!(headless, "  Template path: {template_path}");
    crate::hprogress!(headless, "  Output path: {output_path}");

    // Step 1: Pre-flight validation (Docker + cyan.yaml)
    crate::hprogress!(headless, "🔍 Running pre-flight checks...");
    BuildxBuilder::check_docker().map_err(|e| {
        Box::new(std::io::Error::other(format!("Docker check failed: {e}")))
            as Box<dyn Error + Send>
    })?;
    crate::hprogress!(headless, "  ✓ Docker daemon is running");

    let template_path_abs = Path::new(&template_path)
        .canonicalize()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let cyan_yaml_path = template_path_abs.join("cyan.yaml");
    if !cyan_yaml_path.exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("cyan.yaml not found at: {}", cyan_yaml_path.display()),
        )) as Box<dyn Error + Send>);
    }
    crate::hprogress!(headless, "  ✓ cyan.yaml found");

    // Step 2: Parse config
    let config_file =
        fs::read_to_string(&cyan_yaml_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let template_config: CyanTemplateFileConfig =
        serde_yaml::from_str(&config_file).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Validate this is a group (has dependencies, should not have build section)
    if template_config.templates.is_empty() {
        return Err(Box::new(std::io::Error::other(
            "Not a group template: no template dependencies declared in cyan.yaml. Use 'try template' instead.",
        )) as Box<dyn Error + Send>);
    }
    crate::hprogress!(
        headless,
        "  ✓ Group template with {} dependencies",
        template_config.templates.len()
    );

    // Step 3: Ensure daemon is running
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    ensure_daemon_running(
        &docker,
        disable_daemon_autostart,
        &coordinator_endpoint,
        headless,
    )?;

    // Step 4: Generate IDs
    let id_gen = DefaultSessionIdGenerator;
    let local_template_id = format!("local-{}", id_gen.generate());
    crate::hprogress!(headless, "  Local template ID: {local_template_id}");

    // Step 5: Resolve and pin dependencies from registry
    crate::hprogress!(headless, "📦 Resolving and pinning dependencies...");
    let pinned_deps = resolve_and_pin_dependencies(&registry_client, &template_config)?;
    crate::hprogress!(
        headless,
        "  ✓ Pinned {} dependencies ({} processors, {} plugins, {} templates, {} resolvers)",
        pinned_deps.total_count(),
        pinned_deps.processors.len(),
        pinned_deps.plugins.len(),
        pinned_deps.templates.len(),
        pinned_deps.resolvers.len()
    );

    // Step 6: Build synthetic template (no properties — group has no images)
    let synthetic_template = build_synthetic_template(
        &local_template_id,
        &template_config,
        &pinned_deps,
        true, // dev_mode=true so properties=None (group has no images)
        None, // no build result
    )?;

    // Step 7: Set up composition operator (same as run.rs)
    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());

    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    let merger = Box::new(GitLikeMerger::new(false, 50));
    let writer = Box::new(DiskFileWriter);

    let template_executor = Box::new(DefaultTemplateExecutor::new_with_headless(
        coord_client.endpoint.clone(),
        headless,
    ));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));
    let session_id_generator: Box<dyn SessionIdGenerator> = Box::new(DefaultSessionIdGenerator);
    let template_history = Box::new(cyancoordinator::template::DefaultTemplateHistory::new());

    let template_operator = TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
        registry_client.clone(),
    );

    let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client));

    let mut composition_operator = CompositionOperator::with_client(
        template_operator,
        dependency_resolver,
        coord_client.clone(),
    );
    // Inject the per-node execution cache. The synthetic root node is a local
    // template and is filtered out by `is_cacheable`, but its published registry
    // dependencies are cacheable — so a group re-try reuses unchanged sub-templates.
    // (FR6, FR7, FR14, C3)
    composition_operator.set_cache(cyancoordinator::cache::Cache::new(cache_config));

    // Step 8: Execute composition (resolves deps, warms each, runs Q&A, builds, layers)
    crate::hprogress!(headless, "🚀 Executing group composition...");
    // Headless: seed the supplied answers (namespaced per composed template id);
    // an empty map yields a need_input on the first question.
    let initial_answers: HashMap<String, Answer> = headless_answers;
    let empty_states: HashMap<String, String> = HashMap::new();

    let (vfs_output, final_state, session_ids, resolved_commands) = composition_operator
        .execute_template(
            &synthetic_template,
            &initial_answers,
            &empty_states,
            headless,
        )?;

    // Once the composed dependency executions have acquired coordinator sessions, EVERY
    // return path before the final teardown must clean them — otherwise an error between
    // here and Step 10 (output-dir create, write-to-disk, or a post-command failure)
    // returns to `main` with no access to `session_ids` and leaks them. A single
    // best-effort closure keeps need_input, the write/command error arms, the abort /
    // partial-failure arms, and the success teardown from drifting apart.
    let cleanup_group_sessions = || {
        for sid in &session_ids {
            if let Err(e) = coord_client.clean(sid.clone()) {
                eprintln!("  ⚠️ Failed to cleanup session {sid}: {e}");
            }
        }
    };

    // Headless: a composed template stopped on an unanswered question. Clean up sessions and
    // hand the DOMAIN question up to the CLI boundary, which converts it to the JSON wire DTO
    // and emits the need_input envelope (exit 2) — no files are written and nothing is printed
    // here. The composition layer already carries the domain `Question`; the run layer keeps
    // it that way, so only the emission point depends on the wire representation.
    if let Some(question) = final_state.need_input {
        cleanup_group_sessions();
        return Ok(TryHeadlessOutcome::NeedInput(question));
    }

    // One-line cache summary (printed when caching is enabled). (FR15)
    // Suppressed in headless mode: stdout must carry a single JSON envelope, so any
    // human-readable summary line would corrupt the headless contract.
    if !headless {
        composition_operator.print_cache_summary();
    }

    // Step 9: Write output to disk
    crate::hprogress!(headless, "📝 Writing output to {output_path}...");
    if let Err(e) =
        fs::create_dir_all(&output_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    {
        cleanup_group_sessions();
        return Err(e);
    }

    let output_dir = Path::new(&output_path);
    if let Err(e) = composition_operator
        .get_vfs()
        .write_to_disk(output_dir, &vfs_output)
    {
        cleanup_group_sessions();
        return Err(e);
    }

    // Step 9.5: Execute post-template commands (resolved from all dependency templates)
    if !resolved_commands.is_empty() {
        crate::hprogress!(
            headless,
            "\n⚡ Executing {} post-template command(s)...",
            resolved_commands.len()
        );
        let exec_result = match CommandExecutor::execute_commands_for_mode(
            &resolved_commands,
            output_dir,
            headless,
        ) {
            Ok(result) => result,
            Err(err) => {
                cleanup_group_sessions();
                return Err(err);
            }
        };
        if exec_result.aborted {
            // Clean up sessions before bailing out (same as Step 10 below)
            crate::hprogress!(
                headless,
                "🧹 Cleaning up {} session(s)...",
                session_ids.len()
            );
            cleanup_group_sessions();
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution aborted: {}/{} succeeded, {}/{} failed before abort",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
        if headless && !exec_result.all_succeeded() {
            // The non-interactive path runs every command and records failures in
            // the result but returns Ok — it never sets `aborted`. Without this check a
            // failed post-template command (e.g. one exiting non-zero) would be silently
            // ignored and the try group would report `done` / exit 0. In headless mode
            // there is no interactive "continue?" prompt to surface the failure, so treat
            // any partial failure as an error (clean up sessions first, same as the abort
            // path) → error envelope / exit 1. Interactive mode keeps its existing
            // behavior (the user already chose whether to continue).
            crate::hprogress!(
                headless,
                "🧹 Cleaning up {} session(s)...",
                session_ids.len()
            );
            cleanup_group_sessions();
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution failed: {}/{} succeeded, {}/{} failed",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
    }

    // Step 10: Cleanup sessions
    crate::hprogress!(
        headless,
        "🧹 Cleaning up {} session(s)...",
        session_ids.len()
    );
    cleanup_group_sessions();

    crate::hprogress!(headless, "✅ Try group completed successfully");
    crate::hprogress!(headless, "  Output written to: {output_path}");

    Ok(TryHeadlessOutcome::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_image_ref_with_tag() {
        let image_ref = "ghcr.io/atomicloud/my-template:v1.0.0";
        let (reference, tag) = split_image_ref(image_ref);
        assert_eq!(reference, "ghcr.io/atomicloud/my-template");
        assert_eq!(tag, "v1.0.0");
    }

    #[test]
    fn test_split_image_ref_without_tag() {
        let image_ref = "ghcr.io/atomicloud/my-template";
        let (reference, tag) = split_image_ref(image_ref);
        assert_eq!(reference, "ghcr.io/atomicloud/my-template");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_split_image_ref_with_port() {
        let image_ref = "localhost:5000/my-image:tag";
        let (reference, tag) = split_image_ref(image_ref);
        assert_eq!(reference, "localhost:5000/my-image");
        assert_eq!(tag, "tag");
    }

    #[test]
    fn test_split_image_ref_port_only() {
        // Edge case: port in host, no tag - should not split on port colon
        let image_ref = "localhost:5000/my-image";
        let (reference, tag) = split_image_ref(image_ref);
        assert_eq!(reference, "localhost:5000/my-image");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_pinned_dependencies_default() {
        let deps = PinnedDependencies::default();
        assert_eq!(deps.total_count(), 0);
        assert!(deps.processors.is_empty());
        assert!(deps.plugins.is_empty());
        assert!(deps.templates.is_empty());
        assert!(deps.resolvers.is_empty());
    }

    // --- SetupArtifactsTracker decision logic -------------------------------
    //
    // These tests cover the GUARD MECHANISM (which artifacts it tracks and when it
    // decides to remove them), not the Docker removal itself — exercising a real
    // image/container leak needs a Docker daemon, which is out of scope for this work
    // (no e2e). The pure `removal_targets()` decision is what makes the leak-closing
    // behavior provable without infra: it encodes exactly what `Drop` would tear down.
    // The tracker holds no Docker handle, so it is constructed directly here.
    fn tracker(keep_containers: bool) -> SetupArtifactsTracker {
        SetupArtifactsTracker::new(keep_containers)
    }

    #[test]
    fn setup_guard_inert_when_keep_containers_set() {
        // --keep-containers intentionally preserves artifacts for debugging, so the
        // guard must report nothing to remove even when fully populated and armed.
        let mut guard = tracker(true);
        guard.note_blob_image("reg/blob:tag".to_string());
        guard.note_template_image("reg/template:tag".to_string());
        guard.note_container("cyan-template-abc".to_string());
        assert!(guard.removal_targets().is_none());
    }

    #[test]
    fn setup_guard_inert_after_disarm() {
        // Once setup completes and the guard is disarmed, teardown is handed off to the
        // post-Q&A closures — the guard must remove nothing from that point on, even
        // when it holds a running container and both images.
        let mut guard = tracker(false);
        guard.note_blob_image("reg/blob:tag".to_string());
        guard.note_template_image("reg/template:tag".to_string());
        guard.note_container("cyan-template-abc".to_string());
        guard.disarm();
        assert!(guard.removal_targets().is_none());
    }

    #[test]
    fn setup_guard_targets_blob_only_when_template_build_fails() {
        // Blob image built, template image build fails. Only the blob image exists to
        // leak — the guard must report it (no container, no template).
        let mut guard = tracker(false);
        guard.note_blob_image("reg/blob:tag".to_string());
        let targets = guard
            .removal_targets()
            .expect("armed guard removes artifacts");
        assert_eq!(targets.blob_image_ref, Some("reg/blob:tag"));
        assert_eq!(targets.template_image_ref, None);
        assert_eq!(targets.container_name, None);
    }

    #[test]
    fn setup_guard_targets_running_container_plus_both_images_on_health_check_fail() {
        // Both images built, container started, health check fails. The guard must report
        // the RUNNING container plus both images — the worst case, which previously
        // leaked a live container holding an allocated port too.
        let mut guard = tracker(false);
        guard.note_blob_image("reg/blob:tag".to_string());
        guard.note_template_image("reg/template:tag".to_string());
        guard.note_container("cyan-template-abc".to_string());
        let targets = guard
            .removal_targets()
            .expect("armed guard removes artifacts");
        assert_eq!(targets.container_name, Some("cyan-template-abc"));
        assert_eq!(targets.template_image_ref, Some("reg/template:tag"));
        assert_eq!(targets.blob_image_ref, Some("reg/blob:tag"));
    }

    #[test]
    fn setup_guard_targets_coordinator_session_after_try_setup() {
        // After `try_setup` succeeds the coordinator session is registered. A subsequent
        // setup failure (e.g. container start / health check) must release that session on
        // Boron in addition to the Docker artifacts — otherwise it leaks a try session per
        // failed setup. The decision must surface the (endpoint, id) pair.
        let mut guard = tracker(false);
        guard.note_template_image("reg/template:tag".to_string());
        guard.note_coordinator_session("http://localhost:9000".to_string(), "sess-xyz".to_string());
        guard.note_container("cyan-template-abc".to_string());
        let targets = guard
            .removal_targets()
            .expect("armed guard removes artifacts");
        assert_eq!(
            targets.coordinator_session,
            Some(("http://localhost:9000", "sess-xyz")),
            "an armed guard must release the coordinator session registered by try_setup"
        );
        // ...alongside the Docker artifacts.
        assert_eq!(targets.container_name, Some("cyan-template-abc"));
        assert_eq!(targets.template_image_ref, Some("reg/template:tag"));
    }

    #[test]
    fn setup_guard_session_not_released_before_try_setup_or_when_inert() {
        // Before `try_setup` runs there is no session to release (a pre-try_setup failure
        // leaks nothing on Boron).
        let mut armed_no_session = tracker(false);
        armed_no_session.note_blob_image("reg/blob:tag".to_string());
        assert_eq!(
            armed_no_session
                .removal_targets()
                .expect("armed guard removes artifacts")
                .coordinator_session,
            None
        );

        // --keep-containers preserves the session (debugging), matching the Docker gate.
        let mut keep = tracker(true);
        keep.note_coordinator_session("http://localhost:9000".to_string(), "sess-xyz".to_string());
        assert!(keep.removal_targets().is_none());

        // Once disarmed, the post-Q&A closures own the session; the guard releases nothing.
        let mut disarmed = tracker(false);
        disarmed
            .note_coordinator_session("http://localhost:9000".to_string(), "sess-xyz".to_string());
        disarmed.disarm();
        assert!(disarmed.removal_targets().is_none());
    }

    #[test]
    fn setup_guard_clear_container_drops_a_failed_start_from_targets() {
        // When `start_template_container` fails the container is removed inline and a
        // fresh name is minted on retry; `clear_container` keeps the guard from reporting
        // a name that no longer resolves.
        let mut guard = tracker(false);
        guard.note_blob_image("reg/blob:tag".to_string());
        guard.note_template_image("reg/template:tag".to_string());
        guard.note_container("cyan-template-failed".to_string());
        guard.clear_container();
        let targets = guard
            .removal_targets()
            .expect("armed guard removes artifacts");
        assert_eq!(targets.container_name, None);
        // Images are still tracked and still removed.
        assert_eq!(targets.template_image_ref, Some("reg/template:tag"));
        assert_eq!(targets.blob_image_ref, Some("reg/blob:tag"));
    }

    #[test]
    fn test_pinned_dependencies_total_count() {
        let mut deps = PinnedDependencies::default();
        deps.processors.push(ProcessorVersionPrincipalRes {
            id: "proc1".to_string(),
            version: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            description: "test".to_string(),
            docker_reference: "test".to_string(),
            docker_tag: "latest".to_string(),
        });
        deps.plugins.push(PluginVersionPrincipalRes {
            id: "plugin1".to_string(),
            version: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            description: "test".to_string(),
            docker_reference: "test".to_string(),
            docker_tag: "latest".to_string(),
        });
        assert_eq!(deps.total_count(), 2);
    }

    #[test]
    fn test_build_synthetic_template_normal_mode() {
        let pinned = PinnedDependencies::default();
        let config = CyanTemplateFileConfig {
            username: "test".to_string(),
            name: "test-template".to_string(),
            project: "test-project".to_string(),
            source: "local".to_string(),
            email: "test@example.com".to_string(),
            tags: vec!["test".to_string()],
            description: "Test template".to_string(),
            readme: "# Test".to_string(),
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec![],
        };

        let blob_ref = Some("ghcr.io/test/blob:v1.0.0".to_string());
        let template_ref = Some("ghcr.io/test/template:v1.0.0".to_string());
        let build_result = Some((blob_ref, template_ref));

        let result =
            build_synthetic_template("local-test", &config, &pinned, false, build_result.as_ref());

        assert!(result.is_ok());
        let template = result.unwrap();
        assert_eq!(template.principal.id, "local-test");
        assert!(template.principal.properties.is_some());
        let props = template.principal.properties.unwrap();
        assert_eq!(props.blob_docker_reference, "ghcr.io/test/blob");
        assert_eq!(props.blob_docker_tag, "v1.0.0");
        assert_eq!(props.template_docker_reference, "ghcr.io/test/template");
        assert_eq!(props.template_docker_tag, "v1.0.0");
    }

    #[test]
    fn test_build_synthetic_template_dev_mode() {
        let pinned = PinnedDependencies::default();
        let config = CyanTemplateFileConfig {
            username: "test".to_string(),
            name: "test-template".to_string(),
            project: "test-project".to_string(),
            source: "local".to_string(),
            email: "test@example.com".to_string(),
            tags: vec!["test".to_string()],
            description: "Test template".to_string(),
            readme: "# Test".to_string(),
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec![],
        };

        let result = build_synthetic_template("local-test", &config, &pinned, true, None);

        assert!(result.is_ok());
        let template = result.unwrap();
        assert_eq!(template.principal.id, "local-test");
        assert!(template.principal.properties.is_none());
    }

    #[test]
    fn test_build_synthetic_template_includes_commands() {
        let pinned = PinnedDependencies::default();
        let config = CyanTemplateFileConfig {
            username: "test".to_string(),
            name: "test-template".to_string(),
            project: "test-project".to_string(),
            source: "local".to_string(),
            email: "test@example.com".to_string(),
            tags: vec![],
            description: "Test template".to_string(),
            readme: "".to_string(),
            processors: vec![],
            plugins: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec!["npm install".to_string(), "npm run build".to_string()],
        };

        let result = build_synthetic_template("local-test", &config, &pinned, true, None);

        assert!(result.is_ok());
        let template = result.unwrap();
        assert_eq!(template.commands.len(), 2);
        assert_eq!(template.commands[0], "npm install");
        assert_eq!(template.commands[1], "npm run build");
    }
}
