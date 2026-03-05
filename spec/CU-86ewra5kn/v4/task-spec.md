# CU-86ewra5kn v4: Composable TemplateSpec Operations with DI

## Context

v3 unified all code paths through `batch_process()`, but introduced multiple non-composable helper functions:

- `build_prev_specs()`
- `build_curr_specs_for_create()`
- `build_curr_specs_for_update()`
- `build_specs_for_single_template_update()` (80 lines!)
- `classify_specs_by_upgrade()`

This spec refactors to a **composable, DI-style API** using `TemplateSpecManager`.

## Goal

Replace many specific builder functions with **3 composable primitives** + 1 stateless service class.

## Core Concept

```
TemplateSpec = { username, template_name, version, answers, deterministic_states, installed_at }

TemplateSpecManager (STATELESS - only holds registry dependency)
├── get(state) → Vec<TemplateSpec>         // Read from .cyan_state.yaml
├── update(specs, interactive) → Vec<TemplateSpec>  // Upgrade to latest versions
└── reset(specs) → Vec<TemplateSpec>       // Clear answers (for rerun)

// Free function (doesn't need registry)
sort_specs(specs)                           // LWW ordering
```

## API Design

```rust
// spec.rs

/// Stateless service for TemplateSpec operations.
/// Only holds registry as dependency - no internal state.
pub struct TemplateSpecManager {
    registry: Rc<CyanRegistryClient>,
}

impl TemplateSpecManager {
    pub fn new(registry: Rc<CyanRegistryClient>) -> Self {
        Self { registry }
    }

    /// Read specs from .cyan_state.yaml (pure function)
    pub fn get(&self, state: &CyanState) -> Vec<TemplateSpec> {
        state.templates.iter()
            .filter(|(_, s)| s.active)
            .filter_map(|(key, s)| {
                let (username, template_name) = parse_template_key(key)?;
                let entry = s.history.last()?;
                Some(TemplateSpec {
                    username,
                    template_name,
                    version: entry.version,
                    answers: entry.answers.clone(),
                    deterministic_states: entry.deterministic_states.clone(),
                    installed_at: entry.time,
                })
            })
            .collect()
    }

    /// Update specs to latest versions via registry lookup (pure function)
    /// If interactive=true, prompt user to select versions
    pub fn update(
        &self,
        specs: Vec<TemplateSpec>,
        interactive: bool,
    ) -> Result<Vec<TemplateSpec>, Box<dyn Error + Send>> {
        specs.iter().map(|spec| {
            let versions = fetch_all_template_versions(
                &self.registry,
                &spec.username,
                &spec.template_name
            )?;
            let latest = versions.iter()
                .max_by_key(|v| v.version)
                .ok_or_else(|| ...)?;

            let target = if interactive {
                select_version_interactive(
                    &spec.username,
                    &spec.template_name,
                    spec.version,
                    &versions
                )?
            } else {
                latest.version
            };

            Ok(TemplateSpec { version: target, ..spec.clone() })
        }).collect()
    }

    /// Reset answers to empty HashMap (pure function)
    /// Used for rerun scenario - empty answers trigger fresh Q&A
    pub fn reset(&self, specs: Vec<TemplateSpec>) -> Vec<TemplateSpec> {
        specs.into_iter().map(|s| TemplateSpec {
            answers: HashMap::new(),
            deterministic_states: HashMap::new(),
            ..s
        }).collect()
    }
}

// Free function - doesn't need registry
pub fn sort_specs(specs: &mut [TemplateSpec]) {
    specs.sort_by(|a, b| a.installed_at.cmp(&b.installed_at));
}

impl TemplateSpec {
    /// Create spec for new template (empty answers = triggers Q&A)
    pub fn new_template(username: String, template_name: String, version: i64) -> Self;

    /// Get key in format "username/template_name"
    pub fn key(&self) -> String;
}
```

## Scenario Composition

| Scenario                      | prev_specs           | curr_specs                          | upgraded_specs                     |
| ----------------------------- | -------------------- | ----------------------------------- | ---------------------------------- |
| `pls update` (all)            | `manager.get(state)` | `manager.update(prev, interactive)` | `find_by_version_diff(prev, curr)` |
| `pls create` (fresh)          | `[]`                 | `[TemplateSpec::new_template(...)]` | `[new_spec]`                       |
| `pls create` (add)            | `manager.get(state)` | `prev + [new_spec]`                 | `[new_spec]`                       |
| `pls create` (upgrade single) | `manager.get(state)` | `prev` with one version changed     | `[changed_spec]` (explicit)        |
| `pls create` (rerun)          | `manager.get(state)` | `manager.reset(prev)`               | `[target_spec]` (explicit)         |

