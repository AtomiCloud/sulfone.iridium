# Plan 2: Command Execution — Collect, Approve, Execute post-composition

## Goal

After template composition completes and files are written to disk, collect commands from all templates in the dependency tree, present them to the user for approval, and execute them sequentially in the output directory.

## Scope

- cyancoordinator crate: command collection + execution module
- cyanprint crate: integration into create/update flow
- Depends on plan 1 (needs `commands` on `TemplateVersionRes`)

## Files to Modify

### 1. New: cyancoordinator/src/operations/composition/command_executor.rs

Create a `CommandExecutor` struct responsible for:

**Approval prompt**: Display all commands in a numbered list and ask user to confirm execution. Use `inquire::Confirm` (already a dependency via cyanprompt). Show the count: "N commands will be executed:"

**Sequential execution**: For each command:

- Spawn a shell process with `std::process::Command::new("sh").arg("-c").arg(&cmd)`
- Set working directory to the output directory
- Inherit stdin/stdout/stderr from the parent process
- Wait for completion, check exit status

**Failure handling**: On non-zero exit:

- Print the error (exit code)
- Prompt user with `inquire::Confirm`: "Command failed. Continue executing remaining commands?"
- If yes: continue to next command
- If no: stop and return error with summary

**Return type**: `CommandExecutionResult` enum or struct with:

- Total commands count
- Succeeded count
- Failed count (with which ones)
- Aborted flag

**Public API**:

```rust
pub fn execute_commands(
    commands: &[String],
    working_dir: &Path,
) -> Result<CommandExecutionResult, Box<dyn Error + Send>>
```

### 2. cyancoordinator/src/operations/composition/mod.rs

Export the new `command_executor` module.

### 3. cyancoordinator/src/operations/composition/operator.rs

Add a method to collect commands from resolved dependencies:

```rust
pub fn collect_commands(dependencies: &[ResolvedDependency]) -> Vec<String>
```

Iterate over dependencies (already in post-order), collect non-empty commands from each `template.commands`, flatten into a single vec maintaining order.

### 4. cyancoordinator/src/operations/mod.rs

Expose `collect_commands` through the operations module.

### 5. cyanprint/src/run.rs

**In `batch_process()`**: The function already has access to `curr_template_res_list` and `prev_template_res_list`. After phase 4 (write to disk), collect commands from both lists and return them alongside session IDs.

Change return type to include `Vec<String>` (collected commands):

```rust
pub fn batch_process(...) -> Result<(Vec<String>, Vec<FileConflictEntry>, Vec<String>), ...>
//                                                   ^session    ^conflicts    ^commands
```

Collect commands by iterating over `curr_template_res_list` and `prev_template_res_list`, concatenating their `commands` fields. Deduplicate isn't needed — if the same template appears in both prev and curr, its commands run twice (which is correct for upgrade scenarios).

**In `cyan_run()`**: After `batch_process()` returns, if commands are non-empty, call `CommandExecutor::execute_commands(&commands, target_dir)`.

**In `try_cmd.rs`**: After `execute_template()` returns and VFS is written, collect commands from the synthetic template's `commands` field and execute via `CommandExecutor`.

### 6. cyancoordinator/Cargo.toml

Add `inquire` dependency if not already present (check first — cyanprint already has it, but cyancoordinator may not). Alternatively, keep the executor in cyanprint only since it's a CLI concern.

**Design decision**: The executor uses `inquire` for user prompts. If cyancoordinator should remain free of direct user interaction, the `CommandExecutor` should live in cyanprint and only the `collect_commands` helper lives in cyancoordinator. This matches the existing pattern where cyancoordinator is a library and cyanprint is the CLI.

**Revised approach**:

- `collect_commands()` helper in cyancoordinator (pure function, no I/O)
- `CommandExecutor` in cyanprint (uses `inquire`, spawns processes)

## Testing Strategy

- Unit test `collect_commands()`:

  - Empty dependency list → empty vec
  - Single template with commands → commands collected
  - Multiple templates with commands → ordered by post-order
  - Templates with no commands field (backward compat) → skipped gracefully
  - Mix of empty and non-empty → only non-empty collected

- Unit test `CommandExecutor`:

  - Empty command list → return immediately, no prompt
  - Single command success → verify executed
  - Multiple commands success → verify all executed in order
  - Command failure + user continues → verify next commands execute
  - Command failure + user aborts → verify stops, summary correct

- Integration: `batch_process()` returns commands correctly

## Implementation Checklist

- [ ] Add `collect_commands()` to `cyancoordinator/src/operations/composition/operator.rs`
- [ ] Export through `mod.rs` chain
- [ ] Create `CommandExecutor` in `cyanprint` (or cyancoordinator if `inquire` is available)
- [ ] Update `batch_process()` return type to include commands
- [ ] Integrate command execution in `cyan_run()` after write to disk
- [ ] Integrate command execution in `try_cmd.rs` after write to disk
- [ ] Add unit tests for `collect_commands()`
- [ ] Add unit tests for `CommandExecutor`
- [ ] Verify existing tests pass
