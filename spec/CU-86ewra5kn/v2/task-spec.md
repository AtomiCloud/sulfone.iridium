# Task Specification v2: Unified Batch VFS Processing

## Amendment from v1

v1 had the right idea but overcomplicated the implementation. v2 simplifies to a clean map-then-reduce pattern with a single unified flow for both `create` and `update` commands.

## Core Primitive

```rust
/// Execute a single template (with its dependencies) and return ONE VFS.
/// Dependencies are resolved in post-order and layered internally.
/// Q&A is triggered if answers are empty for required questions.
fn execute_template(
    template_res: TemplateVersionRes,
    answers: HashMap<String, Answer>,
    deterministic_states: HashMap<String, String>,
) -> Result<Vfs, Error>
```

This already exists as `execute_composition()` in `operator.rs`.

## TemplateSpec

A simple data structure representing a template to execute:

```rust
struct TemplateSpec {
    username: String,
    template_name: String,
    version: i64,
    answers: HashMap<String, Answer>,
    deterministic_states: HashMap<String, String>,
}
```

## The Unified Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  PHASE 1: BUILD SPEC LISTS                                      │
│                                                                 │
│  Read .cyan_state.yaml → build prev_specs                       │
│                                                                 │
│  For CREATE:  curr_specs = prev_specs + [new_template_spec]     │
│  For UPDATE:  curr_specs = prev_specs (with upgraded versions)  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  PHASE 2: MAP (execute each template spec → VFS)                │
│                                                                 │
│  prev_vfs_list = map(execute_template, prev_specs)              │
│  curr_vfs_list = map(execute_template, curr_specs)              │
│                                                                 │
│  Note: Q&A happens inside execute_template if answers missing   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  PHASE 3: LAYER (merge each list into ONE VFS)                  │
│                                                                 │
│  prev_vfs = layer_merge(prev_vfs_list)  // LWW by time order    │
│  curr_vfs = layer_merge(curr_vfs_list)  // LWW by time order    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  PHASE 4: MERGE + WRITE                                         │
│                                                                 │
│  local_vfs = load_local_files(target_dir)                       │
│  merged_vfs = merge(prev_vfs, local_vfs, curr_vfs)  // 3-way    │
│  write_to_disk(target_dir, merged_vfs)                          │
│  save_metadata(target_dir, curr_specs)                          │
└─────────────────────────────────────────────────────────────────┘
```

## How Commands Map to This Flow

### `pls update`

```
prev_specs = templates from .cyan_state.yaml (current versions with stored answers)
curr_specs = same templates at latest versions (reuse stored answers, Q&A if new questions)
```

Example:

```
.cyan_state.yaml:  A@v1 (answers_a), B@v2 (answers_b)
Latest available:  A@v1, B@v3

prev_specs: [{A, v1, answers_a}, {B, v2, answers_b}]
curr_specs: [{A, v1, answers_a}, {B, v3, answers_b}]  // B upgraded, answers reused
```

### `pls create` (fresh project)

```
prev_specs = []  (empty - nothing installed yet)
curr_specs = [new_template_spec]  (Q&A happens during execute)
```

### `pls create` (existing project)

```
prev_specs = templates from .cyan_state.yaml (existing with stored answers)
curr_specs = prev_specs + [new_template_spec]  (new template triggers Q&A)
```

Example:

```
.cyan_state.yaml:  A@v1 (answers_a), B@v2 (answers_b)
User runs: pls create C .

prev_specs: [{A, v1, answers_a}, {B, v2, answers_b}]
curr_specs: [{A, v1, answers_a}, {B, v2, answers_b}, {C, v1, <empty>}]
                                                                         ↑
                                                              Q&A triggers here
```

## Key Functions

### Build Spec Lists

```rust
/// Build prev_specs from .cyan_state.yaml
fn build_prev_specs(cyan_state: &CyanState) -> Vec<TemplateSpec> {
    cyan_state.templates.iter()
        .filter(|(_, state)| state.active)
        .filter_map(|(key, state)| {
            let (username, template_name) = parse_template_key(key)?;
            let entry = state.history.last()?;
            Some(TemplateSpec {
                username,
                template_name,
                version: entry.version,
                answers: entry.answers.clone(),
                deterministic_states: entry.deterministic_states.clone(),
            })
        })
        .collect()
}

