use git2::{self, Oid, Repository};
use std::error::Error;
use std::fmt;
use std::path::Path;
use tempfile::tempdir;
use walkdir::WalkDir;

use super::VirtualFileSystem;
use super::traits::FileMerger;

/// Error types for the GitLikeMerger
#[derive(Debug)]
enum MergeError {
    GitError(git2::Error),
    IoError(std::io::Error),
    Other(String),
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::GitError(e) => write!(f, "Git error: {e}"),
            MergeError::IoError(e) => write!(f, "IO error: {e}"),
            MergeError::Other(s) => write!(f, "Error: {s}"),
        }
    }
}

impl Error for MergeError {}

impl From<git2::Error> for MergeError {
    fn from(err: git2::Error) -> Self {
        MergeError::GitError(err)
    }
}

impl From<std::io::Error> for MergeError {
    fn from(err: std::io::Error) -> Self {
        MergeError::IoError(err)
    }
}

impl From<String> for MergeError {
    fn from(err: String) -> Self {
        MergeError::Other(err)
    }
}

/// GitLikeMerger implementation using the git2 library for Git-like merges
pub struct GitLikeMerger {
    debug: bool,
    similarity_threshold: u32, // Threshold percentage for rename detection (0-100)
}

impl GitLikeMerger {
    pub fn new(debug: bool, similarity_threshold: u16) -> Self {
        Self {
            debug,
            similarity_threshold: similarity_threshold.min(100) as u32,
        }
    }

    // Create a temporary git repository and return the repository and its directory
    fn create_temp_repo(
        &self,
        vfs: &VirtualFileSystem,
    ) -> Result<(Repository, tempfile::TempDir), MergeError> {
        // Create a temporary directory for the repo
        let temp_dir = tempdir().map_err(MergeError::IoError)?;

        if self.debug {
            println!("🏗️ Created temp dir at: {}", temp_dir.path().display());
        }

        // Initialize a git repository in the temp directory
        let repo = Repository::init(temp_dir.path())?;

        // Write the VFS files to the temp directory
        for (path, content) in &vfs.files {
            let full_path = temp_dir.path().join(path);

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write file content
            std::fs::write(&full_path, content)?;
        }

        Ok((repo, temp_dir))
    }

    // Add all files to the repository index and create a commit
    fn commit_all(&self, repo: &Repository, message: &str) -> Result<Oid, MergeError> {
        // Add all files to the repository index
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        // Create a commit from the index
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let signature = git2::Signature::now("Merger", "merger@example.com")?;

        // Get HEAD or create initial commit
        let parents = if let Ok(head) = repo.head() {
            let parent = head.peel_to_commit()?;
            vec![parent]
        } else {
            vec![]
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )?;

        if self.debug {
            println!("📝 Created commit: {commit_id}");
        }

        Ok(commit_id)
    }

