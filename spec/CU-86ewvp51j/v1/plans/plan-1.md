# Plan 1: Implement `cyanprint try` Command

> Provide direction and suggestions, not exact code. Plans describe HOW to build, not the exact implementation.

## Goal

Implement the full `cyanprint try` command — CLI definition, config parsing, port allocation, daemon management, template container lifecycle (via bollard), Boron API integration, dependency pinning, Q&A loop, output streaming, and cleanup.

## Scope

### In Scope

- CLI command definition and argument parsing (clap derive)
- Dev section parsing in cyan.yaml (template_url, blob_path)
- Port allocation utility (5600-5900 range)
- Daemon autostart management (reuse existing coordinator infrastructure)
- Pre-flight validation
- Boron API client methods for try endpoints
- Template container startup via bollard SDK (normal mode)
- Dependency resolution and first-layer pinning
- Interactive Q&A loop (adapted from create command, different HTTP target)
- Output tar.gz streaming and unpacking
- Dev mode handling (external template server)
- Session cleanup and keep-containers logic

### Out of Scope

- Snapshot testing (`cyanprint test` — separate ticket CU-86ewvp51i)
- Registry publishing or CI/CD integration
- Multi-template scenarios or template upgrades
- Deep transitive dependency pinning (only first layer)

## Files to Modify

| File                                | Change Type | Notes                                                                                               |
| ----------------------------------- | ----------- | --------------------------------------------------------------------------------------------------- |
| `cyanprint/src/commands.rs`         | modify      | Add `Try` variant to `Commands` enum                                                                |
| `cyanprint/src/main.rs`             | modify      | Add command handler for try, import new modules                                                     |
| `cyanprint/src/try_cmd.rs`          | create      | Main try command logic: orchestration, template container lifecycle, Q&A adaptation, execution flow |
| `cyanprint/src/port.rs`             | create      | Port allocation utility (find_available_port)                                                       |
| `cyanregistry/src/cli/models/`      | modify      | Add `DevConfig` struct (template_url, blob_path) to existing config models                          |
| `cyanregistry/src/cli/mapper.rs`    | modify      | Add `read_dev_config()` for dev section parsing from cyan.yaml                                      |
| `cyancoordinator/src/models/req.rs` | modify      | Add `TrySetupReq` model                                                                             |
| `cyancoordinator/src/client.rs`     | modify      | Add `try_setup()`, `try_cleanup()` methods                                                          |
| `cyanprint/Cargo.toml`              | modify      | Add dependencies if needed (uuid for ID generation)                                                 |

## Technical Approach

### 1. CLI Command Definition

Add `Try` variant to the `Commands` enum in `commands.rs` using clap derive:

- `template_path: String` — local path to template directory (required positional)
- `output_path: String` — output directory (required positional)
- `--dev` — enable dev mode (flag)
- `--keep-containers` — preserve template container and blob volume after execution (flag)
- `--disable-daemon-autostart` — skip automatic daemon start (flag)

Follow the existing pattern in `Commands` enum (struct-like variants with `#[arg(...)]` annotations).

### 2. Dev Section Config Parsing

Add dev section support to cyan.yaml parsing **in the cyanregistry crate** (where all config models live):

- Create `DevConfig` struct in `cyanregistry/src/cli/models/` with fields: `template_url: String`, `blob_path: String`
- Add `read_dev_config(path)` function in `cyanregistry/src/cli/mapper.rs` following the `read_build_config()` pattern
- Dev section is optional in cyan.yaml — only required when `--dev` flag is used

### 3. Port Allocation

Create `cyanprint/src/port.rs` module:

- `find_available_port(range_start: u16, range_end: u16) -> Option<u16>`
- Use `std::net::TcpListener::bind(("0.0.0.0", port))` to check availability
- Scan ports 5600-5900 sequentially, return first available
- Return `None` if range exhausted

### 4. Daemon Management

Reuse existing infrastructure from `cyanprint/src/coord.rs`:

- Check if daemon container `cyanprint-coordinator` is running (use bollard `list_containers` with name filter)
- If not running and `--disable-daemon-autostart` is false: call existing `start_coordinator()` function
- If not running and flag is true: return error with instructions to run `cyanprint daemon start`
- Health check: HTTP GET to `http://localhost:{daemon_port}/` with timeout (60s, 1s intervals)
- Daemon always runs on port 9000

### 5. Pre-flight Validation

Create validation function in `try_cmd.rs`:

- Check Docker daemon running (reuse `BuildxBuilder::check_docker()` pattern or bollard ping)
- Check cyan.yaml exists at template_path
- Normal mode: validate build section exists (use `read_build_config()`)
- Dev mode: validate dev section exists (use `read_dev_config()`), check template_url reachable via HTTP GET
- Clear error messages for each check

### 6. Boron API Models and Client Methods

**Add to `cyancoordinator/src/models/req.rs`:**

`TrySetupReq` matching Boron's actual `TryExecutorReq`:

```
TrySetupReq {
    session_id: String,
    local_template_id: String,
    source: String,           // "image" or "path"
    image_ref: Option<DockerImageReference>,  // required when source="image"
    path: Option<String>,     // required when source="path"
    template: TemplateVersionRes,
    merger_id: String,
}
```

Where `DockerImageReference` has `reference: String` and `tag: String` (matching Boron's Go struct with JSON fields `reference`, `tag`).

Response model `TrySetupRes`:

```
TrySetupRes {
    session_id: String,
    blob_volume: DockerVolumeReference,
    session_volume: DockerVolumeReference,
}
```

**DO NOT create `TryExecuteReq`** — the execution step reuses the existing `StartExecutorReq` which Boron's POST `/executor/:sessionId` already expects.

**Add to `cyancoordinator/src/client.rs`:**

- `try_setup(&self, req: &TrySetupReq) -> Result<TrySetupRes>` — POST `/executor/try`
- `try_cleanup(&self, session_id: &str) -> Result<()>` — DELETE `/executor/{session_id}`

The execution call (POST `/executor/{session_id}`) already exists as `bootstrap()` using `StartExecutorReq`.

### 7. Dependency Pinning

**Why pin?** The user's cyan.yaml references dependencies (processors, plugins, resolvers, sub-templates) that may not specify exact versions. We must resolve each to its latest version at try-time and pin that version for the entire session. This ensures determinism — if a dependency publishes a new version mid-session, the try run still uses the version resolved at startup.

Create pinning logic in `try_cmd.rs`:

- `PinnedDependencies` struct: holds Vec of pinned processors, plugins, resolvers, sub-templates with their resolved versions
- `resolve_and_pin_first_layer(registry: &CyanRegistryClient, config: &CyanTemplateFileConfig) -> Result<PinnedDependencies>`
- Query registry for each first-layer dependency using existing `get_processor()`, `get_plugin()`, `get_resolver()`, `get_template()` methods
- Pin to the latest version returned by the registry at the time of resolution
- Warn on version conflicts (log::warn)

**Build synthetic `TemplateVersionRes`** for Boron:

- `principal.id`: use the `local_template_id` (e.g., `local-{uuid}`)
- `principal.version`: 0
- `principal.properties`:
  - Normal mode: blob_docker_reference + tag from build, template_docker_reference + tag from build
  - Dev mode: empty/placeholder (blob comes from path, not image)
- `template`: synthetic `TemplatePrincipalRes` with name from cyan.yaml
- `processors`, `plugins`, `resolvers`: populated from pinned dependencies
- `templates`: sub-templates from pinned dependencies

This synthetic template is what gets sent to Boron in the `TrySetupReq.template` field.

### 8. Template Container Lifecycle (Normal Mode — Bollard SDK)

**This is the critical new piece.** Iridium must start the template container itself since `/executor/try` does NOT start it.

Mirror Boron's template container pattern (`template_executor.go`) using bollard:

1. **Build template image** using existing buildx infrastructure (`BuildxBuilder`). Generate random tag: `{template_name}-try-{uuid}` to avoid conflicts.

2. **Create and start template container** via bollard:

   - Container name: `cyan-template-{local_template_id_no_dashes}`
   - Image: the just-built template image (`{reference}:{tag}`)
   - Network: `"cyanprint"` (same Docker bridge network as coordinator)
   - Labels: `{"cyanprint.dev": "true"}` (for discovery and cleanup)
   - Port binding: Map internal `5550/tcp` to the dynamically allocated host port (from step 3)
   - No volume mounts needed for template container (unlike blob volumes)

3. **Health check**: HTTP GET to `http://localhost:{allocated_port}/` with retry loop (max 60 attempts, 1s interval). Template container serves on port 5550 internally, mapped to the allocated host port.

4. **Cleanup** (unless `--keep-containers`):
   - Stop and remove template container via bollard
   - Remove built image (optional, best-effort)

### 9. Q&A Loop

Adapt the existing Q&A mechanism from the `create` command flow, but with a **different HTTP target**:

- **Normal mode**: Q&A goes directly to `http://localhost:{allocated_port}` (the template container on the host-mapped port)
  - Init: POST `http://localhost:{allocated_port}/api/template/init`
  - Validate: POST `http://localhost:{allocated_port}/api/template/validate`
- **Dev mode**: Q&A goes to the external template server
  - Init: POST `{template_url}/api/template/init`
  - Validate: POST `{template_url}/api/template/validate`

**Key difference from `create`**: The `create` command uses `TemplateEngine` pointed at `{coordinator_endpoint}/proxy/template/{template_id}`. For `try`, point `TemplateEngine` at the direct URL instead. The Q&A logic (interactive prompting, answer collection, validation) is identical — only the base URL changes.

Collect answers as `HashMap<String, Answer>` and deterministic states as `HashMap<String, String>`, same as `create`.

### 10. Execution Flow (Main Orchestration)

Implement `execute_try_command()` in `try_cmd.rs`:

1. **Pre-flight checks** (Docker, config, mode-specific validation)
2. **Allocate port** (5600-5900 range)
3. **Ensure daemon running** (unless `--disable-daemon-autostart`)
4. **Generate IDs**: `local_template_id = "local-{uuid}"`, `session_id = "session-{uuid}"`
5. **Resolve and pin dependencies** (first-layer only, query registry)
6. **Build synthetic `TemplateVersionRes`** with pinned deps
7. **Mode-specific setup**:
   - Normal mode: Build template + blob images via buildx (random tag `{name}-try-{uuid}`)
   - Dev mode: Skip build, validate dev config
8. **Call Boron POST `/executor/try`** with `TrySetupReq`:
   - Normal: `source="image"`, `image_ref` from build output
   - Dev: `source="path"`, `path` from dev config `blob_path`
9. **Start template container** (normal mode only, via bollard on allocated port)
10. **Run Q&A loop** (direct to template container or external template_url)
11. **Call Boron POST `/executor/{session_id}`** with `StartExecutorReq` (starts processors/plugins, runs merge pipeline, streams tar.gz output)
12. **Unpack tar.gz** to output directory
13. **Cleanup**:
    - Call Boron DELETE `/executor/{session_id}` (cleans session volume)
    - Unless `--keep-containers`: stop/remove template container via bollard
    - Best-effort: warn on cleanup failure, don't abort

### 11. Error Handling

- Use `Result<T, Box<dyn Error + Send>>` (existing pattern)
- Use `tracing` crate for logging (already in project)
- Log levels: debug for details, info for progress, warn for non-fatal issues, error for failures
- Pre-flight failures: stop immediately with clear message
- Build failures: stop immediately, no partial execution
- Q&A cancellation (Ctrl+C): trigger cleanup before exit
- Cleanup failures: warn and continue (best-effort)

## Edge Cases to Handle

- **Port range exhausted**: Scan entire 5600-5900 range, fail with clear message listing range
- **Daemon already running**: Detect via container list, skip start, proceed
- **Daemon start fails**: Error with instructions to run `cyanprint daemon start` manually
- **Config file missing/invalid**: Parse error with file path and specific issue
- **Docker not running**: Use existing check pattern, provide clear "start Docker" message
- **Build failure**: Stop immediately, show build error, no partial execution
- **Template container fails to start**: Timeout after 60 health check attempts, show container logs
- **Dev mode template unreachable**: HTTP GET health check to template_url, show URL and troubleshooting
- **Dev mode blob path invalid**: Validate path exists before calling Boron (Boron also validates against DEV_ROOT)
- **Dependency version conflict**: Pin first found version, log warning
- **Boron API error**: Surface ProblemDetails error from Boron response
- **Cleanup fails**: Warn but continue (best-effort cleanup for all resources)
- **No build section in cyan.yaml**: Error in normal mode, OK in dev mode
- **No dev section in cyan.yaml**: Error in dev mode, OK in normal mode
- **Session volume collision**: Boron returns error if session_id reused — use unique UUID

## How to Test

1. **Unit tests for port allocation**:

   - Test available port found in range
   - Test no port available returns None
   - Test specific port check

2. **Unit tests for configuration parsing**:

   - Test valid normal mode config (build section present)
   - Test valid dev mode config (dev section present)
   - Test missing config file returns error
   - Test missing required section based on mode

3. **Unit tests for dependency pinning**:

   - Test first-layer resolution creates correct synthetic template
   - Test synthetic TemplateVersionRes has correct structure

4. **Manual E2E tests** (left to humans — do not verify in dev-loop):
   - Normal mode, dev mode, --keep-containers, --disable-daemon-autostart

## Integration Points

- **Depends on**: Boron executor try endpoint (CU-86ewvp51f) — must be deployed and available
- **Blocks**: Test command (CU-86ewvp51i) — will reuse dependency pinning and try infrastructure
- **Shared state**: No persistence needed. Session state (IDs, ports, pins) kept in memory only.

## Implementation Checklist

- [ ] Code changes per approach above
- [ ] Run `pls lint` — fix all linting errors
- [ ] Run `pls build` — verify compilation
- [ ] Run `cargo test --workspace` — no regressions
- [ ] Unit tests added for port allocation, config parsing
- [ ] Manual E2E testing left to humans (not automated)

## Success Criteria

- [ ] `cyanprint try --help` shows correct usage
- [ ] Normal mode: builds images (buildx), starts template container (bollard), Q&A works, output generated
- [ ] Dev mode: skips build, Q&A to external template_url, output generated
- [ ] Port allocation finds available port in 5600-5900 range
- [ ] Daemon auto-starts (unless disabled)
- [ ] Dependency pinning resolves first-layer deps from registry
- [ ] Synthetic TemplateVersionRes correctly constructed for Boron
- [ ] Template container health-checked before Q&A
- [ ] Session cleanup works (unless --keep-containers)
- [ ] `pls lint` and `pls build` pass
- [ ] No regressions in existing commands
