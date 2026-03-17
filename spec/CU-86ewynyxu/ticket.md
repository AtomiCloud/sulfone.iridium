# Ticket: CU-86ewynyxu

- **Type**: Bug
- **Status**: todo (unstarted)
- **URL**: https://app.clickup.com/t/86ewynyxu
- **Parent**: none

## Description

Bug

When running cyan test with validate commands that invoke a nested cyan test (e.g. meta template tests where the output is a cyanprint template), the inner test's cleanup_stale_test_containers() kills the outer test's template container.

Root Cause

cleanup_stale_test_containers() in cyanprint/src/test_cmd/template.rs:989-1030 removes ALL Docker containers labeled cyanprint.test=true or cyanprint.dev=true. It doesn't distinguish between containers from previous runs vs containers belonging to the current process.

When a validate command runs cyanprint test template -c http://localhost:9000 . inside the test output directory:

The inner cyan test calls cleanup_stale_test_containers() at startup
This kills the outer test's template container (which also has cyanprint.test=true)
Any subsequent outer test cases fail with connection errors to the dead template container

Reproduction

cyan test template -c http://localhost:9000

# in a test config where validate runs: cyanprint test template -c http://localhost:9000 .

Suggested Fix

Make cleanup_stale_test_containers() aware of containers created by the current run. Options:

Record container IDs before warmup — store the container IDs created in this run, and only clean containers NOT in that set
Use a run-specific label — tag containers with cyanprint.test.run=<uuid> and only clean containers without that label
Skip cleanup when nested — detect nested execution (env var or CLI flag) and skip stale cleanup

Option 2 is the most robust.

## Comments

No comments.