**Note:** `upgraded_specs` is tracked explicitly during scenario construction, not computed after.

## Files to Modify

### 1. `cyanprint/src/update/spec.rs`

**ADD** `TemplateSpecManager`:

```rust
pub struct TemplateSpecManager {
    registry: Rc<CyanRegistryClient>,
}

impl TemplateSpecManager {
    pub fn new(registry: Rc<CyanRegistryClient>) -> Self;
    pub fn get(&self, state: &CyanState) -> Vec<TemplateSpec>;
    pub fn update(&self, specs: Vec<TemplateSpec>, interactive: bool)
        -> Result<Vec<TemplateSpec>, Box<dyn Error + Send>>;
    pub fn reset(&self, specs: Vec<TemplateSpec>) -> Vec<TemplateSpec>;
}

pub fn sort_specs(specs: &mut [TemplateSpec>);
```

**DELETE** these functions:

- `build_prev_specs()` → replaced by `manager.get()`
- `build_curr_specs_for_create()` → inline composition
- `build_curr_specs_for_update()` → replaced by `manager.update()`
- `build_specs_for_single_template_update()` → inline composition
- `sort_specs_by_time()` → replaced by `sort_specs()`
- `classify_specs_by_upgrade()` → no longer needed (track explicitly)

### 2. `cyanprint/src/run.rs`

**MODIFY** `cyan_run()`:

```rust
pub fn cyan_run(...) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // ... setup code ...

    let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

    let state = load_state(&target_dir);
    let mut prev_specs = manager.get(&state);
    sort_specs(&mut prev_specs);

    let (prev_specs, curr_specs, upgraded_specs) = match update_type {
        TemplateUpdateType::NewTemplate => {
            let new_spec = TemplateSpec::new_template(
                username.clone(),
                template.template.name.clone(),
                template.principal.version,
            );
            let curr: Vec<_> = prev_specs.iter()
                .cloned()
                .chain(iter::once(new_spec.clone()))
                .collect();
            (prev_specs, curr, vec![new_spec])
        }

        TemplateUpdateType::UpgradeTemplate { .. } => {
            let target_key = format!("{}/{}", username, template.template.name);
            let curr: Vec<_> = prev_specs.iter().cloned().map(|mut s| {
                if s.key() == target_key {
                    s.version = template.principal.version;
                }
                s
            }).collect();
            // Explicitly track what changed
            let upgraded: Vec<_> = curr.iter()
                .filter(|s| s.key() == target_key)
                .cloned()
                .collect();
            (prev_specs, curr, upgraded)
        }

        TemplateUpdateType::RerunTemplate { .. } => {
            let target_key = format!("{}/{}", username, template.template.name);
            let curr = manager.reset(prev_specs.clone());
            // Explicitly track what changed (the rerun target)
            let upgraded: Vec<_> = curr.iter()
                .filter(|s| s.key() == target_key)
                .cloned()
                .collect();
            (prev_specs, curr, upgraded)
        }
    };

    let upgraded_refs: Vec<_> = upgraded_specs.iter().collect();
    batch_process(&prev_specs, &curr_specs, &upgraded_refs, ...)
}
```

### 3. `cyanprint/src/update/orchestrator.rs`

**SIMPLIFY** `update_templates()`:

```rust
pub fn update_templates(...) -> Result<Vec<String>, Box<dyn Error + Send>> {
    // ... setup code ...

    let manager = TemplateSpecManager::new(Rc::clone(&registry_client));

    let state = load_state(&target_dir);
    let prev_specs = manager.get(&state);
    let curr_specs = manager.update(prev_specs.clone(), interactive)?;

    // Find upgraded by comparing versions
    let upgraded: Vec<_> = curr_specs.iter()
        .filter(|c| {
            prev_specs.iter()
                .find(|p| p.key() == c.key())
                .map(|p| p.version != c.version)
                .unwrap_or(true)  // New template
        })
        .cloned()
        .collect();

    let upgraded_refs: Vec<_> = upgraded.iter().collect();
    batch_process(&prev_specs, &curr_specs, &upgraded_refs, ...)
}
```

