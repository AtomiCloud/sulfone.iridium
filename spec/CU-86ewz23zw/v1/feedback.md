# Feedback for v1

**Date:** 2026-03-18
**PR:** #82
**Ticket Status:** todo
**Version:** 1

## Feedback Items

### 1. E2E tests only build/push — no actual test execution

**Observation:** The current `e2e.sh` only builds and pushes artifacts to the local registry. It does not run any `cyanprint test`, `cyanprint try`, `cyanprint test init`, or `cyanprint create` commands.

**Impact:** The e2e suite doesn't validate that the cyanprint commands actually work end-to-end. The port race condition fix (v1) improved the underlying mechanism, but there are no integration-level tests exercising the full command surface.

**Suggested Change:** Expand e2e to cover all command types: test, try, test init, and create.

### 2. No try command coverage

**Observation:** `cyanprint try template` and `cyanprint try group` are not tested at all.

**Impact:** The try flow (build locally, warm container, Q&A, execute) is the primary developer workflow. No tests mean regressions here would only be caught manually.

**Suggested Change:** Add try tests for template2 (with expect), template5 (with expect), and template4 group (no expect needed).

### 3. No create command coverage

**Observation:** `cyanprint create` is not tested end-to-end.

**Impact:** The create flow is what users actually run. Untested.

**Suggested Change:** Add create tests for a complex template (template2 with expect) and a group (template4, no expect).

### 4. No nested template testing

**Observation:** There are no tests for templates that generate other templates.

**Impact:** Nested/recursive templating is a real use case. If a template generates another template that's broken, there's no way to catch it.

**Suggested Change:** Create template6 — a template whose output is a cyanprint template (template7). Test with 5 cases and high parallelism.

### 5. No test init coverage

**Observation:** `cyanprint test init` is not tested.

**Impact:** Test init is the onboarding path for new templates. If it's broken, users can't easily get started.

**Suggested Change:** Add test init test with expect wrapper for template5.

### 6. e2e.sh should be split into independent phases

**Observation:** Current e2e.sh only has build logic. After building once, you can't run just tests without rebuilding.

**Impact:** Slow iteration. Every e2e run rebuilds everything even if only testing changed.

**Suggested Change:** Split into build/local/full phases that can run independently.

## Summary

V1 successfully fixed the port allocation race condition. V2 should expand the e2e suite to provide comprehensive coverage of all cyanprint commands: test, try, init, and create. Use expect scripts for interactive commands (inquire-based). Create a nested template (template6) to test recursive templating. Split e2e.sh into independent build/local/full phases for faster iteration.
