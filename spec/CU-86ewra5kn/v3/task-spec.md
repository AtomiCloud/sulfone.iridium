# CU-86ewra5kn v3: Unify All Code Paths to batch_process()

## Context

v2 successfully implemented batch processing for `pls update` and some `pls create` scenarios. However, `run.rs` still has mixed code paths:

- **New path (v2)**: Uses `batch_process()` for NewTemplate cases
- **Old path (v0/v1)**: Uses `upgrade_composition()`, `rerun_composition()` for UpgradeTemplate/RerunTemplate cases

This spec completes the migration by unifying ALL code paths through `batch_process()`.

## Goal

**Delete old composition methods and unify all scenarios through `batch_process()`.**

## Key Insight

All scenarios are just different inputs to the same function:

```
batch_process(prev_specs, curr_specs, upgraded_specs, ...)
```

| Scenario                 | prev_specs       | curr_specs                   | upgraded_specs |
| ------------------------ | ---------------- | ---------------------------- | -------------- |
| New project              | `[]`             | `[C]`                        | `[C]`          |
| Add template to existing | `[A, B]`         | `[A, B, C]`                  | `[C]`          |
| Upgrade A v1→v2          | `[A(v1), B]`     | `[A(v2), B]`                 | `[A]`          |
| Rerun A (fresh Q&A)      | `[A(v1), B]`     | `[A(v1), B]` (empty answers) | `[A]`          |
| pls update               | `[A(v1), B(v2)]` | `[A(v3), B(v3)]`             | `[A, B]`       |

## Files to Modify

### 1. `cyancoordinator/src/operations/composition/operator.rs`

**DELETE** (~150 lines):

```rust
pub fn create_new_composition(...)  // lines 102-159
pub fn upgrade_composition(...)     // lines 162-245
pub fn rerun_composition(...)       // lines 248-318
```

**KEEP** (v2 methods):

```rust
pub fn execute_template(...)        // core primitive
pub fn layer_merge(...)             // VFS layering
pub fn merge(...)                   // 3-way merge
pub fn load_local_files(...)        // disk I/O
pub fn write_to_disk(...)           // disk I/O
```

### 2. `cyanprint/src/run.rs`

**DELETE**:

```rust
fn batch_create_for_existing_project(...)  // lines 172-229
```

**MODIFY** `cyan_run()` to handle all `TemplateUpdateType` variants with `batch_process()`:

```rust
match update_type {
    TemplateUpdateType::NewTemplate => {
        let prev_specs = build_prev_specs(&state);
        let new_spec = TemplateSpec::for_new_template(...);
        let curr_specs = build_curr_specs_for_create(prev_specs.clone(), new_spec.clone());
        sort_specs_by_time(&mut prev_specs);
        sort_specs_by_time(&mut curr_specs);
        let upgraded_specs = vec![&new_spec]; // Only new template is "upgraded"
        batch_process(&prev_specs, &curr_specs, Some(&new_spec), target_dir, registry, &composition_operator)
    }
    TemplateUpdateType::UpgradeTemplate { previous_version, previous_answers, previous_states } => {
        let prev_specs = build_prev_specs_with_version(&state, previous_version);
        let curr_specs = build_prev_specs(&state); // Latest versions
        let upgraded_specs = classify_upgraded(&prev_specs, &curr_specs);
        batch_process(&prev_specs, &curr_specs, upgraded_specs, target_dir, registry, &composition_operator)
    }
    TemplateUpdateType::RerunTemplate { previous_version, previous_answers, previous_states } => {
        let prev_specs = build_prev_specs(&state); // With old answers
        let curr_specs = build_prev_specs_with_fresh_answers(&state); // Empty answers = trigger Q&A
        let upgraded_specs = vec![/* the template being rerun */];
        batch_process(&prev_specs, &curr_specs, upgraded_specs, target_dir, registry, &composition_operator)
    }
}
```

### 3. `cyanprint/src/update/spec.rs`

**ADD** helper functions:

```rust
/// Build prev_specs with specific version for a template (for upgrade)
pub fn build_prev_specs_with_version(state: &CyanState, template_key: &str, version: i64) -> Vec<TemplateSpec>;

/// Build specs with empty answers to trigger fresh Q&A (for rerun)
pub fn build_prev_specs_with_fresh_answers(state: &CyanState) -> Vec<TemplateSpec>;

/// Classify which specs were upgraded between prev and curr
pub fn classify_upgraded<'a>(prev: &[TemplateSpec], curr: &'a [TemplateSpec]) -> Vec<&'a TemplateSpec>;
```

## Implementation Plan

### Step 1: Add helper functions to `spec.rs`

- `build_prev_specs_with_version()` - for upgrade scenario
- `build_prev_specs_with_fresh_answers()` - for rerun scenario
- Ensure `classify_specs_by_upgrade()` works for single-template upgrades too

### Step 2: Modify `run.rs::cyan_run()`

- Remove `batch_create_for_existing_project()` function
- Handle `UpgradeTemplate` with `batch_process()`
- Handle `RerunTemplate` with `batch_process()`
- Ensure all paths use the unified flow

### Step 3: Delete old methods from `operator.rs`

- Remove `create_new_composition()`
- Remove `upgrade_composition()`
- Remove `rerun_composition()`
- Keep `execute_template()`, `layer_merge()`, `merge()`, `load_local_files()`, `write_to_disk()`

### Step 4: Run tests and fix any issues

- `pls lint` must pass
- `pls build` must pass
- `pls test` must pass

## Success Criteria

1. `pls update` - works as before (already using batch_process)
2. `pls create <new-template>` on empty project - works
3. `pls create <new-template>` on existing project - works
4. `pls create <existing-template>` with upgrade available - works (upgrade scenario)
5. `pls create <existing-template>` same version - works (rerun scenario)
6. No dead code in `operator.rs`
7. `run.rs` has single unified code path through `batch_process()`

## Code Removal Summary

| File        | Lines Removed | Lines Added | Net Change |
| ----------- | ------------- | ----------- | ---------- |
| operator.rs | ~150          | 0           | -150       |
| run.rs      | ~60           | ~40         | -20        |
| spec.rs     | 0             | ~30         | +30        |
| **Total**   | ~210          | ~70         | **-140**   |

## Risks

1. **Behavior changes**: The old methods might have subtle differences. Test all scenarios.
2. **Metadata saving**: Ensure template metadata is saved correctly for all upgrade types.
3. **Session cleanup**: Ensure session IDs are collected and returned for cleanup.

## Checklist

- [ ] Add helper functions to `spec.rs`
- [ ] Modify `run.rs::cyan_run()` to use batch_process for all cases
- [ ] Delete `batch_create_for_existing_project()` from `run.rs`
- [ ] Delete old methods from `operator.rs`
- [ ] `pls lint` passes
- [ ] `pls build` passes
- [ ] Manual test: `pls create` on empty project
- [ ] Manual test: `pls create` on existing project (add template)
- [ ] Manual test: `pls create` upgrade scenario
- [ ] Manual test: `pls create` rerun scenario
- [ ] Manual test: `pls update`