**DELETE** local `batch_process()` function - extract to shared module or keep in `run.rs`.

### 4. `cyanprint/src/update.rs`

**UPDATE** exports:

```rust
pub use spec::{TemplateSpec, TemplateSpecManager, sort_specs};
```

## Code Changes Summary

| File            | Lines Removed               | Lines Added   | Net Change |
| --------------- | --------------------------- | ------------- | ---------- |
| spec.rs         | ~200 (delete old functions) | ~60 (manager) | -140       |
| run.rs          | ~30 (simplify)              | ~30           | ~0         |
| orchestrator.rs | ~120 (delete batch_process) | ~25           | -95        |
| **Total**       | ~350                        | ~115          | **-235**   |

## Success Criteria

1. `pls update` - works as before
2. `pls create <new-template>` on empty project - works
3. `pls create <new-template>` on existing project - works
4. `pls create <existing-template>` with upgrade available - works
5. `pls create <existing-template>` same version (rerun) - works
6. `pls lint` passes
7. `pls build` passes
8. TemplateSpecManager is stateless (only holds registry)

## Benefits

1. **Stateless** - Manager only holds registry, all methods are pure functions
2. **Composable** - All scenarios built from 3 primitives (get, update, reset)
3. **DI-style** - Registry injected, easy to mock for testing
4. **Less code** - ~235 lines removed
5. **Clearer** - Explicit tracking of what changed vs computed detection

## Developer Documentation Updates

Update existing `docs/developer/` files to reflect the new batch processing architecture:

### Files to Update

#### 1. `docs/developer/features/05-template-composition.md`

**Changes:**

- Replace `create_new_composition`, `upgrade_composition`, `rerun_composition` references with `execute_template`, `layer_merge`, `merge`
- Update sequence diagram to show composable primitives
- Update "Create vs Upgrade vs Rerun" table to show primitive composition
- Add reference to `batch_process()` function

**New Content:**

```markdown
## CompositionOperator API (v3+)

The operator now exposes composable primitives instead of scenario-specific methods:

| Method                                        | Input            | Output                               | Purpose                   |
| --------------------------------------------- | ---------------- | ------------------------------------ | ------------------------- |
| `execute_template(template, answers, states)` | Template + state | (VFS, CompositionState, session_ids) | Execute one template spec |
| `layer_merge([VFS...])`                       | VFS list         | VFS                                  | Merge with LWW semantics  |
| `merge(base, local, incoming)`                | 3 VFS            | VFS                                  | 3-way merge               |
| `load_local_files(dir)`                       | Path             | VFS                                  | Load target directory     |
| `write_to_disk(dir, vfs)`                     | Path + VFS       | ()                                   | Persist files             |

## Caller Responsibility

The caller (`cyanprint/src/run.rs::batch_process()`) now constructs scenarios:

1. Build prev_specs and curr_specs
2. MAP: Execute each spec → VFS
3. LAYER: Merge VFS lists
4. MERGE+WRITE: 3-way merge with local, write to disk
```

#### 2. `docs/developer/modules/01-cyanprint.md`

**Changes:**

- Add `update/spec.rs` to structure
- Add `batch_process()` function description
- Update structure diagram

**New Content:**

```markdown
## Structure

\`\`\`text
cyanprint/
├── src/
│ ├── main.rs # Entry point, command routing
│ ├── commands.rs # Clap CLI definitions
│ ├── run.rs # Template execution + batch_process()
│ ├── update/
│ │ ├── mod.rs # Update module exports
│ │ ├── orchestrator.rs # Update command orchestration
│ │ ├── spec.rs # TemplateSpec + TemplateSpecManager (NEW)
│ │ ├── version_manager.rs
│ │ └── utils.rs
│ ├── coord.rs # Coordinator daemon startup
│ ├── util.rs # Utility functions
│ └── errors.rs # Error types
└── Cargo.toml
\`\`\`

### Batch Processing

\`\`\`rust
// run.rs
fn batch_process(
prev_specs: &[TemplateSpec],
curr_specs: &[TemplateSpec],
upgraded_specs: &[&TemplateSpec],
target_dir: &Path,
registry: &CyanRegistryClient,
operator: &CompositionOperator,
) -> Result<Vec<String>, Box<dyn Error + Send>>
\`\`\`

4-phase model: BUILD → MAP → LAYER → MERGE+WRITE
```

