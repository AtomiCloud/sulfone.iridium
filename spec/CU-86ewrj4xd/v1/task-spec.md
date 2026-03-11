# Task Specification: CU-86ewrj4xd

## Title

Prompt user to proceed if git is dirty during cyanprint update command

## Ticket

- **ID**: CU-86ewrj4xd
- **Status**: todo
- **Assignee**: Adelphi Liong
- **URL**: https://app.clickup.com/t/86ewrj4xd

## Context

The `cyanprint update` command updates template files in-place. When users have uncommitted changes in their working directory, running this command could lead to data loss or merge conflicts. Currently, the command does not check for uncommitted git changes before proceeding.

## Problem

Users may accidentally overwrite or complicate their uncommitted changes when running `cyanprint update`. The command should detect when git has uncommitted changes and prompt the user to confirm they want to proceed.

## Requirements

### Functional Requirements

1. **Git Dirty Check**: Before the update process begins, check if the working directory has uncommitted git changes.

2. **User Prompt**: When uncommitted changes are detected:

   - Display a clear warning message
   - Prompt user to confirm whether to proceed or abort
   - Default behavior: abort if user declines
   - Show the uncommitted files to help user understand the risk

3. **Bypass Option**: Provide a `--force` flag to skip the git check for advanced users who understand the risk.

4. **Error Handling**:

   - If git check fails (e.g., not a git repo, git not installed), log a warning and continue (don't block the update)
   - Handle git command errors gracefully

5. **Exit Behavior**: If user aborts, exit cleanly with status code 0 and a clear message.

### Non-Functional Requirements

1. **Performance**: Git check should be fast (< 1 second for typical repos)

2. **User Experience**:

   - Clear, actionable messages
   - Consistent with existing `cyanprint` CLI patterns
   - Use emoji/icons consistent with existing output style

3. **Code Quality**:
   - Follow existing Rust patterns in the codebase
   - Use existing dependencies (inquire) where appropriate
   - Modular design with separate git utility module

### Out of Scope

- Checking for staged vs unstaged changes (all uncommitted changes treated equally)
- Checking for untracked files (unless they're already shown by `git status --porcelain`)
- Auto-stashing changes
- Integration with CI/CD workflows

## Implementation Notes

### Location

The git dirty check should be added in `UpdateOrchestrator::update_templates()` in `cyanprint/src/update/orchestrator.rs`, right after the target_dir is created but before reading the state file (around line 40-41).

### Technical Approach

1. **Create new module**: `cyanprint/src/git.rs` with:

   - `is_git_dirty(path: &Path) -> Result<bool, GitError>`
   - `prompt_user_to_proceed() -> Result<bool, io::Error>`

2. **Git check command**: Use `git status --porcelain` to detect uncommitted changes

3. **User prompt**: Use `inquire` crate (already a dependency) with `Select` for yes/no confirmation

4. **Flag integration**: Add `--force` option to `Commands::Update` struct in `cyanprint/src/commands.rs`

### Existing Patterns to Follow

- Error handling: Use `Box<dyn Error + Send + Sync>` pattern
- External commands: Use `std::process::Command` (see `cyanprint/src/docker/buildx.rs`)
- User prompts: Use `inquire` crate (see `cyanprint/src/update/version_manager.rs`)

## Success Criteria

1. Running `cyanprint update` in a dirty repo shows warning and prompts user
2. User can accept to proceed or abort
3. Running `cyanprint update --force` bypasses the check
4. Running `cyanprint update` in a clean repo proceeds without prompt
5. Errors during git check are handled gracefully (warning + continue)
6. Clear exit message when user aborts

## Test Plan

1. Unit tests for `is_git_dirty()` function
2. Unit tests for prompt logic (mockable)
3. Manual E2E testing:
   - Clean repo: no prompt
   - Dirty repo: prompt appears
   - Dirty repo with `--force`: bypasses check
   - Non-git directory: warns and continues
4. Add integration test if feasible
