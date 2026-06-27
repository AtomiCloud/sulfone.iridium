use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::iter;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::conflict_file_resolver::FileConflictEntry;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::operations::composition::{CompositionOperator, DefaultDependencyResolver};
use cyancoordinator::state::{DefaultStateManager, StateReader, StateWriter};
use cyancoordinator::template::TemplateHistory;
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::models::question::Question;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateUpdateType};

use cyancoordinator::fs::VirtualFileSystem;

use crate::command_executor::CommandExecutor;
use crate::headless::CyanRunResult;
use crate::update::spec::{TemplateSpec, TemplateSpecManager, sort_specs};

/// cyanprint's own bookkeeping artifacts, excluded from the managed-files manifest.
/// `.cyan_state.yaml` is the state file the loader already special-cases
/// (`fs/loader.rs:25,79`); `.cyan_output` is the default output artifact
/// (`commands.rs`). Matched by exact path (after normalization) or top-level entry.
const CYANPRINT_INTERNAL_FILES: &[&str] = &[".cyan_state.yaml", ".cyan_output"];

/// Normalize a VFS path to the manifest's canonical form: forward-slash separators,
/// no leading `./` or `/`, no trailing `/`. VFS paths are already stored relative
/// (the loader/unpacker strip the target-dir prefix), so this is normalization, not
/// relativization.
fn normalize_path(path: &Path) -> String {
    // Render with '/' regardless of OS separator, using lossy UTF-8 for each component.
    let joined = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");

    // On non-Windows targets, Path::components() does NOT treat '\' as a separator,
    // so literal backslashes (e.g. a Windows-authored "dir\sub\file.txt") survive in
    // the joined string. Replace them explicitly so backslash separators normalize to
    // forward slashes on every platform — requires this canonical form.
    let forward = joined.replace('\\', "/");

    // Strip leading "./" and any leading '/', then any trailing '/'.
    let trimmed = forward.trim_start_matches("./");
    let trimmed = trimmed.trim_start_matches('/');
    trimmed.trim_end_matches('/').to_string()
}

/// True when `path` (already normalized) is one of cyanprint's bookkeeping files —
/// either as an exact path or as a top-level entry.
fn is_cyanprint_internal(path: &str) -> bool {
    let top_level = path.split('/').next().unwrap_or(path);
    CYANPRINT_INTERNAL_FILES
        .iter()
        .any(|internal| path == *internal || top_level == *internal)
}

/// Collect a template's output paths from its VFS as a normalized, filtered, sorted,
/// de-duplicated list of relative paths suitable for the managed-files manifest.
fn normalize_managed_paths(vfs: &VirtualFileSystem) -> Vec<String> {
    let mut v: Vec<String> = vfs
        .get_paths()
        .iter()
        .map(|p| normalize_path(p))
        .filter(|p| !p.is_empty())
        .filter(|p| !is_cyanprint_internal(p))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Check if a template has execution artifacts (Docker properties)
fn has_execution_artifacts(template: &TemplateVersionRes) -> bool {
    template.principal.properties.is_some()
}

/// Check if a template has dependencies
fn has_dependencies(template: &TemplateVersionRes) -> bool {
    !template.templates.is_empty()
}

/// Unified batch processing for both create and update commands.
/// Handles MAP, LAYER, and MERGE+WRITE phases.
/// Returns session IDs for cleanup, file conflicts for state persistence, and commands for execution.
// `headless` controls only whether progress goes to stderr; the per-phase
// inputs are intrinsic to batch processing, so this stays parameter-heavy.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn batch_process(
    prev_specs: &[TemplateSpec],
    curr_specs: &[TemplateSpec],
    upgraded_specs: &[&TemplateSpec], // Templates that need metadata saved
    target_dir: &Path,
    registry: &CyanRegistryClient,
    coord_client: &CyanCoordinatorClient,
    operator: &mut CompositionOperator,
    headless: bool,
) -> Result<
    (
        Vec<String>,
        Vec<FileConflictEntry>,
        Vec<String>,
        HashMap<String, Vec<String>>,
        Option<Question>,
    ),
    Box<dyn Error + Send>,