/// Build curr_specs for create command
fn build_curr_specs_for_create(
    prev_specs: Vec<TemplateSpec>,
    new_template: TemplateSpec,  // answers may be empty (will Q&A)
) -> Vec<TemplateSpec> {
    let mut curr = prev_specs.clone();
    curr.push(new_template);
    curr
}

/// Build curr_specs for update command
fn build_curr_specs_for_update(
    prev_specs: Vec<TemplateSpec>,
    registry: &RegistryClient,
    interactive: bool,
) -> Result<Vec<TemplateSpec>, Error> {
    prev_specs.iter().map(|spec| {
        // Fetch latest version
        let latest = registry.get_latest_version(&spec.username, &spec.template_name)?;
        let target_version = if interactive {
            select_version_interactive(spec.version, &latest)?
        } else {
            latest.version
        };

        Ok(TemplateSpec {
            version: target_version,
            ..spec.clone()  // Reuse answers and deterministic_states
        })
    }).collect()
}
```

### Main Batch Process

```rust
/// Unified batch processing for both create and update.
/// Returns session IDs for cleanup.
fn batch_process(
    prev_specs: Vec<TemplateSpec>,
    curr_specs: Vec<TemplateSpec>,
    target_dir: &Path,
    registry: &RegistryClient,
    operator: &CompositionOperator,
) -> Result<Vec<String>, Error> {
    // Sort by installation time for consistent LWW ordering
    // (Assuming specs carry their original installation time)
    let prev_specs = sort_by_time(prev_specs);
    let curr_specs = sort_by_time(curr_specs);

    // PHASE 2: MAP
    println!("📦 MAP phase: Executing {} prev + {} curr templates",
             prev_specs.len(), curr_specs.len());

    let mut prev_vfs_list = Vec::new();
    let mut prev_session_ids = Vec::new();

    for spec in &prev_specs {
        let template_res = registry.get_template(&spec.username, &spec.template_name, Some(spec.version))?;
        let (vfs, session_ids) = operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        prev_vfs_list.push(vfs);
        prev_session_ids.extend(session_ids);
    }

    let mut curr_vfs_list = Vec::new();
    let mut curr_session_ids = Vec::new();

    for spec in &curr_specs {
        let template_res = registry.get_template(&spec.username, &spec.template_name, Some(spec.version))?;
        let (vfs, session_ids) = operator.execute_template(&template_res, &spec.answers, &spec.deterministic_states)?;
        curr_vfs_list.push(vfs);
        curr_session_ids.extend(session_ids);
    }

    // PHASE 3: LAYER
    println!("🔀 LAYER phase: Merging {} prev + {} curr VFS outputs",
             prev_vfs_list.len(), curr_vfs_list.len());

    let prev_vfs = operator.layer_merge(&prev_vfs_list)?;
    let curr_vfs = operator.layer_merge(&curr_vfs_list)?;

    // PHASE 4: MERGE + WRITE
    println!("📝 MERGE+WRITE phase: 3-way merge with local files");

    let local_vfs = operator.load_local_files(target_dir)?;
    let merged_vfs = operator.merge(&prev_vfs, &local_vfs, &curr_vfs)?;

    operator.write_to_disk(target_dir, &merged_vfs)?;

    // Save metadata for curr specs
    save_metadata(target_dir, &curr_specs, operator)?;

    let mut all_session_ids = prev_session_ids;
    all_session_ids.extend(curr_session_ids);

    println!("✅ Batch process complete");
    Ok(all_session_ids)
}
```

## Command Implementations

### `pls update`

```rust
fn update_templates(path: String, interactive: bool) -> Result<(), Error> {
    let cyan_state = load_cyan_state(&path)?;

    let prev_specs = build_prev_specs(&cyan_state);
    let curr_specs = build_curr_specs_for_update(prev_specs.clone(), &registry, interactive)?;

    let session_ids = batch_process(prev_specs, curr_specs, &path, &registry, &operator)?;

    cleanup_sessions(session_ids);
    Ok(())
}
```

### `pls create`

```rust
fn create_template(template: String, path: String) -> Result<(), Error> {
    let cyan_state = load_cyan_state(&path).unwrap_or_default();

    let prev_specs = build_prev_specs(&cyan_state);

    // Build new template spec (answers empty - will Q&A during execute)
    let new_spec = TemplateSpec {
        username: parse_username(&template)?,
        template_name: parse_name(&template)?,
        version: latest_version,
        answers: HashMap::new(),  // Empty - triggers Q&A
        deterministic_states: HashMap::new(),
    };

    let curr_specs = build_curr_specs_for_create(prev_specs.clone(), new_spec);

    let session_ids = batch_process(prev_specs, curr_specs, &path, &registry, &operator)?;

    cleanup_sessions(session_ids);
    Ok(())
}
```

## Files to Change

| File                 | Action                                                                                                   |
| -------------------- | -------------------------------------------------------------------------------------------------------- |
| `batch_processor.rs` | DELETE (replaced by simpler functions)                                                                   |
| `orchestrator.rs`    | REWRITE to use `build_curr_specs_for_update` + `batch_process`                                           |
| `run.rs`             | REWRITE to use `build_curr_specs_for_create` + `batch_process`                                           |
| `operator.rs`        | MODIFY: expose `execute_template`, `layer_merge`, `merge`, `load_local_files`, `write_to_disk` as public |
| NEW: `spec.rs`       | ADD: `TemplateSpec` struct, `build_prev_specs`, `build_curr_specs_for_*`                                 |

## Refactoring Details

### `operator.rs` Changes

```rust
impl CompositionOperator {
    /// Execute a single template spec and return VFS + session IDs.
    /// This is the core primitive - pure function, no side effects.
    pub fn execute_template(
        &self,
        template: &TemplateVersionRes,
        answers: &HashMap<String, Answer>,
        deterministic_states: &HashMap<String, String>,
    ) -> Result<(VirtualFileSystem, Vec<String>), Box<dyn Error + Send>> {
        let templates = self.dependency_resolver.resolve_dependencies(template)?;

        let shared_state = CompositionState {
            shared_answers: answers.clone(),
            shared_deterministic_states: deterministic_states.clone(),
            execution_order: Vec::new(),
        };

        let (vfs, _, session_ids) = self.execute_composition(&templates, &shared_state)?;
        Ok((vfs, session_ids))
    }

