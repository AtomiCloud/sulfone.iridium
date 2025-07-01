# LLM Development Guide for CyanPrint (sulfone-iridium)

This guide provides essential information for LLMs working on the CyanPrint project, a Rust-based templating engine.

## Project Overview

**CyanPrint** is a command-line templating engine that allows users to create and use project templates.

- **Platform**: sulfone
- **Service**: iridium
- **Language**: Rust (multi-crate workspace)
- **Crates**: `cyanprint`, `cyanprompt`, `cyanregistry`, `cyancoordinator`

## Essential Commands & Aliases

### Task Runner
- **Primary command**: `pls` (alias for `task`)
- **Available tasks**:
  - `pls build` - Build the binary with `cargo build --release`
  - `pls run` - Run the application with `cargo run`
  - `pls lint` - Run all linting and formatting checks

### Critical Rule: Always Lint Before Completion
**🚨 MANDATORY**: Always run `pls lint` before concluding that work is complete. This runs comprehensive pre-commit hooks including:
- Rust Clippy (with warnings as errors)
- Secrets scanning (Infisical)
- Git commit message linting
- Shell script linting
- Docker linting
- Code formatting

## Development Environment

### Nix-Based Setup
This project uses **Nix flakes** for reproducible development environments:

- **Environment loading**: Automatic via `direnv` (`.envrc`)
- **Flake configuration**: `flake.nix` + `nix/` directory
- **Key Nix files**:
  - `nix/packages.nix` - Available tools and dependencies
  - `nix/pre-commit.nix` - Linting and formatting configuration
  - `nix/env.nix` - Environment variables
  - `nix/shells.nix` - Development shell configuration

### When Working with Commands, Scripts, CI, or Installing Binaries:
1. **Always check the `nix/` folder first** - This defines the working environment
2. **Check `nix/packages.nix`** - See what tools are already available
3. **Use the Nix environment** - Don't install tools outside of Nix unless absolutely necessary
4. **Available tools include**:
   - Rust toolchain (via fenix)
   - Docker, Git, Go, GoReleaser
   - Linting tools: clippy, shellcheck, hadolint, gitlint
   - AtomiCloud tools: pls, sg, atomiutils, toml-cli
   - Security: infisical (secrets scanning)

## Workflow Guidelines

### Always Use Common Workflows in Taskfile
- Check `Taskfile.yaml` for existing tasks before creating new ones
- Use `pls <task>` for all common operations
- Extend the Taskfile for new recurring tasks rather than creating ad-hoc scripts

### Available Helper Scripts
Located in `scripts/` directory:
- `build.sh` - Build operations
- `publish.sh` - Publishing to registries
- `release.sh` - Release management
- `bump.sh` - Version bumping
- `fury.sh` - Fury registry operations
- `pre-commit.sh` - Pre-commit hook management

## Code Quality & Standards

### Pre-commit Hooks (via `pls lint`)
- **Rust**: Clippy with warnings as errors (`-D warnings`)
- **Security**: Infisical secrets scanning (both staged files and commit history)
- **Git**: Commit message linting with gitlint
- **Shell**: Shellcheck for shell scripts
- **Docker**: Hadolint for Dockerfiles
- **Formatting**: Treefmt for consistent code formatting
- **Permissions**: Automatic executable permissions for shell scripts

### Rust-Specific Guidelines
- **Workspace structure**: Multi-crate workspace in `Cargo.toml`
- **Build**: Use `cargo build --release` for production builds
- **Testing**: Run tests with `cargo test`
- **Linting**: Clippy is configured to treat warnings as errors

## CI/CD Information

### GitHub Actions Workflows
Located in `.github/workflows/`:
- `ci.yaml` - Continuous integration
- `release.yaml` - Release automation
- `⚡reusable-*.yaml` - Reusable workflow components
- `🛡️merge-gatekeeper.yml` - Merge protection

### Release Management
- Uses `atomi_release.yaml` for release configuration
- GoReleaser for binary distribution
- Automated changelog generation
- Version bumping scripts available

## File Structure Guidelines

### Key Configuration Files
- `Taskfile.yaml` - Task definitions (use `pls` to run)
- `flake.nix` - Nix environment configuration
- `.envrc` - Direnv configuration for automatic environment loading
- `Cargo.toml` - Rust workspace configuration
- `.pre-commit-config.yaml` - Pre-commit hook configuration
- `atomi_release.yaml` - Release configuration

