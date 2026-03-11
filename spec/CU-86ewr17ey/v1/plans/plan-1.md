# Plan 1: Implement File Deletion During Upgrades

## Overview

Single plan — add cleanup logic to the VFS write layer and wire it into all call sites.

## Steps

### Step 1: Add `cleanup` to `FileWriter` trait and `DiskFileWriter`

**File:** `cyancoordinator/src/fs/traits.rs`

- Add `cleanup(&self, target_dir: &Path, files_to_delete: &[PathBuf]) -> Result<(), Box<dyn Error + Send>>` to `FileWriter` trait

**File:** `cyancoordinator/src/fs/writer.rs`

- Implement `cleanup` on `DiskFileWriter`:
  - For each path in `files_to_delete`, join with `target_dir` and `std::fs::remove_file`
  - Collect parent directories, sort by depth (deepest first), remove empty ones with `std::fs::remove_dir` (which only removes empty dirs)
  - Log deleted files in debug mode

### Step 2: Add `cleanup_deleted_files` to `Vfs` trait and `DefaultVfs`

**File:** `cyancoordinator/src/fs/mod.rs`

- Add to `Vfs` trait:
  ```rust
  fn cleanup_deleted_files(
      &self,
      target_dir: &Path,
      local_vfs: &VirtualFileSystem,
      merged_vfs: &VirtualFileSystem,
  ) -> Result<Vec<PathBuf>, Box<dyn Error + Send>>;
  ```
- Implement on `DefaultVfs`:
  - Compute `local_vfs.get_paths()` minus `merged_vfs.get_paths()` (set difference using HashSet)
  - Call `self.writer.cleanup(target_dir, &files_to_delete)`
  - Return deleted paths

### Step 3: Wire into `TemplateOperator` call sites

**File:** `cyancoordinator/src/operations/mod.rs`

- In `upgrade()` (after line 270): add `self.vfs.cleanup_deleted_files(target_dir, &local_vfs, &merged_vfs)?;`
- In `rerun()` (after line 196): same
- In `create_new()` (after line 128): same

### Step 4: Wire into `CompositionOperator`

**File:** `cyancoordinator/src/operations/composition/operator.rs`

- Add `cleanup_deleted_files` method that delegates to `self.template_operator.vfs.cleanup_deleted_files()`

### Step 5: Wire into `batch_process` (composition flow)

**File:** `cyanprint/src/run.rs`

- After `operator.write_to_disk(target_dir, &merged_vfs)?` (line 139), add cleanup call using the `local_vfs` already available at line 136

### Step 6: Add tests

**File:** `cyancoordinator/src/fs/writer.rs` (or new test module)

- Test cleanup deletes specified files
- Test cleanup removes empty parent directories
- Test cleanup ignores non-existent files gracefully

**File:** Integration-level tests if existing test infrastructure supports it

- Test full merge+write+cleanup flow with deletion scenario

## Files Modified

1. `cyancoordinator/src/fs/traits.rs` — add `cleanup` to `FileWriter`
2. `cyancoordinator/src/fs/writer.rs` — implement `cleanup` on `DiskFileWriter`
3. `cyancoordinator/src/fs/mod.rs` — add `cleanup_deleted_files` to `Vfs` trait + `DefaultVfs`
4. `cyancoordinator/src/operations/mod.rs` — wire cleanup into `TemplateOperator`
5. `cyancoordinator/src/operations/composition/operator.rs` — expose cleanup on `CompositionOperator`
6. `cyanprint/src/run.rs` — wire cleanup into `batch_process`
