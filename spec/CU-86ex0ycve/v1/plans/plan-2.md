# Plan 2: cyancoordinator — Dependency resolution injection

## Goal

Change the dependency resolver to carry preset answers per dependency, and inject them into `shared_answers` before each sub-template execution. This is the runtime behavior change.

**Prerequisite**: Plan 1 must be complete (response model `TemplateVersionTemplateRefRes` must exist).

## Files to change

1. `cyancoordinator/src/operations/composition/resolver.rs`
2. `cyancoordinator/src/operations/composition/operator.rs`
3. `cyancoordinator/src/operations/composition/mod.rs` (exports)

## Steps

### 2.1 Add `ResolvedDependency` struct and `serde_json_value_to_answer` helper

**File**: `cyancoordinator/src/operations/composition/resolver.rs`

Add:

```rust
use cyanprompt::domain::models::answer::Answer;

/// A dependency template with its preset answers (declared by the parent)
pub struct ResolvedDependency {
    pub template: cyanregistry::http::models::template_res::TemplateVersionRes,
    pub preset_answers: HashMap<String, Answer>,
}

/// Convert a serde_json::Value to an Answer enum.
/// Returns None for unsupported types (caller should skip).
pub fn serde_json_value_to_answer(value: &serde_json::Value) -> Option<Answer> {
    match value {
        serde_json::Value::String(s) => Some(Answer::String(s.clone())),
        serde_json::Value::Bool(b) => Some(Answer::Bool(*b)),
        serde_json::Value::Array(arr) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if strings.len() == arr.len() {
                Some(Answer::StringArray(strings))
            } else {
                None
            }
        }
        _ => None,
    }
}
```

### 2.2 Change `DependencyResolver` trait return type

**File**: `cyancoordinator/src/operations/composition/resolver.rs`

Change:

```rust
pub trait DependencyResolver {
    fn resolve_dependencies(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<Vec<TemplateVersionRes>, Box<dyn Error + Send>>;
}
```

To:

```rust
pub trait DependencyResolver {
    fn resolve_dependencies(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>>;
}
```

### 2.3 Update `DefaultDependencyResolver` implementation

**File**: `cyancoordinator/src/operations/composition/resolver.rs`

In `flatten_dependencies`, the method receives `TemplateVersionRes` whose `.templates` is now `Vec<TemplateVersionTemplateRefRes>`. Each ref has `.id` and `.preset_answers`.

Changes to `flatten_dependencies`:

- Signature: return `Vec<ResolvedDependency>` instead of `Vec<TemplateVersionRes>`
- For each dependency ref: extract `preset_answers` from `TemplateVersionTemplateRefRes`, convert each value via `serde_json_value_to_answer`
- Recurse: call `flatten_dependencies` on the fetched dep template (which may itself have sub-dependencies with preset answers)
- Post-order: push `ResolvedDependency { template: dep_template, preset_answers }`

Changes to `resolve_dependencies`:

- Build `ResolvedDependency` for the root template (no preset answers for root)
- Return `Vec<ResolvedDependency>`

### 2.4 Update `CompositionOperator.execute_composition`

**File**: `cyancoordinator/src/operations/composition/operator.rs`

Change `execute_composition` signature from:

```rust
fn execute_composition(
    &mut self,
    templates: &[TemplateVersionRes],
    initial_shared_state: &CompositionState,
) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>>
```

To:

```rust
fn execute_composition(
    &mut self,
    dependencies: &[ResolvedDependency],
    initial_shared_state: &CompositionState,
) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>>
```

In the loop body, before each template execution:

```rust
// Merge preset answers into shared_answers for this template only
let mut template_answers = shared_state.shared_answers.clone();
for (key, answer) in &dep.preset_answers {
    template_answers.entry(key.clone()).or_insert(answer.clone());
}

let (archive_data, template_state, actual_session_id) =
    self.template_operator.template_executor.execute_template(
        &dep.template,
        &session_id,
        Some(&template_answers),
        Some(&shared_state.shared_deterministic_states),
    )?;
```

Update `update_from_template_state` call to use `dep.template.principal.id`.

### 2.5 Update `CompositionOperator.execute_template`

**File**: `cyancoordinator/src/operations/composition/operator.rs`

Change `execute_template` to call `resolve_dependencies` (which now returns `Vec<ResolvedDependency>`) and pass result to `execute_composition`:

```rust
pub fn execute_template(
    &mut self,
    template: &TemplateVersionRes,
    answers: &HashMap<String, Answer>,
    deterministic_states: &HashMap<String, String>,
) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>> {
    let dependencies = self.dependency_resolver.resolve_dependencies(template)?;

    let shared_state = CompositionState {
        shared_answers: answers.clone(),
        shared_deterministic_states: deterministic_states.clone(),
        execution_order: Vec::new(),
    };

    let (vfs, final_state, session_ids) =
        self.execute_composition(&dependencies, &shared_state)?;
    Ok((vfs, final_state, session_ids))
}
```

### 2.6 Update `build_resolver_registry` and `build_template_infos`

**File**: `cyancoordinator/src/operations/composition/operator.rs`

Both methods take `&[TemplateVersionRes]`. Update to work with `&[ResolvedDependency]` by accessing `.template` field.

### 2.7 Update module exports

**File**: `cyancoordinator/src/operations/composition/mod.rs`

Export `ResolvedDependency` and `serde_json_value_to_answer` if needed by external code.

### 2.8 Fix all downstream callers

Search the codebase for:

- Direct calls to `execute_template` on `CompositionOperator` — these should still work since the public API signature is unchanged
- Direct calls to `execute_composition` — internal only, already updated
- Any code that destructures or iterates over resolver results — update for `ResolvedDependency`

### 2.9 Tests

- Unit test `serde_json_value_to_answer`: String, Bool, StringArray, Number (returns None), Null (returns None)
- Unit test `DefaultDependencyResolver`: mock Zinc responses with preset_answers on dependency refs, verify resolver carries them through
- Unit test injection: preset answer fills gap when user hasn't answered; user answer wins when both exist

## Verification

- `cargo build` succeeds
- `cargo test` passes (existing tests updated + new tests)
- `pre-commit run --all` passes
- Manual: create a template with preset answers, push, resolve, verify sub-template doesn't prompt for pre-set values