### Directory Structure
```
├── cyanprint/          # Main crate
├── cyanprompt/         # Prompt handling crate
├── cyanregistry/       # Registry management crate
├── cyancoordinator/    # Coordination crate
├── nix/               # Nix configuration files
├── scripts/           # Helper scripts
├── .github/           # CI/CD workflows
└── e2e/               # End-to-end tests
```

## LLM-Specific Best Practices

### Before Making Changes
1. **Check existing tasks**: Review `Taskfile.yaml` for available commands
2. **Understand environment**: Check `nix/packages.nix` for available tools
3. **Review pre-commit hooks**: Understand what will be checked via `nix/pre-commit.nix`

### During Development
1. **Use provided tools**: Leverage tools available in the Nix environment
2. **Follow workspace structure**: Work within the established crate organization
3. **Maintain consistency**: Use existing patterns and conventions

### Before Submitting
1. **Always run `pls lint`**: This is non-negotiable
2. **Test your changes**: Use `pls run` to verify functionality
3. **Check build**: Use `pls build` to ensure compilation
4. **Review commit messages**: Follow gitlint conventions

### When Adding New Dependencies
1. **Check if available in Nix**: Look in `nix/packages.nix` first
2. **Add to appropriate Nix file**: Don't bypass the Nix environment
3. **Update flake.lock**: Run `nix flake update` if needed
4. **Test environment**: Ensure `direnv` reloads correctly

## Common Pitfalls to Avoid

1. **Don't skip `pls lint`**: It catches many issues early
2. **Don't install tools outside Nix**: Use the provided environment
3. **Don't ignore the workspace structure**: Follow the multi-crate pattern
4. **Don't commit without testing**: Always verify changes work
5. **Don't bypass pre-commit hooks**: They enforce quality standards

## Quick Reference Commands

```bash
# Setup (if not using direnv)
nix develop

# Common development workflow
pls build        # Build the project
pls run          # Run the application
pls lint         # Run all checks (MANDATORY before completion)

# Manual operations (if needed)
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Environment Variables

The project uses platform and service variables:
- `PLATFORM=sulfone`
- `SERVICE=iridium`

These are set in `Taskfile.yaml` and used throughout the project.

---

**Remember**: This is a Nix-managed, quality-focused Rust project. Always use the provided tooling and never skip the linting step! 

# Cyan Template System - Workflow & Architecture Documentation

## Overview

The Cyan template system orchestrates template execution through two main workflows:
- **`run.rs`**: Single template execution (new/upgrade/rerun)
- **`update.rs`**: Batch update of all templates in a project

Both workflows follow a session-based architecture with dependency injection and 3-way file merging.

## Core Architecture

### Session Management
- **SessionIdGenerator**: Creates 10-character alphanumeric session IDs
- **Template Execution**: Each template execution consumes one session
- **Session Mapping**: Generated ID may differ from coordinator's actual session ID
- **Cleanup**: Fire-and-forget cleanup with 12-hour auto-expiration

### Key Components
```rust
TemplateOperator {
    session_id_generator: Box<dyn SessionIdGenerator>,
    template_executor: Box<dyn TemplateExecutor>,     // Coordinator communication
    template_history: Box<dyn TemplateHistory>,       // .cyan_state.yaml management
    vfs: Box<dyn Vfs>,                                // File system operations
    registry_client: Rc<CyanRegistryClient>,          // Template fetching (never None)
}
```

## `run.rs` - Single Template Execution

### Input/Output
```rust
pub fn cyan_run(
    session_id_generator: Box<dyn SessionIdGenerator>,
    template: TemplateVersionRes,
    registry_client: Rc<CyanRegistryClient>,  // Always provided
    // ... other params
) -> Result<Vec<String>, Box<dyn Error + Send>>  // Returns session IDs for cleanup
```

### Execution Flow

#### 1. History Analysis
```rust
let update_type = check_template_history(target_dir, &template, &username)?;
```

**Three scenarios determined by `.cyan_state.yaml`:**
- **NewTemplate**: No state file or template not found
- **UpgradeTemplate**: Template exists with different version
- **RerunTemplate**: Template exists with same version

#### 2. Session Generation & Execution

**New Template (1 session)**:
```rust
// Single session for fresh template
let session_id = session_id_generator.generate();
let (archive, state, actual_id) = template_executor.execute_template(
    &template, &session_id, None, None  // No previous answers
)?;