    /// Layer merge a list of VFS into one (LWW semantics).
    pub fn layer_merge(&self, vfs_list: &[VirtualFileSystem]) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.vfs_layerer.layer_merge(vfs_list)
    }

    /// 3-way merge: (base, local, incoming) -> merged.
    pub fn merge(&self, base: &VirtualFileSystem, local: &VirtualFileSystem, incoming: &VirtualFileSystem) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.template_operator.vfs.merge(base, local, incoming)
    }

    /// Load local files from target directory.
    pub fn load_local_files(&self, target_dir: &Path) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.template_operator.vfs.load_local_files(target_dir, &[])
    }

    /// Write VFS to disk.
    pub fn write_to_disk(&self, target_dir: &Path, vfs: &VirtualFileSystem) -> Result<(), Box<dyn Error + Send>> {
        self.template_operator.vfs.write_to_disk(target_dir, vfs)
    }
}
```

### Remove from `operator.rs`

- `collect_upgrade_vfs()` - replaced by `execute_template()`
- `collect_create_vfs()` - replaced by `execute_template()`
- `layer_and_merge_vfs()` - replaced by caller doing `layer_merge()` + `merge()`

## Acceptance Criteria (v2)

- [ ] `TemplateSpec` struct defined
- [ ] `build_prev_specs()` implemented
- [ ] `build_curr_specs_for_create()` implemented
- [ ] `build_curr_specs_for_update()` implemented
- [ ] `execute_template()` exposed as public on `CompositionOperator`
- [ ] `layer_merge()`, `merge()`, `load_local_files()`, `write_to_disk()` exposed
- [ ] `orchestrator.rs` rewritten to use unified flow
- [ ] `run.rs` rewritten to use unified flow
- [ ] `batch_processor.rs` deleted
- [ ] `pls update` works correctly
- [ ] `pls create` (fresh project) works correctly
- [ ] `pls create` (existing project) works correctly
- [ ] All templates appear in BOTH prev and curr VFS lists
- [ ] LWW ordering consistent between prev and curr
- [ ] Tests pass

## Benefits

1. **Simple**: Map-then-reduce, no stateful accumulation
2. **Unified**: Create and update use identical flow
3. **Correct**: All templates appear in both prev and curr (including non-upgraded)
4. **Pure**: Core functions are stateless
5. **Composable**: Easy to add new commands or modify behavior
6. **Less code**: Delete `batch_processor.rs`, simplify `orchestrator.rs` and `run.rs`
