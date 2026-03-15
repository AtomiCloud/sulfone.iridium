# Task Spec: [Ir] Test Command (CU-86ewvp51k)

## Summary

Implement `cyanprint test <type> <path> [options]` — an automated testing command for CyanPrint templates, processors, plugins, and resolvers. Each repo tests ONE artifact type. Tests are defined in `test.cyan.yaml`, executed against a running artifact container, and validated via snapshot comparison.

Also implement `cyanprint test init <path>` — a template-only subcommand that auto-generates test cases by walking the Q&A tree, enumerating answer combinations, running each through execution, and saving initial snapshots.

## CLI Interface

### `cyanprint test <type> <path> [options]` — Run Tests

```
cyanprint test <type> <path> [options]
```

**Positional arguments:**

- `type` — one of: `template`, `processor`, `plugin`, `resolver`
- `path` — working directory containing `cyan.yaml` and `test.cyan.yaml`

**Options:**

- `--test <name>` — run a specific test case by name
- `--parallel <N>` — run N tests concurrently (default: 1, sequential)
- `--update-snapshots` — copy test outputs to expected directories
- `--output <path>` — output root directory (default: `./tmp`)
- `--config <path>` — test config file (default: `test.cyan.yaml`)
- `--junit <path>` — write JUnit XML report to file (for CI/CD integration)
- `--coordinator-endpoint` / `-c` — Boron endpoint (default: `http://coord.cyanprint.dev:9000`)
- `--disable-daemon-autostart` — skip automatic daemon start

### `cyanprint test init <path> [options]` — Generate Test Cases (Templates Only)

```
cyanprint test init <path> [options]
```

**Positional arguments:**

- `path` — working directory containing `cyan.yaml`

**Options:**