// Simple merge: empty base + local files + template output
merge(&empty_vfs, &local_vfs, &incoming_vfs)
// Returns: vec![actual_id]
```

**Upgrade Template (2 sessions)**:
```rust
// Session 1: Recreate previous version
let prev_session_id = session_id_generator.generate();
let (prev_archive, _, prev_actual_id) = template_executor.execute_template(
    &previous_template, &prev_session_id, 
    Some(&previous_answers), Some(&previous_states)  // Use saved answers
)?;

// Session 2: Execute new version  
let curr_session_id = session_id_generator.generate();
let (curr_archive, state, curr_actual_id) = template_executor.execute_template(
    &template, &curr_session_id,
    Some(&previous_answers), Some(&previous_states)  // Try to reuse answers
)?;

// 3-way merge: previous template + local changes + new template
merge(&base_vfs, &local_vfs, &incoming_vfs)
// Returns: vec![prev_actual_id, curr_actual_id]
```

**Rerun Template (2 sessions)**:
```rust
// Same as upgrade but new execution uses fresh Q&A (None, None)
```

### Template Execution Pipeline
Each `template_executor.execute_template()` call follows this pipeline:
1. **Warm Template**: Download template artifacts to coordinator
2. **Warm Executor**: Start containers and processors
3. **Bootstrap**: Setup volumes and merger
4. **Template Engine**: Handle Q&A prompting
5. **Build**: Generate final archive (tar.gz format)

## `update.rs` - Batch Template Updates

### Input/Output
```rust
pub fn cyan_update(
    session_id_generator: Box<dyn SessionIdGenerator>,
    registry_client: Rc<CyanRegistryClient>,
    interactive: bool,
    // ... other params
) -> Result<Vec<String>, Box<dyn Error + Send>>  // All accumulated session IDs
```

### Execution Flow

#### 1. State Processing
```rust
// Read .cyan_state.yaml and filter active templates
state.templates
    .iter()
    .filter(|(_, state)| state.active)
    .filter_map(|(key, state)| state.history.last())
```

#### 2. Version Management
```rust
// Fetch ALL versions with correct pagination
let all_versions = fetch_all_template_versions(registry_client, username, template_name)?;

// Pagination logic (batches of 100):
loop {
    let versions = registry_client.get_template_versions(username, name, skip, 100)?;
    if versions.is_empty() || versions.len() < 100 { break; }
    skip += 100;
}
```

#### 3. Mode Handling

**Non-Interactive Mode**:
```rust
// Auto-upgrade to latest version
if latest_version.version == current_version {
    return Ok(Vec::new()); // Already latest, skip
}
let target_version = latest_version.version;
```

**Interactive Mode**:
```rust
// User selects from version list with friendly dates
let target_version = select_version_interactive(...)?;
// ESC key aborts entire update process
```

#### 4. Session Accumulation
```rust
// Each template upgrade generates 2 sessions
let session_ids = template_operator.upgrade(...)?;  // vec![prev_id, curr_id]

// Accumulate across ALL templates
acc.extend(session_ids);
// Final result: all session IDs from all template operations
```

### Error Handling
- **Abort-First**: Any failure stops entire batch update
- **No Rollback**: File system changes are not reverted
- **Interactive Cancellation**: ESC aborts all remaining templates

## VFS and Merging System

### 3-Way Merge Logic
```rust
// Merge components:
vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?

