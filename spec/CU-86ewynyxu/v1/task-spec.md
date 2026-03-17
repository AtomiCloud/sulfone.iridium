# Task Specification: Fix nested cyan test killing parent template container (CU-86ewynyxu)

## Source

- Ticket: CU-86ewynyxu
- System: ClickUp
- URL: https://app.clickup.com/t/86ewynyxu

## Summary

`cleanup_stale_test_containers()` in `cyanprint/src/test_cmd/template.rs` removes ALL containers labeled `cyanprint.test=true` or `cyanprint.dev=true` at the start of `run_template_tests()`. When a validate command invokes `cyan test template` recursively (nested test), the inner test's cleanup kills the outer test's template container, causing subsequent outer test cases to fail with connection errors.

Fix by replacing the blanket startup cleanup with run-scoped container ownership: each test run labels its containers with a unique UUID and cleans only its own containers at the end via a `Drop` guard.

## Acceptance Criteria

- [ ] No startup cleanup — `cleanup_stale_test_containers()` is removed
- [ ] Each `run_template_tests()` invocation generates a unique run UUID
- [ ] All containers created during a test run are labeled with `cyanprint.test.run=<uuid>`
- [ ] A `Drop` guard cleans up all containers with the run's UUID on scope exit (success, error, or panic)
- [ ] Nested test runs (validate commands invoking `cyan test template`) do not kill parent containers
- [ ] Concurrent test runs do not interfere with each other
- [ ] Existing test behavior is unchanged for non-nested runs

## Out of Scope

- Changes to boron (`../boron/`) — boron manages its own `cyanprint.dev=true` containers and is not modified during test runs by iridium
- Changes to the template container lifecycle beyond labeling and cleanup
- Changes to snapshot comparison or validation logic

## Constraints

- No new dependencies — use existing `uuid` and `bollard` crates already in the project
- The `Drop` guard must work within the async tokio runtime already used by `run_template_tests()`
- Container creation in both `try_cmd.rs:start_template_container` and `test_cmd/container.rs` must receive the run UUID

## Context

The user clarified that the fix should NOT clean other tests' containers at all — the cleanup function is fundamentally broken for any concurrent or nested scenario. The correct model is:

1. Label our own containers with a run UUID
2. Clean only our containers at the end
3. Don't touch containers belonging to other test runs

This handles nested tests, concurrent tests, and avoids the need for any "stale detection" logic.

## Edge Cases

- **Process kill (`kill -9`)**: `Drop` won't run, so containers with the run UUID will remain. This is acceptable — same as today when a test is force-killed.
- **Panic during test**: `Drop` guard handles this — containers are cleaned up even on panic.
- **Triple-nested tests**: Each level gets its own UUID (or inherits from parent if env var propagation is used). Parent containers are never touched.
- **Template container vs test containers**: Both `try_cmd.rs:start_template_container` (template warmup) and `test_cmd/container.rs` (processor/plugin/resolver per-test containers) must be labeled.

---

## Implementation Checklist

### Linting

- [ ] Run `direnv exec . cargo clippy` and fix all warnings

### Testing

- [ ] Verify existing `cyan test template` passes without regression
- [ ] Verify nested test scenario (validate command invoking `cyan test template`) works without killing parent containers

### Notes

- Commit convention: `[CU-86ewynyxu] <description>` (from existing git log pattern)
- The run UUID should be generated once in `run_template_tests()` and passed down to container creation functions
