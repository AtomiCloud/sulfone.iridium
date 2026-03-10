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
use crate::git::{GitError, get_modified_files, is_git_dirty};
use crate::run::batch_process;

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
    pub fn update_templates(
        session_id_generator: Box<dyn SessionIdGenerator>,
        path: String,
        coord_client: CyanCoordinatorClient,
        registry_client: Rc<CyanRegistryClient>,
        debug: bool,
        interactive: bool,
        force: bool,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        let target_dir = Path::new(&path);

        // === GIT DIRTY CHECK STARTS HERE ===
        if !force {
            match is_git_dirty(target_dir) {
                Ok(true) => {
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
        );

        // PHASE 1: BUILD SPEC LISTS
        println!(
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
            println!("⚠️ No templates found in state file");
            return Ok(Vec::new());
        }

        // Create the manager for composable spec operations
        let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

        // Build prev_specs from state
        let mut prev_specs = manager.get(&cyan_state);

        if prev_specs.is_empty() {
            println!("⚠️ No active templates to update");
            return Ok(Vec::new());
        }

        println!("📋 Found {} active templates", prev_specs.len());

        // Build curr_specs for update (with version upgrades)
        let mut curr_specs = manager.update(prev_specs.clone(), interactive)?;

        // Sort both lists by installation time for consistent LWW ordering
        sort_specs(&mut prev_specs);
        sort_specs(&mut curr_specs);

        // Find upgraded by comparing versions
        let upgraded: Vec<TemplateSpec> = curr_specs
            .iter()
            .filter(|c| {
                prev_specs
                    .iter()
                    .find(|p| p.key() == c.key())
                    .map(|p| p.version != c.version)
                    .unwrap_or(true) // New template
            })
            .cloned()
            .collect();

        println!(
            "📊 Template classification: {} total, {} being upgraded",
            curr_specs.len(),
            upgraded.len()
        );

        // Convert to references for batch_process
        let upgraded_refs: Vec<&TemplateSpec> = upgraded.iter().collect();

        // PHASE 2-4: BATCH PROCESS
        let (session_ids, file_conflicts) = batch_process(
            &prev_specs,
            &curr_specs,
            &upgraded_refs,
            target_dir,
            &registry_client,
            &coord_client,
            &mut composition_operator,
        )?;

        // Persist file conflicts to state file (always update to clear stale entries)
        let conflicts_count = file_conflicts.len();
        cyan_state.file_conflicts = file_conflicts;
        state_manager.save_state_file(&cyan_state, &state_file_path)?;
        if conflicts_count > 0 {
            println!("📝 Saved {conflicts_count} file conflict(s) to state");
        }

        println!("✅ Batch update complete");
        Ok(session_ids)
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