- `--max-combinations <N>` — cap on total generated test cases (default: 30)
- `--output <path>` — output root directory for snapshots (default: `./tmp`)
- `--config <path>` — output test config file (default: `test.cyan.yaml`)
- `--text-seed <value>` — default value for Text questions (default: `"dummy"`)
- `--password-seed <value>` — default value for Password questions (default: `"secret"`)
- `--date-seed <value>` — default value for Date questions (default: today's date in YYYY-MM-DD)
- `--coordinator-endpoint` / `-c` — Boron endpoint
- `--disable-daemon-autostart` — skip automatic daemon start

## Config: `test.cyan.yaml`

```yaml
tests:
  # Template test case
  - name: standard-typescript-yes
    expected: ./fixtures/expected/standard-typescript-yes
    answer_state:
      project_name: { type: String, value: test-project }
      template_type: { type: String, value: standard }
      use_typescript: { type: Bool, value: true }
    deterministic_state:
      seed: '12345'
    validate:
      - nix flake check
      - nix build .

  # Processor test case
  - name: typescript-transform
    input: ./fixtures/input/ts-files
    expected: ./fixtures/expected/ts-output
    globs:
      - { pattern: '**/*.ts', type: Template }
      - { pattern: '**/*.json', type: Copy }
    config:
      vars:
        project_name: my-project

  # Plugin test case
  - name: prettier-format
    input: ./fixtures/input/unformatted
    expected: ./fixtures/expected/formatted
    config:
      formatter: prettier

  # Resolver test case
  - name: json-deep-merge
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
      - path: package.json
        content: '{"name": "project", "dependencies": {}, "devDependencies": {}}'
```

### Config Parsing Rules

- `name` — required for all test types, must be unique
- `expected` — required, path to expected output directory (template/processor/plugin) or inline expected results (resolver)
- `answer_state` — template tests only, maps question IDs to `Answer` values (matches `cyanprompt::domain::models::answer::Answer` enum)
- `deterministic_state` — template tests only, maps state keys to string values
- `validate` — optional list of shell commands to run in the output directory (all must exit 0)
- `input` — processor/plugin tests: path to input directory; resolver tests: structured input object
- `globs` — processor tests only: file patterns and types for the processor
- `config` — processor/plugin tests: config object passed to the artifact's API

## Architecture: Four Test Flows

### Constraint: One Repo = One Type

Each repo is either a template, processor, plugin, or resolver. The `<type>` argument determines the test flow. All test cases in `test.cyan.yaml` must be for that same type.

### Flow 1: Template Tests (`cyanprint test template <path>`)

**Warm-up phase (once):**

1. Pre-flight: Docker daemon check, `cyan.yaml` validation, `test.cyan.yaml` validation
2. Read and parse `cyan.yaml` (build config) and `test.cyan.yaml`
3. Generate `local_template_id` (once for all tests)
4. Resolve and pin dependencies from registry
5. Build images (blob + template) using buildx — reuse existing `build_image()` from `try_cmd.rs`
6. Build synthetic template — reuse existing `build_synthetic_template()` from `try_cmd.rs`
7. Allocate port and start template container — reuse existing `start_template_container()` from `try_cmd.rs`
8. Health check template container

**Per test case (parallelizable via semaphore):**

1. Generate unique `session_id` and `merger_id`
2. Call `coord_client.try_setup()` with same `local_template_id` (reuses blob volume) but unique session
3. Run Q&A with pre-supplied `answer_state` and `deterministic_state` — use `TemplateEngine.start_with(Some(answers), Some(states))`. Since all answers are pre-supplied, the template server should resolve to `Final` without interactive prompts
4. Call `coord_client.bootstrap()` with the new session
5. Call `execute_and_stream_output()` — `POST /executor/{session_id}`, unpack tar.gz to `{output}/{test_name}/`
6. Call `coord_client.try_cleanup(session_id)` — session cleanup only (NOT container/blob cleanup)

**After ALL test cases complete:**

1. Run validation commands (if any) for each test case in its output directory
2. Snapshot comparison: compare each `{output}/{test_name}/` against its `expected` directory

**Final cleanup (once):**

1. Stop and remove template container
2. Remove built images
3. Remove blob volume (via Boron cleanup or Docker volume rm)
4. Remove `{output}/` tmp directory

### Flow 2: Processor Tests (`cyanprint test processor <path>`)

**Warm-up phase (once):**

1. Pre-flight: Docker daemon check, `cyan.yaml` validation, `test.cyan.yaml` validation
2. Build processor Docker image from `cyan.yaml` build config
3. Collect all test input directories and the output root directory
4. Start processor container with bind mounts:
   - All test input dirs → `/workspace/cyanprint/{test_name}/` (read-only)
   - Output root → `/workspace/area/` (read-write)
5. Health check processor endpoint (`GET /` on port 5551)

**Per test case (parallelizable):**

1. `POST /api/process` to the processor container with:
   - `readDir: "/workspace/cyanprint/{test_name}"`
   - `writeDir: "/workspace/area/{test_name}"`
   - `globs` and `config` from test case
2. Output lands in `{output}/{test_name}/` on host (bind-mounted from `/workspace/area/{test_name}`)

**After ALL test cases complete:**

1. Run validation commands (if any) for each test case in its output directory
2. Snapshot comparison: compare each `{output}/{test_name}/` against its `expected` directory

**Final cleanup (once):**

1. Stop and remove processor container
2. Remove built image
3. Remove `{output}/` tmp directory

### Flow 3: Plugin Tests (`cyanprint test plugin <path>`)

**Warm-up phase (once):**

1. Pre-flight: Docker daemon check, `cyan.yaml` validation, `test.cyan.yaml` validation
2. Build plugin Docker image from `cyan.yaml` build config
3. Collect all test input directories and the output root directory
4. For each test case: copy `input` → `{output}/{test_name}/` (plugins modify in-place)
5. Start plugin container with bind mount:
   - Output root → `/workspace/area/` (read-write)
6. Health check plugin endpoint (`GET /` on port 5552)

**Per test case (parallelizable):**

1. `POST /api/plug` to the plugin container with:
   - `directory: "/workspace/area/{test_name}"`
   - `config` from test case
2. Output lands in `{output}/{test_name}/` on host (modified in-place)

**After ALL test cases complete:**

1. Run validation commands (if any) for each test case in its output directory
2. Snapshot comparison: compare each `{output}/{test_name}/` against its `expected` directory

**Final cleanup (once):**

1. Stop and remove plugin container
2. Remove built image
3. Remove `{output}/` tmp directory

### Flow 4: Resolver Tests (`cyanprint test resolver <path>`)

**Warm-up phase (once):**

1. Pre-flight: Docker daemon check, `cyan.yaml` validation, `test.cyan.yaml` validation
2. Build resolver Docker image from `cyan.yaml` build config
3. Start resolver container on dynamically allocated port
4. Health check: `GET /` returns 200 (port 5553)

**Per test case (parallelizable):**

1. `POST /api/resolve` with test `input` (config + files)
2. Compare response against `expected` (array of `{path, content}` pairs)
3. JSON deep comparison (field order doesn't matter)

**Final cleanup (once):**

1. Stop and remove resolver container
2. Remove built image
3. Remove `{output}/` tmp directory: Auto-Generate Template Test Cases

### Overview

`cyanprint test init <path>` walks the Q&A tree of a template to discover all possible answer paths, generates a test case for each unique combination, runs them to produce initial snapshots, and writes the `test.cyan.yaml` config.

### Algorithm

1. **Warm-up**: same as template test warm-up (build, start container, etc.)

2. **Tree exploration**:

   - Start Q&A with empty answers: `TemplateEngine.start_with(None, None)`
   - At each question, inspect the question type:

   | Question Type | Strategy                                                                                  |
   | ------------- | ----------------------------------------------------------------------------------------- |
   | `Text`        | Use `--text-seed` value (default: `"dummy"`) — same value for all branches                |
   | `Password`    | Use `--password-seed` value (default: `"secret"`) — same value for all branches           |
   | `Select`      | Enumerate all options → one branch per option                                             |
   | `Confirm`     | Two branches: `true` and `false`                                                          |
   | `Checkbox`    | Enumerate: empty selection + each individual option + all selected (avoid full power set) |
   | `Date`        | Use `--date-seed` value (default: today YYYY-MM-DD) — same value for all branches         |

   - For branching question types (Select, Confirm, Checkbox): fork the Q&A state and explore each branch independently
   - Continue until `TemplateState::Complete` or combination cap reached

3. **Combination cap**: stop exploring new branches once `--max-combinations` (default 30) test cases have been generated. Complete any in-progress branches but don't start new forks.

4. **Name generation**: concatenate answer values with `-` separator, sanitized for filesystem:

   - `standard-typescript-true` (from Select:standard → Select:typescript → Confirm:true)
   - Truncate if too long (max 80 chars)

5. **Per combination**: run the full template execution (try_setup → bootstrap → execute → unpack) to generate the initial snapshot output

6. **Write outputs**:

   - `test.cyan.yaml` with all generated test cases (answer_state, deterministic_state, expected path)
   - `fixtures/expected/{test_name}/` directories with snapshot output from each combination

7. **Cleanup**: remove `{output}/` tmp directory after copying snapshots to `fixtures/expected/`

### Exploration Implementation

The tree exploration requires calling the template server's Q&A endpoint repeatedly with different answer combinations. The key insight is that `TemplateEngine.start_with()` sends answers to the template server, which returns either another question or `Final`. By forking at each branching question:

1. Save current state (answers + deterministic_state so far)
2. For each possible answer value:
   a. Clone saved state
   b. Add this answer
   c. Call template server with updated state
   d. If more questions → recurse
   e. If `Final` → record as a complete test case

This is a depth-first tree walk with the combination cap acting as a global counter.

## Validation Engine

### Validate Commands

For each test case with a `validate` list:

```
cd {output}/{test_name}/
{command}
# Must return exit code 0
```

If any command fails: mark test as FAIL, capture stderr, continue to next test.

### Snapshot Comparison

Compare `{output}/{test_name}/` against `expected` directory:

| Rule                    | Behavior                                         |
| ----------------------- | ------------------------------------------------ |
| Extra files in output   | FAIL                                             |
| Missing files in output | FAIL                                             |
| File content mismatch   | FAIL with diff                                   |
| JSON files (`.json`)    | Deep comparison (field order doesn't matter)     |
| All other files         | Exact string match (trimmed trailing whitespace) |

### `--update-snapshots`

When set: after each test case, copy `{output}/{test_name}/` to the `expected` directory, replacing existing contents. Skip validation comparison (always PASS).

## Output Formats

### Human-readable report (stdout)

```
================================================================================
CYANPRINT TEST RESULTS
================================================================================

basic-nix-project .................................................... PASS
typescript-transform ................................................ PASS
prettier-format ..................................................... FAIL

--------------------------------------------------------------------------------
FAILED: prettier-format
--------------------------------------------------------------------------------
Snapshot mismatch:
  Missing: src/index.ts
  Extra:   src/index.tsx

Content mismatch in package.json:
  - expected: "name": "my-project"
  + actual:   "name": "project"

================================================================================
SUMMARY: 2 passed, 1 failed
================================================================================
```

### JUnit XML (`--junit <path>`)

Standard JUnit XML format for CI/CD integration:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="cyanprint" tests="3" failures="1" time="12.5">
  <testsuite name="template" tests="3" failures="1" time="12.5">
    <testcase name="basic-nix-project" time="4.2"/>
    <testcase name="typescript-transform" time="3.1"/>
    <testcase name="prettier-format" time="5.2">
      <failure message="Snapshot mismatch">
Missing: src/index.ts
Extra:   src/index.tsx
      </failure>
    </testcase>
  </testsuite>
</testsuites>
```

Exit code: 0 if all pass, 1 if any fail.

## Parallelism

Use `tokio::sync::Semaphore` with `--parallel N` to limit concurrent test cases. Each test case gets a unique `session_id`. Default is 1 (sequential).

For template tests, the template container, blob volume, and pinned dependencies are shared across all concurrent test cases. Each test case only creates its own session.

For processor/plugin/resolver tests, the single container is shared. Each test case operates on its own subdirectory within the mounted volumes, so parallel execution is safe.

## Error Handling

| Error                          | Behavior                         |
| ------------------------------ | -------------------------------- |
| Docker not running             | Exit immediately with error      |
| `cyan.yaml` not found          | Exit immediately with error      |
| `test.cyan.yaml` not found     | Exit immediately with error      |
| Build failed                   | Exit immediately with error      |
| Container health check timeout | Exit immediately with error      |
| Test validation command fails  | Mark FAIL, continue to next test |
| Snapshot mismatch              | Mark FAIL, show diff, continue   |
| Session cleanup fails          | Log warning, continue            |

## Acceptance Criteria

1. `cyanprint test template .` — runs all template tests in `test.cyan.yaml`
2. `cyanprint test processor .` — runs all processor tests
3. `cyanprint test plugin .` — runs all plugin tests
4. `cyanprint test resolver .` — runs all resolver tests
5. `cyanprint test template . --parallel 4` — runs up to 4 tests concurrently
6. `cyanprint test template . --update-snapshots` — copies outputs to expected dirs
7. `cyanprint test template . --test basic-nix-project` — runs only that test
8. `cyanprint test template . --junit results.xml` — writes JUnit XML report
9. Validation commands that fail cause test to be marked FAIL
10. Snapshot mismatches show clear diffs
11. Exit code 0 on all pass, 1 on any failure
12. Template tests reuse a single warm-up (build once, execute many)
13. Session-only cleanup between test cases (container/blob persist across tests)
14. Pre-supplied `answer_state` and `deterministic_state` drive Q&A non-interactively
15. Processor/plugin containers pre-mount all test I/O directories at startup
16. Snapshot comparison happens after ALL test cases complete (not per-case)
17. `{output}/` tmp directory cleaned up after test run and after init
18. `cyanprint test init .` — walks Q&A tree, generates up to 30 test cases with snapshots
19. `cyanprint test init . --max-combinations 50` — adjustable cap
20. `test init` uses default seeds: Text→`"dummy"`, Password→`"secret"`, Date→today
21. `test init` seeds overridable via `--text-seed`, `--password-seed`, `--date-seed`

## Constraints

- Reuse `try_cmd.rs` functions where possible: `pre_flight_validation`, `build_image`, `build_synthetic_template`, `resolve_and_pin_dependencies`, `start_template_container`, `health_check_template_container`, `execute_and_stream_output`, `find_available_port`
- Follow existing clap `Commands` enum pattern for CLI registration
- Follow existing error handling pattern: `Result<(), Box<dyn Error + Send>>`
- New modules go under `cyanprint/src/` (e.g., `test_cmd.rs` or `test_cmd/` module directory)
- Test config types go in `cyanregistry/src/cli/models/` (following `build_config.rs` pattern)

## Out of Scope

- `--dev` mode for tests (not needed; build once and reuse)
- Interactive Q&A during test runs (all answers must be pre-supplied)
- Running mixed artifact types in a single test run
- Group template tests (can be added later)
- `test init` for non-template types (processors/plugins/resolvers don't have Q&A trees)
