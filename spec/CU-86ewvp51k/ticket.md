# Ticket: CU-86ewvp51k

- **Type**: Task
- **Status**: todo
- **URL**: https://app.clickup.com/t/86ewvp51k
- **Parent**: CU-86et8z88g

## Title

[Ir] Test Command

## Description

Overview

Implement cyanprint test command for automated testing with snapshot validation.

Key Insight: Test = try_function × N + validation

### Scope

#### 1. Config Parsing: test.cyan.yaml

```yaml
tests:
  # Template test
  - name: basic-nix-project
    expected: ./fixtures/expected/basic-nix-project
    answer_state:
      project_name: { type: String, value: test-project }
      template_type: { type: String, value: standard }
    ideterminism_state: { seed: 12345 }
    validate:
      - nix flake check
      - nix build .

  # Processor test
  - name: typescript-transform
    input: ./fixtures/input/ts-files
    expected: ./fixtures/expected/ts-output
    globs:
      - { pattern: '**/*.ts', type: Template }
      - { pattern: '**/*.json', type: Copy }
    config:
      rename_rules:
        - { from: '.template', to: '' }

  # Plugin test
  - name: prettier-format
    input: ./fixtures/input/unformatted
    expected: ./fixtures/expected/formatted
    config:
      formatter: prettier
      rules: { semi: true, singleQuote: true }

  # Resolver test
  - name: json-deep-merge
    type: resolver
    resolver: atomi/json-merge-resolver:v1.0.0
    input:
      config: { strategy: deep-merge }
      files:
        - path: package.json
          content: '{"name": "project", "dependencies": {}}'
          origin: { template: 'atomi/frontend:5', layer: 4 }
        - path: package.json
          content: '{"name": "project", "devDependencies": {}}'
          origin: { template: 'atomi/backend:3', layer: 3 }
    expected:
      path: package.json
      content: '{"name": "project", "dependencies": {}, "devDependencies": {}}'
```

#### 2. Command: cyanprint test

```
Usage:
cyanprint test <type> <path> [options]

# Options
--update-snapshots    Copy tmp outputs to expected (update test cases)
--parallel N          Run N tests in parallel (default: 1, sequential)
--output <path>       Output directory (default: ./tmp)
--type <type>         Run only specific test type (template, processor, plugin, resolver)
--test <name>         Run specific test by name
```

#### 3. Test Flow

- SETUP (once, shared):

  1. Pre-checks: Verify Docker daemon, Load cyan.yaml, Load test.cyan.yaml
  2. Resolve & Pin Dependencies (once)
  3. Build Images (once) - if not --dev

- FOR EACH TEST CASE (parallel if --parallel):

  - Template/Processor/Plugin: Call try_function(test_case)
  - Resolver: Pull image, start container, POST /api/resolve, compare response

- Validation Engine:
  - Run validate commands (all must return 0)
  - Compare output vs expected directories

#### 4. The try_function

Internal function, not CLI. Reuses logic from cyanprint try.

```rust
async fn try_function(
    config: &TryConfig,
    test_case: &TestCase,
    output_path: &Path,
) -> Result<TryOutput> {
    // Same as try, but:
    // - Uses answer_state instead of user input
    // - Uses ideterminism_state for reproducibility
    // - Outputs to specified path
    // - Returns result for validation
}
```

#### 5. Automated Q&A

Match questions from answer_state by: ID match, question text match, pattern match, order match.

#### 6. Validation Engine

- Run validate commands in output dir (all must return 0)
- Snapshot comparison: all files must exist in both dirs, contents must match
- JSON: Deep comparison (field order doesn't matter)
- Text: Exact string match (trimmed)

#### 7. Resolver Test Flow

1. Pull resolver image from registry
2. Start resolver container on port 5553
3. Wait for health check
4. POST /api/resolve with test input
5. Compare response to expected
6. Cleanup resolver container

#### 8. Parallelism

Semaphore-based parallelism with configurable concurrency. Each test gets unique session_id.

### Acceptance Criteria

1. `cyanprint test` - Runs all tests in test.cyan.yaml, reports pass/fail
2. `cyanprint test --parallel 4` - Runs up to 4 tests concurrently
3. `cyanprint test --update-snapshots` - Copies outputs to expected
4. `cyanprint test --type resolver` - Runs only resolver tests
5. `cyanprint test --test basic-nix-project` - Runs only that test
6. Validation commands work (fail → test FAIL)
7. Snapshot comparison shows diff on mismatch

### Error Handling

| Error                    | Behavior                  |
| ------------------------ | ------------------------- |
| Docker not running       | Exit immediately          |
| test.cyan.yaml not found | Exit immediately          |
| Build failed             | Exit immediately          |
| Validate command fails   | Mark FAIL, continue       |
| Snapshot mismatch        | Mark FAIL, show diff      |
| Resolver image not found | Mark FAIL, continue       |
| Container timeout        | Kill, mark FAIL, continue |

### Files to Create/Modify

- src/commands/test.rs - Test command implementation
- src/config/test.rs - Parse test.cyan.yaml
- src/test/runner.rs - Test runner (parallel execution)
- src/test/validation.rs - Validation engine
- src/test/snapshot.rs - Snapshot comparison
- src/test/resolver.rs - Resolver test execution

## Comments

(none)

---

# Parent: CU-86et8z88g (Task)

- **Title**: [Ir, B] Allow for local-testing
- **Status**: in progress
- **URL**: https://app.clickup.com/t/86et8z88g

## Description

Local Testing Strategy for CyanPrint

### Purpose

Enable local development and testing of CyanPrint templates, processors, and plugins without requiring full production deployment.

### Problem Statement

Currently, testing a CyanPrint template requires building images, pushing to registry, and deploying to production infrastructure. This is slow, requires network access, and creates friction in the development loop.

### Solution

A coordinated effort between Iridium (CLI) and Boron (Executor) to support local testing:

- Iridium handles: Config parsing, dependency resolution, image building, interactive Q&A, test orchestration
- Boron handles: Volume management, container execution, resolver support

### Commands Delivered

```bash
# Build images (CI/CD)
cyanprint build v1.0.0

# Local testing (interactive)
cyanprint try . ./output
cyanprint try . ./output --dev      # Dev mode: external template server

# Automated testing
cyanprint test
cyanprint test --update-snapshots
cyanprint test --parallel 4

# Publish
cyanprint push --build v1.0.0
```

### Subtasks

1. [B] Executor Try Endpoint (review)
2. [Ir] Build + Push Commands (done)
3. [Ir] Try Command (done)
4. [Ir] Test Command (todo) ← **this ticket**

### Critical Path

boron-1 (Executor Try) → iridium-2 (Try) → iridium-3 (Test)

### Architecture

- Iridium (CLI): Pre-flight checks, dependency resolution, image building, Q&A, Boron API calls, validation
- Boron (Executor): Blob setup, session volumes, dependency warming, processor/merger/plugin execution, resolver proxy
