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
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use cyancoordinator::fs::DefaultVfs;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::template::DefaultTemplateExecutor;
use cyancoordinator::template::{DefaultTemplateHistory, TemplateUpdateType};

use cyancoordinator::fs::VirtualFileSystem;

use crate::command_executor::CommandExecutor;
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
    // forward slashes on every platform — AC7/FR7 requires this canonical form.
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
#[allow(clippy::type_complexity)]
pub fn batch_process(
    prev_specs: &[TemplateSpec],
    curr_specs: &[TemplateSpec],
    upgraded_specs: &[&TemplateSpec], // Templates that need metadata saved
    target_dir: &Path,
    registry: &CyanRegistryClient,
    coord_client: &CyanCoordinatorClient,
    operator: &mut CompositionOperator,
) -> Result<
    (
        Vec<String>,
        Vec<FileConflictEntry>,
        Vec<String>,
        HashMap<String, Vec<String>>,
    ),
    Box<dyn Error + Send>,
> {
    // PHASE 2: MAP (execute each template spec → VFS)
    println!(
        "\n📦 PHASE 2: MAP - Executing {} prev + {} curr templates",
        prev_specs.len(),
        curr_specs.len()
    );

    // Execute prev_specs and collect template responses for horizontal layering
    let mut prev_vfs_list = Vec::new();
    let mut prev_session_ids = Vec::new();
    let mut prev_template_res_list = Vec::new();

    for spec in prev_specs {
        println!("  🔄 Executing prev: {} v{}", spec.key(), spec.version);
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, _final_state, session_ids, _commands) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        prev_vfs_list.push(vfs);
        prev_session_ids.extend(session_ids);
        prev_template_res_list.push(template_res);
    }

    // Execute curr_specs and track final states for metadata
    let mut curr_vfs_list = Vec::new();
    let mut curr_session_ids = Vec::new();
    // Map template_key -> final answers for metadata persistence
    let mut final_answers_map: HashMap<String, HashMap<String, Answer>> = HashMap::new();
    let mut curr_template_res_list = Vec::new();
    let mut curr_resolved_commands = Vec::new();
    // Per-template managed-files manifest, keyed by "<user>/<template>". Sourced from
    // each ACTIVE template's own output VFS BEFORE the LAYER/MERGE phases consume it,
    // so it reflects template output — never the merged result or the user's local files.
    let mut managed_by_template: HashMap<String, Vec<String>> = HashMap::new();

    for spec in curr_specs {
        println!("  🔄 Executing curr: {} v{}", spec.key(), spec.version);
        let template_res = registry.get_template(
            spec.username.clone(),
            spec.template_name.clone(),
            Some(spec.version),
        )?;
        let (vfs, final_state, session_ids, commands) =
            operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        curr_vfs_list.push(vfs);
        // Collect this template's normalized output paths from its own VFS (the active
        // set), before layering merges them into one.
        managed_by_template.insert(
            spec.key(),
            normalize_managed_paths(curr_vfs_list.last().unwrap()),
        );
        curr_session_ids.extend(session_ids);
        // Store the final answers for this template (includes Q&A answers)
        final_answers_map.insert(spec.key(), final_state.shared_answers);
        curr_template_res_list.push(template_res);
        // Collect commands from the full dependency tree (not just root templates)
        curr_resolved_commands.extend(commands);
    }

    // PHASE 3: LAYER (merge each list into ONE VFS)
    // Horizontal layering: collect resolvers from ONLY root templates (not dependencies)
    println!(
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
        )?
    };

    // PHASE 4: MERGE + WRITE
    println!("\n📝 PHASE 4: MERGE+WRITE - 3-way merge with local files");

    let local_vfs = operator.load_local_files(target_dir)?;
    let merged_vfs = operator.merge(&prev_vfs, &local_vfs, &curr_vfs)?;

    // Clean up files that were deleted during merge
    let deleted = operator.cleanup_deleted_files(target_dir, &local_vfs, &merged_vfs)?;
    if !deleted.is_empty() {
        println!("🗑️ Removed {} file(s) no longer in template", deleted.len());
    }

    operator.write_to_disk(target_dir, &merged_vfs)?;

    // Save metadata for upgraded templates only
    if !upgraded_specs.is_empty() {
        println!(
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

    let mut all_session_ids = prev_session_ids;
    all_session_ids.extend(curr_session_ids);

    // Collect file conflicts from operator for state persistence
    let file_conflicts = operator.get_file_conflicts().to_vec();

    // Use resolved commands from execute_template which includes the full dependency tree
    // (prev is just the 3-way-merge baseline; its commands would be duplicates or stale)
    let all_commands = curr_resolved_commands;

    println!("✅ Batch process complete");
    Ok((
        all_session_ids,
        file_conflicts,
        all_commands,
        managed_by_template,
    ))
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
) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // Handle the target directory
    let path = path.unwrap_or(".".to_string());
    let path_buf = PathBuf::from(&path);
    let target_dir = path_buf.as_path();
    println!("📁 Target directory: {target_dir:?}");
    fs::create_dir_all(target_dir).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Create all components for dependency injection at the highest level
    let unpacker = Box::new(TarGzUnpacker);
    let loader = Box::new(DiskFileLoader);
    let merger = Box::new(GitLikeMerger::new(debug, 50));
    let writer = Box::new(DiskFileWriter);

    // Setup services with explicit dependencies
    let template_history = Box::new(DefaultTemplateHistory::new());
    let template_executor = Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
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

    // Create the CompositionOperator with client for resolver-aware layering
    // Clone the client since we also need it for batch_process
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
            println!(
                "🔗 Template with {} dependencies and execution artifacts - using composition execution",
                template.templates.len()
            );
        } else {
            println!(
                "🔗 Template group with {} dependencies (no execution artifacts) - using composition execution",
                template.templates.len()
            );
        }
    } else if has_execution_artifacts(&template) {
        println!("📦 Single template with execution artifacts - using single template execution");
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

    // Convert upgraded_specs to references for batch_process
    let upgraded_refs: Vec<&TemplateSpec> = upgraded_specs.iter().collect();

    // Execute unified batch processing
    let (session_ids, file_conflicts, commands, managed_by_template) = batch_process(
        &prev_specs,
        &curr_specs,
        &upgraded_refs,
        target_dir,
        &registry_client,
        &coord_client,
        &mut composition_operator,
    )?;

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
    state_manager.save_state_file(&cyan_state, &state_file_path)?;
    if conflicts_count > 0 {
        println!("📝 Saved {conflicts_count} file conflict(s) to state");
    }
    if managed_count > 0 {
        println!("📝 Recorded {managed_count} managed file(s) in state");
    }

    // Execute commands if any were collected
    if !commands.is_empty() {
        println!(
            "\n⚡ Executing {} post-template command(s)...",
            commands.len()
        );
        let exec_result = match CommandExecutor::execute_commands(&commands, target_dir) {
            Ok(result) => result,
            Err(err) => {
                // Clean up coordinator sessions before propagating the error
                for sid in &session_ids {
                    let _ = coord_client.clean(sid.clone());
                }
                return Err(err);
            }
        };
        if exec_result.aborted {
            // Clean up coordinator sessions before returning on abort
            for sid in &session_ids {
                let _ = coord_client.clean(sid.clone());
            }
            return Err(Box::new(std::io::Error::other(format!(
                "Command execution aborted: {}/{} succeeded, {}/{} failed before abort",
                exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
            ))));
        }
    }

    Ok(session_ids)
}

/// Parse template key from the update module
fn parse_template_key(template_key: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = template_key.split('/').collect();
    (parts.len() == 2).then(|| (parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