#### 3. `docs/developer/surfaces/cli/02-create.md`

**Changes:**

- Update flow diagram to show batch processing phases
- Add reference to `batch_process()` and `TemplateSpec`

**New Content:**

```markdown
## Flow (v3+ Batch Processing)

\`\`\`mermaid
sequenceDiagram
participant U as User
participant CLI as cyanprint
participant REG as Registry
participant COORD as Coordinator
participant FS as Filesystem

    U->>CLI: 1. pls create template:version ./path
    CLI->>REG: 2. GET /template
    CLI->>CLI: 3. BUILD: prev_specs=[], curr_specs=[new_spec]
    loop MAP: For each spec
        CLI->>COORD: 4. execute_template(spec)
        COORD-->>CLI: 5. VFS output
    end
    CLI->>CLI: 6. LAYER: merge VFS outputs
    CLI->>FS: 7. Load local files
    CLI->>CLI: 8. MERGE: 3-way merge
    CLI->>FS: 9. WRITE: persist files
    CLI->>COORD: 10. Clean sessions

\`\`\`

**Key File**: `cyanprint/src/run.rs::batch_process()`
```

#### 4. `docs/developer/surfaces/cli/03-update.md`

**Changes:**

- Update flow diagram to show batch processing phases
- Update "Execute old" and "Execute new" to "MAP phase"
- Add LAYER phase

**New Content:**

```markdown
## Flow (v3+ Batch Processing)

\`\`\`mermaid
sequenceDiagram
participant U as User
participant CLI as cyanprint
participant FS as Filesystem
participant REG as Registry
participant COORD as Coordinator

    U->>CLI: 1. pls update ./path
    CLI->>FS: 2. Read .cyan_state.yaml
    CLI->>REG: 3. Fetch latest versions
    CLI->>CLI: 4. BUILD: prev_specs, curr_specs
    loop MAP: For each spec in prev + curr
        CLI->>COORD: 5. execute_template(spec)
        COORD-->>CLI: 6. VFS output
    end
    CLI->>CLI: 7. LAYER: merge prev_vfs, curr_vfs
    CLI->>FS: 8. Load local files
    CLI->>CLI: 9. MERGE: 3-way merge
    CLI->>FS: 10. WRITE: persist files
    CLI->>COORD: 11. Clean sessions

\`\`\`

| #   | Step           | What                             | Key File                   |
| --- | -------------- | -------------------------------- | -------------------------- |
| 1   | Parse command  | Parse path and options           | `commands.rs:52-72`        |
| 2   | Load state     | Read `.cyan_state.yaml`          | `state/reader.rs`          |
| 3   | Fetch versions | Get latest from registry         | `registry/client.rs`       |
| 4   | BUILD          | Construct prev_specs, curr_specs | `update/spec.rs`           |
| 5-6 | MAP            | Execute each spec → VFS          | `run.rs::batch_process`    |
| 7   | LAYER          | Merge VFS lists                  | `operator.rs::layer_merge` |
| 8-9 | MERGE          | 3-way merge with local           | `merger.rs`                |
| 10  | WRITE          | Persist merged result            | `fs/writer.rs`             |
| 11  | Cleanup        | Remove sessions                  | `main.rs`                  |
```

## Checklist

### Documentation

- [ ] Update `docs/developer/features/05-template-composition.md`
- [ ] Update `docs/developer/modules/01-cyanprint.md`
- [ ] Update `docs/developer/surfaces/cli/02-create.md`
- [ ] Update `docs/developer/surfaces/cli/03-update.md`

### Code

- [ ] Create `TemplateSpecManager` in `spec.rs` (stateless, holds registry only)
- [ ] Add `sort_specs()` free function
- [ ] Delete old helper functions from `spec.rs`
- [ ] Update `run.rs::cyan_run()` to use manager
- [ ] Update `orchestrator.rs::update_templates()` to use manager
- [ ] Delete duplicate `batch_process()` from orchestrator.rs
- [ ] Update exports in `update.rs`
- [ ] `pls lint` passes
- [ ] `pls build` passes
