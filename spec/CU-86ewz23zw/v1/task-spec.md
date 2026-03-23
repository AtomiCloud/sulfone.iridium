# CU-86ewz23zw: Fix Port Allocation Race Condition Across All Test/Try Commands

## Problem

The current `find_available_port()` in `cyanprint/src/port.rs` has a check-then-use race condition: it probes a port via `TcpListener::bind()` and immediately `drop()`s the listener. Between the check and the actual Docker bind, another parallel process can grab the same port. This affects all 4 call sites when tests run in parallel.

Additionally, port ranges overlap between template tests (5600-5900) and plugin tests (5600-5699), increasing collision probability.

## Scope

### In scope

- Rewrite `cyanprint/src/port.rs` with bind-and-hold pattern
- Define per-artifact port range constants (5 non-overlapping ranges, 200 ports each)
- Update all 4 call sites with retry logic (3 retries)
- Move port allocation late in `try_cmd.rs` (before Step 10, not Step 2)

### Out of scope

- Port configuration via CLI flags or config files
- Global port registry or cross-process coordination
- Changes to dev_mode behavior in `try_cmd.rs`

## Requirements

### R1: Bind-and-hold port allocation

Replace the current `find_available_port(range_start, range_end) -> Option<u16>` with a new API:

- **`PortAllocation` struct**: holds `port: u16` + `listener: TcpListener`. The `TcpListener` remains bound for the lifetime of the struct, preventing other processes from claiming the port.
- **`PortAllocation::release(self) -> u16`**: drops the listener and returns the port. Must be called immediately before Docker bind to minimize the race window.
- **`allocate_port(range_start, range_end) -> Option<PortAllocation>`**: allocates a port using:
  1. Random selection within the range (using `rand`)
  2. Tracks attempted ports in a local `HashSet` to avoid repeats
  3. Falls back to sequential scan for remaining ports in the range
  4. Returns `None` only if all ports in range are exhausted

### R2: Per-artifact port range constants

Define 5 non-overlapping ranges using high ephemeral ports (49152-50151):

| Constant         | Range       | Used by                                    |
| ---------------- | ----------- | ------------------------------------------ |
| `TEMPLATE_TRY`   | 49152-49351 | `try_cmd.rs`                               |
| `TEMPLATE_TEST`  | 49352-49551 | `test_cmd/template.rs`, `test_cmd/init.rs` |
| `PROCESSOR_TEST` | 49552-49751 | `test_cmd/container.rs` (processor)        |
| `PLUGIN_TEST`    | 49752-49951 | `test_cmd/container.rs` (plugin)           |
| `RESOLVER_TEST`  | 49952-50151 | `test_cmd/container.rs` (resolver)         |

### R3: Update call sites with retry logic

All 4 call sites must:

1. Call `allocate_port()` with the appropriate range constant
2. Call `.release()` immediately before Docker bind
3. If Docker bind fails (narrow race window), retry up to 3 times with a fresh allocation
4. Only hard-fail if all 3 retries are exhausted

### R4: Late allocation in try_cmd.rs

In `cyanprint/src/try_cmd.rs`, move port allocation from Step 2 to immediately before Step 10 (the Docker bind step). This minimizes the time the `TcpListener` is held and reduces the chance of interference.

### R5: Remove old API

Remove the old `find_available_port()` and `is_port_available()` functions. Update all tests in `port.rs` to test the new API.

## Non-Goals

- No configuration file for port ranges
- No shared state or cross-process locking
- No changes to the container networking model
