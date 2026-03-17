# Plan 1: Run-scoped container ownership with Drop guard cleanup

## Goal

Replace blanket startup cleanup with run-scoped container labeling and end-of-run cleanup via a `Drop` guard.

## Scope

### In Scope

- Remove `cleanup_stale_test_containers()` and its call site
- Add run UUID generation in `run_template_tests()`
- Label all containers created during the run with `cyanprint.test.run=<uuid>`
- Implement a `Drop` guard that cleans up containers with the run's UUID on scope exit

### Out of Scope

- Boron changes
- Changes to container lifecycle beyond labeling and cleanup

## Files to Modify

| File                                     | Change Type | Notes                                                                                                             |
| ---------------------------------------- | ----------- | ----------------------------------------------------------------------------------------------------------------- |
| `cyanprint/src/test_cmd/template.rs`     | modify      | Remove `cleanup_stale_test_containers()`, generate run UUID, create `Drop` guard, pass UUID to container creation |
| `cyanprint/src/try_cmd.rs`               | modify      | Accept run UUID in `start_template_container()`, add `cyanprint.test.run` label to container                      |
| `cyanprint/src/test_cmd/container.rs`    | modify      | Accept run UUID in container creation, add `cyanprint.test.run` label to containers                               |
| `docs/developer/modules/01-cyanprint.md` | modify      | Update container cleanup documentation to reflect run-scoped ownership model                                      |

## Technical Approach

1. **Generate run UUID** at the top of `run_template_tests()` — use `uuid::Uuid::new_v4().to_string()`

2. **Create a `RunGuard` struct** with a `Drop` implementation that:

   - Stores the run UUID and a reference to the Docker client (or creates one on drop)
   - On `drop()`, lists all containers with `label=cyanprint.test.run=<uuid>` and removes them
   - Uses a blocking tokio runtime (same pattern as the current cleanup function) since `drop()` is sync

3. **Thread the run UUID** through the call chain:

   - `run_template_tests()` → `template_warmup()` → `start_template_container()` in `try_cmd.rs`
   - `run_template_tests()` → `run_single_test()` → container creation in `container.rs`
   - The `TemplateWarmup` struct already exists as a carry — add `run_id: String` to it

4. **Add the `cyanprint.test.run` label** in:

   - `try_cmd.rs:start_template_container()` — template warmup container
   - `container.rs` — processor/plugin/resolver test containers (both the label construction and any other container creation sites)

5. **Remove `cleanup_stale_test_containers()`** — delete the function and its call at line 204 of `template.rs`

6. **Update `cleanup_warmup()`** — it already cleans up the template container by name. This can stay as-is (belt-and-suspenders with the `Drop` guard), or be simplified since the `Drop` guard will handle it.

## Edge Cases to Handle

- **Tokio runtime in Drop**: `Drop::drop` is sync, so the guard needs to create its own tokio runtime (same pattern as current `cleanup_stale_test_containers()`)
- **Panic safety**: `Drop` guard is the mechanism — it runs on panic, so containers are cleaned
- **Nested tests**: Inner `run_template_tests()` generates its own UUID, labels its own containers, and cleans only its own. Parent's containers have a different UUID and are untouched.

## How to Test

1. Run existing `cyan test template` to verify no regression for non-nested runs
2. Verify nested test scenario: a validate command that runs `cyan test template` should not kill the parent's template container
3. Verify containers are cleaned up after test completion (check `docker ps -a` for no leftover `cyanprint.test.run` containers)
4. Verify cleanup happens on test failure (force a test failure, check containers are removed)

## Integration Points

- **Depends on**: nothing
- **Blocks**: nothing
- **Shared state**: run UUID threaded through `TemplateWarmup` struct and function parameters

## Implementation Checklist

- [ ] Code changes per approach above
- [ ] Update `docs/developer/modules/01-cyanprint.md` — document the run-scoped container ownership model and `cyanprint.test.run` label
- [ ] Check and update other docs in `docs/` that reference container cleanup behavior
- [ ] Linting passes (`direnv exec . cargo clippy`)
- [ ] No regressions in existing functionality

## Success Criteria

- [ ] `cleanup_stale_test_containers()` no longer exists
- [ ] All test-created containers carry `cyanprint.test.run` label
- [ ] `RunGuard::drop()` cleans up all containers for its run
- [ ] Nested test runs do not kill parent containers
- [ ] `cargo clippy` passes with no warnings
