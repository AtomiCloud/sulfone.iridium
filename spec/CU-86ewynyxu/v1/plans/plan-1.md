# Plan 1: Run-scoped container ownership with shared RunGuard

## Goal

Replace blanket startup cleanup with run-scoped container labeling and end-of-run cleanup via a shared `Drop` guard, applied uniformly across all test commands (template, plugin, processor, resolver).

## Scope

### In Scope

- Move `RunGuard` struct from `template.rs` to `container.rs` (shared)
- Add run UUID generation in ALL test entry points: `run_template_tests()`, `run_plugin_tests()`, `run_processor_tests()`, `run_resolver_tests()`
- Label all containers created during any test run with `cyanprint.test.run=<uuid>`
- Each entry point creates a `RunGuard` for panic-safe cleanup
- Remove `cleanup_stale_test_containers()` and its call site from `template.rs`
- Thread run UUID through all container creation paths

### Out of Scope

- Boron changes
- `init.rs`'s `qa_warmup()` — init tests delegate to `run_template_tests()` which has its own RunGuard; qa_warmup's template container is explicitly cleaned at end of `qa_warmup()`
- Changes to container lifecycle beyond labeling and cleanup

## Files to Modify

| File                                     | Change Type | Notes                                                                                                              |
| ---------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------ |
| `cyanprint/src/test_cmd/container.rs`    | modify      | Move `RunGuard` here (from `template.rs`), make `pub`; already accepts `run_id` in `build_and_start_container()`   |
| `cyanprint/src/test_cmd/template.rs`     | modify      | Remove `RunGuard` definition (moved to `container.rs`); remove `cleanup_stale_test_containers()` and its call site |
| `cyanprint/src/test_cmd/plugin.rs`       | modify      | Generate run UUID, create `RunGuard`, pass `run_id` to `plugin_warmup()` → `build_and_start_container()`           |
| `cyanprint/src/test_cmd/processor.rs`    | modify      | Generate run UUID, create `RunGuard`, pass `run_id` to `processor_warmup()` → `build_and_start_container()`        |
| `cyanprint/src/test_cmd/resolver.rs`     | modify      | Generate run UUID, create `RunGuard`, pass `run_id` to `resolver_warmup()` → `build_and_start_container()`         |
| `cyanprint/src/try_cmd.rs`               | modify      | No changes needed — already accepts `run_id` in `start_template_container()`                                       |
| `docs/developer/modules/01-cyanprint.md` | modify      | Update container cleanup documentation to reflect run-scoped ownership model                                       |

## Technical Approach

1. **Move `RunGuard` to `container.rs`** — Extract the existing `RunGuard` struct and its `Drop` impl from `template.rs` into `container.rs`, make it `pub(crate)`. This is the only change needed in `container.rs` beyond what's already implemented.

2. **Generate run UUID in each test entry point** — At the top of `run_template_tests()`, `run_plugin_tests()`, `run_processor_tests()`, and `run_resolver_tests()`, generate a run UUID with `uuid::Uuid::new_v4().to_string()`.

3. **Create `RunGuard` in each entry point** — Immediately after UUID generation, create a `let _guard = RunGuard::new(run_id.clone());` that will clean up all containers with that UUID on scope exit (success, error, or panic).

4. **Thread the run UUID to container creation**:

   - **Plugin**: `run_plugin_tests()` → `plugin_warmup()` → `build_and_start_container(artifact_path, "plugin", binds, 5552, Some(&run_id))`
   - **Processor**: `run_processor_tests()` → `processor_warmup()` → `build_and_start_container(artifact_path, "processor", binds, 5551, Some(&run_id))`
   - **Resolver**: `run_resolver_tests()` → `resolver_warmup()` → `build_and_start_container(artifact_path, "resolver", None, 5553, Some(&run_id))`
   - **Template**: Already implemented — `run_template_tests()` → `template_warmup()` → `start_template_container()` with `run_id`

5. **Update warmup functions to accept `run_id`** — Add `run_id: &str` parameter to `plugin_warmup()`, `processor_warmup()`, and `resolver_warmup()`, pass it through to `build_and_start_container()`.

6. **Remove `cleanup_stale_test_containers()`** — Delete the function and its call at line 204 of `template.rs` (on main). The `RunGuard` replaces this entirely.

7. **Keep explicit `cleanup_container()` calls** — Each test entry point still calls `cleanup_container(&container)` or `cleanup_warmup(&warmup)` after tests complete. This is belt-and-suspenders with the `RunGuard` and provides clearer error messages. The `RunGuard` is the safety net for panics.

## Edge Cases to Handle

- **Tokio runtime in Drop**: `Drop::drop` is sync, so the guard needs to create its own tokio runtime (same pattern as current `cleanup_stale_test_containers()`)
- **Panic safety**: `Drop` guard is the mechanism — it runs on panic, so containers are cleaned
- **Nested tests**: Inner test generates its own UUID, labels its own containers, and cleans only its own. Parent's containers have a different UUID and are untouched.
- **Concurrent tests**: Each test run has a unique UUID. No cross-contamination.

## How to Test

1. Run existing `cyan test template` to verify no regression for non-nested runs
2. Run existing `cyan test plugin` to verify no regression
3. Run existing `cyan test processor` to verify no regression
4. Run existing `cyan test resolver` to verify no regression
5. Verify nested test scenario: a validate command that runs `cyan test template` should not kill the parent's container
6. Verify containers are cleaned up after test completion (check `docker ps -a` for no leftover `cyanprint.test.run` containers)
7. Verify cleanup happens on test failure (force a test failure, check containers are removed)

## Integration Points

- **Depends on**: nothing
- **Blocks**: nothing
- **Shared state**: run UUID threaded through warmup structs and function parameters

## Implementation Checklist

- [ ] Move `RunGuard` from `template.rs` to `container.rs`, make `pub(crate)`
- [ ] Update `template.rs` to import `RunGuard` from `container.rs`
- [ ] Generate run UUID and create `RunGuard` in `run_plugin_tests()`
- [ ] Thread run_id through `plugin_warmup()` → `build_and_start_container()`
- [ ] Generate run UUID and create `RunGuard` in `run_processor_tests()`
- [ ] Thread run_id through `processor_warmup()` → `build_and_start_container()`
- [ ] Generate run UUID and create `RunGuard` in `run_resolver_tests()`
- [ ] Thread run_id through `resolver_warmup()` → `build_and_start_container()`
- [ ] Remove `cleanup_stale_test_containers()` from `template.rs`
- [ ] Update `docs/developer/modules/01-cyanprint.md`
- [ ] Linting passes (`direnv exec . cargo clippy`)
- [ ] No regressions in existing functionality

## Success Criteria

- [ ] `cleanup_stale_test_containers()` no longer exists
- [ ] `RunGuard` is defined in `container.rs` and used by all four test entry points
- [ ] ALL test-created containers carry `cyanprint.test.run` label
- [ ] `RunGuard::drop()` cleans up all containers for its run
- [ ] Nested test runs do not kill parent containers
- [ ] `cargo clippy` passes with no warnings
