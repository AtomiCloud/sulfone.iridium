# Task Specification: Fix nested cyan test killing parent template container (CU-86ewynyxu)

## Source

- Ticket: CU-86ewynyxu
- System: ClickUp
- URL: https://app.clickup.com/t/86ewynyxu

## Summary

`cleanup_stale_test_containers()` in `cyanprint/src/test_cmd/template.rs` removes ALL containers labeled `cyanprint.test=true` or `cyanprint.dev=true` at the start of `run_template_tests()`. This causes two problems:

1. **Nested tests**: When a validate command invokes `cyan test template` recursively (e.g. from a plugin test that validates template output), the inner test's cleanup kills the outer test's container.
2. **No panic safety**: Plugin, processor, and resolver tests rely on explicit `cleanup_container()` calls but have no `Drop` guard — if the test panics before cleanup, containers leak.

Fix by applying run-scoped container ownership uniformly across ALL test commands (template, plugin, processor, resolver). Each test run labels its containers with a unique UUID and cleans only its own containers at the end via a shared `Drop` guard.

## Acceptance Criteria

- [ ] `cleanup_stale_test_containers()` is removed from `template.rs`
- [ ] A shared `RunGuard` struct (in `container.rs`) provides run-scoped `Drop` cleanup
- [ ] ALL test entry points generate a unique run UUID: `run_template_tests()`, `run_plugin_tests()`, `run_processor_tests()`, `run_resolver_tests()`
- [ ] ALL containers created during a test run are labeled with `cyanprint.test.run=<uuid>` — including warmup containers and per-test-case containers
- [ ] `RunGuard::drop()` cleans up all containers with the run's UUID on scope exit (success, error, or panic)
- [ ] Nested test runs (validate commands invoking `cyan test`) do not kill parent containers
- [ ] Concurrent test runs do not interfere with each other
- [ ] Existing test behavior is unchanged for non-nested runs
- [ ] Explicit `cleanup_container()` calls are kept as belt-and-suspenders with the `Drop` guard

## Out of Scope

- Changes to boron (`../boron/`) — boron manages its own `cyanprint.dev=true` containers and is not modified during test runs by iridium
- Changes to the template container lifecycle beyond labeling and cleanup
- Changes to snapshot comparison or validation logic
- Changes to `init.rs`'s `qa_warmup()` — init tests call `run_template_tests()` which handles its own `RunGuard`; init's `qa_warmup` template container is short-lived and cleaned by explicit `remove_container` call at end of `qa_warmup()`

## Constraints

- No new dependencies — use existing `uuid` and `bollard` crates already in the project
- The `Drop` guard must create its own tokio runtime (same pattern as the current `cleanup_stale_test_containers()`) since `drop()` is sync
- Container creation in both `try_cmd.rs:start_template_container` and `test_cmd/container.rs:build_and_start_container` must receive the run UUID
- `RunGuard` must be defined in `container.rs` (shared module) so all test types can use it

## Context

The user clarified that the fix should NOT clean other tests' containers at all — the cleanup function is fundamentally broken for any concurrent or nested scenario. The correct model is:

1. Label our own containers with a run UUID
2. Clean only our containers at the end
3. Don't touch containers belonging to other test runs

This handles nested tests, concurrent tests, and avoids the need for any "stale detection" logic.

**Why all test types need the RunGuard:**

- `cleanup_stale_test_containers()` kills containers from ANY test type (they all use `cyanprint.test=true` label via `build_and_start_container()`)
- Plugin/processor/resolver tests only have explicit `cleanup_container()` (by name) — no panic safety
- If any test type panics before explicit cleanup, its container leaks forever
- `RunGuard` provides uniform safety: every test run cleans up its own containers regardless of how it exits

## Edge Cases

- **Process kill (`kill -9`)**: `Drop` won't run, so containers with the run UUID will remain. This is acceptable — same as today when a test is force-killed.
- **Panic during test**: `Drop` guard handles this — containers are cleaned up even on panic.
- **Triple-nested tests**: Each level gets its own UUID. Parent containers are never touched.
- **Template container vs test containers**: Both `try_cmd.rs:start_template_container` (template warmup) and `test_cmd/container.rs` (processor/plugin/resolver per-test containers) must be labeled.
- **`init.rs` calls `run_template_tests()`**: The inner template test creates its own `RunGuard` with its own UUID. Init's template container (created by `qa_warmup`) is NOT labeled with the template test's UUID, so it's safe.

---

## Implementation Checklist

### Linting

- [ ] Run `direnv exec . cargo clippy` and fix all warnings

### Testing

- [ ] Verify existing `cyan test template` passes without regression
- [ ] Verify existing `cyan test plugin` passes without regression
- [ ] Verify nested test scenario (validate command invoking `cyan test`) works without killing parent containers

### Notes

- Commit convention: `[CU-86ewynyxu] <description>` (from existing git log pattern)
- The run UUID should be generated once per test entry point and passed down to container creation functions
