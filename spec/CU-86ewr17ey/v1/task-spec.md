# Task Spec: Fix File Deletion During Upgrades

**Ticket:** CU-86ewr17ey
**Version:** 1

## Problem

During template upgrades (and reruns), the VFS 3-way merge correctly determines which files should be deleted, but the `write_to_disk` step only writes files — it never removes files from disk that are absent from the merged VFS. This causes orphaned files to persist after upgrades.

## Root Cause

`DiskFileWriter::write()` iterates over `merged_vfs.files` and writes each one. It has no concept of deletion. Files that existed on disk before the merge but are absent from the merged result are never cleaned up.

## Verified Safety

Git's 3-way merge semantics ensure safety:

- **Unmodified template files deleted in new version** → correctly absent from merged VFS → should be deleted from disk
- **Locally-modified files deleted in new template** → merge conflict → file kept in merged VFS → NOT deleted (user edits safe)
- **User-created files** (not in base) → kept in merged VFS → NOT deleted

## Solution

Compare the set of files in `local_vfs` (pre-merge disk state) against `merged_vfs` (post-merge result). Files present in `local_vfs` but absent from `merged_vfs` should be deleted from disk. Empty parent directories should be cleaned up afterward.

### Changes

#### 1. `FileWriter` trait (`cyancoordinator/src/fs/traits.rs`)

Add a `cleanup` method:

```rust
fn cleanup(
    &self,
    target_dir: &Path,
    files_to_delete: &[PathBuf],
) -> Result<(), Box<dyn Error + Send>>;
```

#### 2. `DiskFileWriter` (`cyancoordinator/src/fs/writer.rs`)

Implement `cleanup`:

- Delete each file in `files_to_delete` from `target_dir`
- After all deletions, walk up parent directories and remove empty ones (stop at `target_dir`)

#### 3. `Vfs` trait (`cyancoordinator/src/fs/mod.rs`)

Add a `cleanup_deleted_files` method:

```rust
fn cleanup_deleted_files(
    &self,
    target_dir: &Path,
    local_vfs: &VirtualFileSystem,
    merged_vfs: &VirtualFileSystem,
) -> Result<Vec<PathBuf>, Box<dyn Error + Send>>;
```

Computes `local_vfs.paths - merged_vfs.paths` and calls `writer.cleanup()`. Returns deleted paths for logging.

#### 4. Call sites — add cleanup after every `write_to_disk`

**`TemplateOperator`** (`cyancoordinator/src/operations/mod.rs`):

- `upgrade()`: after `write_to_disk`, call `cleanup_deleted_files(target_dir, &local_vfs, &merged_vfs)`
- `rerun()`: same
- `create_new()`: same (for consistency, though unlikely to delete anything)

**`CompositionOperator`** (`cyancoordinator/src/operations/composition/operator.rs`):

- Expose `cleanup_deleted_files` method that delegates to `template_operator.vfs`

**`batch_process` in `cyanprint/src/run.rs`**:

- After `operator.write_to_disk(target_dir, &merged_vfs)`, call `operator.cleanup_deleted_files(target_dir, &local_vfs, &merged_vfs)`

#### 5. Tests

- Test: unmodified file deleted in incoming → removed from disk
- Test: user-modified file deleted in incoming → kept on disk (conflict)
- Test: user-created file → kept on disk
- Test: empty directory cleanup after file deletion

## Out of Scope

- No changes to `VirtualFileSystem` struct (no deletion tracking needed)
- No changes to `GitLikeMerger` (merge logic is already correct)
- No changes to layerers (layering operates on VFS, not disk)
