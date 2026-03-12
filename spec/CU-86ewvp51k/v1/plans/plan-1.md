# Plan 1: Template Tests — End-to-End

## Goal

Deliver a fully working `cyanprint test template <path>` command. This includes all shared infrastructure (CLI, config parsing, validation engine, reports) plus the complete template test execution flow. At the end of this plan, users can run template tests, see results, get JUnit output, and update snapshots.

## Verifiable Outcome

`cyanprint test template .` runs all template tests in `test.cyan.yaml`, compares outputs against expected snapshots, prints results, and exits with appropriate code.

## Dependencies

None — this is the first plan.

## Documentation Requirements

All public types, functions, and modules created in this plan must include Rust doc comments (`///`). Specifically:

- `test_cmd/mod.rs` — module-level docs (`//!`) explaining the test command subsystem
- `config.rs` — doc comments on `TestConfig`, `TestCase`, all field-level docs explaining what each field does and which test type uses it
- `validation.rs` — doc comments on `ComparisonResult`, `compare_directories`, `run_validate_commands`, explaining snapshot comparison rules (JSON deep compare, trimmed string compare)
- `report.rs` — doc comments on `TestResult`, report formatters
- `template.rs` — doc comments on the template test flow entry point
- CLI help text — all `#[arg(...)]` and `#[command(...)]` annotations must include `help = "..."` strings, following the existing pattern in `commands.rs` (see `TryCommands`, `DaemonCommands`)

## Files to Create

### `cyanprint/src/test_cmd/mod.rs`

Module root for the test command. Declare sub-modules:

- `config` — test.cyan.yaml parsing
- `validation` — validate commands + snapshot comparison
- `report` — human-readable and JUnit XML output
- `template` — template test execution flow
- `init` — test init logic (skeleton with `todo!()`, filled in Plan 3)

### `cyanprint/src/test_cmd/config.rs`

Parse `test.cyan.yaml` into strongly-typed Rust structs using `serde_yaml`.

Key types:

- `TestConfig` — top-level with `tests: Vec<TestCase>`
- `TestCase` — struct with name, expected, and optional type-specific fields (answer_state, deterministic_state, validate, input, globs, config)
- `AnswerStateEntry` — `{ type: String, value: serde_yaml::Value }` mapping to `cyanprompt::domain::models::answer::Answer`
- `GlobEntry` — `{ pattern: String, type: String }`
- `ResolverInput` / `ResolverExpected` — structured types for resolver tests (defined now, used in Plan 2)

Follow the pattern in `cyanregistry/src/cli/models/build_config.rs` for serde derives and validation. Add a `read_test_config(path: String) -> Result<TestConfig, ...>` function mirroring `read_build_config`.

**Important**: The `config` field in test cases (for processors/plugins) is YAML in `test.cyan.yaml` but must be converted to `serde_json::Value` for the HTTP API calls. Use `serde_json::Value` as the Rust type (serde_yaml can deserialize into serde_json::Value).

### `cyanprint/src/test_cmd/validation.rs`

Two components:

1. **Validate commands runner**: given an output directory and a list of shell commands, run each via `std::process::Command`, capture stdout/stderr, return pass/fail per command.

2. **Snapshot comparison**: given two directory paths (output vs expected):
   - Walk both directory trees, collect relative paths
   - Report missing/extra files
   - For `.json` files: parse both, compare with `serde_json::Value` equality (handles field order)
   - For all other files: compare trimmed string content
   - Return a structured `ComparisonResult` with file-level details

### `cyanprint/src/test_cmd/report.rs`

Two output formatters:

1. **Human-readable** (stdout): the dotted-line report format from the spec. Takes a `Vec<TestResult>` and prints the summary.

2. **JUnit XML**: standard format. Write to file path from `--junit`. Use string formatting (no XML library needed for this simple structure).

Shared type: `TestResult { name: String, passed: bool, duration: Duration, failure_message: Option<String> }`

### `cyanprint/src/test_cmd/template.rs`

Complete template test execution flow.

**Critical implementation note about `TemplateEngine.start_with()`**: This method has a hardcoded interactive prompt chain: `prompt_mapper()` (free function creating `inquire` widgets) → `prompt()` (free function calling `.prompt_skippable()`). Neither function is behind a trait — there's no way to inject a non-interactive implementation. If the server returns `QnA` even once, it will try to prompt interactively.

**Required approach**: Call `CyanHttpRepo.prompt_template()` directly (bypass `TemplateEngine` entirely). Replicate the engine's simple loop:

1. Build `TemplateAnswerInput { answers, deterministic_state }` with pre-supplied values
2. Call `repo.prompt_template(input)`
3. If `TemplateOutput::Final(c)` → done, proceed with `c.cyan`
4. If `TemplateOutput::QnA(q)` → look up `q.question.id()` in pre-supplied answers map:
   - Found → add to answers, update `deterministic_state` from `q.deterministic_state`, loop back to step 1
   - Not found → fail the test with "Missing answer for question '{id}'"
5. Construct `CyanHttpRepo` exactly as `run_qa_loop()` does in `try_cmd.rs:840-861` (uses `CyanClient` with `reqwest::blocking::Client`)

**Warm-up (reuse from `try_cmd.rs`):**

- `pre_flight_validation()` — Docker + cyan.yaml check (note: pass `dev_mode=false` since tests always build)
- `resolve_and_pin_dependencies()` — pin deps from registry
- `build_image()` — build blob + template images
- `build_synthetic_template()` — create synthetic template object
- `find_available_port()` + `start_template_container()` — start template
- `health_check_template_container()` — wait for ready

