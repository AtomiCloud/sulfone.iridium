# Plan 2: Processor + Plugin + Resolver Tests

## Goal

Add the remaining three test flows so `cyanprint test processor .`, `cyanprint test plugin .`, and `cyanprint test resolver .` all work. This builds on Plan 1's shared infrastructure (config parsing, validation engine, reports) and adds Docker container management for non-template artifacts.

## Verifiable Outcome

All four `cyanprint test <type> .` commands work end-to-end. Can be verified against the e2e fixtures (`e2e/processor1`, `e2e/plugin1`, `e2e/resolver1`).

## Dependencies

- Plan 1 must be complete (CLI, config parsing, validation, reports, template flow all working)

## Files to Create

### `cyanprint/src/test_cmd/container.rs`

Docker container management for processor/plugin/resolver tests. Provides:

1. **`build_and_start_container`**: build image from `cyan.yaml` build config, start container with bind mounts, health check. Returns container name + allocated port.

   - For processors: bind-mount all test input dirs to `/workspace/cyanprint/{test_name}/` (read-only) and output root to `/workspace/area/` (read-write). Port 5551.
   - For plugins: pre-copy inputs to output dir (plugins modify in-place), bind-mount output root to `/workspace/area/` (read-write). Port 5552.
   - For resolvers: no file mounts needed (API-only). Port 5553.

2. **`cleanup_container`**: stop and remove container, remove built image.

3. **Port allocation**: use existing `find_available_port()` from `crate::port`.

4. **Health check**: `GET /` with retries, following the pattern from `health_check_template_container` in `try_cmd.rs`.

Use `bollard` Docker client for container lifecycle. Follow the container creation pattern in `try_cmd.rs::start_template_container`.

### `cyanprint/src/test_cmd/processor.rs`

Processor test flow:

**Warm-up:**

- Pre-flight checks (Docker + cyan.yaml + test.cyan.yaml)
- Build processor image from `cyan.yaml` build config
- Prepare bind mounts: collect all `input` paths from test cases, canonicalize to absolute paths
- Start container via `container::build_and_start_container` with all inputs mounted
- Health check on port 5551

**Per test case (semaphore-gated):**

- `POST http://localhost:{port}/api/process` with:
  - `readDir: "/workspace/cyanprint/{test_name}"`
  - `writeDir: "/workspace/area/{test_name}"`
  - `globs` and `config` from test case
- Output appears in bind-mounted `{output}/{test_name}/` on host

**After all complete:** validation + snapshot comparison

**Cleanup:** stop container, remove image, remove tmp

### `cyanprint/src/test_cmd/plugin.rs`

Plugin test flow:

**Warm-up:**

- Pre-flight checks
- Build plugin image from `cyan.yaml` build config
- For each test case: copy `input` dir → `{output}/{test_name}/` (plugins modify in-place)
- Start container with `{output}/` mounted to `/workspace/area/` (read-write)
- Health check on port 5552

**Per test case (semaphore-gated):**

- `POST http://localhost:{port}/api/plug` with:
  - `directory: "/workspace/area/{test_name}"`
  - `config` from test case
- Output modified in-place in `{output}/{test_name}/` on host

**After all complete:** validation + snapshot comparison

**Cleanup:** stop container, remove image, remove tmp

### `cyanprint/src/test_cmd/resolver.rs`

Resolver test flow (simplest — no file I/O):

**Warm-up:**

- Pre-flight checks
- Build resolver image from `cyan.yaml` build config
- Start container on dynamically allocated port
- Health check: `GET /` returns 200 on port 5553

**Per test case (semaphore-gated):**

- `POST http://localhost:{port}/api/resolve` with test `input` body (config + files)
- Compare JSON response against `expected` (array of `{path, content}` pairs)
- JSON deep comparison (field order doesn't matter)

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

1. Implement `container.rs` — shared Docker container lifecycle management
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

## Testing Strategy

- Unit tests for `container.rs`: container name generation, bind mount path construction
- Manual testing against e2e fixtures (`e2e/processor1`, `e2e/plugin1`, `e2e/resolver1`)
- Create minimal test.cyan.yaml configs for each type using existing e2e fixtures

## Implementation Checklist

- [ ] Implement `container.rs` — build, start with bind mounts, health check, cleanup
- [ ] Implement `resolver.rs` — resolver test flow
- [ ] Implement `processor.rs` — processor test flow with volume mounting
- [ ] Implement `plugin.rs` — plugin test flow with input copying
- [ ] Wire up all three in `main.rs` (replace `todo!()` stubs)
- [ ] Add `--update-snapshots` support for all three types
- [ ] Add tmp cleanup for all three types
- [ ] Test against e2e fixtures
