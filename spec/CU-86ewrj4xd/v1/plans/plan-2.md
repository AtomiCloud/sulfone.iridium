# Plan 2: Orchestrator Integration & User Prompt

## Overview

Integrate the git dirty check into the update orchestrator and add the user prompt logic. This completes the feature implementation.

## Files to Modify

### `cyanprint/src/update/orchestrator.rs`

Integrate the git dirty check and user prompt at the start of `update_templates()`.

**Changes to add:**

```rust
use super::git::{is_git_dirty, get_modified_files, GitError};
use inquire::Select;
use std::path::Path;

impl UpdateOrchestrator {
    pub async fn update_templates(&self, target_dir: &Path, force: bool) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        // ... existing setup code (target_dir creation, etc.)

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
                            for file in files.iter().take(10) {  // Limit to 10 files
                                eprintln!("  {}", file);
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
                        vec!["No, abort", "Yes, proceed"]
                    )
                    .with_help_message("Uncommitted changes may be overwritten or cause conflicts")
                    .prompt()
                    .map_err(|e| format!("Prompt failed: {}", e))?;

                    if proceed == "No, abort" {
                        eprintln!("🚫 Update aborted by user");
                        return Ok(Vec::new());
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
                    eprintln!("⚠️  Warning: Could not check git status: {}", format_git_error(&e));
                    eprintln!();
                }
            }
        } else {
            // Force mode - skip check but inform user
            eprintln!("ℹ️  Force mode enabled - skipping git dirty check");
            eprintln!();
        }
        // === GIT DIRTY CHECK ENDS HERE ===

        // ... rest of existing code (PHASE 1, PHASE 2-4, etc.)

        // ... existing code ...
    }
}

fn format_git_error(err: &GitError) -> String {
    match err {
        GitError::NotAGitRepository => "Not a git repository".to_string(),
        GitError::GitNotInstalled => "Git not installed".to_string(),
        GitError::CommandFailed(msg) => format!("Git command failed: {}", msg),
        GitError::IoError(e) => format!("IO error: {}", e),
    }
}
```

**Update function signature:**

```rust
// Change from:
pub async fn update_templates(&self, target_dir: &Path) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {

// To:
pub async fn update_templates(&self, target_dir: &Path, force: bool) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
```

### `cyanprint/src/update/spec.rs`

Update `TemplateSpecManager::update()` to accept `force` parameter if needed (may not require changes depending on implementation details).

## Files to Update (Dependencies)

### `cyanprint/src/Cargo.toml`

Ensure `inquire` dependency is available (it should already be there):

```toml
[dependencies]
inquire = "0.7.5"
```

## Testing

### Unit Tests

Add to `cyanprint/src/update/orchestrator.rs`:

```rust
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
    }
}
```

### Manual E2E Testing Checklist

Test in a clean git repo:

```bash
# Setup
git init
git add .
git commit -m "clean"

# Run
cyanprint update

# Expected: No prompt, proceeds normally
```

Test in a dirty git repo:

```bash
# Setup
echo "changed" > some_file

# Run
cyanprint update

# Expected: Shows warning, lists modified files, prompts user
# Selecting "No" exits with message "🚫 Update aborted by user"
# Selecting "Yes" proceeds with update
```

Test with `--force` flag:

```bash
# Setup (dirty repo)
echo "changed" > some_file

# Run
cyanprint update --force

# Expected: "ℹ️  Force mode enabled - skipping git dirty check", proceeds without prompt
```

Test outside a git repo:

```bash
# Setup in non-git directory
mkdir /tmp/test_cyanprint
cd /tmp/test_cyanprint

# Run
cyanprint update

# Expected: "ℹ️  Note: Not a git repository, skipping dirty check", proceeds
```

## Success Criteria

- ✅ Git dirty check runs at start of update (when not in force mode)
- ✅ User is warned and prompted when git is dirty
- ✅ Modified files are displayed (up to 10)
- ✅ User can choose to abort or proceed
- ✅ Abort exits cleanly with clear message
- ✅ `--force` flag bypasses check and prompt
- ✅ Non-git repos show info message and continue
- ✅ Git errors show warning and continue
- ✅ Clean repos proceed without any prompt
- ✅ Code compiles without errors
- ✅ All unit tests pass

## Dependencies

- Depends on **Plan 1** being complete (git module exists, force flag threaded through)

## Notes

- The git check is intentionally non-blocking: if git is unavailable or errors occur, we warn and continue
- Exit code 0 on user abort to distinguish from errors (exit code 1)
- Use emoji consistency: ⚠️ for warnings, 🚫 for abort, ✅ for confirmation, ℹ️ for info
- Follow existing prompt patterns from `version_manager.rs` using `inquire::Select`
- Limit file display to 10 files to avoid overwhelming output
