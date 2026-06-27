use std::error::Error;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::session::SessionIdGenerator;
use cyancoordinator::state::{DefaultStateManager, StateReader, StateWriter};
use cyanregistry::http::client::CyanRegistryClient;
use inquire::Select;

use super::operator_factory::OperatorFactory;
use super::spec::{TemplateSpec, TemplateSpecManager, sort_specs};
use crate::command_executor::CommandExecutor;
use crate::git::{GitError, get_modified_files, is_git_dirty};
use crate::headless::CyanRunResult;
use crate::run::{SessionCleanupGuard, batch_process, release_session};

/// Error type for user-initiated abort
#[derive(Debug)]
pub struct UserAborted;

impl fmt::Display for UserAborted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Update aborted by user")
    }
}

impl Error for UserAborted {}

/// Main orchestrator for the template update process
pub struct UpdateOrchestrator;

impl UpdateOrchestrator {
    /// Update all templates in a project to their latest versions with automatic composition detection
    /// Uses unified batch VFS processing: MAP -> LAYER -> MERGE+WRITE
    /// Returns all session IDs that were created and need to be cleaned up
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    pub fn update_templates(
        session_id_generator: Box<dyn SessionIdGenerator>,
        path: String,
        coord_client: CyanCoordinatorClient,
        registry_client: Rc<CyanRegistryClient>,
        debug: bool,
        interactive: bool,
        force: bool,
        cache_config: cyancoordinator::cache::CacheConfig,
        headless: bool,
        headless_answers: std::collections::HashMap<
            String,
            cyanprompt::domain::models::answer::Answer,
        >,
    ) -> Result<CyanRunResult, Box<dyn Error + Send>> {
        let target_dir = Path::new(&path);

        // `--headless` and `--interactive` are mutually exclusive at the clap layer,
        // but force auto-latest version selection here too as defense-in-depth. Interactive
        // version selection (`select_version_interactive`) does a bare `println!` and an
        // `inquire::Select` prompt, both of which would break headless mode (no TTY
        // interaction, non-JSON stdout). Headless always takes the auto-latest path.
        let interactive = interactive && !headless;

        // === GIT DIRTY CHECK STARTS HERE ===
        if !force {
            match is_git_dirty(target_dir) {
                Ok(true) => {
                    if headless {
                        // Headless mode cannot answer an interactive Select prompt,
                        // and silently overwriting uncommitted changes would be unsafe.
                        // Surface the dirty state as an error envelope (exit 1) so the
                        // caller resolves the working tree or re-runs with --force.
                        return Err(Box::new(std::io::Error::other(
                            "working directory has uncommitted changes; commit/stash them or re-run with --force",
                        )) as Box<dyn Error + Send>);
                    }

                    // Git is dirty - prompt user
                    eprintln!("⚠️  Warning: Working directory has uncommitted changes");
                    eprintln!();

                    // Show modified files
                    if let Ok(files) = get_modified_files(target_dir) {
                        if !files.is_empty() {
                            eprintln!("Modified files:");
                            for file in files.iter().take(10) {
                                eprintln!("  {file}");
                            }
                            if files.len() > 10 {
                                eprintln!("  ... and {} more", files.len() - 10);
                            }
                            eprintln!();
                        }
                    }

                    // Prompt user
                    let proceed = Select::new(
                        "Do you want to proceed with the update?",
                        vec!["No, abort", "Yes, proceed"],
                    )
                    .with_help_message("Uncommitted changes may be overwritten or cause conflicts")
                    .prompt()
                    .map_err(|e| {
                        Box::new(std::io::Error::other(format!("Prompt failed: {e}")))
                            as Box<dyn Error + Send>
                    })?;

                    if proceed == "No, abort" {
                        eprintln!("🚫 Update aborted by user");
                        return Err(Box::new(UserAborted) as Box<dyn Error + Send>);
                    }

                    eprintln!("✅ Proceeding with update...");
                    eprintln!();
                }
                Ok(false) => {
                    // Git is clean, no action needed
                }
                Err(GitError::NotAGitRepository) => {
                    // Not a git repo - warn and continue
                    eprintln!("ℹ️  Note: Not a git repository, skipping dirty check");
                    eprintln!();
                }
                Err(GitError::GitNotInstalled) => {
                    // Git not installed - warn and continue
                    eprintln!("⚠️  Warning: Git not found, skipping dirty check");
                    eprintln!();
                }
                Err(e) => {
                    // Other git error - warn and continue
                    eprintln!(
                        "⚠️  Warning: Could not check git status: {}",
                        format_git_error(&e)
                    );
                    eprintln!();
                }
            }
        } else {
            // Force mode - skip check but inform user
            eprintln!("ℹ️  Force mode enabled - skipping git dirty check");
            eprintln!();
        }
        // === GIT DIRTY CHECK ENDS HERE ===

        // Create the composition operator (clone coord_client since we also need it for batch_process)
        let mut composition_operator = OperatorFactory::create_composition_operator(
            session_id_generator,
            coord_client.clone(),
            registry_client.clone(),
            debug,
            cache_config,
            headless,
        );

        // PHASE 1: BUILD SPEC LISTS
        crate::hprogress!(
            headless,
            "🔍 PHASE 1: Reading template state from: {:?}",
            target_dir.join(".cyan_state.yaml")
        );
        let state_file_path = target_dir.join(".cyan_state.yaml");
        let state_manager = DefaultStateManager::new();
        let mut cyan_state = state_manager
            .load_state_file(&state_file_path)
            .map_err(|e| {
                Box::new(std::io::Error::other(format!("Failed to load state: {e}")))
                    as Box<dyn Error + Send>
            })?;

        if cyan_state.templates.is_empty() {
            crate::hprogress!(headless, "⚠️ No templates found in state file");
            return Ok(CyanRunResult::completed(Vec::new()));
        }

        // Create the manager for composable spec operations
        let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

        // Build prev_specs from state
        let mut prev_specs = manager.get(&cyan_state);

        if prev_specs.is_empty() {
            crate::hprogress!(headless, "⚠️ No active templates to update");
            return Ok(CyanRunResult::completed(Vec::new()));
        }

        crate::hprogress!(headless, "📋 Found {} active templates", prev_specs.len());

        // Build curr_specs for update (with version upgrades)
        let mut curr_specs = manager.update(prev_specs.clone(), interactive)?;

        // Sort both lists by installation time for consistent LWW ordering
        sort_specs(&mut prev_specs);
        sort_specs(&mut curr_specs);

        // A spec is "upgraded" when its version changed vs the previous state (or it is
        // new). Compute this BEFORE seeding so headless answers can be scoped to exactly
        // those templates — the version comparison does not depend on answers.
        let is_upgraded = |c: &TemplateSpec| {
            prev_specs
                .iter()
                .find(|p| p.key() == c.key())
                .map(|p| p.version != c.version)
                .unwrap_or(true) // New template
        };

        // Headless: seed supplied answers ONLY into the template(s) actually being
        // upgraded — never into pre-existing, already-installed templates kept at their
        // current version. A flat answer map has no per-template scoping, so seeding it
        // into every curr_spec would let an answer intended for the upgraded template
        // silently satisfy an unrelated installed template's same-named question,
        // skipping its expected `need_input` and generating with the wrong value.
        // Auto-latest version selection above is unaffected.
        if headless && !headless_answers.is_empty() {
            for spec in curr_specs.iter_mut().filter(|c| is_upgraded(c)) {
                for (k, v) in &headless_answers {
                    spec.answers.insert(k.clone(), v.clone());
                }
            }
        }

        // Find upgraded by comparing versions
        let upgraded: Vec<TemplateSpec> = curr_specs
            .iter()
            .filter(|c| is_upgraded(c))
            .cloned()
            .collect();

        crate::hprogress!(
            headless,
            "📊 Template classification: {} total, {} being upgraded",
            curr_specs.len(),
            upgraded.len()
        );

        // Convert to references for batch_process
        let upgraded_refs: Vec<&TemplateSpec> = upgraded.iter().collect();

        // PHASE 2-4: BATCH PROCESS
        let (session_ids, file_conflicts, commands, managed_by_template, need_input) =
            batch_process(
                &prev_specs,
                &curr_specs,
                &upgraded_refs,
                target_dir,
                &registry_client,
                &coord_client,
                &mut composition_operator,
                headless,
            )?;

        // Headless: a question is pending — surface it without writing state/files. The
        // sessions go to the caller, which cleans them at the headless boundary.
        if let Some(question) = need_input {
            return Ok(CyanRunResult {
                session_ids,
                need_input: Some(question),
            });
        }

        // The coordinator sessions were created during `batch_process` above, but several
        // fallible steps remain (state save, post-template commands) BEFORE the ids reach
        // `finish_headless`. A `?`/early `return Err` from any of them would drop the ids
        // and leak the sessions until the coordinator's own timeout. Hand them to a cleanup
        // guard so every error path below releases them; `take()` disarms it on the happy
        // `done` return (where `finish_headless` then cleans normally).
        let mut session_guard =
            SessionCleanupGuard::new(|sid: &str| release_session(&coord_client, sid), session_ids);

        // One-line cache summary (always printed when caching is enabled). (FR15)
        composition_operator.print_cache_summary();

        // Persist file conflicts to state file (always update to clear stale entries)
        let conflicts_count = file_conflicts.len();
        cyan_state.file_conflicts = file_conflicts;
        // Recompute the managed-files manifest wholesale from this run's active templates.
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
            // Each error arm below returns `Err` while `session_guard` is still armed, so
            // its `Drop` releases the coordinator sessions — no per-arm cleanup closure
            // needed (previously these paths leaked or duplicated a cleanup loop).
            let exec_result =
                CommandExecutor::execute_commands_for_mode(&commands, target_dir, headless)?;
            if exec_result.aborted {
                return Err(Box::new(std::io::Error::other(format!(
                    "Command execution aborted: {}/{} succeeded, {}/{} failed before abort",
                    exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
                ))));
            }
            if headless && !exec_result.all_succeeded() {
                // The non-interactive path runs every command and records failures
                // in the result but returns Ok — it never sets `aborted`. Without this
                // check a failed post-template command (e.g. one exiting non-zero) would
                // be silently ignored and the update would report `done` / exit 0. In
                // headless mode there is no interactive "continue?" prompt to surface the
                // failure, so treat any partial failure as an error → error envelope /
                // exit 1 (the still-armed `session_guard` releases the sessions on return).
                // Interactive mode keeps its existing behavior (the user already chose
                // whether to continue).
                return Err(Box::new(std::io::Error::other(format!(
                    "Command execution failed: {}/{} succeeded, {}/{} failed",
                    exec_result.succeeded, exec_result.total, exec_result.failed, exec_result.total
                ))));
            }
        }

        crate::hprogress!(headless, "✅ Batch update complete");
        // `done`: disarm the guard and hand the sessions to `finish_headless`.
        Ok(CyanRunResult::completed(session_guard.take()))
    }
}

/// Format git error for display
fn format_git_error(err: &GitError) -> String {
    match err {
        GitError::NotAGitRepository => "Not a git repository".to_string(),
        GitError::GitNotInstalled => "Git not installed".to_string(),
        GitError::CommandFailed(msg) => format!("Git command failed: {msg}"),
        GitError::IoError(e) => format!("IO error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_git_error() {
        assert_eq!(
            format_git_error(&GitError::NotAGitRepository),
            "Not a git repository"
        );
        assert_eq!(
            format_git_error(&GitError::GitNotInstalled),
            "Git not installed"
        );
        assert_eq!(
            format_git_error(&GitError::CommandFailed("test error".to_string())),
            "Git command failed: test error"
        );
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        assert_eq!(
            format_git_error(&GitError::IoError(io_err)),
            "IO error: test"
        );
    }
}