// Where:
// - base_vfs: Previous template state (or empty for new templates)
// - local_vfs: Current filesystem state  
// - incoming_vfs: New template output
```

### Conflict Resolution
- **Automatic**: Non-conflicting changes merge automatically
- **Manual**: Conflicts presented as git-style diffs
- **User Responsibility**: Manual conflict resolution required

### Path Loading
```rust
// Current implementation issue (intentional bad code):
let all_paths = Vec::new();  // Empty vector
let local_vfs = self.vfs.load_local_files(target_dir, &all_paths)?;
// The VFS implementation handles empty paths correctly
```

## Session Cleanup Architecture

### Main Process Cleanup
```rust
// In main.rs after cyan_run() or cyan_update()
match result {
    Ok(session_ids) => {
        for session_id in session_ids {
            let _ = coord_client.clean(session_id);  // Fire-and-forget
        }
    }
    Err(_) => {
        // Sessions left dangling - 12-hour auto-cleanup
    }
}
```

### Session Lifecycle
- **Generation**: 10-char alphanumeric string
- **Execution**: May be modified by coordinator service
- **Return**: Actual session ID returned for cleanup
- **Cleanup**: DELETE request to `/executor/{session_id}`
- **Expiration**: 12-hour automatic cleanup if not manually cleaned

## Error Handling Strategy

### Failure Modes
- **Coordinator Unreachable**: Abort operation
- **Registry Unavailable**: Abort operation (no fallback)
- **Template Execution Failure**: Abort operation
- **VFS Write Failure**: Sessions may be left dangling
- **Interactive Cancellation**: Abort entire batch

### Recovery
- **Manual File Recovery**: Users can use git commands
- **Session Cleanup**: Automatic after 12 hours
- **State File Corruption**: Users can manually edit `.cyan_state.yaml`

## Configuration and State

### Template State (`.cyan_state.yaml`)
```yaml
templates:
  "username/template-name":
    active: true
    history:
      - version: 42
        answers: {...}  # User responses to template questions
        deterministic_states: {...}  # Template internal state
        created_at: "2024-01-01T00:00:00Z"
        # ... additional metadata
