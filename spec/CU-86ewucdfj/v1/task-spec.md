# Task Spec: Daemon Shutdown and Cleanup

**Ticket**: CU-86ewucdfj
**Version**: 1

## Overview

Add daemon shutdown and cleanup functionality to Iridium (cyanprint CLI). This allows users to stop the Boron coordinator daemon, which:

1. Calls the Boron `DELETE /cleanup` endpoint to clean up all Docker resources
2. Removes the Boron container itself

## Command Design

Convert the existing `cyanprint daemon` command into a subcommand group:

```
cyanprint daemon start [options]   # Start the Boron coordinator (existing behavior)
cyanprint daemon stop [options]    # Stop and cleanup the Boron coordinator (NEW)
```

### `cyanprint daemon start` (existing, renamed)

Preserves all existing options:

- `--version` / `-v`: Boron image version (default: `latest`)
- `--port` / `-p`: Port to expose (default: `9000`)
- `--registry` / `-r`: Registry endpoint for the coordinator

### `cyanprint daemon stop` (NEW)

Options:

- `--port` / `-p`: Port where the daemon is running (default: `9000`)

Behavior:

1. Call `DELETE http://localhost:{port}/cleanup` on the Boron container
2. Wait for cleanup to complete
3. Remove the `cyanprint-coordinator` container
4. Report success/failure

## Acceptance Criteria

1. **AC1**: `cyanprint daemon start` works identically to the current `cyanprint daemon` command
2. **AC2**: `cyanprint daemon stop` calls `DELETE /cleanup` on the Boron container at the specified port
3. **AC3**: After calling cleanup, `cyanprint daemon stop` removes the `cyanprint-coordinator` container
4. **AC4**: Clear output showing results (success/failure)
5. **AC5**: Graceful error handling when:
   - Daemon is not running
   - Cleanup endpoint fails
   - Container removal fails

## Files to Modify

| File                                       | Changes                                                               |
| ------------------------------------------ | --------------------------------------------------------------------- |
| `cyanprint/src/commands.rs`                | Convert `Daemon` to subcommand group with `Start` and `Stop` variants |
| `cyanprint/src/main.rs`                    | Handle new `DaemonStart` and `DaemonStop` command variants            |
| `cyanprint/src/coord.rs`                   | Add `stop_coordinator()` async function                               |
| `cyancoordinator/src/client.rs`            | Add `cleanup()` method to call `DELETE /cleanup`                      |
| `e2e/setup.sh`                             | Change `cyanprint daemon` to `cyanprint daemon start` (line 10)       |
| `docs/developer/surfaces/cli/04-daemon.md` | Update for subcommand structure, add `daemon stop` documentation      |

## Edge Cases

1. **Daemon not running**: `stop` should detect if no container exists and report clearly
2. **Cleanup endpoint unreachable**: Report error, offer to force remove container
3. **Container in unexpected state**: Handle both running and stopped containers
4. **Multiple cleanup calls**: Should be idempotent

## Constraints

- Must use existing `bollard` Docker client (already in use)
- Must use existing `reqwest` HTTP client patterns (see `CyanCoordinatorClient`)
- Follow existing code style and error handling patterns
- Use conventional commits: `feat(daemon): add stop subcommand for cleanup`

## Context

### Boron Cleanup Endpoint

From Boron's `server.go`:

- **Endpoint**: `DELETE /cleanup`
- **Purpose**: Removes all Docker resources labeled with `cyanprint.dev=true`
- **Returns**: JSON with lists of removed containers, images, and volumes

### Current Daemon Flow

1. `start_coordinator()` in `coord.rs`:
   - Checks for existing `cyanprint-coordinator` container
   - Pulls Boron image
   - Runs setup container to create Docker network
   - Starts main coordinator container

### Container Names

- Setup container: `cyanprint-coordinator-setup`
- Main container: `cyanprint-coordinator`
- Docker network: `cyanprint`