> {
    // PHASE 2: MAP (execute each template spec → VFS)
    crate::hprogress!(
        headless,
        "\n📦 PHASE 2: MAP - Executing {} prev + {} curr templates",
        prev_specs.len(),
        curr_specs.len()
    );

    // Execute prev_specs and collect template responses for horizontal layering.
    // Every coordinator session acquired below is registered with `session_guard`, which
    // releases them on ANY early `?` return between acquisition and the point the ids are
    // handed back to the caller — closing the window where a post-acquisition failure
    // (layering, merge, write, metadata save) would drop the ids and leak the sessions.
    let mut prev_vfs_list = Vec::new();
    let mut session_guard =
        SessionCleanupGuard::new(|sid: &str| release_session(coord_client, sid), Vec::new());
    let mut prev_template_res_list = Vec::new();

    for spec in prev_specs {
        crate::hprogress!(
            headless,
            "  🔄 Executing prev: {} v{}",
            spec.key(),
            spec.version
        );
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, final_state, session_ids, _commands) = operator.execute_template(
            &template_res,
            &spec.answers,
            &spec.deterministic_states,
            headless,
        )?;
        session_guard.extend(session_ids);
        // Headless: a (re-created) prev template stopped on an unanswered question.
        // Surface it immediately; no files are written. Hand the sessions to the caller
        // (which cleans them at the headless boundary) by disarming via `take`.
        if let Some(question) = final_state.need_input {
            return Ok((
                session_guard.take(),
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                Some(question),
            ));
        }
        prev_vfs_list.push(vfs);
        prev_template_res_list.push(template_res);
    }

    // Execute curr_specs and track final states for metadata. Sessions acquired here join
    // the same `session_guard` accumulation as the prev loop.
    let mut curr_vfs_list = Vec::new();
    // Map template_key -> final answers for metadata persistence
    let mut final_answers_map: HashMap<String, HashMap<String, Answer>> = HashMap::new();
    let mut curr_template_res_list = Vec::new();
    let mut curr_resolved_commands = Vec::new();
    // Per-template managed-files manifest, keyed by "<user>/<template>". Sourced from
    // each ACTIVE template's own output VFS BEFORE the LAYER/MERGE phases consume it,
    // so it reflects template output — never the merged result or the user's local files.
    let mut managed_by_template: HashMap<String, Vec<String>> = HashMap::new();

    for spec in curr_specs {
        crate::hprogress!(
            headless,
            "  🔄 Executing curr: {} v{}",
            spec.key(),
            spec.version
        );
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, final_state, session_ids, commands) = operator.execute_template(
            &template_res,
            &spec.answers,
            &spec.deterministic_states,
            headless,
        )?;
        session_guard.extend(session_ids);
        // Headless: this template stopped on an unanswered question. Surface it and
        // stop the batch before any layering / merge / write happens. Hand the accumulated
        // (prev + curr) sessions to the caller by disarming via `take`.
        if let Some(question) = final_state.need_input {
            return Ok((
                session_guard.take(),
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                Some(question),
            ));
        }
        curr_vfs_list.push(vfs);
        // Collect this template's normalized output paths from its own VFS (the active
        // set), before layering merges them into one.
        managed_by_template.insert(
            spec.key(),
            normalize_managed_paths(curr_vfs_list.last().unwrap()),
        );
        // Store the final answers for this template (includes Q&A answers)
        final_answers_map.insert(spec.key(), final_state.shared_answers);
        curr_template_res_list.push(template_res);
        // Collect commands from the full dependency tree (not just root templates)
        curr_resolved_commands.extend(commands);
    }

    // PHASE 3: LAYER (merge each list into ONE VFS)
    // Horizontal layering: collect resolvers from ONLY root templates (not dependencies)
    crate::hprogress!(
        headless,
        "\n🔀 PHASE 3: LAYER - Merging {} prev + {} curr VFS outputs",
        prev_vfs_list.len(),
        curr_vfs_list.len()
    );

    let prev_vfs = if prev_vfs_list.is_empty() {
        cyancoordinator::fs::VirtualFileSystem::new()
    } else if prev_vfs_list.len() == 1 {
        prev_vfs_list.into_iter().next().unwrap()
    } else {
        // Use resolver-aware horizontal layering
        operator.layer_merge_with_resolvers(
            &prev_vfs_list,
            &prev_template_res_list,
            coord_client,
            headless,
        )?
    };

    let curr_vfs = if curr_vfs_list.is_empty() {
        cyancoordinator::fs::VirtualFileSystem::new()
    } else if curr_vfs_list.len() == 1 {
        curr_vfs_list.into_iter().next().unwrap()
    } else {
        // Use resolver-aware horizontal layering
        operator.layer_merge_with_resolvers(
            &curr_vfs_list,
            &curr_template_res_list,
            coord_client,
            headless,
        )?
    };

    // PHASE 4: MERGE + WRITE
    crate::hprogress!(
        headless,
        "\n📝 PHASE 4: MERGE+WRITE - 3-way merge with local files"
    );

    let local_vfs = operator.load_local_files(target_dir)?;
    let merged_vfs = operator.merge(&prev_vfs, &local_vfs, &curr_vfs)?;

    // Clean up files that were deleted during merge
    let deleted = operator.cleanup_deleted_files(target_dir, &local_vfs, &merged_vfs)?;
    if !deleted.is_empty() {
        crate::hprogress!(
            headless,
            "🗑️ Removed {} file(s) no longer in template",
            deleted.len()
        );
    }

    operator.write_to_disk(target_dir, &merged_vfs)?;

    // Save metadata for upgraded templates only
    if !upgraded_specs.is_empty() {
        crate::hprogress!(
            headless,
            "💾 Saving template metadata for {} upgraded templates",
            upgraded_specs.len()
        );

        for spec in upgraded_specs {
            let template_res = registry.get_template(
                spec.username.clone(),
                spec.template_name.clone(),
                Some(spec.version),
            )?;

            // Use final answers from execution (includes Q&A answers) if available,
            // otherwise fall back to spec.answers
            let final_answers = final_answers_map
                .get(&spec.key())
                .cloned()
                .unwrap_or_else(|| spec.answers.clone());

            let template_state = TemplateState::Complete(
                Cyan {
                    processors: Vec::new(),
                    plugins: Vec::new(),
                },
                final_answers,
            );

            operator.get_template_history().save_template_metadata(
                target_dir,
                &template_res,
                &template_state,
                &spec.username,
            )?;
        }
    }

    // All fallible work is past — disarm the guard and hand the sessions to the caller,
    // which cleans them at the headless boundary (a `?` from any operation above instead
    // drops the still-armed guard and releases them).
    let all_session_ids = session_guard.take();

    // Collect file conflicts from operator for state persistence
    let file_conflicts = operator.get_file_conflicts().to_vec();

    // Use resolved commands from execute_template which includes the full dependency tree
    // (prev is just the 3-way-merge baseline; its commands would be duplicates or stale)
    let all_commands = curr_resolved_commands;

    crate::hprogress!(headless, "✅ Batch process complete");
    Ok((
        all_session_ids,
        file_conflicts,
        all_commands,
        managed_by_template,
        None,
    ))
}

