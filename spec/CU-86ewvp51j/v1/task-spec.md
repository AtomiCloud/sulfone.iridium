# Task Specification: [Ir] Try Command (CU-86ewvp51j)

## Source

- Ticket: CU-86ewvp51j
- System: ClickUp
- URL: https://app.clickup.com/t/86ewvp51j

## Summary

Implement `cyanprint try` command for interactive local testing of CyanPrint templates. The command supports both **normal mode** (build images from local template) and **dev mode** (use external template server with local blob files), enabling rapid development iteration without requiring full production deployment.

## Acceptance Criteria

- [ ] Normal mode: `cyanprint try <template_path> <output_path>` builds, prompts for Q&A, generates files
- [ ] Dev mode: `cyanprint try <template_path> <output_path> --dev` skips build, uses external template endpoint
- [ ] Port allocation: Dynamically find available port in range 5600-5900
- [ ] Dependency pinning: Resolve and pin first-layer dependencies at try time
- [ ] Boron integration: Automatically start daemon (unless `--disable-daemon-autostart`)
- [ ] Q&A loop: Interactive prompting identical to `cyanprint create` mechanism
- [ ] Keep containers flag: `--keep-containers` preserves template container and blob volume
- [ ] Pre-flight checks: Docker daemon, cyan.yaml validity, build section (normal mode only)
- [ ] Error handling: Clear messages for Docker/config/daemon/port issues

## Out of Scope

- **Not for upgrades**: Try command is for single template testing only, not template upgrades or multi-template scenarios
- **Not for production deployment**: No registry publishing, no CI/CD integration
- **Not for version management**: No dependency version comparison or conflict resolution
- **Not for snapshot testing**: Use `cyanprint test` for automated snapshot comparison

## Constraints

### Boron API Integration

- **POST /executor/try** - Setup try session

  - Request: `{ session_id, local_template_id, source ("image"|"path"), image_ref?, path?, template, merger_id }`
  - Response: `{ session_id, blob_volume, session_volume }`

- **POST /executor/:sessionId** - Execute and stream tar.gz output

  - Request: `{ template, merger_id }`
  - Response: Stream of tar.gz file

- **DELETE /executor/:sessionId** - Cleanup session

  - Cleans session volume, preserves blob volume for reuse

- **POST /template/warm** - Warm template (pull images, create volumes)
  - Request: `{ template: { principal, resolvers } }`
  - Response: `{ status: "OK" }`

### Port Allocation

- Range: **5600-5900** (expanded from original 5550-5600)
- Dynamic allocation with availability check
- Template container uses allocated port
- Other services use fixed ports: Processor (5551), Plugin (5552), Resolver (5553), Merger (9000)

### Dev Section Structure

- **No authentication required** for dev mode
- Fields:
  - `template_url`: External template server endpoint (e.g., `http://localhost:3000`)
  - `blob_path`: Local filesystem path to blob files (for dev mode)

### Dependency Pinning

- Store in RAM during execution
- Reuse `CyanRegistryClient` for queries
- Pin ONLY the first layer: root's sub-templates, processors, resolvers, plugins
- For dependent templates, use the normal template execution flow (no pinning needed for deep transitive dependencies)
- Pins maintained across the whole try session
- Pin management is EXTERNAL (future test commands need to maintain pins across whole test suite)
- **Need synthetic template at top level** with pinned versions

### Cleanup Behavior

- **Session cleanup**: Default behavior - cleanup containers and volumes after Q&A loop
- **Keep containers flag** (`--keep-containers`):
  - Preserve template container and blob volume for next try
  - Processors, plugins, and session artifacts are typically cleaned up regardless

### Boron Integration

- **Daemon autostart**: Automatically run `cyanprint daemon start` unless `--disable-daemon-autostart` flag
- **Daemon runs on port 9000**
- **Dev mode**: Use `source="path"` with local blob path
- **Normal mode**: Use `source="image"` with built image reference

### Q&A Loop

- **Exact same mechanism** as `cyanprint create`
- **Direct to template**: Go to template endpoint directly (not via proxy)
- **Interactive prompting**: Same prompt format and validation

### Error Handling

- **Normal operations**: Perform as usual
- **Pre-flight checks**:
  - Docker daemon running
  - cyan.yaml exists and valid
  - Build section present (normal mode only)
  - Boron daemon running and ports ready (for `--dev` mode)
- **No scope expansion**: Don't add comprehensive error recovery

### Dev Mode

