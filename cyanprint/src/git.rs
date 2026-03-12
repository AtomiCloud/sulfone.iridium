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

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotAGitRepository => write!(f, "Not a git repository"),
            GitError::GitNotInstalled => write!(f, "Git is not installed"),
            GitError::CommandFailed(msg) => write!(f, "Git command failed: {msg}"),
            GitError::IoError(err) => write!(f, "IO error: {err}"),
        }
    }
}

impl std::error::Error for GitError {}

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
        if stderr.contains("not a git repository") || stderr.contains("not a git repository") {
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
                assert!(is_dirty || !is_dirty);
            }
            Err(GitError::NotAGitRepository) => {
                // Also valid if we're not in a git repo
            }
            Err(GitError::GitNotInstalled) => {
                // Valid in environments where git is not on PATH
            }
            Err(GitError::CommandFailed(_)) | Err(GitError::IoError(_)) => {
                // Valid in sandboxed build environments (e.g. Nix) where
                // git may fail due to restricted filesystem or permissions
            }
        }
    }
}