/// The shallowest path component of `target_dir` that does not yet exist.
///
/// `create_dir_all(target_dir)` creates this directory and everything beneath it, so this
/// is the single directory whose removal undoes the whole creation (including any empty
/// intermediate parents of a nested new path like `a/b/c`). `ancestors()` yields deepest →
/// shallowest, so the LAST non-existent ancestor is the shallowest one to be created;
/// `None` means the target already existed and nothing will be created. A headless run that
/// stops before `done` removes exactly this directory so the filesystem is left untouched.
fn shallowest_uncreated_ancestor(target_dir: &Path) -> Option<PathBuf> {
    target_dir
        .ancestors()
        .filter(|a| !a.as_os_str().is_empty())
        .filter(|a| !a.exists())
        .last()
        .map(|p| p.to_path_buf())
}

/// Remove the directory tree this invocation created, if any. A headless run that stops
/// before `done` (a pending `need_input` or an error) must leave the filesystem exactly as
/// it found it; removing the shallowest created ancestor also drops any empty parent dirs
/// created for a nested new path. A no-op when nothing was created (`None`) — so a
/// pre-existing target directory is never touched. Best-effort: a removal error is ignored
/// (the directory is empty at this point, and a leftover empty dir must not mask the real
/// outcome being surfaced).
fn remove_created_dir(first_created_dir: &Option<PathBuf>) {
    if let Some(created) = first_created_dir {
        let _ = fs::remove_dir_all(created);
    }
}

/// RAII guard that removes the directory tree this headless invocation created if it
/// drops while still armed.
///
/// A headless run must leave the filesystem exactly as it found it until it reaches
/// `done`. Many return paths sit between `create_dir_all` and the point where output is
/// committed — a pending `need_input`, a batch transport/validation error, and pre-batch
/// early returns (e.g. an invalid template with neither dependencies nor execution
/// artifacts). Patching each return site individually has historically missed paths, so
/// the cleanup is bound to the guard's `Drop` instead: arm it right after the directory is
/// created and `disarm()` it exactly once the run is committed to writing output (the
/// `need_input` check has passed). Every error/`need_input` return before that disarm
/// removes the created tree automatically; the committed `done` path keeps it.
///
/// Interactive runs pass `headless = false`, so the guard is inert and the interactive
/// path is byte-identical (no removal ever happens).
struct CreatedDirGuard {
    first_created_dir: Option<PathBuf>,
    headless: bool,
    armed: bool,
}

impl CreatedDirGuard {
    fn new(first_created_dir: Option<PathBuf>, headless: bool) -> Self {
        Self {
            first_created_dir,
            headless,
            armed: true,
        }
    }

    /// Mark the run as committed to its output so dropping the guard no longer removes the
    /// created directory. Called once the `need_input` check has passed and the happy path
    /// is about to persist state / write files.
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CreatedDirGuard {
    fn drop(&mut self) {
        if self.headless && self.armed {
            remove_created_dir(&self.first_created_dir);
        }
    }
}

/// Release a single coordinator session against the given client, best-effort. A release
/// error is ignored: cleanup must not mask the run's own outcome, and the session will be
/// reclaimed by the coordinator's own timeout regardless.
pub(crate) fn release_session(coord_client: &CyanCoordinatorClient, session_id: &str) {
    let _ = coord_client.clean(session_id.to_string());
}

/// RAII guard that releases coordinator sessions on `Drop` while armed.
///
/// Coordinator sessions are acquired incrementally as templates execute, but several
/// fallible operations (resolver-aware layering, file load, merge, deleted-file cleanup,
/// `write_to_disk`, metadata + state save, post-template commands) run BETWEEN acquisition
/// and the point the ids are handed to the headless cleanup boundary (`finish_headless`).
/// A `?` in that window drops the local id vectors and returns `Err` before the boundary
/// ever sees the ids — leaking the sessions until the coordinator's own timeout. Patching
/// each return site individually has historically missed paths (the same recurring class as
/// the directory leak), so cleanup is bound to this guard's `Drop`: register each id as it
/// is acquired and it is released best-effort on ANY early return. [`take`](Self::take)
/// disarms and hands ownership of the ids back once they reach a caller that WILL clean them
/// (a `done`/`need_input` boundary return), preventing a double release.
///
/// Releasing a session on error is correct in BOTH headless and interactive modes (it is a
/// coordinator HTTP call, not user-facing output), so — unlike [`CreatedDirGuard`] — this
/// guard is NOT mode-gated; the interactive path is unaffected in its output while also no
/// longer leaking sessions on these error paths.
///
/// The release action is a closure (production passes [`release_session`] bound to the
/// coordinator client) so the guard's DECISION — which ids it releases, and that `take`
/// disarms it — is unit-testable with a recorder closure and no live coordinator.
pub(crate) struct SessionCleanupGuard<F: FnMut(&str)> {
    release: F,
    session_ids: Vec<String>,
    armed: bool,
}

impl<F: FnMut(&str)> SessionCleanupGuard<F> {
    pub(crate) fn new(release: F, session_ids: Vec<String>) -> Self {
        Self {
            release,
            session_ids,
            armed: true,
        }
    }

