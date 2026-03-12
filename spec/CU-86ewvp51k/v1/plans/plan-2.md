# Plan 2: Processor + Plugin + Resolver Tests

## Goal

Add the remaining three test flows so `cyanprint test processor .`, `cyanprint test plugin .`, and `cyanprint test resolver .` all work. This builds on Plan 1's shared infrastructure (config parsing, validation engine, reports) and adds Docker container management for non-template artifacts.

## Verifiable Outcome

All four `cyanprint test <type> .` commands work end-to-end. Can be verified against the e2e fixtures with build configs (`e2e/processor2`, `e2e/plugin2`, `e2e/resolver2`).

## Dependencies

- Plan 1 must be complete (CLI, config parsing, validation, reports, template flow all working)

## Documentation Requirements

All public types, functions, and modules created in this plan must include Rust doc comments (`///`). Specifically:

- `container.rs` — doc comments on all public functions explaining container lifecycle, bind mount logic, and port allocation
- `processor.rs` / `plugin.rs` / `resolver.rs` — doc comments on entry point functions explaining the test flow
- Any new shared types (e.g., container handles, test context structs)

## Files to Create

### `cyanprint/src/test_cmd/container.rs`

Docker container management for processor/plugin/resolver tests. Provides:

1. **`build_and_start_container`**: build image from Dockerfile, start container with bind mounts, health check. Returns container name + allocated port.

   - For processors: bind-mount all test input dirs to `/workspace/cyanprint/{test_name}/` (read-only) and output root to `/workspace/area/` (read-write). Internal port 5551.
   - For plugins: pre-copy inputs to output dir (plugins modify in-place), bind-mount output root to `/workspace/area/` (read-write). Internal port 5552.
   - For resolvers: no file mounts needed (API-only). Internal port 5553.

2. **`cleanup_container`**: stop and remove container, remove built image.

3. **Port allocation**: use existing `find_available_port()` from `crate::port`.

4. **Health check**: `GET /` with retries, following the pattern from `health_check_template_container` in `try_cmd.rs`.

**Building images**: Use `BuildxBuilder` from `crate::docker::buildx` (same as `try_cmd.rs`), NOT `bollard` for building. Read the build config from `cyan.yaml` using `read_build_config()` → extract the relevant image config (processor/plugin/resolver). The e2e "2" fixtures (`e2e/processor2`, `e2e/plugin2`, `e2e/resolver2`) all have proper `build:` sections with registry, image name, dockerfile, and context — use these for testing.

**Container lifecycle**: Use `bollard` for container create/start/stop/remove (same pattern as `start_template_container` in `try_cmd.rs`). Note the blocking/async pattern: create a `tokio::runtime` inline and `block_on()` the async bollard calls, exactly as `try_cmd.rs` does.

### `cyanprint/src/test_cmd/processor.rs`

Processor test flow:

**Warm-up:**

- Pre-flight checks (Docker running + cyan.yaml exists)
- Build processor image via `container::build_and_start_container`
- Prepare bind mounts: collect all `input` paths from test cases, canonicalize to absolute paths
- Start container with all inputs mounted to `/workspace/cyanprint/{test_name}/` (read-only) and tmp output root to `/workspace/area/` (read-write)
- Health check on allocated host port (maps to internal 5551)

**Per test case (semaphore-gated):**

- `POST http://localhost:{host_port}/api/process` with JSON body:
  - `readDir: "/workspace/cyanprint/{test_name}"`
  - `writeDir: "/workspace/area/{test_name}"`
  - `globs` from test case (array of `{pattern, type}`)
  - `config` from test case (YAML in test.cyan.yaml → `serde_json::Value` in API call)
- Output appears in bind-mounted `{tmp_output}/{test_name}/` on host

**After all complete:** validation + snapshot comparison

**Cleanup:** stop container, remove image, remove tmp

### `cyanprint/src/test_cmd/plugin.rs`

Plugin test flow:

**Warm-up:**

- Pre-flight checks
- Build plugin image
- For each test case: copy `input` dir → `{tmp_output}/{test_name}/` (plugins modify in-place)
- Start container with `{tmp_output}/` mounted to `/workspace/area/` (read-write)
- Health check on allocated host port (maps to internal 5552)

**Per test case (semaphore-gated):**

- `POST http://localhost:{host_port}/api/plug` with JSON body:
  - `directory: "/workspace/area/{test_name}"`
  - `config` from test case (`serde_json::Value`)
- Output modified in-place in `{tmp_output}/{test_name}/` on host

**After all complete:** validation + snapshot comparison

**Cleanup:** stop container, remove image, remove tmp

### `cyanprint/src/test_cmd/resolver.rs`

Resolver test flow (simplest — no file I/O):

**Warm-up:**

- Pre-flight checks
- Build resolver image
- Start container on dynamically allocated host port (maps to internal 5553)
- Health check: `GET /` returns 200

**Per test case (semaphore-gated):**

- `POST http://localhost:{host_port}/api/resolve` with test `input` body (config + files from test.cyan.yaml)
- Compare JSON response against `expected` (array of `{path, content}` pairs)
- JSON deep comparison (field order doesn't matter) using `serde_json::Value` equality

**Cleanup:** stop container, remove image

## Files to Modify

### `cyanprint/src/test_cmd/mod.rs`

Add new sub-modules:

- `pub mod container;`
- `pub mod processor;`
- `pub mod plugin;`
- `pub mod resolver;`

### `cyanprint/src/main.rs`

Replace `todo!()` stubs for `TestCommands::Processor`, `TestCommands::Plugin`, `TestCommands::Resolver` with calls to the new modules. Follow the same flow as template: parse config → filter → run → validate → compare → report → cleanup.

## Approach

1. Implement `container.rs` — shared Docker container lifecycle management (build with BuildxBuilder, lifecycle with bollard)
2. Implement `resolver.rs` (simplest — API-only, no volume mounts, good for validating the pattern)
3. Implement `processor.rs` (volume mounting + API calls)
4. Implement `plugin.rs` (similar to processor but with input copying and in-place modification)
5. Wire up all three in `main.rs`
6. Test against e2e fixtures

## Edge Cases

- Processor/plugin API returns error → mark test FAIL, capture error, continue
- Container startup timeout → fail entire test run
- Bind mount paths must be absolute (canonicalize before passing to Docker)
- Plugin input directory doesn't exist → fail with descriptive error
- Resolver response format doesn't match expected structure → descriptive error
- Multiple test cases with same input dir → safe because each gets unique write dir (processors) or copied input (plugins)
- `cyan.yaml` has no build section for the artifact type → fail with descriptive error (all production repos should have this)
- `cyan.yaml` has no Dockerfile → fail with "No Dockerfile found" error

## Testing Strategy

- Unit tests for `container.rs`: container name generation, bind mount path construction
- Manual testing against e2e fixtures with build configs (`e2e/processor2`, `e2e/plugin2`, `e2e/resolver2`)
- Create minimal test.cyan.yaml configs for each type using existing e2e fixtures

## Implementation Checklist

- [ ] Implement `container.rs` — build (BuildxBuilder), start with bind mounts (bollard), health check, cleanup
- [ ] Implement `resolver.rs` — resolver test flow (with doc comments)
- [ ] Implement `processor.rs` — processor test flow with volume mounting (with doc comments)
- [ ] Implement `plugin.rs` — plugin test flow with input copying (with doc comments)
- [ ] Wire up all three in `main.rs` (replace `todo!()` stubs)
- [ ] Add `--update-snapshots` support for all three types
- [ ] Add tmp cleanup for all three types
- [ ] Test against e2e fixtures