```

### Template Key Format
- **Format**: `"username/template-name"`
- **Validation**: Strict format required (no malformed key handling)
- **Usage**: Used for history lookup and registry fetching

## Network and Registry

### Registry Operations
- **Template Fetching**: Always fetch from registry (no caching)
- **Version Metadata**: Fetched in paginated batches
- **Authentication**: All templates assumed public
- **Timeout**: 600-second timeout for HTTP operations

### Coordinator Communication
- **Warming**: Parallel template and executor warming
- **Volumes**: Unique merger IDs for volume management
- **Build**: Streaming tar.gz archive response
- **Cleanup**: DELETE endpoints for session cleanup

## Concurrency and Performance

### Current Implementation
- **Sequential Processing**: Templates processed one at a time
- **Parallel Warming**: Template and executor warm in parallel
- **Single Directory**: Only one cyan process per directory
- **No Locking**: No concurrent access protection

### Memory Usage
- **Archive Loading**: Full tar.gz loaded into memory
- **Template State**: Full state kept in memory during processing
- **VFS Operations**: File contents buffered during merge

## Future Considerations

### Potential Improvements
- **Parallel Template Processing**: Could process multiple templates concurrently
- **Session Pooling**: Could reuse sessions for related operations
- **Incremental Updates**: Could optimize for partial template changes
- **Rollback Support**: Could add transactional file operations

### Architecture Flexibility
- **Dependency Injection**: Clean separation allows component replacement
- **Trait-Based Design**: Easy to mock/test individual components
- **Session Abstraction**: Session management can be enhanced without API changes 

## Recursive Template Composition System

### Architecture Overview

The composition system extends the single-template execution to support **recursive template dependencies** with shared state and VFS layering. The system is organized into clean, focused modules for maintainability.

### Module Structure

```
cyancoordinator/src/operations/composition/
├── mod.rs              # Module exports and re-exports (9 lines)
├── state.rs            # CompositionState - shared state management (50 lines)
├── resolver.rs         # DependencyResolver - post-order traversal (90 lines)
├── layerer.rs          # VfsLayerer - VFS merging logic (33 lines)
└── operator.rs         # CompositionOperator - main orchestration (273 lines)
```

**Total: 455 lines across 5 focused files** - each module has a single responsibility and clean boundaries for maintainability.

### Key Components

#### **CompositionOperator**
```rust
pub struct CompositionOperator {
    template_operator: TemplateOperator,        // Core single-template logic
    dependency_resolver: Box<dyn DependencyResolver>,  // Post-order traversal
    vfs_layerer: Box<dyn VfsLayerer>,          // Multi-VFS merging
}
```

#### **DependencyResolver** 
- **Input**: Root `TemplateVersionRes`
- **Process**: Post-order traversal of `template.templates` dependencies
- **Output**: Flattened execution order `Vec<TemplateVersionRes>`
- **Resolution**: Uses `registry.get_template_version_by_id()` for each dependency

#### **CompositionState**
```rust
pub struct CompositionState {
    pub shared_answers: HashMap<String, Answer>,        // Global Q&A state
    pub shared_deterministic_states: HashMap<String, String>,  // Global template state
    pub execution_order: Vec<String>,                   // Execution tracking
}
```

#### **VfsLayerer**
- **Input**: Multiple `VirtualFileSystem` outputs from template executions
- **Process**: Sequential layering (later templates overwrite earlier)
- **Output**: Single layered `VirtualFileSystem`
- **Precedence**: Root template has highest priority

### Execution Flow

#### **1. Dependency Resolution**
```rust
// Example: Template A depends on [B, C], B depends on [D]
// Post-order result: [D, B, C, A]
let templates = dependency_resolver.resolve_dependencies(&root_template)?;
```

#### **2. Sequential Execution with State Sharing**
```rust
for template in templates {
    // Each template inherits all previous answers/states
    let (archive, state, session_id) = executor.execute_template(
        template, 
        &session_id,
        Some(&shared_state.shared_answers),      // Inherited answers
        Some(&shared_state.shared_deterministic_states), // Inherited state
    )?;
    
    // Update shared state for next template
    shared_state.update_from_template_state(&state, template.id);
    vfs_outputs.push(vfs.unpack_archive(archive)?);
}
```

#### **3. VFS Layering**
```rust
// Layer VFS outputs: [D_vfs, B_vfs, C_vfs, A_vfs]
// Result: A_vfs overwrites C_vfs overwrites B_vfs overwrites D_vfs
let layered_vfs = vfs_layerer.layer_merge(&vfs_outputs)?;
```

#### **4. Final 3-way Merge**
```rust
// Same as single template - layer replaces "incoming"
let merged = vfs.merge(&base_vfs, &local_vfs, &layered_vfs)?;
```

### Session Management

#### **New Composition**: N sessions
- One session per template in execution order
- Example: Template with 4 dependencies = 4 sessions

#### **Upgrade Composition**: 2N sessions  
- N sessions for previous composition recreation
- N sessions for current composition execution
- Example: Template with 4 dependencies = 8 sessions total

#### **Rerun Composition**: 2N sessions
- Same as upgrade but current composition uses fresh Q&A

### Integration Points

#### **Auto-Detection**
```rust
// main.rs automatically chooses execution mode
if template_has_dependencies(&template) {
    cyan_run_composition(...)  // Use composition system
} else {
    cyan_run(...)              // Use single template system
}
```

#### **Backward Compatibility**
- Single templates work unchanged via `CompositionOperator`
- Existing templates without dependencies use single-template path
- All session cleanup and error handling patterns preserved

### Error Handling

#### **Abort-First Strategy**
- **Dependency fetch failure**: Abort entire composition
- **Template execution failure**: Abort entire composition  
- **Type conflict in shared state**: Panic with clear error message
- **VFS layering failure**: Abort entire composition

#### **State Sharing Rules**
- **Value conflicts**: Allowed (later template overwrites)
- **Type conflicts**: Abort execution immediately
- **Missing dependencies**: Abort during resolution phase

### Template Dependency Format

```rust
// TemplateVersionRes contains dependency list
pub struct TemplateVersionRes {
    pub templates: Vec<TemplateVersionPrincipalRes>,  // Dependencies
    // ... other fields
}

// Each dependency references by version ID
pub struct TemplateVersionPrincipalRes {
    pub id: String,        // Template version ID (UUID)
    pub version: i64,      // Version number
    // ... other metadata
}
```

### Composition Workflows

#### **`cyan_run_auto()` - Smart Template Execution**
```rust
// Automatically detects dependencies and chooses execution mode
if template.templates.is_empty() {
    cyan_run(...)              // Single template
} else {
    cyan_run_composition(...)  // Recursive composition
}
```

#### **`cyan_update_auto()` - Composition-Aware Updates**
```rust
// Batch updates with composition support
for template in project_templates {
    if template_has_dependencies(&template) {
        composition_operator.upgrade_composition(...)
    } else {
        template_operator.upgrade(...)
    }
}
```

---

**Template Composition System: Production Ready! 🚀** 