    /// Register newly-acquired session ids with the guard so they are released if it drops
    /// while still armed.
    fn extend(&mut self, ids: impl IntoIterator<Item = String>) {
        self.session_ids.extend(ids);
    }

    /// Disarm and return the accumulated session ids. After this the caller owns cleanup
    /// (e.g. the headless boundary `finish_headless`), so the guard must not also release
    /// them.
    pub(crate) fn take(&mut self) -> Vec<String> {
        self.armed = false;
        std::mem::take(&mut self.session_ids)
    }
}

impl<F: FnMut(&str)> Drop for SessionCleanupGuard<F> {
    fn drop(&mut self) {
        if self.armed {
            for sid in &self.session_ids {
                (self.release)(sid);
            }
        }
    }
}

/// Run the cyan template generation process with automatic composition detection
/// Returns all session IDs that were created and need to be cleaned up
#[allow(clippy::too_many_arguments)]
pub fn cyan_run(
    session_id_generator: Box<dyn SessionIdGenerator>,
    path: Option<String>,
    template: TemplateVersionRes,
    coord_client: CyanCoordinatorClient,
    username: String,
    registry_client: Rc<CyanRegistryClient>,
    debug: bool,
    cache_config: cyancoordinator::cache::CacheConfig,
    headless: bool,
    headless_answers: HashMap<String, Answer>,
) -> Result<CyanRunResult, Box<dyn Error + Send>> {
    // Handle the target directory
    let path = path.unwrap_or(".".to_string());
    let path_buf = PathBuf::from(&path);
    let target_dir = path_buf.as_path();
    crate::hprogress!(headless, "📁 Target directory: {target_dir:?}");
    // Find the shallowest path component that does not yet exist: `create_dir_all`
    // below will create this directory and everything beneath it. A headless
    // `need_input` (which must leave the filesystem untouched until `done`) removes
    // exactly this directory, so a nested new path (e.g. `a/b/c`) leaves no empty
    // `a/` and `a/b/` parents behind. `ancestors()` yields deepest → shallowest, so
    // the last non-existent ancestor is the shallowest one to be created; `None`
    // means the target already existed and nothing is created. See the `need_input`
    // short-circuit below.
    let first_created_dir = shallowest_uncreated_ancestor(target_dir);
    fs::create_dir_all(target_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    // Arm the directory guard immediately after creation: ANY return before the run is
    // committed to writing output (a pre-batch early return, a batch error, or a pending
    // `need_input`) drops the guard while armed and removes exactly the tree this
    // invocation created, leaving the filesystem as it was found. Disarmed once the
    // `need_input` check passes (the `done` path then keeps its output). Inert when
    // interactive, so that path is unchanged.
    let mut dir_guard = CreatedDirGuard::new(first_created_dir, headless);

    // Create all components for dependency injection at the highest level
    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    // In headless mode the merger's debug output uses plain `println!`, which would
    // pollute the single-JSON-on-stdout contract. Disable merger debug under
    // headless so stdout stays reserved for the envelope; interactive `--debug` is
    // unchanged.
    let merger = Box::new(GitLikeMerger::new(debug && !headless, 50));
    let writer = Box::new(DiskFileWriter);

    // Setup services with explicit dependencies
    let template_history = Box::new(DefaultTemplateHistory::new());
    let template_executor = Box::new(DefaultTemplateExecutor::new_with_headless(
        coord_client.endpoint.clone(),
        headless,
    ));
    let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

    // Create the TemplateOperator
    let template_operator = TemplateOperator::new(
        session_id_generator,
        template_executor,
        template_history,
        vfs,
        registry_client.clone(),
    );

    // Check template history to determine update scenario
    let update_type =
        DefaultTemplateHistory::new().check_template_history(target_dir, &template, &username)?;

    // Create composition-specific components (needed for both single and composition templates
    // to support batch processing when adding to existing projects)
    let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));

