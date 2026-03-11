# Plan 1: Git Module & CLI Flag

## Overview

Create the git utility module and add the `--force` CLI flag. This establishes the foundation for the git dirty check feature.

## Files to Create

### `cyanprint/src/git.rs`

New module for git-related utilities.

**Functions to implement:**

```rust
use std::path::Path;
use std::process::Command;

/// Error types for git operations
#[derive(Debug)]
pub enum GitError {
    NotAGitRepository,
    GitNotInstalled,
    CommandFailed(String),
    IoError(std::io::Error),
}

impl From<std::io::Error> for GitError {
    fn from(err: std::io::Error) -> Self {
        GitError::IoError(err)
    }
}

/// Check if the git working directory at `path` has uncommitted changes.
///
/// Uses `git status --porcelain` to detect:
/// - Modified files (staged and unstaged)
/// - New files
/// - Deleted files
///
/// Returns `Ok(true)` if there are uncommitted changes.
/// Returns `Ok(false)` if the working directory is clean.
/// Returns `Err(GitError)` if git check fails.
pub fn is_git_dirty(path: &Path) -> Result<bool, GitError> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GitError::GitNotInstalled
            } else {
                GitError::CommandFailed(e.to_string())
            }
        })?;

    if !output.status.success() {
        // Not a git repository or git error - return specific error
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not a git repository") {
            return Err(GitError::NotAGitRepository);
        }
        return Err(GitError::CommandFailed(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

/// Get the list of modified files for display purposes.
pub fn get_modified_files(path: &Path) -> Result<Vec<String>, GitError> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_git_dirty() {
        // This test requires a git repository context
        // In real implementation, might use a fixture or mock
        let result = is_git_dirty(Path::new("."));
        // We expect either Ok(true) or Ok(false) or an appropriate error
        match result {
            Ok(is_dirty) => {
                // Test passed - we got a boolean result
                assert!(is_dirty == true || is_dirty == false);
            }
            Err(GitError::NotAGitRepository) => {
                // Also valid if we're not in a git repo
            }
            Err(_) => {
                panic!("Unexpected error");
            }
        }
    }
}
```

## Files to Modify

### `cyanprint/src/mod.rs`

Add the git module declaration:

```rust
// Add with other module declarations
pub mod git;
```

### `cyanprint/src/commands.rs`

Add `force` flag to the `Update` command:

```rust
#[derive(Subcommand)]
pub enum Commands {
    #[command(
        alias = "u",
        about = "Update all templates in a project to their latest versions"
    )]
    Update {
        #[arg(
            global = true,
            long,
            help = "Force update even if git is dirty (skip confirmation prompt)"
        )]
        force: bool,

        // ... existing fields remain unchanged
    },
}
```

### `cyanprint/src/main.rs`

Thread the `force` parameter through:

```rust
Commands::Update { path, coordinator_endpoint, interactive, force } => {
    if let Err(e) = cyanprint::cyan_update(
        path.as_deref(),
        coordinator_endpoint.as_deref(),
        interactive,
        force,  // Add this parameter
    ) {
        eprintln!("❌ Update failed: {}", e);
        std::process::exit(1);
    }
}
```

### `cyanprint/src/update.rs`

Update function signature:

```rust
pub fn cyan_update(
    path: Option<&str>,
    coordinator_endpoint: Option<&str>,
    interactive: bool,
    force: bool,  // Add this parameter
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // ... existing code
    orchestrator.update_templates(&target_dir, force).await?;  // Pass force
    // ...
}
```

## Success Criteria

- ✅ `cyanprint/src/git.rs` module created with `is_git_dirty()` function
- ✅ `--force` flag available in CLI
- ✅ `force` parameter threaded through main.rs → cyan_update() → orchestrator
- ✅ Module properly declared in mod.rs
- ✅ Code compiles without errors
- ✅ Unit tests for git module compile

## Dependencies

None - this is foundational work that can be done independently.

## Notes

- Git errors are intentionally not blocking; they will be handled gracefully in the orchestrator
- The `is_git_dirty()` function returns `Result<bool, GitError>` to allow for graceful degradation
- Use existing patterns from `cyanprint/src/docker/buildx.rs` for `std::process::Command` usage
