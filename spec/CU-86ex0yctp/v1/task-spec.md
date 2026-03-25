# Task Spec: CU-86ex0yctp — Push command configs + execute on template completion

## Context

Templates scaffold project files. After scaffolding, users often need to run setup commands (e.g., `npm install`, `git init`). Template authors should be able to declare these commands so Iridium can execute them automatically after template composition completes.

**Sibling ticket**: [Zn] Store command configs in registry (Zinc-side changes) — assumed merged before this ticket.

## Scope

Iridium-side only:

1. Parse `commands` array from `cyan.yaml`
2. Include `commands` in template push requests to Zinc
3. Handle `commands` in Zinc response models (backward compatible with older APIs)
4. After composition completes, collect commands from all templates in the dependency tree
5. Present commands to user for one-time approval
6. Execute approved commands sequentially in the output directory with inherited stdio
7. On command failure, ask user whether to continue

## Current State

- `CyanTemplateFileConfig` (cyan.yaml): no `commands` field
- `TemplateReq` (push request): no `commands` field
- `TemplateVersionRes` (Zinc response): no `commands` field
- `ResolvedDependency`: carries `template` + `preset_answers` — no commands
- `CompositionOperator.execute_composition()`: returns `(VFS, CompositionState, Vec<String>)` — no post-execution step
- No command execution infrastructure exists anywhere in the codebase

## Design

### cyan.yaml format (backward compatible)

```yaml
# New top-level field — optional, defaults to empty
commands:
  - 'npm install'
  - 'git init'

# Existing fields unchanged
username: atomi
name: workspace
# ...
```

Old cyan.yaml files without `commands` continue to work — the field defaults to `Vec::new()` via `#[serde(default)]`.

### Data model chain

```
cyan.yaml (CyanTemplateFileConfig.commands: Vec<String>)
  → CyanTemplateConfig (domain).commands
    → TemplateReq (HTTP push request).commands
      → Zinc stores → Zinc returns
        → TemplateVersionRes.commands (with #[serde(default)])
          → Collected during/after composition
            → Presented to user → approved → executed
```

### Command collection strategy

After composition completes, commands are collected from ALL templates in the flattened dependency tree (including the root template). The order follows dependency execution order (post-order: dependencies first, root last).

Commands from a template are appended in the order declared in cyan.yaml.

### Execution flow

1. Composition completes, all files written to output directory
2. Collect commands from all `ResolvedDependency` templates
3. If no commands: skip entire flow
4. Display all commands to user in a numbered list
5. Ask user: "Execute these N commands? (y/n)"
6. On approval, execute commands sequentially:
   - Working directory: output directory (where template files were written)
   - Environment: inherit current process environment
   - stdio: inherit (stdout/stderr pass through to terminal)
   - On success: continue to next command
   - On failure: display error, ask user "Command failed. Continue? (y/n)"
     - Yes: continue to next command
     - No: stop execution, return error
7. Return execution result (all succeeded / some failed / user aborted)

### serde naming

Rust: `commands` | JSON: `commands` (no rename needed — field name is the same in both)

### Backward compatibility

- **Push side**: Old cyan.yaml without `commands` → serde defaults to empty vec → empty array sent to Zinc
- **Fetch side**: Older Zinc API responses without `commands` → serde defaults to empty vec → no commands executed
- Both directions are transparent via `#[serde(default)]`

## Changes

### 1. cyanregistry — Config parsing

**`cyanregistry/src/cli/models/template_config.rs`**

- Add `#[serde(default)] pub commands: Vec<String>` to `CyanTemplateFileConfig`

### 2. cyanregistry — Domain model

**`cyanregistry/src/domain/config/template_config.rs`**

- Add `commands: Vec<String>` to `CyanTemplateConfig`

### 3. cyanregistry — CLI mapper

**`cyanregistry/src/cli/mapper.rs`**

- Map `commands` from `CyanTemplateFileConfig` to `CyanTemplateConfig`

### 4. cyanregistry — HTTP request model

**`cyanregistry/src/http/models/template_req.rs`**

- Add `#[serde(default)] pub commands: Vec<String>` to `TemplateReq`

### 5. cyanregistry — HTTP response model

**`cyanregistry/src/http/models/template_res.rs`**

- Add `#[serde(default)] pub commands: Vec<String>` to `TemplateVersionRes`

### 6. cyanregistry — HTTP mapper

**`cyanregistry/src/http/mapper.rs`**

- Update `template_req_mapper` to copy `commands` from `CyanTemplateConfig` to `TemplateReq`
- Update existing tests, add tests for commands field

### 7. cyancoordinator — Command executor module

**New: `cyancoordinator/src/operations/composition/command_executor.rs`**

- `CommandExecutor` struct with method to execute a list of commands
- Takes command list + output directory
- Prompts user for approval before execution
- Executes commands sequentially with inherited stdio
- On failure, prompts user to continue or abort
- Returns summary of results

### 8. cyancoordinator — Integration into composition

**`cyancoordinator/src/operations/composition/operator.rs`**

- After `execute_composition` completes, collect commands from all `ResolvedDependency` templates
- Return commands alongside VFS + state (or expose via new method)
- Caller invokes `CommandExecutor` with collected commands and output directory

### 9. Tests

- Unit: cyan.yaml parsing with and without `commands` field
- Unit: CLI mapper with commands
- Unit: HTTP mapper with commands
- Unit: serde round-trip for `TemplateReq` and `TemplateVersionRes` with commands
- Unit: command collection from dependency tree (empty, single template, multiple templates)
- Unit: `CommandExecutor` — approval flow, success path, failure with continue, failure with abort

## Out of scope

- Zinc backend storage (sibling ticket)
- Command environment variable injection (just inherit current env)
- Parallel command execution
- Command timeout configuration
- Per-command `continue_on_error` flags