- **Skip build**: No image building
- **User provides**: Host-mounted file path and endpoint directly on host
- **No registry queries for blob/template**: Dev mode assumes pre-resolved dependencies
- **Still resolves and pins top-level dependencies**: Sub-templates, processors, plugins, resolvers are still resolved and pinned
- **Only blob and script (template) are resolved differently**: Uses external template server with local blob files

### Command Help

- **Use urfav**: Automatic formatted help text from clap
- **Don't put in spec**: Help text generated from command structure

## Context

### Design Philosophy

The `try` command enables **rapid local development iteration** by:

1. **Normal mode**: Build images locally, execute through Boron, test output
2. **Dev mode**: Skip build, use external template server, test Q&A and execution
3. **Interactive feedback**: Immediate results without full production deployment
4. **Isolated testing**: Each try is independent, no state pollution

### Integration Points

- **Boron executor** (`/executor/try`, `/executor/:sessionId`): Core execution
- **Boron daemon** (port 9000): Auto-started for local testing
- **CyanPrint registry**: Dependency resolution and pinning
- **Docker daemon**: Image building (normal mode), container execution
- **Template Q&A system**: Same as `cyanprint create`

### Key Differentiators

| Aspect        | `try`                  | `create`         | `test`              |
| ------------- | ---------------------- | ---------------- | ------------------- |
| **Purpose**   | Local testing          | Production use   | Automated testing   |
| **Build**     | Yes (normal), No (dev) | No               | No                  |
| **Q&A**       | Interactive            | Interactive      | Snapshot only       |
| **Output**    | Local directory        | Local directory  | Snapshot comparison |
| **Registry**  | Pin dependencies       | Use published    | Use published       |
| **Isolation** | Independent            | Persistent state | Isolated per test   |

### Naming Conventions

- **Local template ID**: `local-{uuid}` (synthetic, no registry lookup)
- **Blob volume**: `cyan-{LOCAL_TEMPLATE_ID}`
- **Session volume**: `cyan-{LOCAL_TEMPLATE_ID}-{SESSION_ID}`
- **Processor container**: `cyan-processor-{PROC_ID}-{SESSION_ID}`
- **Plugin container**: `cyan-plugin-{PLUGIN_ID}-{SESSION_ID}`

## Edge Cases

- **Port unavailable**: Scan range 5600-5900, fail if no available port
- **Daemon not running**: Auto-start unless `--disable-daemon-autostart`
- **Daemon start fails**: Clear error message, instructions to run `cyanprint daemon start`
- **Dependency version conflict**: Pin first found version, warn on conflicts
- **Transitive dependency cycle**: Abort with clear error message
- **Template Q&A timeout**: No timeout (user can cancel with Ctrl+C)
- **Build failure**: Stop immediately, show build error, no partial execution
- **Dev mode blob path invalid**: Clear error with expected path format
- **Dev mode template unreachable**: Clear error with template URL and troubleshooting
- **Cleanup fails**: Warn but continue (best-effort cleanup)
- **No build section in cyan.yaml**: Error in normal mode, OK in dev mode
- **No dev section in cyan.yaml**: Error in dev mode, OK in normal mode

## Implementation Checklist

**Ensure all relevant skills in skill folders are applied to this implementation.**

### Linting

- [ ] Run `pls lint` (mandatory before completion)
- [ ] Fix all linting errors before committing

### Testing

Check all that apply:

- [ ] Unit tests (port allocation, config parsing, dependency pinning)
- [ ] Integration tests (Boron API integration, daemon autostart)
- [ ] Manual E2E (normal mode, dev mode, keep-containers flag)

**Test location**: `cyanprint/tests/` (follow existing pattern)

### Observability

Check all that apply:

- [ ] **Metrics**: N/A (CLI command, no persistent monitoring)
- [ ] **Logging**: Correct levels (debug/info/warn/error)
  - Logging library: `tracing` (existing in cyanprint)
- [ ] **Alerts**: N/A (CLI command)
- [ ] **Dashboards**: N/A (CLI command)

### Notes

**Any additional implementation notes or considerations**

- **Lint command**: `pls lint` (mandatory)
- **Build verification**: `pls build` before completion
- **Test execution**: `cargo test --workspace` passes
- **Integration testing**: Manual E2E with Boron daemon running
- **Port allocation**: Must be robust (handle Docker port conflicts)
- **Dependency pinning**: Pin only first-layer dependencies (sub-templates, processors, plugins, resolvers)
- **Dev mode**: Assumes user runs external template server separately
- **Daemon autostart**: Must check if already running before attempting start