**Per test case (semaphore-gated):**

- Generate session_id, merger_id
- `coord_client.try_setup()` with shared `local_template_id`
- Non-interactive Q&A loop (see approach above): convert `answer_state` to `HashMap<String, Answer>` and `deterministic_state` to `HashMap<String, String>`, drive the template server until `Final`
- `coord_client.bootstrap()` with session
- `POST /executor/{session_id}` → unpack tar.gz to `{output}/{test_name}/` (reuse `execute_and_stream_output` logic, but note it also calls `verify_qa_applied_to_configs` which may need to be optional for tests)
- `coord_client.try_cleanup(session_id)` — session only

**After all complete:** validation + snapshot comparison (using validation engine)

**Cleanup:** stop container, remove images, remove blob, remove tmp

### `cyanprint/src/test_cmd/init.rs` (skeleton)

Define the `run_init` function signature that Plan 3 will implement:

- Takes path, CLI options (seeds, max-combinations)
- Returns `Result<(), ...>`
- Stub with `todo!()`

## Files to Modify

### `cyanprint/src/commands.rs`

Add `Test` variant to `Commands` enum with subcommands:

- `TestCommands::Template { path, test, parallel, update_snapshots, output, config, junit, coordinator_endpoint, disable_daemon_autostart }`
- `TestCommands::Processor { path, ... }` (stub, same shared options)
- `TestCommands::Plugin { path, ... }` (stub)
- `TestCommands::Resolver { path, ... }` (stub)
- `TestCommands::Init { path, max_combinations, text_seed, password_seed, date_seed, output, config, coordinator_endpoint, disable_daemon_autostart }`

Follow the existing `TryCommands` subcommand pattern. Include `help = "..."` on all args.

**CLI alias**: Use `#[command(alias = "te", about = "Run automated tests against CyanPrint artifacts")]` for the Test command (following the alias pattern of other commands).

### `cyanprint/src/main.rs`

- Add `pub mod test_cmd;`
- Add match arm for `Commands::Test { command }` dispatching to:
  - `TestCommands::Template` → full template test flow
  - `TestCommands::Processor/Plugin/Resolver` → `todo!()` with message "Not yet implemented, coming in next plan"
  - `TestCommands::Init` → `todo!()` stub
- Wire up: parse config → filter by `--test` → run template tests → validation + comparison → `--update-snapshots` handling → report → JUnit → cleanup tmp → exit code

### `cyanprint/src/try_cmd.rs`

Make shared functions accessible to `test_cmd`:

- Change visibility of: `pre_flight_validation`, `resolve_and_pin_dependencies`, `build_synthetic_template`, `build_image`, `start_template_container`, `health_check_template_container`, `execute_and_stream_output`, `ensure_daemon_running`, `split_image_ref`, `PinnedDependencies`
- From `fn` to `pub(crate) fn`
- Minimal change — just visibility, no logic changes

**Note on `pre_flight_validation`**: This function takes a `dev_mode` bool and validates either the dev section or build section of cyan.yaml. For template tests, pass `dev_mode=false`. For processor/plugin/resolver tests (Plan 2), a separate pre-flight function will be needed since those don't have template-specific build configs.

## Approach

1. Make `try_cmd.rs` shared functions `pub(crate)`
2. Add clap CLI definitions — get `cyanprint test template . --help` working
3. Implement config parsing with serde derives
4. Implement validation engine (validate commands + snapshot comparison)
5. Implement report formatters (human-readable + JUnit)
6. Implement template test flow in `template.rs` (warm-up, per-test execution, cleanup)
7. Wire everything in main.rs
8. Add unit tests

## Edge Cases

- `test.cyan.yaml` with zero test cases → print "No tests found" and exit 0
- `--test <name>` with non-existent name → error with "Test case '{name}' not found"
- `--junit` path in non-existent directory → create parent dirs
- JSON comparison with nested objects and arrays
- Snapshot comparison with empty directories
- Template Q&A returns `QnA` state for an answer not in the pre-supplied map → fail with descriptive error showing which question ID is missing
- Parallel test cases: ensure unique session IDs, unique output subdirectories
- Binary files in snapshot (skip comparison, report as binary)

## Testing Strategy

- Unit tests for `config.rs`: parse valid/invalid YAML, all test case types
- Unit tests for `validation.rs`: directory comparison with various mismatch scenarios
- Unit tests for `report.rs`: verify output format
- CLI parsing tests following the existing `commands.rs` test pattern
- Manual testing against e2e template fixtures

## Implementation Checklist

- [ ] Make `try_cmd.rs` shared functions `pub(crate)`
- [ ] Add `Test` command with subcommands to `commands.rs` (with help text on all args)
- [ ] Wire up test command dispatch in `main.rs`
- [ ] Create `test_cmd/mod.rs` module structure (with module docs)
- [ ] Implement `test.cyan.yaml` parsing in `config.rs` (with doc comments on all types)
- [ ] Implement validate command runner in `validation.rs`
- [ ] Implement snapshot comparison in `validation.rs`
- [ ] Implement human-readable report in `report.rs`
- [ ] Implement JUnit XML report in `report.rs`
- [ ] Implement non-interactive Q&A loop (direct `prompt_template` calls, not `TemplateEngine.start_with`)
- [ ] Implement template warm-up in `template.rs`
- [ ] Implement per-test-case execution in `template.rs`
- [ ] Implement `--update-snapshots` flow
- [ ] Implement `--test` filtering
- [ ] Add tmp cleanup after test run
- [ ] Add unit tests for config parsing, snapshot comparison, CLI parsing