    // Create branches for each VFS and perform a 3-way merge
    fn perform_git_merge(
        &self,
        base: &VirtualFileSystem,
        current: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, MergeError> {
        if self.debug {
            println!("🔄 Starting Git folder-level 3-way merge");
        }

        // Create temporary repository with base VFS
        let (repo, temp_dir) = self.create_temp_repo(base)?;

        // Commit the base state
        let base_commit = self.commit_all(&repo, "Base state")?;

        // Create branches for current and incoming
        repo.branch("current", &repo.find_commit(base_commit)?, false)?;
        repo.branch("incoming", &repo.find_commit(base_commit)?, false)?;

        if self.debug {
            println!("🌿 Created branches: current and incoming from base {base_commit}");
        }

        // Checkout current branch and apply current VFS
        let current_branch = repo.find_branch("current", git2::BranchType::Local)?;
        repo.set_head(current_branch.get().name().unwrap())?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;

        // Clear the working directory and apply current VFS
        for entry in repo.index()?.iter() {
            let path = std::str::from_utf8(&entry.path).unwrap();
            let full_path = temp_dir.path().join(path);
            if full_path.exists() {
                std::fs::remove_file(full_path)?;
            }
        }

        // Write current VFS to the working directory
        for (path, content) in &current.files {
            let full_path = temp_dir.path().join(path);

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write file content
            std::fs::write(&full_path, content)?;
        }

        // Commit current state
        let current_commit = self.commit_all(&repo, "Current state")?;

        // Checkout incoming branch and apply incoming VFS
        let incoming_branch = repo.find_branch("incoming", git2::BranchType::Local)?;
        repo.set_head(incoming_branch.get().name().unwrap())?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;

        // Clear the working directory and apply incoming VFS
        for entry in repo.index()?.iter() {
            let path = std::str::from_utf8(&entry.path).unwrap();
            let full_path = temp_dir.path().join(path);
            if full_path.exists() {
                std::fs::remove_file(full_path)?;
            }
        }

        // Write incoming VFS to the working directory
        for (path, content) in &incoming.files {
            let full_path = temp_dir.path().join(path);

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write file content
            std::fs::write(&full_path, content)?;
        }

        // Commit incoming state
        let incoming_commit = self.commit_all(&repo, "Incoming state")?;

        if self.debug {
            println!("📊 Committed states: current={current_commit}, incoming={incoming_commit}");
        }

        // Checkout current branch for the merge
        repo.set_head(current_branch.get().name().unwrap())?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;

        // Get the annotated commits
        let _current_annotated = repo.find_annotated_commit(current_commit)?;
        let incoming_annotated = repo.find_annotated_commit(incoming_commit)?;

        // Set up merge options with find_renames enabled
        let mut merge_opts = git2::MergeOptions::new();
        merge_opts.find_renames(true);
        merge_opts.rename_threshold(self.similarity_threshold);

        // Set up checkout options
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();

        // Perform the merge analysis
        let analysis = repo.merge_analysis(&[&incoming_annotated])?;

        if analysis.0.is_up_to_date() {
            if self.debug {
                println!("✅ Merge analysis: up-to-date, no merge needed");
            }
            // No changes needed, return current VFS
            let result = current.clone();
            Ok(result)
        } else if analysis.0.is_normal() {
            if self.debug {
                println!("🔄 Merge analysis: normal merge required");
            }

            // Perform the merge
            repo.merge(
                &[&incoming_annotated],
                Some(&mut merge_opts),
                Some(&mut checkout_opts),
            )?;

            // Check if we have conflicts
            if repo.index()?.has_conflicts() {
                if self.debug {
                    println!("⚠️ Merge resulted in conflicts");
                }

                // In case of conflicts, we keep the conflicts in the working directory
            } else {
                if self.debug {
                    println!("✅ Merge successful, committing");
                }

                // No conflicts, commit the merge
                let tree_id = repo.index()?.write_tree()?;
                let tree = repo.find_tree(tree_id)?;

                let current_commit_obj = repo.find_commit(current_commit)?;
                let incoming_commit_obj = repo.find_commit(incoming_commit)?;

                let signature = git2::Signature::now("Merger", "merger@example.com")?;

                repo.commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    "Merge result",
                    &tree,
                    &[&current_commit_obj, &incoming_commit_obj],
                )?;
            }

            // Create a VFS from the result in the working directory
            let result_vfs = self.read_vfs_from_dir(temp_dir.path())?;
            Ok(result_vfs)
        } else {
            Err(MergeError::Other(
                "Unable to perform a normal merge".to_string(),
            ))
        }
    }

    // Read a VirtualFileSystem from a directory
    fn read_vfs_from_dir(&self, dir_path: &Path) -> Result<VirtualFileSystem, MergeError> {
        let mut vfs = VirtualFileSystem::new();
        let repo_root = dir_path.to_path_buf();

        // Walk the directory recursively
        let walker = WalkDir::new(&repo_root)
            .min_depth(1) // Skip the root directory itself
            .into_iter()
            .filter_entry(|e| {
                // Skip .git directory
                !e.path().components().any(|c| c.as_os_str() == ".git")
            });

        for entry in walker.filter_map(Result::ok) {
            let path = entry.path();

            // Only process files, not directories
            if path.is_file() {
                let relative_path = path.strip_prefix(&repo_root).unwrap();
                let content = std::fs::read(path)?;

                vfs.add_file(relative_path.to_path_buf(), content);

                if self.debug {
                    println!("📄 Added to result VFS: {}", relative_path.display());
                }
            }
        }

        Ok(vfs)
    }
}

impl FileMerger for GitLikeMerger {
    fn merge(
        &self,
        base: &VirtualFileSystem,
        current: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.perform_git_merge(base, current, incoming)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }
}