    // Create the CompositionOperator with client for resolver-aware layering.
    // Clone the client since we also need it for batch_process.
    let mut composition_operator = CompositionOperator::with_client(
        template_operator,
        dependency_resolver,
        coord_client.clone(),
    );
    // Inject the per-node execution cache (honors --no-output-cache / --cache-dir / env).
    composition_operator.set_cache(cyancoordinator::cache::Cache::new(cache_config));

    // Check if this is a composition template (has dependencies)
    let is_composition = has_dependencies(&template);

    // Log template type
    if is_composition {
        if has_execution_artifacts(&template) {
            crate::hprogress!(
                headless,
                "🔗 Template with {} dependencies and execution artifacts - using composition execution",
                template.templates.len()
            );
        } else {
            crate::hprogress!(
                headless,
                "🔗 Template group with {} dependencies (no execution artifacts) - using composition execution",
                template.templates.len()
            );
        }
    } else if has_execution_artifacts(&template) {
        crate::hprogress!(
            headless,
            "📦 Single template with execution artifacts - using single template execution"
        );
    } else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid template: no dependencies and no execution artifacts. Templates must have either dependencies or execution artifacts.",
        )) as Box<dyn Error + Send>);
    }

    // Create the manager for composable spec operations
    let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

    // Load the current state (may be empty for new projects)
    let state_file_path = target_dir.join(".cyan_state.yaml");
    let state = DefaultStateManager::new()
        .load_state_file(&state_file_path)
        .unwrap_or_default();

    // Build specs using composable primitives
    let mut prev_specs = manager.get(&state);
    sort_specs(&mut prev_specs);

    let (prev_specs, curr_specs, upgraded_specs): (
        Vec<TemplateSpec>,
        Vec<TemplateSpec>,
        Vec<TemplateSpec>,
    ) = match update_type {
        TemplateUpdateType::NewTemplate => {
            // New template being added
            let new_spec = TemplateSpec::new_template(
                username.clone(),
                template.template.name.clone(),
                template.principal.version,
            );

            let curr: Vec<_> = prev_specs
                .iter()
                .cloned()
                .chain(iter::once(new_spec.clone()))
                .collect();

            // The new template is the only one that needs metadata saved
            (prev_specs, curr, vec![new_spec])
        }
        TemplateUpdateType::UpgradeTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Upgrade: prev uses old version, curr uses new version
            let target_key = format!("{}/{}", username, template.template.name);

            // Build prev_specs with the OLD version for the target template
            let prev: Vec<_> = state
                .templates
                .iter()
                .filter(|(_, s)| s.active)
                .filter_map(|(key, s)| {
                    let (u, t) = parse_template_key(key)?;
                    let entry = s.history.last()?;

                    if key == &target_key {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            previous_version,
                            previous_answers.clone(),
                            previous_states.clone(),
                            entry.time,
                        ))
                    } else {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            entry.version,
                            entry.answers.clone(),
                            entry.deterministic_states.clone(),
                            entry.time,
                        ))
                    }
                })
                .collect();

            // Build curr_specs with the NEW version for the target template
            let curr: Vec<_> = state
                .templates
                .iter()
                .filter(|(_, s)| s.active)
                .filter_map(|(key, s)| {
                    let (u, t) = parse_template_key(key)?;
                    let entry = s.history.last()?;

                    if key == &target_key {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            template.principal.version,
                            previous_answers.clone(),
                            previous_states.clone(),
                            entry.time,
                        ))
                    } else {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            entry.version,
                            entry.answers.clone(),
                            entry.deterministic_states.clone(),
                            entry.time,
                        ))
                    }
                })
                .collect();

            // Explicitly track what changed
            let upgraded: Vec<_> = curr
                .iter()
                .filter(|s| s.key() == target_key)
                .cloned()
                .collect();

            (prev, curr, upgraded)
        }
        TemplateUpdateType::RerunTemplate {
            previous_version,
            previous_answers,
            previous_states,
        } => {
            // Rerun: prev uses old answers, curr uses empty answers (triggers Q&A)
            let target_key = format!("{}/{}", username, template.template.name);

            // Build prev_specs with the OLD answers for the target template
            let prev: Vec<_> = state
                .templates
                .iter()
                .filter(|(_, s)| s.active)
                .filter_map(|(key, s)| {
                    let (u, t) = parse_template_key(key)?;
                    let entry = s.history.last()?;

                    if key == &target_key {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            previous_version,
                            previous_answers.clone(),
                            previous_states.clone(),
                            entry.time,
                        ))
                    } else {
                        Some(TemplateSpec::new(
                            u.to_string(),
                            t.to_string(),
                            entry.version,
                            entry.answers.clone(),
                            entry.deterministic_states.clone(),
                            entry.time,
                        ))
                    }
                })
                .collect();

            // Use manager.reset() to clear answers for curr_specs
            let curr = manager.reset(prev.clone());

            // Explicitly track what changed (the rerun target)
            let upgraded: Vec<_> = curr
                .iter()
                .filter(|s| s.key() == target_key)
                .cloned()
                .collect();

            (prev, curr, upgraded)
        }
    };

    // Headless: seed the supplied answers ONLY into the template(s) this invocation
    // is creating/upgrading/rerunning — never into pre-existing, already-installed
    // templates that happen to be re-executed as part of the batch. A flat answer map
    // has no per-template scoping, so seeding it into every curr_spec would let an
    // answer intended for the target template (e.g. `name`, `token`) silently satisfy
    // an unrelated installed template's same-named question, skipping its expected
    // `need_input` and generating with the wrong value. Scoping to the upgraded set
    // (NewTemplate → the new spec; Upgrade/Rerun → the target) keeps each template's
    // Q&A independent.
    let curr_specs: Vec<TemplateSpec> = if headless && !headless_answers.is_empty() {
        let upgraded_keys: std::collections::HashSet<String> =
            upgraded_specs.iter().map(|s| s.key()).collect();
        curr_specs
            .into_iter()
            .map(|mut s| {
                if upgraded_keys.contains(&s.key()) {
                    for (k, v) in &headless_answers {
                        s.answers.insert(k.clone(), v.clone());
                    }
                }
                s
            })
            .collect()
    } else {
        curr_specs
    };

    // Convert upgraded_specs to references for batch_process
    let upgraded_refs: Vec<&TemplateSpec> = upgraded_specs.iter().collect();

    // Execute unified batch processing
    let (session_ids, file_conflicts, commands, managed_by_template, need_input) =
        match batch_process(
            &prev_specs,
            &curr_specs,
            &upgraded_refs,
            target_dir,
            &registry_client,
            &coord_client,
            &mut composition_operator,
            headless,
        ) {
            Ok(v) => v,
            Err(err) => {
                // Headless: a supplied-answer validation failure or a transport error during
                // the batch surfaces here (it never reaches a write). The still-armed
                // `dir_guard` drops on this return and removes the directory tree THIS
                // invocation created, leaving the filesystem as it was found. Interactive
                // runs are untouched (the guard is inert when not headless).
                return Err(err);
            }
        };

    // Headless: a question is pending. Do NOT persist state, write files, or run
    // post-commands — surface the question and stop (stateless replay). The still-armed
    // `dir_guard` drops on this return and removes the directory tree THIS invocation
    // created (the walk stopped before PHASE 4, so the tree is empty and removal is safe),
    // leaving the filesystem exactly as found; a pre-existing target is untouched.
    if let Some(question) = need_input {
        return Ok(CyanRunResult {
            session_ids,
            need_input: Some(question),
        });
    }

    // Past the `need_input` check the run is committed to producing output (`done`):
    // disarm the directory guard so the created tree is kept from here on.
    dir_guard.disarm();

    // The run is committed, but several fallible steps remain (state save, post-template
    // commands) BEFORE the session ids reach `finish_headless`. A `?`/early `return Err`
    // from any of them would drop the ids and leak the coordinator sessions. Hand them to a
    // cleanup guard so every error path below releases them; `take()` disarms it on the
    // happy `done` return (where `finish_headless` then cleans normally).
    let mut session_guard =
        SessionCleanupGuard::new(|sid: &str| release_session(&coord_client, sid), session_ids);

    // One-line cache summary (always printed when caching is enabled). (FR15)
    composition_operator.print_cache_summary();

    // Persist file conflicts to state file (always update to clear stale entries)
    let state_manager = DefaultStateManager::new();
    let mut cyan_state = state_manager
        .load_state_file(&state_file_path)
        .unwrap_or_default();
    let conflicts_count = file_conflicts.len();
    cyan_state.file_conflicts = file_conflicts;
    // Recompute the managed-files manifest wholesale from this run's active
    // templates: sets each template's `files` and the top-level `managed_files`
    // union, clearing stale entries (e.g. for now-deactivated templates).
    cyan_state.set_managed_files(&managed_by_template);
    let managed_count = cyan_state.managed_files.len();
    // A save failure here drops the still-armed `session_guard`, releasing the sessions.
    state_manager.save_state_file(&cyan_state, &state_file_path)?;
    if conflicts_count > 0 {
        crate::hprogress!(
            headless,
            "📝 Saved {conflicts_count} file conflict(s) to state"
        );
    }
    if managed_count > 0 {
        crate::hprogress!(
            headless,
            "📝 Recorded {managed_count} managed file(s) in state"
        );
    }

    // Execute commands if any were collected
    if !commands.is_empty() {
        crate::hprogress!(
            headless,
            "\n⚡ Executing {} post-template command(s)...",
            commands.len()
        );
        // Each error arm below returns `Err` while `session_guard` is still armed, so its
        // `Drop` releases the coordinator sessions — no per-arm cleanup loop needed.
        let exec_result =
            CommandExecutor::execute_commands_for_mode(&commands, target_dir, headless)?;
        if exec_result.aborted {
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution aborted: {}/{} succeeded, {}/{} failed before abort",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
        if headless && !exec_result.all_succeeded() {
            // The non-interactive path runs every command and records failures in
            // the result but returns Ok — it never sets `aborted`. Without this check a
            // failed post-template command (e.g. a command exiting non-zero) would be
            // silently ignored and the run would report `done` / exit 0. In headless mode
            // there is no interactive "continue?" prompt to surface the failure, so treat
            // any partial failure as an error → error envelope / exit 1 (the still-armed
            // `session_guard` releases the sessions on return). Interactive mode keeps its
            // existing behavior: the user already chose whether to continue.
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution failed: {}/{} succeeded, {}/{} failed",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
    }

    // `done`: disarm the guard and hand the sessions to `finish_headless`, which cleans
    // them at the command boundary.
    Ok(CyanRunResult::completed(session_guard.take()))
}

/// Parse template key from the update module
fn parse_template_key(template_key: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = template_key.split('/').collect();
    (parts.len() == 2).then(|| (parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A headless run that stops before `done` (need_input OR error) must remove the
    // directory tree IT created so the filesystem is left exactly as found. For a nested
    // new path the shallowest created ancestor is identified and removing it drops the
    // empty parents too. This is the cleanup shared by the need_input short-circuit and
    // the headless-error arm in `cyan_run`.
    #[test]
    fn created_dir_cleanup_removes_nested_tree() {
        let base = tempfile::TempDir::new().expect("temp dir");
        // Target is two new levels below an existing base: base/a/b. Neither `a` nor
        // `a/b` exists yet, so the shallowest uncreated ancestor is `base/a`.
        let target = base.path().join("a").join("b");
        let shallowest = shallowest_uncreated_ancestor(&target);
        assert_eq!(
            shallowest.as_deref(),
            Some(base.path().join("a").as_path()),
            "shallowest uncreated ancestor must be the first new level"
        );

        fs::create_dir_all(&target).expect("create target tree");
        assert!(target.exists(), "create_dir_all built the nested tree");

        // Removing the shallowest created ancestor removes the whole created tree,
        // leaving the pre-existing base untouched.
        remove_created_dir(&shallowest);
        assert!(
            !base.path().join("a").exists(),
            "the created tree (including empty parent `a/`) must be gone"
        );
        assert!(base.path().exists(), "the pre-existing base must remain");
    }

    // When the target directory already exists, nothing is created, so there is nothing
    // to remove: `shallowest_uncreated_ancestor` is None and `remove_created_dir` is a
    // no-op that leaves the existing directory intact (the default "." case).
    #[test]
    fn created_dir_cleanup_is_noop_for_existing_target() {
        let base = tempfile::TempDir::new().expect("temp dir");
        let target = base.path().to_path_buf();
        assert!(
            shallowest_uncreated_ancestor(&target).is_none(),
            "an existing target has no uncreated ancestor"
        );
        // A None cleanup must not touch the existing directory.
        remove_created_dir(&None);
        assert!(
            base.path().exists(),
            "existing target must be left untouched"
        );
    }

    // An armed headless guard that drops before the run commits (any pre-batch early
    // return, batch error, or pending need_input) removes the created tree. This is the
    // single mechanism that now covers ALL of those return paths, not just the batch
    // error / need_input sites the per-path cleanup previously covered.
    #[test]
    fn dir_guard_removes_created_tree_when_armed_and_headless() {
        let base = tempfile::TempDir::new().expect("temp dir");
        let target = base.path().join("a").join("b");
        let first_created = shallowest_uncreated_ancestor(&target);
        fs::create_dir_all(&target).expect("create target tree");
        assert!(target.exists());

        {
            // Simulate any non-committed return: the guard is still armed when it drops.
            let _guard = CreatedDirGuard::new(first_created, true);
        }

        assert!(
            !base.path().join("a").exists(),
            "an armed headless guard must remove the created tree on drop"
        );
        assert!(base.path().exists(), "the pre-existing base must remain");
    }

    // Once the run commits to output (`need_input` check passed), the guard is disarmed
    // and dropping it keeps the written tree — the `done` path must not delete its own
    // output.
    #[test]
    fn dir_guard_keeps_created_tree_after_disarm() {
        let base = tempfile::TempDir::new().expect("temp dir");
        let target = base.path().join("a").join("b");
        let first_created = shallowest_uncreated_ancestor(&target);
        fs::create_dir_all(&target).expect("create target tree");

        {
            let mut guard = CreatedDirGuard::new(first_created, true);
            guard.disarm();
        }

        assert!(
            target.exists(),
            "a disarmed guard must keep the created tree (the committed `done` path)"
        );
    }

    // AC7: the guard is inert in interactive mode — dropping it while armed must NOT
    // remove the directory, so the non-headless path is byte-identical to before.
    #[test]
    fn dir_guard_is_inert_when_not_headless() {
        let base = tempfile::TempDir::new().expect("temp dir");
        let target = base.path().join("a").join("b");
        let first_created = shallowest_uncreated_ancestor(&target);
        fs::create_dir_all(&target).expect("create target tree");

        {
            // Interactive run: headless = false, guard armed, dropped without disarm.
            let _guard = CreatedDirGuard::new(first_created, false);
        }

        assert!(
            target.exists(),
            "an interactive (non-headless) guard must never remove the directory"
        );
    }

    // AC7 (FR7): paths are relative, forward-slash, no leading "./" or '/', no
    // trailing '/'; backslash / "./"-prefixed inputs normalize to canonical form.
    #[test]
    fn normalize_path_canonicalizes() {
        let cases = [
            ("a.txt", "a.txt"),
            ("./a.txt", "a.txt"),
            ("/a.txt", "a.txt"),
            ("dir/b.txt", "dir/b.txt"),
            ("./dir/b.txt", "dir/b.txt"),
            ("dir/", "dir"),
            ("./nested/deep/c.txt", "nested/deep/c.txt"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                normalize_path(Path::new(input)),
                expected,
                "normalize_path({input:?})"
            );
        }
    }

    // AC7 (FR7): a path built from separate components renders with forward-slash
    // separators (the join uses '/' regardless of the OS separator).
    #[test]
    fn normalize_path_uses_forward_slashes() {
        let p: PathBuf = ["dir", "sub", "file.txt"].iter().collect();
        assert_eq!(normalize_path(&p), "dir/sub/file.txt");
    }

    // AC7 (FR7): a LITERAL backslash-separated input (Windows-style, authored as a
    // single string) normalizes to forward-slash form on every platform. This is the
    // case `normalize_path_uses_forward_slashes` does NOT cover: on Unix,
    // `Path::components()` does not split on '\', so the raw backslashes reach the
    // explicit `\` -> `/` substitution rather than being pre-split into components.
    #[test]
    fn normalize_path_converts_literal_backslashes() {
        assert_eq!(
            normalize_path(&PathBuf::from(r"dir\sub\file.txt")),
            "dir/sub/file.txt"
        );
        // A leading ".\" (backslash form) also normalizes away to the canonical form.
        assert_eq!(
            normalize_path(&PathBuf::from(r".\dir\file.txt")),
            "dir/file.txt"
        );
    }

    // AC6 (FR6): cyanprint bookkeeping files are recognized as internal; ordinary
    // files are not.
    #[test]
    fn is_cyanprint_internal_matches_bookkeeping() {
        assert!(is_cyanprint_internal(".cyan_state.yaml"));
        assert!(is_cyanprint_internal(".cyan_output"));
        // Top-level bookkeeping directory entries are excluded too.
        assert!(is_cyanprint_internal(".cyan_output/foo.txt"));

        assert!(!is_cyanprint_internal("a.txt"));
        assert!(!is_cyanprint_internal("src/.cyan_state.yaml"));
        assert!(!is_cyanprint_internal("dir/normal.txt"));
    }

    // AC6 + AC7: normalize_managed_paths excludes bookkeeping, normalizes,
    // sorts, and de-duplicates.
    #[test]
    fn normalize_managed_paths_filters_and_sorts() {
        let mut vfs = VirtualFileSystem::new();
        vfs.add_file(PathBuf::from("./b.txt"), vec![]);
        vfs.add_file(PathBuf::from("a.txt"), vec![]);
        vfs.add_file(PathBuf::from(".cyan_state.yaml"), vec![]);
        vfs.add_file(PathBuf::from(".cyan_output"), vec![]);
        vfs.add_file(PathBuf::from("dir/c.txt"), vec![]);

        let paths = normalize_managed_paths(&vfs);
        assert_eq!(
            paths,
            vec![
                "a.txt".to_string(),
                "b.txt".to_string(),
                "dir/c.txt".to_string()
            ]
        );
    }

    // Coordinator sessions acquired during a run must not leak when a fallible step
    // between acquisition and the headless cleanup boundary fails. `SessionCleanupGuard`
    // releases every registered id on `Drop` while armed. Here a recorder closure stands in
    // for the real `release_session` so the guard's DECISION is asserted with no live
    // coordinator: an armed drop releases ALL accumulated ids exactly once.
    #[test]
    fn session_guard_releases_all_ids_on_armed_drop() {
        let released = std::cell::RefCell::new(Vec::<String>::new());
        {
            let mut guard = SessionCleanupGuard::new(
                |sid: &str| released.borrow_mut().push(sid.to_string()),
                vec!["s1".to_string()],
            );
            // Sessions acquired incrementally (the prev + curr template loops).
            guard.extend(["s2".to_string(), "s3".to_string()]);
            // Simulate a `?` error after acquisition: the guard drops while still armed.
        }
        assert_eq!(
            released.borrow().as_slice(),
            &["s1".to_string(), "s2".to_string(), "s3".to_string()],
            "an armed guard must release every accumulated session id on drop"
        );
    }

    // Once the ids are handed to the caller (the `done`/`need_input` boundary, which cleans
    // them itself), `take()` disarms the guard so dropping it does NOT release them again —
    // preventing a double release.
    #[test]
    fn session_guard_take_disarms_and_returns_ids() {
        let released = std::cell::RefCell::new(Vec::<String>::new());
        let taken;
        {
            let mut guard = SessionCleanupGuard::new(
                |sid: &str| released.borrow_mut().push(sid.to_string()),
                vec!["a".to_string(), "b".to_string()],
            );
            taken = guard.take();
            // Guard drops here, disarmed — must NOT release.
        }
        assert_eq!(
            taken,
            vec!["a".to_string(), "b".to_string()],
            "take() must hand back the accumulated ids"
        );
        assert!(
            released.borrow().is_empty(),
            "a disarmed guard must not release sessions on drop (caller owns cleanup)"
        );
    }
}
